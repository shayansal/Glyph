use glyphspace_dev::{
    DevNotificationBackend, DevProjectConfig, DevSupervisor, NativeOsWatcherBridge, OsFileEvent,
    OsFileEventKind, WatchKind,
};

#[test]
fn native_os_watcher_bridge_converts_os_events_into_reload_batches() {
    let supervisor = DevSupervisor::new(".", DevProjectConfig::default());
    let mut bridge = NativeOsWatcherBridge::new(DevNotificationBackend::native(), supervisor)
        .watch_recursive(".");

    bridge.ingest_event(OsFileEvent::new(
        OsFileEventKind::Modify,
        "src/native_window.rs",
    ));
    bridge.ingest_event(OsFileEvent::new(
        OsFileEventKind::Create,
        "schemas/glyphspace.schema.json",
    ));

    let batch = bridge.drain_reload_batch();

    assert_eq!(batch.changes.len(), 2);
    assert!(
        batch
            .changes
            .iter()
            .any(|change| change.kind == WatchKind::Rust)
    );
    assert!(
        batch
            .changes
            .iter()
            .any(|change| change.kind == WatchKind::Schema)
    );
    assert!(batch.requires_validation);
    assert!(batch.stream_diagnostics);
    assert!(
        batch
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("Modify"))
    );
}

#[test]
fn native_os_watcher_bridge_reports_real_backend_capabilities() {
    let supervisor = DevSupervisor::new(".", DevProjectConfig::default());
    let bridge = NativeOsWatcherBridge::new(DevNotificationBackend::native(), supervisor)
        .watch_recursive("crates")
        .watch_recursive("examples");
    let report = bridge.capability_report();

    assert_eq!(report.backend, "notify-native");
    assert!(report.uses_os_notifications);
    assert!(report.recursive);
    assert_eq!(report.watched_roots.len(), 2);
    assert!(report.event_kinds.contains(&"create".to_string()));
    assert!(report.event_kinds.contains(&"modify".to_string()));
    assert!(report.event_kinds.contains(&"remove".to_string()));
}
