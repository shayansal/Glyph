use glyphspace_dev::{
    DevDiagnosticSeverity, DevProjectConfig, DevSupervisor, ManagedProcessSpec, WatchKind,
    WatchRule,
};
use std::path::PathBuf;

#[test]
fn supervisor_classifies_watch_paths_and_plans_incremental_reloads() {
    let supervisor = DevSupervisor::new(
        PathBuf::from("demo"),
        DevProjectConfig::default()
            .with_native_command("cargo run -p crm-dashboard-rust")
            .with_wasm_command("scripts/build-wasm.sh")
            .with_ssr_command("cargo run -p glyphspace-cli -- ssr"),
    )
    .with_watch_rules(WatchRule::default_project_rules());

    let rust = supervisor.plan_change("src/main.rs");
    assert_eq!(rust.kind, WatchKind::Rust);
    assert!(rust.rebuild_native);
    assert!(rust.restart_processes.contains(&"native".to_string()));
    assert!(rust.preserve_state);
    assert!(rust.incremental_reload);

    let glyph = supervisor.plan_change("examples/crm-dashboard/app.glyph.json");
    assert_eq!(glyph.kind, WatchKind::GlyphManifest);
    assert!(glyph.revalidate_schema);
    assert!(glyph.revalidate_policy);
    assert!(glyph.stream_diagnostics);

    let asset = supervisor.plan_change("assets/logo.png");
    assert_eq!(asset.kind, WatchKind::Asset);
    assert!(asset.rebuild_wasm);
    assert!(!asset.restart_ssr);
}

#[test]
fn supervisor_loads_project_config_and_reports_process_health() {
    let config = DevProjectConfig::parse(
        r#"
        native = "cargo run -p crm-dashboard-rust"
        wasm = "scripts/build-wasm.sh"
        ssr = "cargo run -p glyphspace-cli -- ssr"
        open_browser = true
        open_native = true
        "#,
    )
    .expect("config parses");
    let supervisor = DevSupervisor::new(PathBuf::from("demo"), config)
        .with_process(ManagedProcessSpec::new("native", current_exe_command()))
        .with_process(ManagedProcessSpec::new("wasm", current_exe_command()));

    let health = supervisor.health_report();
    assert_eq!(health.processes.len(), 2);
    assert!(health.open_browser);
    assert!(health.open_native);
    assert!(health.profiling_enabled);
    assert!(health.logs.iter().any(|line| line.contains("native")));
}

#[test]
fn supervisor_turns_failures_into_friendly_diagnostics_and_recovery_plan() {
    let supervisor = DevSupervisor::new(PathBuf::from("demo"), DevProjectConfig::default());
    let diagnostic =
        supervisor.friendly_error("schema", "missing field `spec_version` at line 1 column 2");

    assert_eq!(diagnostic.severity, DevDiagnosticSeverity::Error);
    assert_eq!(diagnostic.source, "schema");
    assert!(diagnostic.message.contains("spec_version"));
    assert!(diagnostic.hint.contains("gx schema check"));

    let recovery = supervisor.crash_recovery("ssr", 101);
    assert!(recovery.should_restart);
    assert!(recovery.preserve_state);
    assert!(recovery.backoff_ms >= 250);
    assert!(recovery.diagnostic.message.contains("ssr crashed"));
}

fn current_exe_command() -> String {
    std::env::current_exe().unwrap().display().to_string()
}
