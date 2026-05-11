use glyphspace_core::{Capability, Glyph, GlyphPatch, PatchOp, PolicyContext, Priority, RiskLevel};
use glyphspace_dsl::GlyphApp;
use glyphspace_input::{GlyphspaceRuntime, InputEvent};
use serde_json::json;

#[derive(Default)]
struct CrmState {
    stage: String,
}

#[test]
fn native_runtime_invokes_capability_applies_patch_and_audits() {
    let app = GlyphApp::new("crm_rust_runtime", "CRM Runtime")
        .capability(
            Capability::builder("deal.update_stage", "Update Deal Stage")
                .permission("crm.deal.write")
                .risk(RiskLevel::Medium)
                .build(),
        )
        .glyph(Glyph::button("deal_northstar", "Northstar Health").binds("deal.update_stage"));

    let mut runtime = GlyphspaceRuntime::new(
        app.compile().unwrap(),
        CrmState {
            stage: "proposal".into(),
        },
        PolicyContext::demo_user(),
    );
    runtime.register("deal.update_stage", |state, input, _world| {
        state.stage = input["stage"].as_str().unwrap().to_string();
        Ok(glyphspace_input::CapabilityResult {
            output: json!({ "stage": state.stage }),
            patch: Some(GlyphPatch::new(
                "stage_update",
                "Updated stage",
                vec![
                    PatchOp::SetStyleToken {
                        glyph_id: "deal_northstar".into(),
                        key: "stage".into(),
                        value: state.stage.clone(),
                    },
                    PatchOp::SetPriority {
                        glyph_id: "deal_northstar".into(),
                        priority: Priority::High,
                    },
                ],
            )),
        })
    });

    runtime
        .handle_input(InputEvent::GlyphClick {
            glyph_id: "deal_northstar".into(),
            input: json!({ "stage": "negotiation" }),
        })
        .unwrap();

    assert_eq!(runtime.state().stage, "negotiation");
    assert_eq!(
        runtime.world().glyphs["deal_northstar"].style.tokens["stage"],
        "negotiation"
    );
    assert_eq!(runtime.patch_store().len(), 1);
    assert_eq!(runtime.audit_log()[0].action, "capability.invoked");
}

#[test]
fn native_runtime_blocks_missing_permission_before_handler_runs() {
    let app = GlyphApp::new("secure_runtime", "Secure Runtime")
        .capability(
            Capability::builder("deal.update_stage", "Update Deal Stage")
                .permission("crm.deal.write")
                .build(),
        )
        .glyph(Glyph::button("deal", "Deal").binds("deal.update_stage"));
    let mut context = PolicyContext::demo_user();
    context
        .permissions
        .retain(|permission| permission != "crm.deal.write");
    let mut runtime = GlyphspaceRuntime::new(app.compile().unwrap(), CrmState::default(), context);
    runtime.register("deal.update_stage", |_state, _input, _world| unreachable!());

    let error = runtime
        .invoke_capability("deal.update_stage", json!({ "stage": "negotiation" }))
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("missing permission crm.deal.write")
    );
    assert_eq!(runtime.audit_log()[0].action, "capability.rejected");
}
