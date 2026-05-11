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
