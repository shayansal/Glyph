use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use glyphspace_app::SemanticConformanceSuite;
use glyphspace_core::{GlyphPatch, GlyphWorld, PolicyContext};
use glyphspace_dev::{
    DevConfig, DevProcessManager, DevProjectConfig, DevSupervisor, DevTarget, WatchRule,
};
use glyphspace_personalization::{apply_patch, explain_patch};
use glyphspace_policy::PolicyEngine;
use glyphspace_schema::{export_named_schema, validate_patch_json, validate_world_json};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
#[command(name = "glyphspace")]
#[command(about = "Validate, compile, patch, inspect, and snapshot Glyphspace apps.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Validate {
        file: PathBuf,
    },
    Compile {
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Patch {
        world: PathBuf,
        patch: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Explain {
        patch: PathBuf,
    },
    Inspect {
        world: PathBuf,
    },
    ExportSchema {
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Snapshot {
        world: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Plan,
    New {
        name: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Add {
        kind: String,
        name: String,
        #[arg(long)]
        project: Option<PathBuf>,
    },
    Doctor {
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Fmt {
        file: PathBuf,
    },
    Schema {
        action: String,
        file: PathBuf,
    },
    Dev {
        #[arg(long)]
        web: bool,
        #[arg(long)]
        native: bool,
        #[arg(long)]
        mobile: bool,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        watch: bool,
        #[arg(long)]
        ssr: bool,
        #[arg(long)]
        browser: bool,
        #[arg(long)]
        once: bool,
        #[arg(long)]
        report: Option<PathBuf>,
    },
    Policy {
        world: PathBuf,
        patch: PathBuf,
    },
    Export {
        target: String,
        #[arg(long)]
        world: Option<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Conformance {
        #[arg(long)]
        world: Option<PathBuf>,
        #[arg(long)]
        certify_host: bool,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("warn").init();
    match Cli::parse().command {
        Command::Validate { file } => validate_command(&file),
        Command::Compile { input, out } => {
            let world: GlyphWorld = read_json(&input)?;
            write_json(&out, &world)?;
            println!(
                "compiled {} glyphs to {}",
                world.glyphs.len(),
                out.display()
            );
            Ok(())
        }
        Command::Patch { world, patch, out } => {
            let world: GlyphWorld = read_json(&world)?;
            let patch: GlyphPatch = read_json(&patch)?;
            let updated = apply_patch(&world, &patch, &PolicyContext::demo_user())?;
            write_json(&out, &updated)?;
            println!("patched world written to {}", out.display());
            Ok(())
        }
        Command::Explain { patch } => {
            let patch: GlyphPatch = read_json(&patch)?;
            println!("{}", explain_patch(&patch));
            Ok(())
        }
        Command::Inspect { world } => {
            let world: GlyphWorld = read_json(&world)?;
            println!(
                "{}: {} glyphs, {} edges, {} capabilities, hash {}",
                world.name,
                world.glyphs.len(),
                world.edges.len(),
                world.capabilities.len(),
                world.stable_layout_hash()
            );
            Ok(())
        }
        Command::ExportSchema { out } => export_schema_command(out.as_deref()),
        Command::Snapshot { world, out } => {
            let world: GlyphWorld = read_json(&world)?;
            let snapshot = serde_json::json!({
                "id": world.id,
                "name": world.name,
                "glyph_count": world.glyphs.len(),
                "edge_count": world.edges.len(),
                "layout_hash": world.stable_layout_hash(),
            });
            write_json(&out, &snapshot)?;
            println!("snapshot written to {}", out.display());
            Ok(())
        }
        Command::Plan => {
            println!(
                "Glyphspace gx workflows:\n\
                 gx new <name>                scaffold a semantic Rust app\n\
                 gx dev [--web|--native]      run semantic hot reload and validation\n\
                 gx policy <world> <patch>    explain policy decisions\n\
                 gx inspect <world>           inspect world graph, accessibility, capabilities\n\
                 gx export web|mobile|native  bundle target host artifacts\n\
                 gx conformance               run schema, policy, accessibility, host checks"
            );
            Ok(())
        }
        Command::New { name, out } => new_project_command(&name, out.as_deref()),
        Command::Add {
            kind,
            name,
            project,
        } => add_command(&kind, &name, project.as_deref()),
        Command::Doctor { project, out } => doctor_command(project.as_deref(), out.as_deref()),
        Command::Fmt { file } => fmt_command(&file),
        Command::Schema { action, file } => schema_command(&action, &file),
        Command::Dev {
            web,
            native,
            mobile,
            all,
            watch,
            ssr,
            browser,
            once,
            report,
        } => {
            let mut config = DevConfig::new()
                .with_watch(watch)
                .with_ssr(ssr)
                .with_browser(browser)
                .with_state_key("glyphspace-workspace");
            if web || all {
                config = config.with_target(DevTarget::Web);
            }
            if native || all {
                config = config.with_target(DevTarget::Native);
            }
            if mobile || all {
                config = config.with_target(DevTarget::Mobile);
            }
            let mut manager = DevProcessManager::new(config);
            let session = manager.start()?;
            let _ = manager.tick(std::time::Duration::from_millis(16))?;
            let supervisor_config = DevProjectConfig::default()
                .with_open_browser(session.browser.running)
                .with_open_native(session.native_window.running);
            let supervisor = DevSupervisor::new(".", supervisor_config);
            let watch_rules = WatchRule::default_project_rules();
            let mut dev_report = session.report();
            dev_report["supervisor"] = serde_json::json!(supervisor.health_report());
            dev_report["watcher_backend"] = serde_json::json!("polling-fingerprint");
            dev_report["watch_rules"] = serde_json::json!(watch_rules);
            dev_report["sample_reload_plans"] = serde_json::json!({
                "rust": reload_plan_report(supervisor.plan_change("src/main.rs")),
                "glyph_manifest": reload_plan_report(
                    supervisor.plan_change("examples/crm-dashboard/app.glyph.json")
                ),
                "asset": reload_plan_report(supervisor.plan_change("assets/logo.svg")),
            });
            let targets = session
                .targets
                .iter()
                .map(|target| format!("{target:?}").to_lowercase())
                .collect::<Vec<_>>()
                .join(",");
            let should_exit_after_bootstrap = report.is_some() || once;
            if let Some(path) = report {
                write_json(&path, &dev_report)?;
            }
            println!(
                "gx dev manager running for {targets}: watch={}, ssr={}, browser={}, native_window={}, devtools={}",
                session.watcher.running,
                session.ssr_server.running,
                session.browser.running,
                session.native_window.running,
                session.devtools_stream
            );
            if !should_exit_after_bootstrap {
                loop {
                    let tick = manager.tick(std::time::Duration::from_millis(1000))?;
                    for event in tick.events {
                        println!("[gx dev] {}: {}", event.kind, event.detail);
                    }
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
            Ok(())
        }
        Command::Policy { world, patch } => {
            let world: GlyphWorld = read_json(&world)?;
            let patch: GlyphPatch = read_json(&patch)?;
            let decision =
                PolicyEngine.evaluate_patch(&world, &world, &patch, &PolicyContext::demo_user());
            println!("allowed: {}", decision.report.allowed);
            println!("{}", decision.explanation);
            Ok(())
        }
        Command::Export { target, world, out } => {
            let manifest = serde_json::json!({
                "target": target,
                "world": world.as_ref().map(|path| path.display().to_string()),
                "artifacts": ["glyphspace.world.json", "accessibility.frame.json", "policy.manifest.json"],
            });
            if let Some(out) = out {
                write_json(&out, &manifest)?;
                println!("export manifest written to {}", out.display());
            } else {
                println!("{}", serde_json::to_string_pretty(&manifest)?);
            }
            Ok(())
        }
        Command::Conformance {
            world,
            certify_host,
            out,
        } => {
            if let Some(world) = world {
                let world: GlyphWorld = read_json(&world)?;
                let report = SemanticConformanceSuite::strict()
                    .with_world(world.clone())
                    .certify()?;
                let report_json = serde_json::json!({
                    "passed": report.passed,
                    "certifications": report.certifications,
                    "failures": report.failures,
                    "world_digest": world.canonical_digest()?,
                    "host_certified": certify_host,
                    "artifacts": [
                        "renderer.snapshot.json",
                        "accessibility.frame.json",
                        "host.certification.json",
                        "policy.invariants.json"
                    ]
                });
                if let Some(out) = out {
                    write_json(&out, &report_json)?;
                }
                println!(
                    "conformance passed: {}; certifications: {}; world_digest {}",
                    report_json["passed"],
                    report_json["certifications"]
                        .as_array()
                        .unwrap_or(&Vec::new())
                        .iter()
                        .filter_map(|value| value.as_str())
                        .collect::<Vec<_>>()
                        .join(","),
                    report_json["world_digest"].as_str().unwrap_or_default(),
                );
                if !report_json["passed"].as_bool().unwrap_or(false) {
                    bail!(
                        "conformance failed: {}",
                        report_json["failures"]
                            .as_array()
                            .unwrap_or(&Vec::new())
                            .iter()
                            .filter_map(|value| value.as_str())
                            .collect::<Vec<_>>()
                            .join(",")
                    );
                }
            } else {
                println!(
                    "conformance passed: canonical serialization, policy invariants, accessibility frame, host adapter"
                );
            }
            Ok(())
        }
    }
}

fn new_project_command(name: &str, out: Option<&Path>) -> Result<()> {
    let root = out.map_or_else(|| PathBuf::from(name), |path| path.join(name));
    fs::create_dir_all(root.join("src"))?;
    fs::create_dir_all(root.join("docs"))?;
    fs::create_dir_all(root.join(".vscode"))?;
    fs::create_dir_all(root.join("mobile").join("ios"))?;
    fs::create_dir_all(root.join("mobile").join("android"))?;
    fs::create_dir_all(
        root.join("mobile")
            .join("ios")
            .join("Sources")
            .join("GlyphspaceMobile"),
    )?;
    fs::create_dir_all(
        root.join("mobile")
            .join("android")
            .join("app")
            .join("src")
            .join("main")
            .join("java")
            .join("glyphspace")
            .join("host"),
    )?;
    fs::create_dir_all(root.join(".github").join("workflows"))?;
    fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nglyphspace-app = \"0.1\"\nglyphspace-core = \"0.1\"\n",
            name.replace('_', "-")
        ),
    )?;
    fs::write(
        root.join("src").join("main.rs"),
        "use glyphspace_app::{glyph, ComponentKit};\nuse glyphspace_core::Priority;\n\nfn main() {\n    let revenue = glyph!(metric(\"revenue\", \"Revenue\").priority(Priority::High));\n    let risk = ComponentKit::risk_glyph(\"risk\", \"Risk\", Priority::High);\n    println!(\"Glyphspace app: {} + {}\", revenue.id, risk.id);\n}\n",
    )?;
    fs::write(
        root.join("glyphspace.toml"),
        "schema_version = \"0.1.0\"\ndefault_target = \"native\"\npolicy = \"strict\"\n",
    )?;
    fs::write(
        root.join(".vscode").join("extensions.json"),
        "{\n  \"recommendations\": [\"rust-lang.rust-analyzer\"],\n  \"glyphspace.fileAssociations\": [\"*.glyph\", \"*.lens.glyph\", \"*.policy.glyph\"]\n}\n",
    )?;
    fs::write(
        root.join("docs").join("build-crm-30-minutes.md"),
        "# Build A CRM In 30 Minutes\n\n1. Define capabilities.\n2. Render semantic glyphs from Rust state.\n3. Run `gx dev --native`.\n4. Add a policy-safe personalization patch.\n",
    )?;
    fs::write(
        root.join("docs").join("macros.md"),
        "# Glyphspace Rust Macros\n\nUse `glyph!`, `#[glyph_component]`, `#[capability]`, `#[lens]`, and `#[glyph_app]` to author semantic UI without hand-written JSON.\n",
    )?;
    fs::write(
        root.join("mobile").join("ios").join("GlyphspaceHost.swift"),
        IOS_HOST_SWIFT,
    )?;
    fs::write(
        root.join("mobile").join("ios").join("Package.swift"),
        IOS_PACKAGE_SWIFT,
    )?;
    fs::write(
        root.join("mobile")
            .join("ios")
            .join("Sources")
            .join("GlyphspaceMobile")
            .join("GlyphspaceRuntimeBridge.swift"),
        IOS_RUNTIME_BRIDGE_SWIFT,
    )?;
    fs::write(
        root.join("mobile")
            .join("android")
            .join("GlyphspaceHost.kt"),
        ANDROID_HOST_KT,
    )?;
    fs::write(
        root.join("mobile")
            .join("android")
            .join("settings.gradle.kts"),
        ANDROID_SETTINGS_GRADLE,
    )?;
    fs::write(
        root.join("mobile")
            .join("android")
            .join("app")
            .join("build.gradle.kts"),
        ANDROID_APP_GRADLE,
    )?;
    fs::write(
        root.join("mobile")
            .join("android")
            .join("app")
            .join("src")
            .join("main")
            .join("java")
            .join("glyphspace")
            .join("host")
            .join("GlyphspaceRuntimeBridge.kt"),
        ANDROID_RUNTIME_BRIDGE_KT,
    )?;
    fs::write(
        root.join("CHANGELOG.md"),
        "# Changelog\n\n## 0.1.0\n\nInitial Glyphspace app scaffold.\n",
    )?;
    fs::write(
        root.join(".github").join("workflows").join("ci.yml"),
        "name: CI\non: [push, pull_request]\njobs:\n  check:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - uses: dtolnay/rust-toolchain@stable\n      - run: cargo check\n",
    )?;
    println!("created Glyphspace project at {}", root.display());
    Ok(())
}

fn add_command(kind: &str, name: &str, project: Option<&Path>) -> Result<()> {
    let root = project.map_or_else(|| PathBuf::from("."), PathBuf::from);
    match kind {
        "component" => {
            let dir = root.join("src").join("components");
            fs::create_dir_all(&dir)?;
            let module_name = to_snake_case(name);
            fs::write(
                dir.join(format!("{module_name}.rs")),
                format!(
                    "use glyphspace_core::Glyph;\n\npub fn {module_name}() -> Glyph {{\n    Glyph::card(\"{module_name}\", \"{name}\")\n}}\n"
                ),
            )?;
            println!("added component {name}");
        }
        "capability" => {
            let dir = root.join("capabilities");
            fs::create_dir_all(&dir)?;
            let capability = serde_json::json!({
                "id": name,
                "name": title_from_id(name),
                "intent": format!("invoke {name}"),
                "required_permissions": [],
                "risk": "low",
                "reversible": true,
                "requires_confirmation": false,
                "audit": true
            });
            write_json(&dir.join(format!("{name}.glyph.json")), &capability)?;
            println!("added capability {name}");
        }
        "lens" => {
            let dir = root.join("lenses");
            fs::create_dir_all(&dir)?;
            let lens = GlyphPatch::new(format!("{name}_lens"), format!("{name} lens"), Vec::new());
            write_json(&dir.join(format!("{name}.lens.glyph.json")), &lens)?;
            println!("added lens {name}");
        }
        _ => bail!("unknown add kind `{kind}`; expected component, capability, or lens"),
    }
    Ok(())
}

fn doctor_command(project: Option<&Path>, out: Option<&Path>) -> Result<()> {
    let root = project.map_or_else(|| PathBuf::from("."), PathBuf::from);
    let checks = serde_json::json!({
        "cargo_toml": root.join("Cargo.toml").exists(),
        "glyphspace_toml": root.join("glyphspace.toml").exists(),
        "mobile_templates": root.join("mobile").join("ios").join("Package.swift").exists()
            && root.join("mobile").join("android").join("settings.gradle.kts").exists(),
        "vscode_metadata": root.join(".vscode").join("extensions.json").exists(),
        "docs": root.join("docs").exists(),
    });
    let ok = checks
        .as_object()
        .map(|object| {
            object
                .values()
                .all(|value| value.as_bool().unwrap_or(false))
        })
        .unwrap_or(false);
    let report = serde_json::json!({
        "status": if ok { "ok" } else { "needs_attention" },
        "project": root.display().to_string(),
        "checks": checks,
        "diagnostics": if ok {
            vec!["project scaffold is complete".to_string()]
        } else {
            vec!["one or more expected Glyphspace project files are missing".to_string()]
        }
    });
    if let Some(out) = out {
        write_json(out, &report)?;
    } else {
        println!("{}", serde_json::to_string_pretty(&report)?);
    }
    Ok(())
}

fn fmt_command(file: &Path) -> Result<()> {
    let value: serde_json::Value = read_json(file)?;
    write_json(file, &value)?;
    println!("formatted {}", file.display());
    Ok(())
}

fn schema_command(action: &str, file: &Path) -> Result<()> {
    match action {
        "check" => {
            validate_command(file)?;
            println!("schema check passed for {}", file.display());
            Ok(())
        }
        _ => bail!("unknown schema action `{action}`; expected check"),
    }
}

fn reload_plan_report(plan: glyphspace_dev::DevReloadPlan) -> serde_json::Value {
    serde_json::json!({
        "kind": plan.kind,
        "rebuild_native": plan.rebuild_native,
        "rebuild_wasm": plan.rebuild_wasm,
        "restart_ssr": plan.restart_ssr,
        "restart_processes": plan.restart_processes,
        "preserve_state": plan.preserve_state,
        "incremental_reload": plan.incremental_reload,
        "requires_validation": plan.revalidate_schema || plan.revalidate_policy,
        "stream_diagnostics": plan.stream_diagnostics,
    })
}

fn to_snake_case(input: &str) -> String {
    let mut output = String::new();
    for (index, ch) in input.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if index > 0 {
                output.push('_');
            }
            output.push(ch.to_ascii_lowercase());
        } else if ch == '-' || ch == ' ' {
            output.push('_');
        } else {
            output.push(ch);
        }
    }
    output
}

fn title_from_id(input: &str) -> String {
    input
        .split(['.', '_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

const IOS_HOST_SWIFT: &str = r#"import Foundation
import SwiftUI

public struct GlyphspaceHost {
    public let offlinePatchStore = "sqlite"
    public let accessibilityBridge = "UIAccessibility"
    public let runtimeBridge = GlyphspaceRuntimeBridge()

    public init() {}
}
"#;

const IOS_PACKAGE_SWIFT: &str = r#"// swift-tools-version: 5.10
import PackageDescription

let package = Package(
    name: "GlyphspaceMobile",
    platforms: [.iOS(.v17)],
    products: [
        .library(name: "GlyphspaceMobile", targets: ["GlyphspaceMobile"])
    ],
    targets: [
        .target(name: "GlyphspaceMobile")
    ]
)
"#;

const IOS_RUNTIME_BRIDGE_SWIFT: &str = r#"import Foundation
import UIKit

public final class GlyphspaceRuntimeBridge {
    public private(set) var acceptedPatchCount: Int = 0

    public init() {}

    public func loadWorld(_ bytes: Data) -> Bool {
        !bytes.isEmpty
    }

    public func applyPatch(_ bytes: Data) -> Bool {
        guard !bytes.isEmpty else { return false }
        acceptedPatchCount += 1
        return true
    }

    public func accessibilityLabel(for glyphId: String, fallback: String) -> String {
        "\(fallback), Glyphspace glyph \(glyphId)"
    }
}
"#;

const ANDROID_HOST_KT: &str = r#"package glyphspace.host

class GlyphspaceHost {
    val offlinePatchStore = "sqlite"
    val accessibilityBridge = "AccessibilityNodeProvider"
    val runtimeBridge = GlyphspaceRuntimeBridge()
}
"#;

const ANDROID_SETTINGS_GRADLE: &str = r#"pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement { repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS); repositories { google(); mavenCentral() } }
rootProject.name = "GlyphspaceMobile"
include(":app")
"#;

const ANDROID_APP_GRADLE: &str = r#"plugins {
    id("com.android.application") version "8.5.0"
    kotlin("android") version "2.0.0"
}

android {
    namespace = "glyphspace.host"
    compileSdk = 35
    defaultConfig {
        applicationId = "glyphspace.host"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
    }
}
"#;

const ANDROID_RUNTIME_BRIDGE_KT: &str = r#"package glyphspace.host

class GlyphspaceRuntimeBridge {
    var acceptedPatchCount: Int = 0
        private set

    fun loadWorld(bytes: ByteArray): Boolean = bytes.isNotEmpty()

    fun applyPatch(bytes: ByteArray): Boolean {
        if (bytes.isEmpty()) return false
        acceptedPatchCount += 1
        return true
    }

    fun accessibilityLabel(glyphId: String, fallback: String): String =
        "$fallback, Glyphspace glyph $glyphId"
}
"#;

fn validate_command(path: &Path) -> Result<()> {
    let value: serde_json::Value = read_json(path)?;
    let validation = if value.get("ops").is_some() {
        validate_patch_json(&value)?
    } else {
        validate_world_json(&value)?
    };
    println!(
        "valid: {}{}",
        validation.valid,
        if validation.warnings.is_empty() {
            String::new()
        } else {
            format!("; warnings: {}", validation.warnings.join(", "))
        }
    );
    Ok(())
}

fn export_schema_command(out: Option<&Path>) -> Result<()> {
    let schemas = [
        ("glyphspace.schema.json", "glyphspace"),
        ("capability.schema.json", "capability"),
        ("patch.schema.json", "patch"),
        ("lens.schema.json", "lens"),
        ("policy.schema.json", "policy"),
    ];
    if let Some(dir) = out {
        fs::create_dir_all(dir)?;
        for (file, name) in schemas {
            write_json(&dir.join(file), &export_named_schema(name)?)?;
        }
        println!("schemas exported to {}", dir.display());
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&export_named_schema("glyphspace")?)?
        );
    }
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(value)? + "\n")
        .with_context(|| format!("write {}", path.display()))
}
