use anyhow::Result;
use glyphspace_core::{Capability, Glyph, GlyphPatch, PatchOp, PolicyContext, Priority, RiskLevel};
use glyphspace_dsl::{GlyphApp, Lens};
use glyphspace_input::{CapabilityResult, GlyphspaceRuntime, InputEvent};
use glyphspace_layout::{DeviceProfile, Viewport};
use glyphspace_render::NativeRendererHost;
use serde_json::json;

#[derive(Clone, Debug)]
struct CrmState {
    selected_deal: String,
    stage: String,
}

fn main() -> Result<()> {
    let export = std::env::args().any(|arg| arg == "--export");
    let app = crm_app();
    let world = app.compile()?;
    if export {
        println!("{}", app.to_glyph_json()?);
        return Ok(());
    }

    let mut runtime = GlyphspaceRuntime::new(
        world,
        CrmState {
            selected_deal: "deal_northstar".to_string(),
            stage: "proposal".to_string(),
        },
        PolicyContext::demo_user(),
    );
    runtime.register("deal.update_stage", |state, input, _world| {
        state.stage = input["stage"].as_str().unwrap_or("negotiation").to_string();
        Ok(CapabilityResult {
            output: json!({
                "deal_id": state.selected_deal,
                "stage": state.stage,
            }),
            patch: Some(GlyphPatch::new(
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
            )),
        })
    });

    let mut host = NativeRendererHost::headless(Viewport::desktop(), DeviceProfile::desktop());
    let frame = host.render_world(runtime.world())?;
    let deal_region = frame
        .hit_regions
        .iter()
        .find(|region| region.glyph_id == "deal_northstar")
        .expect("deal glyph should render");
    let hit = host
        .hit_test(deal_region.center_x, deal_region.center_y)
        .unwrap_or_else(|| "deal_northstar".to_string());
    runtime.handle_input(InputEvent::GlyphClick {
        glyph_id: hit,
        input: json!({ "stage": "negotiation" }),
    })?;

    println!(
        "Glyphspace Rust CRM: {} glyphs, stage {}, audit events {}",
        runtime.world().glyphs.len(),
        runtime.state().stage,
        runtime.audit_log().len()
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
        .glyph(Glyph::metric("revenue", "Revenue").priority(Priority::High))
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
