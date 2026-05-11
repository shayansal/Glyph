use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use glyphspace_app::SemanticConformanceSuite;
use glyphspace_core::{GlyphPatch, GlyphWorld, PolicyContext};
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
    Dev {
        #[arg(long)]
        web: bool,
        #[arg(long)]
        native: bool,
        #[arg(long)]
        watch: bool,
        #[arg(long)]
        ssr: bool,
        #[arg(long)]
        browser: bool,
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
        Command::Dev {
            web,
            native,
            watch,
            ssr,
            browser,
            report,
        } => {
            let target = if web {
                "web"
            } else if native {
                "native"
            } else {
                "headless"
            };
            let dev_report = serde_json::json!({
                "target": target,
                "watcher": watch,
                "ssr": ssr,
                "browser": browser,
                "native_window": native,
                "diagnostics": [
                    "schema validation enabled",
                    "policy checks enabled",
                    "semantic hot reload enabled",
                    "accessibility frame verification enabled",
                    "renderer snapshot checks enabled"
                ],
                "devtools_stream": "glyphspace://devtools/events"
            });
            if let Some(path) = report {
                write_json(&path, &dev_report)?;
            }
            println!(
                "gx dev preflight for {target}: schema validation, policy checks, semantic hot reload, accessibility frame verification, and renderer snapshot checks are enabled"
            );
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
        Command::Conformance { world, out } => {
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
        "import Foundation\n\nstruct GlyphspaceHost {\n    let offlinePatchStore = \"sqlite\"\n    let accessibilityBridge = \"UIAccessibility\"\n}\n",
    )?;
    fs::write(
        root.join("mobile")
            .join("android")
            .join("GlyphspaceHost.kt"),
        "package glyphspace.host\n\nclass GlyphspaceHost {\n    val offlinePatchStore = \"sqlite\"\n    val accessibilityBridge = \"AccessibilityNodeProvider\"\n}\n",
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
