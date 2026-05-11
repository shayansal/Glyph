use glyphspace_app::{
    AdminKit, AgentKit, CrmKit, DashboardKit, DevtoolsReplay, FinanceKit, HotReloadEngine,
    NativeHostRuntime, PatchTimeline, ReactiveEffect, SemanticConformanceSuite, SemanticSsrServer,
    SuspenseBoundary, TypedSignal, WorkflowKit, glyph,
};
use glyphspace_core::{
    Capability, GlyphPatch, GlyphWorld, PatchOp, PolicyContext, Priority, RiskLevel,
};
use glyphspace_dsl::GlyphApp;
use glyphspace_layout::{DeviceProfile, Viewport};
use glyphspace_render::{ProductionRenderer, RenderSnapshot};

fn crm_world() -> GlyphWorld {
    GlyphApp::new("crm", "CRM")
        .capability(
            Capability::builder("deal.update_stage", "Update Deal Stage")
                .permission("crm.deal.write")
                .risk(RiskLevel::Medium)
                .build(),
        )
        .glyph(glyph!(
            metric("revenue", "Revenue").priority(Priority::Critical)
        ))
        .glyph(glyph!(button("deal", "Deal").binds("deal.update_stage")))
        .compile()
        .expect("world compiles")
}

#[test]
fn production_renderer_generates_deterministic_snapshots_and_applies_scene_diffs() {
    let world = crm_world();
    let mut renderer = ProductionRenderer::headless(Viewport::desktop(), DeviceProfile::desktop());

    let first = renderer.render_world(&world).expect("render succeeds");
    let snapshot = RenderSnapshot::from_frame(&first);
    let second = renderer
        .render_world(&world)
        .expect("second render succeeds");

    assert_eq!(snapshot.digest, RenderSnapshot::from_frame(&first).digest);
    assert_eq!(first.scene_patch.operations.len(), 0);
    assert_eq!(second.scene_patch.operations.len(), 0);
    assert!(snapshot.primitive_count >= 2);
    assert!(snapshot.accessibility_node_count >= 2);
}

#[test]
fn hot_reload_engine_reloads_glyph_lens_and_policy_files_with_semantic_diff_preview() {
    let world = crm_world();
    let mut hot_reload = HotReloadEngine::new(world.clone());
    let event = hot_reload
        .reload_manifest_text("app.glyph.json", &world.to_canonical_json().unwrap())
        .expect("manifest reloads");
    let lens_event = hot_reload
        .reload_patch_text(
            "founder.lens.glyph.json",
            &serde_json::to_string(&GlyphPatch::new(
                "founder",
                "Founder lens",
                vec![PatchOp::SetPriority {
                    glyph_id: "deal".to_string(),
                    priority: Priority::High,
                }],
            ))
            .unwrap(),
        )
        .expect("patch reloads");

    assert_eq!(event.path, "app.glyph.json");
    assert!(event.preserved_state);
    assert!(lens_event.semantic_diff.has_changes());
    assert!(
        hot_reload
            .devtools_events()
            .iter()
            .any(|event| event.kind == "patch_reloaded")
    );
}

#[test]
fn semantic_ssr_server_handles_capability_http_and_streams_world_updates() {
    let world = crm_world();
    let mut server = SemanticSsrServer::new(world.clone(), PolicyContext::demo_user());
    server.register_capability("deal.update_stage", |_input| {
        Ok(GlyphPatch::new(
            "server_stage",
            "Server moved stage",
            vec![PatchOp::SetPriority {
                glyph_id: "deal".to_string(),
                priority: Priority::High,
            }],
        ))
    });

    let html = server.render_accessibility_html().expect("html renders");
    let response = server
        .handle_capability_http(
            "deal.update_stage",
            serde_json::json!({"stage": "proposal"}),
        )
        .expect("capability route works");
    let stream = server.stream_world_updates();

    assert!(html.contains("role="));
    assert_eq!(response.status, 200);
    assert_eq!(response.patch.id, "server_stage");
    assert!(
        stream
            .events
            .iter()
            .any(|event| event.kind == "world.snapshot")
    );
}

#[test]
fn typed_reactivity_supports_memos_effects_suspense_and_glyph_invalidations() {
    let mut revenue = TypedSignal::new("revenue", 120_i64);
    let mut memo = revenue.memo("forecast", |value| value * 2);
    let mut effect = ReactiveEffect::new("audit_revenue", |value: &i64| {
        format!("revenue changed to {value}")
    });
    let mut suspense = SuspenseBoundary::new("deals_loader");

    revenue.set(150);
    let invalidations = revenue.take_invalidated_glyphs();
    let effect_message = effect.run(&revenue.get());
    memo.recompute(&revenue.get());
    suspense.pending();
    suspense.ready();

    assert_eq!(memo.value(), 300);
    assert!(effect_message.contains("150"));
    assert!(invalidations.contains(&"revenue".to_string()));
    assert!(suspense.is_ready());
}

#[test]
fn native_host_runtime_tracks_input_focus_mobile_profiles_and_offline_patches() {
    let world = crm_world();
    let mut host = NativeHostRuntime::desktop("crm")
        .with_mobile_profile("low_vision")
        .with_offline_patch_store("memory");
    let frame = host.render(&world).expect("host renders");
    let focused = host.focus_next(&frame).expect("focus advances");
    host.store_offline_patch(GlyphPatch::new("local", "Local patch", Vec::new()));

    assert!(host.input_events().contains(&"window.resumed".to_string()));
    assert!(!focused.is_empty());
    assert_eq!(host.offline_patches().len(), 1);
    assert!(
        host.mobile_lens_profiles()
            .contains(&"low_vision".to_string())
    );
}

#[test]
fn devtools_timeline_replays_unsafe_ai_proposals_and_layout_debug_info() {
    let world = crm_world();
    let mut timeline = PatchTimeline::new();
    timeline.record_patch(GlyphPatch::new("safe", "Safe patch", Vec::new()));
    timeline.record_audit("capability.invoked", "deal.update_stage");
    let replay = DevtoolsReplay::unsafe_ai_proposal(
        &world,
        GlyphPatch::new(
            "unsafe",
            "Hide deal",
            vec![PatchOp::Hide {
                glyph_id: "deal".to_string(),
            }],
        ),
        PolicyContext::demo_user(),
    );

    assert_eq!(timeline.events().len(), 2);
    assert!(replay.policy_explanation.summary.contains("Patch"));
    assert!(replay.layout_debug.render_primitive_count > 0);
    assert!(replay.accessibility_frame.verified);
}

#[test]
fn semantic_component_ecosystem_covers_core_application_domains() {
    let glyphs = [
        CrmKit::deal_card("deal", "Northstar"),
        FinanceKit::runway_metric("runway", 18),
        WorkflowKit::approval_task("approval", "Approve quote"),
        AdminKit::security_notice("security", "SOC2 notice"),
        AgentKit::operator("agent", "AI Operator"),
        DashboardKit::kpi_tile("kpi", "ARR", "$1.2M"),
    ];

    assert_eq!(glyphs.len(), 6);
    assert!(
        glyphs
            .iter()
            .all(|glyph| glyph.metadata.contains_key("kit"))
    );
}

#[test]
fn conformance_suite_certifies_renderer_policy_accessibility_host_and_patch_compatibility() {
    let world = crm_world();
    let report = SemanticConformanceSuite::strict()
        .with_world(world)
        .certify()
        .expect("suite runs");

    assert!(report.passed);
    assert!(
        report
            .certifications
            .contains(&"renderer_determinism".to_string())
    );
    assert!(
        report
            .certifications
            .contains(&"patch_compatibility".to_string())
    );
    assert!(report.certifications.contains(&"host_adapter".to_string()));
}
