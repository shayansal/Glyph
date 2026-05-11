use glyphspace_dev::{
    DevNotificationBackend, DevProjectConfig, DevRuntimeLoop, DevSupervisor, ManagedProcessSpec,
    OsFileEvent, OsFileEventKind, SupervisorStateSnapshot,
};
use std::time::Duration;

#[test]
fn dev_runtime_loop_combines_native_watcher_events_restarts_and_state_preservation() {
    let snapshot = SupervisorStateSnapshot::new("session-loop")
        .with_world_digest("world:loop")
        .with_patch_count(4);
    let supervisor = DevSupervisor::new(".", DevProjectConfig::default())
        .with_process(ManagedProcessSpec::new("native", "cargo run -p crm"))
        .with_process(ManagedProcessSpec::new("ssr", "cargo run -p server"));
    let mut runtime = DevRuntimeLoop::new(supervisor, DevNotificationBackend::native())
        .with_state_snapshot(snapshot.clone())
        .watch_recursive(".");

    runtime.ingest_file_event(OsFileEvent::new(
        OsFileEventKind::Modify,
        "examples/crm-dashboard/founder.lens.glyph.json",
    ));
    runtime.record_process_exit("ssr", 101);
    let tick = runtime.tick(Duration::from_millis(16));

    assert_eq!(tick.elapsed_ms, 16);
    assert!(tick.reload_batch.requires_validation);
    assert!(tick.reload_batch.preserve_state);
    assert_eq!(tick.restart_report.restart_attempts, 1);
    assert_eq!(tick.restart_report.preserved_state, Some(snapshot));
    assert!(
        tick.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.source == "notify-native")
    );
    assert!(
        tick.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.source == "ssr")
    );
    assert!(tick.events.iter().any(|event| event.kind == "reload_batch"));
    assert!(
        tick.events
            .iter()
            .any(|event| event.kind == "process_restart_scheduled")
    );
}

#[test]
fn dev_runtime_loop_spawns_child_processes_and_exposes_watcher_subscriptions() {
    let supervisor = DevSupervisor::new(".", DevProjectConfig::default()).with_process(
        ManagedProcessSpec::new("rustc-version", rustc_command()).with_args(["--version"]),
    );
    let runtime = DevRuntimeLoop::new(supervisor, DevNotificationBackend::native())
        .watch_recursive("examples")
        .watch_recursive("crates");

    let subscriptions = runtime.watcher_subscriptions();
    assert_eq!(subscriptions.len(), 2);
    assert!(
        subscriptions
            .iter()
            .all(|subscription| subscription.uses_os_notifications)
    );
    assert!(
        subscriptions
            .iter()
            .all(|subscription| subscription.event_kinds.contains(&"modify".to_string()))
    );

    let mut processes = runtime
        .spawn_child_processes()
        .expect("runtime launches child processes");
    let statuses = processes.processes();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].name, "rustc-version");
    assert!(statuses[0].pid > 0);

    let report = processes
        .wait_for_all(Duration::from_secs(5))
        .expect("child exits");
    assert!(report.exits.iter().all(|exit| exit.success));
    assert!(
        report
            .events
            .iter()
            .any(|event| event.kind == "child_process_started")
    );
    assert!(
        report
            .events
            .iter()
            .any(|event| event.kind == "child_process_exited")
    );
}

fn rustc_command() -> String {
    std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string())
}
