use glyphspace_render::{RenderCommand, RenderCommandFrame};
use glyphspace_render_wgpu::{
    HardwareGlyphPipeline, HardwareShaderInputPlan, PrimitivePipelineSet, SurfaceSize,
    WgpuSurfaceBindingPlan,
};

fn full_surface_frame() -> RenderCommandFrame {
    RenderCommandFrame {
        frame_index: 42,
        native_backend: "wgpu".to_string(),
        browser_backend: "webgpu".to_string(),
        commands: vec![
            RenderCommand::Card {
                glyph_id: "command_panel".to_string(),
                x: 24.0,
                y: 28.0,
                width: 240.0,
                height: 112.0,
                radius: 8.0,
            },
            RenderCommand::Dot {
                glyph_id: "revenue".to_string(),
                x: 84.0,
                y: 72.0,
                z: 0.1,
                radius: 14.0,
            },
            RenderCommand::Edge {
                from: "revenue".to_string(),
                to: "risk".to_string(),
                x1: 84.0,
                y1: 72.0,
                x2: 190.0,
                y2: 92.0,
            },
            RenderCommand::Text {
                glyph_id: "revenue".to_string(),
                text: "Revenue".to_string(),
                x: 50.0,
                y: 116.0,
                shaped_width: 88.0,
            },
            RenderCommand::FocusRing {
                glyph_id: "revenue".to_string(),
                x: 68.0,
                y: 56.0,
                width: 34.0,
                height: 34.0,
            },
        ],
        applied_scene_operations: 3,
    }
}

#[test]
fn primitive_pipeline_set_routes_draws_to_specialized_gpu_pipelines() {
    let pipeline = HardwareGlyphPipeline::from_command_frame(
        &full_surface_frame(),
        SurfaceSize::new(1280, 720),
    );
    let binding = WgpuSurfaceBindingPlan::from_pipeline(&pipeline);
    let shader_plan = HardwareShaderInputPlan::from_pipeline(&pipeline, &binding);

    let primitive_set = PrimitivePipelineSet::from_shader_plan(&shader_plan);

    assert!(primitive_set.hardware_ready());
    assert_eq!(primitive_set.draw_routes.len(), shader_plan.draws.len());
    assert!(primitive_set.pipeline("cards_panels").is_some());
    assert!(primitive_set.pipeline("dots_glows").is_some());
    assert!(primitive_set.pipeline("edges").is_some());
    assert!(primitive_set.pipeline("text").is_some());
    assert!(primitive_set.pipeline("focus_policy_overlays").is_some());
    assert!(primitive_set.pipeline("text").unwrap().uses_text_atlas);
    assert!(primitive_set.pipeline("dots_glows").unwrap().uses_blending);
    assert!(
        primitive_set
            .pipeline("focus_policy_overlays")
            .unwrap()
            .policy_overlay
    );
    assert!(
        primitive_set
            .draw_routes
            .iter()
            .all(|route| primitive_set.pipeline(&route.pipeline_name).is_some())
    );
}
