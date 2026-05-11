use anyhow::Result;
use glyphspace_app::{
    AppRuntime, CapabilityOutput, GlyphDeviceProfile, GlyphInputEvent, GlyphViewport,
    HeadlessSemanticHost, SemanticHost, component, typed_capability,
};
use glyphspace_core::{Capability, Glyph, GlyphPatch, PatchOp, PolicyContext, Priority, RiskLevel};
use glyphspace_dsl::{GlyphApp, Lens};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Debug)]
struct CrmState {
    selected_deal: String,
    stage: String,
    revenue_label: String,
}

#[derive(Clone, Debug, Deserialize)]
struct UpdateStageInput {
    stage: String,
}

#[derive(Clone, Debug, Serialize)]
struct UpdateStageOutput {
    deal_id: String,
    stage: String,
}

fn main() -> Result<()> {
    let export = std::env::args().any(|arg| arg == "--export");
    let mut runtime = AppRuntime::new(
        crm_app(),
        CrmState {
            selected_deal: "deal_northstar".to_string(),
            stage: "proposal".to_string(),
            revenue_label: "Revenue: $1.2M ARR".to_string(),
        },
        PolicyContext::demo_user(),
    )
    .with_component(component(|state: &CrmState| {
        vec![
            Glyph::metric("revenue", state.revenue_label.clone()).priority(Priority::High),
            Glyph::metric("stage_status", format!("Stage: {}", state.stage))
                .priority(Priority::Normal),
        ]
    }))
    .mount()?;

    if export {
        println!("{}", runtime.world().to_canonical_json()?);
        return Ok(());
    }

    runtime.register_typed(
        typed_capability::<UpdateStageInput, UpdateStageOutput>("deal.update_stage"),
        |state, input, _world| {
            state.stage = input.stage;
            Ok(CapabilityOutput::new(UpdateStageOutput {
                deal_id: state.selected_deal.clone(),
                stage: state.stage.clone(),
            })
            .with_patch(GlyphPatch::new(
                "rust_stage_update",
                "Rust CRM moved deal stage",
                vec![
                    PatchOp::SetStyleToken {
                        glyph_id: state.selected_deal.clone(),
                        key: "stage".to_string(),
                        value: state.stage.clone(),
                    },
                    PatchOp::SetPriority {
                        glyph_id: state.selected_deal.clone(),
                        priority: Priority::High,
                    },
                ],
            )))
        },
    );

    let mut host =
        HeadlessSemanticHost::new(GlyphViewport::desktop(), GlyphDeviceProfile::desktop());
    let frame = runtime.render(&mut host)?;
    let deal_region = frame
        .native_frame
        .hit_regions
        .iter()
        .find(|region| region.glyph_id == "deal_northstar")
        .expect("deal glyph should render");
    let hit = host
        .hit_test(deal_region.center_x, deal_region.center_y)
        .unwrap_or_else(|| "deal_northstar".to_string());
    runtime.handle_input(GlyphInputEvent::GlyphClick {
        glyph_id: hit,
        input: json!({ "stage": "negotiation" }),
    })?;
    let next_frame = runtime.render(&mut host)?;

    println!(
        "Glyphspace Rust CRM: {} glyphs, stage {}, audit events {}, scene changes {}",
        runtime.world().glyphs.len(),
        runtime.state().stage,
        runtime.audit_log().len(),
        next_frame.scene_diff.changed.len()
    );
    Ok(())
}

fn crm_app() -> GlyphApp {
    GlyphApp::new("crm_dashboard_rust", "Rust CRM Dashboard")
        .capability(
            Capability::builder("deal.update_stage", "Update Deal Stage")
                .description("Move a sales opportunity to a new pipeline stage.")
                .intent("move a sales opportunity to a new pipeline stage")
                .permission("crm.deal.write")
                .risk(RiskLevel::Medium)
                .build(),
        )
        .glyph(Glyph::metric("runway", "Runway").priority(Priority::High))
        .glyph(Glyph::button("deal_northstar", "Northstar Health").binds("deal.update_stage"))
        .glyph(Glyph::panel("pipeline", "Pipeline stages"))
        .lens(
            Lens::new("founder", "Founder command center").op(PatchOp::SetPriority {
                glyph_id: "revenue".to_string(),
                priority: Priority::Critical,
            }),
        )
}
