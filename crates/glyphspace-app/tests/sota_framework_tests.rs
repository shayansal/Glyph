use glyphspace_app::{
    AsyncResource, AsyncResourceState, ConformanceHarness, HeadlessSemanticHost, HostAdapterSpec,
    NativeWindowOptions, PolicyStudio, ReactiveGraph, accessibility_frame, capability, component,
    glyph_app, glyph_component, interop, lens,
};
use glyphspace_core::{
    Capability, Glyph, GlyphPatch, PatchOp, PolicyContext, PolicyZone, Priority,
};
use glyphspace_dsl::{GlyphApp, Lens};
use glyphspace_layout::{DeviceProfile, Viewport};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
struct MacroState {
    stage: String,
}

#[derive(Clone, Debug, Deserialize)]
struct StageInput {
    stage: String,
}

#[derive(Clone, Debug, Serialize)]
struct StageOutput {
    stage: String,
}

#[glyph_component]
fn stage_component(state: &MacroState) -> Vec<Glyph> {
    vec![Glyph::metric("stage", format!("Stage: {}", state.stage)).priority(Priority::High)]
}

#[capability(
    id = "deal.update_stage",
    name = "Update Deal Stage",
    permission = "crm.deal.write",
    risk = "medium"
)]
fn update_stage(state: &mut MacroState, input: StageInput) -> StageOutput {
    state.stage = input.stage;
    StageOutput {
        stage: state.stage.clone(),
    }
}

#[lens(id = "founder", description = "Founder command center")]
fn founder_lens() -> GlyphPatch {
    Lens::new("founder", "Founder command center")
        .op(PatchOp::SetPriority {
            glyph_id: "stage".to_string(),
            priority: Priority::Critical,
        })
        .into()
}

#[glyph_app(id = "crm_macro", name = "CRM Macro App")]
fn macro_app() -> GlyphApp {
    GlyphApp::new("crm_macro", "CRM Macro App")
        .capability(update_stage_manifest())
        .glyph(Glyph::button("deal", "Deal").binds("deal.update_stage"))
}

#[test]
fn proc_macros_make_rust_authoring_feel_native() {
    let app = macro_app();
    let world = app.compile().expect("macro app compiles");
    let manifest = update_stage_manifest();
    let patch = founder_lens();

    assert_eq!(world.id, "crm_macro");
    assert_eq!(manifest.id, "deal.update_stage");
    assert_eq!(manifest.required_permissions, vec!["crm.deal.write"]);
    assert_eq!(patch.id, "founder");

    let mut state = MacroState {
        stage: "proposal".to_string(),
    };
    let output = update_stage(
        &mut state,
        StageInput {
            stage: "negotiation".to_string(),
        },
    );
    assert_eq!(output.stage, "negotiation");
    assert_eq!(stage_component(&state)[0].label, "Stage: negotiation");
}

#[test]
fn reactive_graph_computes_dependencies_and_cancelable_resources() {
    let mut graph = ReactiveGraph::new();
    let revenue = graph.signal("revenue", 120);
    let risk = graph.signal("risk", 30);
    let health = graph.computed("health", [revenue, risk], |values| values[0] - values[1]);

    assert_eq!(graph.value(health), Some(90));
    graph.set(revenue, 150);
    assert_eq!(graph.value(health), Some(120));
    assert_eq!(graph.dirty_components(), vec!["health"]);

    let mut resource = AsyncResource::<String>::pending("deals.fetch");
    let token = resource.cancel_token();
    resource.resolve("loaded".to_string());
    assert_eq!(
        resource.state(),
        &AsyncResourceState::Ready("loaded".to_string())
    );
    token.cancel();
    resource.resolve("late".to_string());
    assert_eq!(resource.state(), &AsyncResourceState::Canceled);
}

#[test]
fn host_spec_interop_and_conformance_are_executable_contracts() {
    let host_spec = HostAdapterSpec::native_window("desktop")
        .render_surface("wgpu")
        .accessibility_mirror("semantic-tree")
        .audit_sink("memory")
        .storage("patch-store");
    let interop = interop::FrameworkBridge::yew("crm")
        .imports_state("deals")
        .with_semantic_mirror_export();
    let report = ConformanceHarness::new()
        .require_canonical_serialization()
        .require_policy_invariants()
        .require_accessibility_frame()
        .require_host_adapter(host_spec.clone())
        .check();

    assert!(host_spec.is_complete());
    assert_eq!(interop.framework(), "yew");
    assert!(interop.exports_semantic_mirror());
    assert!(report.passed);
    assert!(report.checks.iter().any(|check| check == "host_adapter"));
}

#[test]
fn accessibility_frame_survives_personalization_and_reports_focus() {
    let app = GlyphApp::new("access", "Access")
        .glyph(Glyph::button("deal", "Deal").binds("deal.update_stage"))
        .capability(
            Capability::builder("deal.update_stage", "Update Deal Stage")
                .permission("crm.deal.write")
                .risk(glyphspace_core::RiskLevel::Medium)
                .build(),
        );
    let runtime = glyphspace_app::AppRuntime::new(
        app,
        MacroState {
            stage: "proposal".to_string(),
        },
        PolicyContext::demo_user(),
    )
    .with_component(component(stage_component))
    .mount()
    .expect("runtime mounts");
    let mut host = HeadlessSemanticHost::new(Viewport::desktop(), DeviceProfile::desktop());
    let frame = runtime.render(&mut host).expect("render succeeds");
    let access_frame = accessibility_frame(&frame);

    assert!(access_frame.verified);
    assert!(access_frame.focus_order.contains(&"deal".to_string()));
    assert!(access_frame.spatial_descriptions["deal"].contains("glyph deal"));
}

#[test]
fn policy_studio_explains_rejections_and_safe_movable_parts() {
    let world = GlyphApp::new("policy", "Policy")
        .glyph(
            Glyph::button("payment_confirmation", "Payment confirmation")
                .with_policy_zone(PolicyZone::Payment)
                .mandatory(),
        )
        .compile()
        .expect("world compiles");
    let patch = GlyphPatch::new(
        "unsafe",
        "Hide payment confirmation",
        vec![
            PatchOp::Hide {
                glyph_id: "payment_confirmation".to_string(),
            },
            PatchOp::Move {
                glyph_id: "payment_confirmation".to_string(),
                pose: glyphspace_core::GlyphPose::at(1.0, 1.0, 0.0),
            },
        ],
    );
    let studio = PolicyStudio::new(PolicyContext::demo_user());
    let explanation = studio.explain_patch(&world, &world, &patch);

    assert!(!explanation.allowed);
    assert!(explanation.summary.contains("cannot hide"));
    assert!(
        explanation
            .allowed_operations
            .iter()
            .any(|op| op.contains("move"))
    );
    assert!(
        explanation
            .denied_operations
            .iter()
            .any(|op| op.contains("hide"))
    );
    assert_eq!(explanation.audit_events[0].subject, "unsafe");
}

#[test]
fn native_window_options_describe_real_winit_wgpu_runner() {
    let options = NativeWindowOptions::new("Glyphspace CRM")
        .with_viewport(Viewport::desktop())
        .with_camera_controls(true)
        .with_animation_ticks(true)
        .with_focus_traversal(true);

    assert_eq!(options.title, "Glyphspace CRM");
    assert!(options.camera_controls);
    assert!(options.animation_ticks);
    assert!(options.focus_traversal);
}
