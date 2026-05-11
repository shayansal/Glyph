use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn gx_cli_exposes_project_dev_policy_and_export_workflows() {
    let gx = std::env::var("CARGO_BIN_EXE_gx").expect("gx binary path");

    let output = Command::new(gx).arg("plan").output().expect("gx plan runs");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");

    assert!(stdout.contains("gx new"));
    assert!(stdout.contains("gx dev"));
    assert!(stdout.contains("gx policy"));
    assert!(stdout.contains("gx export web"));
}

#[test]
fn gx_new_scaffolds_a_semantic_rust_project() {
    let gx = std::env::var("CARGO_BIN_EXE_gx").expect("gx binary path");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("glyphspace-gx-test-{unique}"));

    let output = Command::new(gx)
        .args(["new", "crm_semantic", "--out"])
        .arg(&root)
        .output()
        .expect("gx new runs");

    assert!(output.status.success());
    assert!(root.join("crm_semantic").join("Cargo.toml").exists());
    assert!(
        root.join("crm_semantic")
            .join("src")
            .join("main.rs")
            .exists()
    );
    assert!(root.join("crm_semantic").join("glyphspace.toml").exists());
    assert!(
        root.join("crm_semantic")
            .join(".vscode")
            .join("extensions.json")
            .exists()
    );
    assert!(
        root.join("crm_semantic")
            .join("docs")
            .join("build-crm-30-minutes.md")
            .exists()
    );
    assert!(
        root.join("crm_semantic")
            .join("docs")
            .join("macros.md")
            .exists()
    );
    assert!(
        root.join("crm_semantic")
            .join("mobile")
            .join("ios")
            .join("GlyphspaceHost.swift")
            .exists()
    );
    assert!(
        root.join("crm_semantic")
            .join("mobile")
            .join("ios")
            .join("Package.swift")
            .exists()
    );
    assert!(
        root.join("crm_semantic")
            .join("mobile")
            .join("ios")
            .join("Sources")
            .join("GlyphspaceMobile")
            .join("GlyphspaceRuntimeBridge.swift")
            .exists()
    );
    assert!(
        root.join("crm_semantic")
            .join("mobile")
            .join("android")
            .join("GlyphspaceHost.kt")
            .exists()
    );
    assert!(
        root.join("crm_semantic")
            .join("mobile")
            .join("android")
            .join("settings.gradle.kts")
            .exists()
    );
    assert!(
        root.join("crm_semantic")
            .join("mobile")
            .join("android")
            .join("app")
            .join("build.gradle.kts")
            .exists()
    );

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn gx_conformance_runs_certification_against_world_fixture() {
    let gx = std::env::var("CARGO_BIN_EXE_gx").expect("gx binary path");
    let world = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("crm-dashboard")
        .join("app.glyph.json");

    let output = Command::new(gx)
        .args(["conformance", "--world"])
        .arg(world)
        .output()
        .expect("gx conformance runs");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("renderer_determinism"));
    assert!(stdout.contains("patch_compatibility"));
}

#[test]
fn gx_dev_and_conformance_write_report_artifacts() {
    let gx = std::env::var("CARGO_BIN_EXE_gx").expect("gx binary path");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("glyphspace-gx-report-{unique}"));
    std::fs::create_dir_all(&root).unwrap();
    let dev_report = root.join("dev-report.json");
    let conformance_report = root.join("conformance-report.json");
    let world = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("crm-dashboard")
        .join("app.glyph.json");

    let dev = Command::new(&gx)
        .args(["dev", "--all", "--watch", "--ssr", "--browser", "--report"])
        .arg(&dev_report)
        .output()
        .expect("gx dev runs");
    let conformance = Command::new(&gx)
        .args(["conformance", "--world"])
        .arg(world)
        .args(["--out"])
        .arg(&conformance_report)
        .output()
        .expect("gx conformance runs");

    assert!(dev.status.success());
    assert!(conformance.status.success());
    let dev_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&dev_report).unwrap()).unwrap();
    let conformance_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&conformance_report).unwrap()).unwrap();
    assert_eq!(dev_json["watcher"], true);
    assert_eq!(dev_json["ssr"], true);
    assert_eq!(dev_json["browser"], true);
    assert_eq!(dev_json["process_manager"], true);
    assert_eq!(dev_json["long_running"], true);
    assert_eq!(dev_json["state_preservation"], true);
    assert_eq!(dev_json["devtools_stream"], "glyphspace://devtools/events");
    assert_eq!(dev_json["native_window"], true);
    assert_eq!(dev_json["supervisor"]["profiling_enabled"], true);
    assert_eq!(dev_json["supervisor"]["open_browser"], true);
    assert_eq!(dev_json["watcher_backend"], "polling-fingerprint");
    assert_eq!(dev_json["os_watcher"]["backend"], "notify-native");
    assert_eq!(dev_json["os_watcher"]["uses_os_notifications"], true);
    assert!(dev_json["watch_rules"].as_array().unwrap().len() >= 6);
    assert_eq!(
        dev_json["sample_reload_plans"]["rust"]["rebuild_native"],
        true
    );
    assert_eq!(
        dev_json["sample_reload_plans"]["glyph_manifest"]["requires_validation"],
        true
    );
    assert!(
        dev_json["targets"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("native"))
    );
    assert!(
        dev_json["targets"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("web"))
    );
    assert!(
        dev_json["targets"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("mobile"))
    );
    assert_eq!(conformance_json["passed"], true);
    assert!(
        conformance_json["kernel_conformance"]["invalid_fixture_cases"]
            .as_u64()
            .unwrap()
            >= 12
    );
    assert_eq!(
        conformance_json["kernel_conformance"]["api_stability"]["spec_version"],
        "0.1.0"
    );
    assert!(
        conformance_json["kernel_conformance"]["api_stability"]["public_types"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("GlyphWorld"))
    );
    assert!(
        conformance_json["kernel_conformance"]["formal_error_codes"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("invalid_patch"))
    );

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn gx_developer_experience_commands_create_artifacts_and_reports() {
    let gx = std::env::var("CARGO_BIN_EXE_gx").expect("gx binary path");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("glyphspace-gx-dx-{unique}"));

    let new_output = Command::new(&gx)
        .args(["new", "crm_dx", "--out"])
        .arg(&root)
        .output()
        .expect("gx new runs");
    assert!(new_output.status.success());
    let project = root.join("crm_dx");

    let add_component = Command::new(&gx)
        .args(["add", "component", "RevenueCard", "--project"])
        .arg(&project)
        .output()
        .expect("gx add component runs");
    let add_capability = Command::new(&gx)
        .args(["add", "capability", "deal.update_stage", "--project"])
        .arg(&project)
        .output()
        .expect("gx add capability runs");
    let add_lens = Command::new(&gx)
        .args(["add", "lens", "founder", "--project"])
        .arg(&project)
        .output()
        .expect("gx add lens runs");

    assert!(add_component.status.success());
    assert!(add_capability.status.success());
    assert!(add_lens.status.success());
    assert!(
        project
            .join("src")
            .join("components")
            .join("revenue_card.rs")
            .exists()
    );
    assert!(
        project
            .join("capabilities")
            .join("deal.update_stage.glyph.json")
            .exists()
    );
    assert!(
        project
            .join("lenses")
            .join("founder.lens.glyph.json")
            .exists()
    );

    let messy = project.join("lenses").join("messy.lens.glyph.json");
    std::fs::write(
        &messy,
        "{\"ops\":[],\"id\":\"messy\",\"description\":\"Messy\",\"spec_version\":\"0.1.0\"}",
    )
    .unwrap();
    let fmt = Command::new(&gx)
        .args(["fmt"])
        .arg(&messy)
        .output()
        .expect("gx fmt runs");
    assert!(fmt.status.success());
    assert!(std::fs::read_to_string(&messy).unwrap().contains('\n'));

    let doctor_report = root.join("doctor.json");
    let doctor = Command::new(&gx)
        .args(["doctor", "--project"])
        .arg(&project)
        .args(["--out"])
        .arg(&doctor_report)
        .output()
        .expect("gx doctor runs");
    assert!(doctor.status.success());
    let doctor_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&doctor_report).unwrap()).unwrap();
    assert_eq!(doctor_json["status"], "ok");
    assert_eq!(doctor_json["checks"]["mobile_templates"], true);

    let world = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("crm-dashboard")
        .join("app.glyph.json");
    let schema = Command::new(&gx)
        .args(["schema", "check"])
        .arg(&world)
        .output()
        .expect("gx schema check runs");
    assert!(schema.status.success());

    let host_report = root.join("host-conformance.json");
    let host = Command::new(&gx)
        .args(["conformance", "--world"])
        .arg(world)
        .args(["--certify-host", "--out"])
        .arg(&host_report)
        .output()
        .expect("gx conformance host runs");
    assert!(host.status.success());
    let host_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&host_report).unwrap()).unwrap();
    assert_eq!(host_json["host_certified"], true);

    std::fs::remove_dir_all(root).ok();
}
