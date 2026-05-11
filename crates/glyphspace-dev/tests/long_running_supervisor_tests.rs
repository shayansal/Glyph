use glyphspace_dev::{
    DevProjectConfig, DevSupervisor, LongRunningDevSupervisor, ManagedProcessSpec,
    SupervisorStateSnapshot,
};

#[test]
fn long_running_supervisor_restarts_crashed_process_and_preserves_state() {
    let snapshot = SupervisorStateSnapshot::new("session-99")
        .with_world_digest("world:99")
        .with_patch_count(7);
    let supervisor = DevSupervisor::new(".", DevProjectConfig::default())
        .with_process(ManagedProcessSpec::new("ssr", "cargo run -p ssr"))
        .with_process(ManagedProcessSpec::new("native", "cargo run -p app"));
    let mut runtime =
        LongRunningDevSupervisor::new(supervisor).with_state_snapshot(snapshot.clone());

    runtime.record_process_exit("ssr", 101);
    let report = runtime.drain_restart_report();

    assert_eq!(report.restart_attempts, 1);
    assert_eq!(report.preserved_state, Some(snapshot));
    assert!(
        report
            .events
            .iter()
            .any(|event| event.kind == "process_crashed")
    );
    assert!(
        report
            .events
            .iter()
            .any(|event| event.kind == "process_restart_scheduled")
    );
    assert!(
        report
            .processes
            .iter()
            .any(|process| process.name == "ssr" && process.restart_count == 1)
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("ssr crashed"))
    );
}

#[test]
fn long_running_supervisor_reports_healthy_processes_without_restart() {
    let supervisor = DevSupervisor::new(".", DevProjectConfig::default())
        .with_process(ManagedProcessSpec::new("native", "cargo run -p app"));
    let mut runtime = LongRunningDevSupervisor::new(supervisor);

    runtime.record_process_heartbeat("native");
    let report = runtime.drain_restart_report();

    assert_eq!(report.restart_attempts, 0);
    assert!(
        report
            .events
            .iter()
            .any(|event| event.kind == "process_heartbeat")
    );
    assert!(
        report
            .processes
            .iter()
            .any(|process| process.name == "native" && process.running)
    );
}
