use glyphspace_render::{RenderCommand, RenderCommandFrame};
use glyphspace_render_wgpu::{
    HardwareGlyphPipeline, HardwareShaderInputPlan, SurfaceSize, WgpuSurfaceBindingPlan,
};

fn command_frame() -> RenderCommandFrame {
    RenderCommandFrame {
        frame_index: 17,
        native_backend: "wgpu".to_string(),
        browser_backend: "webgpu".to_string(),
        commands: vec![
            RenderCommand::Card {
                glyph_id: "panel".to_string(),
                x: 20.0,
                y: 24.0,
                width: 144.0,
                height: 72.0,
                radius: 6.0,
            },
            RenderCommand::Dot {
                glyph_id: "deal".to_string(),
                x: 48.0,
                y: 56.0,
                z: 0.2,
                radius: 8.0,
            },
            RenderCommand::FocusRing {
                glyph_id: "deal".to_string(),
                x: 38.0,
                y: 46.0,
                width: 24.0,
                height: 24.0,
            },
        ],
        applied_scene_operations: 2,
    }
}

#[test]
fn shader_input_plan_declares_vertex_instance_layouts_and_indexed_draws() {
    let pipeline =
        HardwareGlyphPipeline::from_command_frame(&command_frame(), SurfaceSize::new(800, 600));
    let binding = WgpuSurfaceBindingPlan::from_pipeline(&pipeline);
    let plan = HardwareShaderInputPlan::from_pipeline(&pipeline, &binding);

    assert_eq!(plan.vertex_layouts.len(), 2);
    assert_eq!(plan.vertex_layouts[0].name, "glyph_vertex");
    assert_eq!(plan.vertex_layouts[0].stride, 8);
    assert_eq!(plan.vertex_layouts[1].name, "glyph_instance");
    assert_eq!(plan.vertex_layouts[1].stride, 40);
    assert!(
        plan.vertex_layouts
            .iter()
            .flat_map(|layout| layout.attributes.iter())
            .any(|attribute| attribute.name == "position" && attribute.location == 0)
    );
    assert!(
        plan.vertex_layouts
            .iter()
            .flat_map(|layout| layout.attributes.iter())
            .any(|attribute| attribute.name == "kind_and_opacity" && attribute.location == 4)
    );
    assert_eq!(plan.index_format, "Uint32");
    assert_eq!(plan.draws.len(), pipeline.draw_passes.len());
    assert!(plan.draws.iter().all(|draw| draw.index_count == 6));
    assert!(plan.draws.iter().all(|draw| draw.instance_count > 0));
    assert!(plan.shader_wgsl.contains("@location(0) position"));
    assert!(plan.shader_wgsl.contains("@location(4) kind_and_opacity"));
}
