use glyphspace_render::{RenderCommand, RenderCommandFrame};
use glyphspace_render_wgpu::{
    HardwareGlyphPipeline, SurfaceSize, WgpuSurfaceBindingPlan, WinitWgpuSurfacePresenter,
};

fn frame() -> RenderCommandFrame {
    RenderCommandFrame {
        frame_index: 9,
        native_backend: "wgpu".to_string(),
        browser_backend: "webgpu".to_string(),
        commands: vec![
            RenderCommand::Card {
                glyph_id: "account".to_string(),
                x: 12.0,
                y: 16.0,
                width: 180.0,
                height: 72.0,
                radius: 8.0,
            },
            RenderCommand::Dot {
                glyph_id: "urgent".to_string(),
                x: 36.0,
                y: 44.0,
                z: 0.08,
                radius: 9.0,
            },
            RenderCommand::Text {
                glyph_id: "label".to_string(),
                text: "Renewal risk".to_string(),
                x: 44.0,
                y: 52.0,
                shaped_width: 116.0,
            },
        ],
        applied_scene_operations: 1,
    }
}

#[test]
fn surface_binding_plan_maps_encoded_payloads_to_wgpu_resources() {
    let pipeline = HardwareGlyphPipeline::from_command_frame(&frame(), SurfaceSize::new(960, 540));
    let plan = WgpuSurfaceBindingPlan::from_pipeline(&pipeline);

    assert_eq!(plan.buffer_uploads.len(), 4);
    assert!(
        plan.buffer_uploads
            .iter()
            .any(|buffer| buffer.label == "glyph_vertices" && buffer.usage.contains("VERTEX"))
    );
    assert!(
        plan.buffer_uploads
            .iter()
            .any(|buffer| buffer.label == "glyph_indices" && buffer.usage.contains("INDEX"))
    );
    assert!(
        plan.buffer_uploads
            .iter()
            .any(|buffer| buffer.label == "glyph_instances" && buffer.usage.contains("STORAGE"))
    );
    assert!(
        plan.buffer_uploads
            .iter()
            .any(|buffer| buffer.label == "camera_uniforms" && buffer.usage.contains("UNIFORM"))
    );
    assert_eq!(plan.texture_uploads.len(), 1);
    assert_eq!(plan.texture_uploads[0].label, "text_atlas_texture");
    assert!(plan.texture_uploads[0].bytes > 0);
    assert_eq!(
        plan.submit_order,
        vec![
            "glyph_vertices",
            "glyph_indices",
            "glyph_instances",
            "camera_uniforms",
            "text_atlas_texture",
            "render_passes"
        ]
    );
    assert!(plan.readback_enabled);
    assert!(plan.total_bytes > pipeline.encoded_frame.uniform_bytes.len());
}

#[test]
fn surface_presenter_contract_mentions_hardware_pipeline_binding() {
    let contract = WinitWgpuSurfacePresenter::required_runtime_contract();

    assert!(
        contract
            .resources
            .contains(&"HardwareGlyphPipeline".to_string())
    );
    assert!(
        contract
            .resources
            .contains(&"wgpu::Buffer(vertex/index/instance/uniform)".to_string())
    );
    assert!(
        contract
            .resources
            .contains(&"wgpu::Texture(text_atlas)".to_string())
    );
}
