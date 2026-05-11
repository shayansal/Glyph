use glyphspace_app::{
    CapabilityFunctionRegistry, ComponentKit, DevtoolsSnapshot, MobileHostAdapter, RouteTarget,
    SemanticRoute, SemanticRouter, SemanticSsrSnapshot, glyph,
};
use glyphspace_core::{
    Capability, GlyphPatch, GlyphWorld, PatchOp, PolicyContext, Priority, RiskLevel,
};
use glyphspace_dsl::GlyphApp;

#[test]
fn glyph_macro_and_component_kit_author_semantic_ui_without_json() {
    let revenue = glyph!(metric("revenue", "Revenue").priority(Priority::Critical));
    let close = glyph!(button("close_deal", "Close deal").binds("deal.close"));
    let risk = ComponentKit::risk_glyph("risk", "Pipeline risk", Priority::High);
    let confirmation = ComponentKit::confirmation_glyph("confirm_close", "Confirm close deal");

    assert_eq!(revenue.id, "revenue");
    assert_eq!(revenue.priority, Priority::Critical);
    assert_eq!(close.capability_bindings[0].capability_id, "deal.close");
    assert!(risk.metadata.contains_key("kit"));
    assert!(confirmation.mandatory);
}

#[test]
fn semantic_router_maps_urls_to_lenses_focus_and_accessibility_landmarks() {
    let router = SemanticRouter::new()
        .route(SemanticRoute::new("/", RouteTarget::World).lens("founder"))
        .route(
            SemanticRoute::new("/deals/:id", RouteTarget::Glyph("deal".to_string()))
                .camera("deal-focus")
                .accessibility_landmark("Deal detail"),
        );

    let matched = router.resolve("/deals/123").expect("route matches");

    assert_eq!(matched.target, RouteTarget::Glyph("deal".to_string()));
    assert_eq!(matched.params["id"], "123");
    assert_eq!(matched.camera.as_deref(), Some("deal-focus"));
    assert_eq!(
        matched.accessibility_landmark.as_deref(),
        Some("Deal detail")
    );
}

#[test]
fn capability_functions_are_policy_audited_semantic_server_functions() {
    let mut world = GlyphWorld::new("crm", "CRM");
    world.capabilities.insert(
        "deal.update_stage".to_string(),
        Capability::builder("deal.update_stage", "Update Deal Stage")
            .permission("crm.deal.write")
            .risk(RiskLevel::Medium)
            .build(),
    );
    let mut registry = CapabilityFunctionRegistry::new(PolicyContext::demo_user());
    registry.register("deal.update_stage", |_input| {
        Ok(GlyphPatch::new(
            "stage_patch",
            "Stage moved",
            vec![PatchOp::SetPriority {
                glyph_id: "deal".to_string(),
                priority: Priority::High,
            }],
        ))
    });

    let result = registry
        .invoke(
            &world,
            "deal.update_stage",
            serde_json::json!({"stage": "proposal"}),
        )
        .expect("capability runs");

    assert_eq!(result.patch.id, "stage_patch");
    assert_eq!(result.audit.action, "capability.function.invoked");
}

#[test]
fn semantic_ssr_snapshot_hydrates_world_accessibility_policy_and_patch_digest() {
    let world = GlyphApp::new("crm", "CRM")
        .glyph(glyph!(metric("revenue", "Revenue")))
        .compile()
        .expect("world compiles");
    let snapshot = SemanticSsrSnapshot::from_world(&world, &PolicyContext::demo_user())
        .expect("snapshot serializes");
    let hydrated = snapshot.hydrate().expect("snapshot hydrates");

    assert_eq!(hydrated.world.id, "crm");
    assert!(hydrated.accessibility_tree.nodes.contains_key("revenue"));
    assert_eq!(
        snapshot.world_digest,
        hydrated.world.canonical_digest().unwrap()
    );
}

#[test]
fn mobile_host_adapter_declares_native_accessibility_and_offline_patch_storage() {
    let adapter = MobileHostAdapter::ios("crm")
        .with_native_accessibility_bridge("UIAccessibility")
        .with_offline_patch_store("sqlite")
        .with_lens_profile("low_vision");

    assert!(adapter.is_complete());
    assert_eq!(adapter.platform, "ios");
    assert_eq!(adapter.lens_profiles, vec!["low_vision"]);
}

#[test]
fn devtools_snapshot_exposes_world_policy_audit_accessibility_and_capabilities() {
    let world = GlyphApp::new("crm", "CRM")
        .capability(
            Capability::builder("deal.update_stage", "Update Deal Stage")
                .permission("crm.deal.write")
                .risk(RiskLevel::Medium)
                .build(),
        )
        .glyph(glyph!(button("deal", "Deal").binds("deal.update_stage")))
        .compile()
        .expect("world compiles");
    let snapshot = DevtoolsSnapshot::inspect(&world, &PolicyContext::demo_user());

    assert_eq!(snapshot.world_id, "crm");
    assert!(
        snapshot
            .capabilities
            .contains(&"deal.update_stage".to_string())
    );
    assert!(snapshot.accessibility_nodes.contains(&"deal".to_string()));
    assert!(snapshot.policy_summary.contains("authority"));
}
