use glyphspace_dev::{DevFileWatcher, DevProjectConfig, DevSupervisor, WatchKind};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn file_watcher_detects_changed_project_files_and_builds_reload_batch() {
    let root = temp_project_root("watcher-detects-changes");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("examples").join("crm-dashboard")).unwrap();
    fs::create_dir_all(root.join("assets")).unwrap();
    fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
    fs::write(
        root.join("examples")
            .join("crm-dashboard")
            .join("app.glyph.json"),
        "{\"spec_version\":\"0.1.0\"}\n",
    )
    .unwrap();
    fs::write(root.join("assets").join("logo.svg"), "<svg />\n").unwrap();

    let supervisor = DevSupervisor::new(&root, DevProjectConfig::default());
    let mut watcher = DevFileWatcher::new(&root, supervisor);
    watcher.prime().unwrap();

    fs::write(
        root.join("src").join("main.rs"),
        "fn main() { println!(\"hi\"); }\n",
    )
    .unwrap();
    fs::write(
        root.join("assets").join("logo.svg"),
        "<svg><path /></svg>\n",
    )
    .unwrap();

    let batch = watcher.scan_changes().unwrap();
    assert_eq!(batch.changes.len(), 2);
    assert!(batch.preserve_state);
    assert!(batch.stream_diagnostics);
    assert!(
        batch
            .changes
            .iter()
            .any(|change| change.kind == WatchKind::Rust && change.plan.rebuild_native)
    );
    assert!(
        batch
            .changes
            .iter()
            .any(|change| change.kind == WatchKind::Asset && change.plan.rebuild_wasm)
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn file_watcher_classifies_new_lens_policy_and_schema_files() {
    let root = temp_project_root("watcher-classifies-new-files");
    fs::create_dir_all(root.join("examples").join("crm-dashboard")).unwrap();
    fs::create_dir_all(root.join("schemas")).unwrap();

    let supervisor = DevSupervisor::new(&root, DevProjectConfig::default());
    let mut watcher = DevFileWatcher::new(&root, supervisor);
    watcher.prime().unwrap();

    fs::write(
        root.join("examples")
            .join("crm-dashboard")
            .join("founder.lens.glyph.json"),
        "{}\n",
    )
    .unwrap();
    fs::write(
        root.join("examples")
            .join("crm-dashboard")
            .join("enterprise.policy.glyph.json"),
        "{}\n",
    )
    .unwrap();
    fs::write(root.join("schemas").join("glyphspace.schema.json"), "{}\n").unwrap();

    let batch = watcher.scan_changes().unwrap();
    let kinds = batch
        .changes
        .iter()
        .map(|change| change.kind)
        .collect::<Vec<_>>();
    assert!(kinds.contains(&WatchKind::Lens));
    assert!(kinds.contains(&WatchKind::Policy));
    assert!(kinds.contains(&WatchKind::Schema));
    assert!(batch.requires_validation);
    assert!(!batch.requires_full_restart);

    let _ = fs::remove_dir_all(root);
}

fn temp_project_root(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("glyphspace-{label}-{nanos}"))
}

#[allow(dead_code)]
fn assert_exists(path: &Path) {
    assert!(path.exists(), "{} should exist", path.display());
}
