use glyphspace_app::{
    AxumSsrAdapter, HotReloadEngine, MobileShell, MobileShellKind, SemanticSsrServer,
};
use glyphspace_core::{
    Capability, Glyph, GlyphPatch, GlyphWorld, PatchOp, PolicyContext, Priority, RiskLevel,
};
use std::fs;

fn crm_world() -> GlyphWorld {
    let mut world = GlyphWorld::new("crm", "CRM");
    world.capabilities.insert(
        "deal.update_stage".to_string(),
        Capability::builder("deal.update_stage", "Update Deal Stage")
            .permission("crm.deal.write")
            .risk(RiskLevel::Medium)
            .build(),
    );
    world
        .add_glyph(Glyph::button("deal", "Deal").binds("deal.update_stage"))
        .unwrap();
    world
}

#[test]
fn hot_reload_watches_files_preserves_state_and_emits_devtools_stream() {
    let world = crm_world();
    let dir = std::env::temp_dir().join(format!("glyphspace-hot-reload-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let app_path = dir.join("app.glyph.json");
    let lens_path = dir.join("founder.lens.glyph.json");
    fs::write(&app_path, world.to_canonical_json().unwrap()).unwrap();
    fs::write(
        &lens_path,
        serde_json::to_string(&GlyphPatch::new(
            "founder",
            "Founder lens",
            vec![PatchOp::SetPriority {
                glyph_id: "deal".to_string(),
                priority: Priority::Critical,
            }],
        ))
        .unwrap(),
    )
    .unwrap();

    let mut engine = HotReloadEngine::new(world)
        .watch_manifest(app_path.clone())
        .watch_patch(lens_path.clone());
    let batch = engine.reload_changed_files().expect("watched files reload");

    assert_eq!(batch.events.len(), 2);
    assert!(batch.preserved_state);
    assert!(batch.semantic_diff.has_changes());
    assert!(
        engine
            .devtools_event_stream()
            .iter()
            .any(|event| event.kind == "hot_reload.batch")
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn axum_ssr_adapter_exposes_world_accessibility_capability_and_stream_routes() {
    let world = crm_world();
    let mut server = SemanticSsrServer::new(world, PolicyContext::demo_user());
    server.register_capability("deal.update_stage", |_input| {
        Ok(GlyphPatch::new(
            "stage",
            "Stage update",
            vec![PatchOp::SetPriority {
                glyph_id: "deal".to_string(),
                priority: Priority::High,
            }],
        ))
    });

    let adapter = AxumSsrAdapter::new(server)
        .route_world("/glyphspace/world")
        .route_accessibility("/glyphspace/a11y")
        .route_capability("/glyphspace/capability/:id")
        .route_stream("/glyphspace/stream");
    let manifest = adapter.route_manifest();

    assert!(manifest.axum_backed);
    assert!(manifest.routes.contains(&"/glyphspace/world".to_string()));
    assert!(
        adapter
            .render_accessibility_response()
            .expect("accessibility response")
            .body
            .contains("data-glyphspace-accessibility")
    );
}

#[test]
fn mobile_shells_queue_offline_patches_and_export_native_bridge_frames() {
    let world = crm_world();
    let mut shell = MobileShell::ios("crm")
        .with_lens_profile("mobile-founder")
        .with_offline_store("sqlite")
        .with_native_accessibility_bridge("ui-accessibility")
        .with_push_channel("glyphspace://patches");
    shell.queue_offline_patch(GlyphPatch::new("offline", "Offline edit", Vec::new()));
    let frame = shell
        .render_bridge_frame(&world)
        .expect("mobile bridge frame");

    assert_eq!(shell.kind(), MobileShellKind::Ios);
    assert_eq!(shell.queued_patches().len(), 1);
    assert!(frame.accessibility_nodes >= 1);
    assert_eq!(frame.patch_queue_depth, 1);
    assert!(frame.native_bridge.contains("ui-accessibility"));
}
