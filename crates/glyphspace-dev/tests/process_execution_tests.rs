use glyphspace_dev::{
    DevCommandExecutor, DevNotificationBackend, DevProcessSupervisor, DevProjectConfig,
    ManagedProcessSpec, SupervisorStateSnapshot,
};

#[test]
fn command_executor_runs_rebuild_commands_and_streams_diagnostics() {
    let result = DevCommandExecutor::new()
        .run_once(ManagedProcessSpec::new("rust-version", rustc_command()).with_args(["--version"]))
        .expect("run rustc");

    assert!(result.success);
    assert_eq!(result.process, "rust-version");
    assert!(result.stdout.contains("rustc"));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.source == "rust-version")
    );
}

#[test]
fn process_supervisor_restarts_ssr_and_preserves_session_state() {
    let config =
        DevProjectConfig::default().with_ssr_command(format!("{} --version", rustc_command()));
    let snapshot = SupervisorStateSnapshot::new("session-1")
        .with_world_digest("world-a")
        .with_patch_count(3);
    let mut supervisor = DevProcessSupervisor::new(config).with_state_snapshot(snapshot);

    let restart = supervisor.restart_ssr_safely().expect("restart ssr");

    assert!(restart.success);
    assert_eq!(restart.process, "ssr");
    assert_eq!(restart.restart_count, 1);
    assert_eq!(
        restart.preserved_state.as_ref().unwrap().state_key,
        "session-1"
    );
    assert!(
        restart
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("ssr restarted"))
    );
}

#[test]
fn native_notification_backend_describes_os_file_watching_contract() {
    let backend = DevNotificationBackend::native();

    assert!(backend.uses_os_notifications);
    assert!(backend.recursive);
    assert_eq!(backend.backend_name, "notify-native");
    assert!(
        backend
            .watched_kinds
            .contains(&"glyph_manifest".to_string())
    );
    assert!(backend.watched_kinds.contains(&"rust".to_string()));
}

fn rustc_command() -> String {
    std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string())
}
