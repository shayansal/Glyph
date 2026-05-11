use glyphspace_app::{
    AppRuntime, CapabilityOutput, HeadlessSemanticHost, Signal, component, typed_capability,
};
use glyphspace_core::{
    Capability, Glyph, GlyphPatch, PatchOp, PolicyContext, Priority, RiskLevel, SemanticChangeKind,
};
use glyphspace_dsl::GlyphApp;
use glyphspace_input::InputEvent;
use glyphspace_layout::{DeviceProfile, Viewport};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Clone, Debug)]
struct CrmState {
    stage: String,
    risk: Priority,
}

#[derive(Clone, Debug, Deserialize)]
struct UpdateStageInput {
    stage: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct UpdateStageOutput {
    stage: String,
}

fn crm_app() -> GlyphApp {
    GlyphApp::new("crm", "CRM")
        .capability(
            Capability::builder("deal.update_stage", "Update Deal Stage")
                .permission("crm.deal.write")
                .risk(RiskLevel::Medium)
                .build(),
        )
        .capability(
            Capability::builder("deal.close_automatic", "Close Deal Automatically")
                .permission("crm.deal.write")
                .risk(RiskLevel::High)
                .build(),
        )
        .glyph(Glyph::button("deal", "Deal").binds("deal.update_stage"))
}

#[test]
fn signal_versions_and_subscriptions_track_state() {
    let mut signal = Signal::new(10);
    let observations = Rc::new(RefCell::new(Vec::new()));
    let captured = Rc::clone(&observations);
    signal.subscribe(move |value, version| captured.borrow_mut().push((*value, version)));

    signal.set(12);
    signal.update(|value| *value += 3);

    assert_eq!(signal.get(), 15);
    assert_eq!(signal.version(), 2);
    assert_eq!(*observations.borrow(), vec![(12, 1), (15, 2)]);
}

#[test]
fn rust_component_state_change_streams_semantic_diff() {
    let mut runtime = AppRuntime::new(
        crm_app(),
        CrmState {
            stage: "proposal".to_string(),
            risk: Priority::Normal,
        },
        PolicyContext::demo_user(),
    )
    .with_component(component(|state: &CrmState| {
        vec![
            Glyph::metric("stage", format!("Stage: {}", state.stage)),
            Glyph::metric("risk", "Deal Risk").priority(state.risk.clone()),
        ]
    }))
    .mount()
    .expect("runtime should mount");

    let diff = runtime
        .update_state(|state| {
            state.stage = "negotiation".to_string();
            state.risk = Priority::High;
        })
        .expect("state update should rebuild semantic world");

    assert!(diff.has_changes());
    assert!(diff.changes.iter().any(|change| {
        change.kind == SemanticChangeKind::GlyphChanged && change.path == "glyphs.risk.priority"
    }));
    assert_eq!(
        runtime.world().glyphs["stage"].accessibility.label,
        "Stage: negotiation"
    );
}

#[test]
fn glyph_click_invokes_typed_capability_applies_patch_and_audits() {
    let mut runtime = AppRuntime::new(
        crm_app(),
        CrmState {
            stage: "proposal".to_string(),
            risk: Priority::Normal,
        },
        PolicyContext::demo_user(),
    )
    .mount()
    .expect("runtime should mount");

    runtime.register_typed(
        typed_capability::<UpdateStageInput, UpdateStageOutput>("deal.update_stage"),
        |state, input, _world| {
            state.stage = input.stage;
            Ok(CapabilityOutput::new(UpdateStageOutput {
                stage: state.stage.clone(),
            })
            .with_patch(GlyphPatch::new(
                "stage_update",
                "Update semantic stage state",
                vec![PatchOp::SetPriority {
                    glyph_id: "deal".to_string(),
                    priority: Priority::High,
                }],
            )))
        },
    );

    let result = runtime
        .handle_input(InputEvent::GlyphClick {
            glyph_id: "deal".to_string(),
            input: serde_json::json!({ "stage": "negotiation" }),
        })
        .expect("capability should run")
        .expect("click should invoke capability");

    assert_eq!(result.output["stage"], "negotiation");
    assert_eq!(runtime.state().stage, "negotiation");
    assert_eq!(runtime.world().glyphs["deal"].priority, Priority::High);
    assert!(runtime.audit_log().iter().any(|event| {
        event.action == "capability.invoked" && event.subject == "deal.update_stage"
    }));
    assert_eq!(runtime.patch_store().len(), 1);
}

#[test]
fn unsafe_high_risk_capability_is_blocked_before_handler_runs() {
    let mut world = crm_app().compile().expect("app compiles");
    world
        .glyphs
        .get_mut("deal")
        .expect("deal exists")
        .capability_bindings[0]
        .capability_id = "deal.close_automatic".to_string();

    let app = GlyphApp::new("unsafe", "Unsafe").glyph(world.glyphs["deal"].clone());
    let mut runtime = AppRuntime::from_world(
        world,
        app,
        CrmState {
            stage: "proposal".to_string(),
            risk: Priority::Normal,
        },
        PolicyContext::demo_user(),
    )
    .expect("runtime should mount from world");
    let handler_called = Rc::new(Cell::new(false));
    let captured_handler_called = Rc::clone(&handler_called);
    runtime.register_typed(
        typed_capability::<UpdateStageInput, UpdateStageOutput>("deal.close_automatic"),
        move |_state, input, _world| {
            captured_handler_called.set(true);
            Ok(CapabilityOutput::new(UpdateStageOutput {
                stage: input.stage,
            }))
        },
    );

    let error = runtime
        .handle_input(InputEvent::GlyphClick {
            glyph_id: "deal".to_string(),
            input: serde_json::json!({ "stage": "closed_won" }),
        })
        .expect_err("unsafe capability should be rejected");

    assert!(
        error
            .to_string()
            .contains("high risk capabilities require confirmation")
    );
    assert!(!handler_called.get());
}

#[test]
fn headless_host_renders_visual_scene_accessibility_tree_and_scene_diff() {
    let mut runtime = AppRuntime::new(
        crm_app(),
        CrmState {
            stage: "proposal".to_string(),
            risk: Priority::Normal,
        },
        PolicyContext::demo_user(),
    )
    .with_component(component(|state: &CrmState| {
        vec![Glyph::metric("risk", "Deal Risk").priority(state.risk.clone())]
    }))
    .mount()
    .expect("runtime should mount");
    let mut host = HeadlessSemanticHost::new(Viewport::desktop(), DeviceProfile::desktop());

    let first = runtime.render(&mut host).expect("first render succeeds");
    runtime
        .update_state(|state| state.risk = Priority::Critical)
        .expect("state update succeeds");
    let second = runtime.render(&mut host).expect("second render succeeds");

    assert!(first.native_frame.prepared_scene.primitive_count > 0);
    assert!(first.accessibility_tree.nodes.contains_key("deal"));
    assert!(
        second
            .scene_diff
            .changed
            .iter()
            .any(|key| key.contains("risk"))
    );
}
