use glyphspace_dev::{
    CompilerDiagnosticParser, DevNotificationBackend, DevOrchestrator, DevProjectConfig,
    DevSupervisor, LiveWatcherStream, ManagedProcessSpec, SupervisorStateSnapshot,
};
use std::path::PathBuf;

#[cfg(windows)]
fn ok_process(name: &str) -> ManagedProcessSpec {
    ManagedProcessSpec::new(name, "cmd").with_args(["/C", "echo", name])
}

#[cfg(not(windows))]
fn ok_process(name: &str) -> ManagedProcessSpec {
    ManagedProcessSpec::new(name, "sh").with_args(["-c", &format!("echo {name}")])
}

#[test]
fn dev_orchestrator_executes_processes_preserves_state_and_streams_events() {
    let config = DevProjectConfig::new()
        .with_open_browser(true)
        .with_open_native(true)
        .with_ssr_command(ok_process("ssr").command)
        .with_profiling_enabled(true);
    let snapshot = SupervisorStateSnapshot::new("session-1")
        .with_world_digest("world:abc")
        .with_patch_count(4);

    let report = DevOrchestrator::new(PathBuf::from("examples/crm-dashboard-rust"), config)
        .with_process(ok_process("native"))
        .with_process(ok_process("wasm"))
        .with_process(ok_process("ssr"))
        .with_state_snapshot(snapshot.clone())
        .bootstrap()
        .expect("dev orchestrator boots");

    assert_eq!(report.processes.len(), 3);
    assert!(report.processes.iter().all(|process| process.success));
    assert_eq!(report.preserved_state, Some(snapshot));
    assert!(report.browser_session.is_some());
    assert!(report.native_session.is_some());
    assert!(
        report
            .devtools_events
            .iter()
            .any(|event| event.kind == "process_started")
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.source == "orchestrator")
    );
}

#[test]
fn live_watcher_stream_turns_os_notifications_into_reload_batches() {
    let supervisor = DevSupervisor::new(".", DevProjectConfig::default());
    let mut stream = LiveWatcherStream::from_backend(DevNotificationBackend::native(), supervisor);

    stream.ingest("src/app.rs");
    stream.ingest("examples/crm-dashboard/founder.lens.glyph.json");
    let batch = stream.next_batch();

    assert_eq!(batch.changes.len(), 2);
    assert!(batch.preserve_state);
    assert!(batch.stream_diagnostics);
    assert!(batch.requires_validation);
    assert!(
        batch
            .changes
            .iter()
            .any(|change| change.plan.rebuild_native && change.plan.rebuild_wasm)
    );
    assert!(
        batch
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.source == "notify-native")
    );
}

#[test]
fn compiler_diagnostic_parser_extracts_rust_error_code_location_and_hint() {
    let raw = "error[E0425]: cannot find value `stage` in this scope\n --> src/main.rs:17:9\n";
    let diagnostics = CompilerDiagnosticParser::parse("rust", raw);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].source, "rust");
    assert!(diagnostics[0].message.contains("E0425"));
    assert!(diagnostics[0].message.contains("src/main.rs:17"));
    assert!(diagnostics[0].hint.contains("cargo check"));
}
