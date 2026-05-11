use glyphspace_dev::{DevConfig, DevProcessManager, DevTarget};
use std::time::Duration;

#[test]
fn dev_process_manager_boots_targets_preserves_state_and_streams_events() {
    let config = DevConfig::new()
        .with_target(DevTarget::Native)
        .with_target(DevTarget::Web)
        .with_ssr(true)
        .with_watch(true)
        .with_browser(true)
        .with_state_key("crm-dashboard");

    let mut manager = DevProcessManager::new(config);
    let session = manager.start().expect("dev manager starts");

    assert!(session.long_running);
    assert!(session.targets.contains(&DevTarget::Native));
    assert!(session.targets.contains(&DevTarget::Web));
    assert!(session.ssr_server.running);
    assert!(session.watcher.running);
    assert_eq!(
        session.preserved_state_key.as_deref(),
        Some("crm-dashboard")
    );

    let tick = manager
        .tick(Duration::from_millis(16))
        .expect("dev manager ticks");
    assert!(
        tick.events
            .iter()
            .any(|event| event.kind == "hot_reload_idle")
    );
    assert!(
        tick.events
            .iter()
            .any(|event| event.kind == "devtools_heartbeat")
    );

    let report = session.report();
    assert_eq!(report["process_manager"], true);
    assert_eq!(report["state_preservation"], true);
    assert_eq!(report["devtools_stream"], "glyphspace://devtools/events");
}
