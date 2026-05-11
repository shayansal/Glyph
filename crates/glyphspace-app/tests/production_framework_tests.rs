use glyphspace_app::{
    AccessiblePrimitiveSet, ComponentLifecycle, ComponentProps, DevtoolsProductApp,
    DiagnosticBundle, DistributionReadiness, FormGlyph, ListGlyph, MenuGlyph, MobileFfiBuildPlan,
    NativeDesktopIntegration, NoJsWebRuntime, PatchPersistence, ProductComponent,
    RuntimeTransaction, SemanticSyncEngine, SlotChildren, TableGlyph, TypedEvent,
};
use glyphspace_core::{Glyph, GlyphPatch, PatchOp};

#[test]
fn production_components_support_props_slots_typed_events_and_lifecycle() {
    let props = ComponentProps::new("deal-card")
        .with("deal_id", "deal_1")
        .with_bool("selected", true);
    let children = SlotChildren::new()
        .slot("header", Glyph::metric("revenue", "Revenue"))
        .slot("actions", Glyph::button("close", "Close Deal"));
    let component = ProductComponent::new("DealCard", props)
        .with_children(children)
        .on(TypedEvent::click("deal.open"))
        .on(TypedEvent::keyboard("Enter", "deal.open"))
        .with_lifecycle(ComponentLifecycle::mounted().then("hydrate_capabilities"));

    assert_eq!(component.name, "DealCard");
    assert_eq!(component.props.get("deal_id").unwrap(), "deal_1");
    assert!(component.children.slots.contains_key("header"));
    assert_eq!(component.events.len(), 2);
    assert_eq!(
        component.lifecycle.hooks,
        vec!["mounted", "hydrate_capabilities"]
    );
}

#[test]
fn semantic_primitives_include_accessible_forms_tables_lists_menus_dialogs_and_nav() {
    let primitives = AccessiblePrimitiveSet::production_defaults()
        .with_form(
            FormGlyph::new("deal-form")
                .field("stage")
                .submit("deal.update_stage"),
        )
        .with_table(TableGlyph::new("pipeline").column("stage").column("amount"))
        .with_list(ListGlyph::new("followups").item("Call CFO"))
        .with_menu(MenuGlyph::new("main-menu").item("Dashboard", "/"));

    assert!(primitives.all_have_accessible_defaults());
    assert!(
        primitives
            .keyboard_bindings
            .contains(&"Enter activates button".to_string())
    );
    assert!(
        primitives
            .keyboard_bindings
            .contains(&"Escape closes dialog".to_string())
    );
    assert_eq!(primitives.forms[0].submit_capability, "deal.update_stage");
    assert_eq!(primitives.tables[0].columns, vec!["stage", "amount"]);
}

#[test]
fn runtime_transactions_undo_redo_persistence_and_sync_conflicts_are_first_class() {
    let patch = GlyphPatch::new(
        "move_revenue",
        "Move revenue",
        vec![PatchOp::Move {
            glyph_id: "revenue".to_string(),
            pose: glyphspace_core::GlyphPose::at(1.0, 2.0, 0.0),
        }],
    );
    let mut transaction = RuntimeTransaction::new("txn-1")
        .push_patch(patch.clone())
        .commit("user");
    transaction.undo().expect("undo");
    transaction.redo().expect("redo");

    let mut store = PatchPersistence::memory("device-a");
    store.save(&patch).expect("save patch");

    let conflict = SemanticSyncEngine::new()
        .with_server_patch(patch.clone())
        .with_user_patch(patch)
        .detect_conflicts();

    assert!(transaction.committed);
    assert_eq!(transaction.undo_stack.len(), 1);
    assert_eq!(store.pending_offline_queue().len(), 1);
    assert!(conflict.has_conflict);
    assert!(
        conflict
            .resolution_options
            .contains(&"manual_review".to_string())
    );
}

#[test]
fn no_js_web_runtime_owns_routing_state_events_accessibility_and_hydration() {
    let runtime = NoJsWebRuntime::rust_owned("crm")
        .route("/", "world")
        .event("glyph.click", "capability.invoke")
        .with_webgpu_renderer()
        .with_dom_accessibility_mirror()
        .with_ssr_hydration("world-digest")
        .with_streaming_semantic_diffs();

    assert!(runtime.minimal_js_glue);
    assert!(runtime.rust_owned_routing);
    assert!(runtime.rust_owned_state);
    assert!(runtime.webgpu_renderer);
    assert!(runtime.dom_accessibility_mirror_from_rust);
    assert_eq!(runtime.hydration_digest.as_deref(), Some("world-digest"));
}

#[test]
fn native_mobile_devtools_and_distribution_surfaces_are_trackable() {
    let desktop = NativeDesktopIntegration::new()
        .with_menus()
        .with_clipboard()
        .with_drag_drop()
        .with_file_dialogs()
        .with_notifications()
        .with_ime()
        .with_packaging("msi");
    let mobile = MobileFfiBuildPlan::ios_and_android("glyphspace_crm")
        .with_swift_bindings()
        .with_kotlin_bindings()
        .with_touch_gestures()
        .with_deep_links()
        .with_lifecycle_hooks();
    let devtools = DevtoolsProductApp::new("crm")
        .with_visual_inspector()
        .with_performance_flamegraph()
        .with_hot_reload_timeline();
    let bundle = DiagnosticBundle::capture("session-1")
        .with_world_graph("graph.json")
        .with_audit_log("audit.json");
    let release = DistributionReadiness::new("0.1.0")
        .with_crates()
        .with_npm_wrapper()
        .with_schema_package()
        .with_docs_site()
        .with_ci_matrix()
        .with_security_policy();

    assert!(desktop.ready_for_packaged_desktop());
    assert!(mobile.has_native_bindings());
    assert!(devtools.inspectors.contains(&"render_frame".to_string()));
    assert_eq!(bundle.artifacts.len(), 2);
    assert!(release.publishable());
}
