use glyphspace_render::{RenderCommand, RenderCommandFrame};
use glyphspace_render_wgpu::{HardwareGlyphPipeline, SurfaceSize};

fn rich_frame() -> RenderCommandFrame {
    RenderCommandFrame {
        frame_index: 42,
        native_backend: "wgpu".to_string(),
        browser_backend: "webgpu".to_string(),
        commands: vec![
            RenderCommand::Card {
                glyph_id: "deal_card".to_string(),
                x: 24.0,
                y: 32.0,
                width: 220.0,
                height: 96.0,
                radius: 8.0,
            },
            RenderCommand::Dot {
                glyph_id: "risk_dot".to_string(),
                x: 82.0,
                y: 76.0,
                z: 0.15,
                radius: 12.0,
            },
            RenderCommand::Edge {
                from: "risk_dot".to_string(),
                to: "deal_card".to_string(),
                x1: 82.0,
                y1: 76.0,
                x2: 168.0,
                y2: 80.0,
            },
            RenderCommand::Text {
                glyph_id: "deal_label".to_string(),
                text: "Close risk: legal review".to_string(),
                x: 42.0,
                y: 64.0,
                shaped_width: 174.0,
            },
            RenderCommand::FocusRing {
                glyph_id: "deal_card".to_string(),
                x: 20.0,
                y: 28.0,
                width: 228.0,
                height: 104.0,
            },
        ],
        applied_scene_operations: 3,
    }
}

#[test]
fn hardware_pipeline_encodes_gpu_buffers_passes_and_text_atlas() {
    let frame = rich_frame();
    let pipeline = HardwareGlyphPipeline::from_command_frame(&frame, SurfaceSize::new(800, 480));

    assert!(pipeline.hardware_ready());
    assert!(pipeline.encoded_frame.vertex_bytes.len() >= frame.commands.len() * 16);
    assert!(pipeline.encoded_frame.index_bytes.len() >= frame.commands.len() * 6);
    assert!(pipeline.encoded_frame.instance_bytes.len() >= frame.commands.len() * 32);
    assert_eq!(pipeline.encoded_frame.uniform_bytes.len(), 256);
    assert!(pipeline.encoded_frame.text_atlas_bytes.len() >= "Close risk: legal review".len());

    assert!(
        pipeline
            .uploads
            .iter()
            .any(|upload| upload.label == "glyph_vertices")
    );
    assert!(
        pipeline
            .uploads
            .iter()
            .any(|upload| upload.label == "glyph_instances")
    );
    assert!(
        pipeline
            .uploads
            .iter()
            .any(|upload| upload.label == "text_atlas_texture")
    );
    assert!(
        pipeline
            .shader_modules
            .contains(&"glyphspace_cards.wgsl".to_string())
    );
    assert!(
        pipeline
            .shader_modules
            .contains(&"glyphspace_dots.wgsl".to_string())
    );
    assert!(
        pipeline
            .shader_modules
            .contains(&"glyphspace_edges.wgsl".to_string())
    );
    assert!(
        pipeline
            .shader_modules
            .contains(&"glyphspace_text.wgsl".to_string())
    );
    assert!(
        pipeline
            .shader_modules
            .contains(&"glyphspace_focus_policy.wgsl".to_string())
    );
    assert!(
        pipeline
            .bind_groups
            .contains(&"camera_uniforms".to_string())
    );
    assert!(
        pipeline
            .bind_groups
            .contains(&"glyph_instance_buffer".to_string())
    );
    assert!(
        pipeline
            .bind_groups
            .contains(&"text_atlas_sampler".to_string())
    );
    assert!(
        pipeline
            .draw_passes
            .iter()
            .any(|pass| pass.name == "cards_panels")
    );
    assert!(
        pipeline
            .draw_passes
            .iter()
            .any(|pass| pass.name == "dots_glows")
    );
    assert!(pipeline.draw_passes.iter().any(|pass| pass.name == "edges"));
    assert!(pipeline.draw_passes.iter().any(|pass| pass.name == "text"));
    assert!(
        pipeline
            .draw_passes
            .iter()
            .any(|pass| pass.name == "focus_policy_overlays")
    );
}

#[test]
fn hardware_pipeline_produces_deterministic_pixel_snapshot_contract() {
    let frame = rich_frame();
    let first = HardwareGlyphPipeline::from_command_frame(&frame, SurfaceSize::new(640, 360));
    let second = HardwareGlyphPipeline::from_command_frame(&frame, SurfaceSize::new(640, 360));

    assert_eq!(first.pixel_snapshot.digest, second.pixel_snapshot.digest);
    assert_eq!(first.pixel_snapshot.width, 640);
    assert_eq!(first.pixel_snapshot.height, 360);
    assert!(first.pixel_snapshot.non_transparent_pixels > 0);
    assert!(first.pixel_snapshot.coverage.contains(&"card".to_string()));
    assert!(first.pixel_snapshot.coverage.contains(&"dot".to_string()));
    assert!(first.pixel_snapshot.coverage.contains(&"edge".to_string()));
    assert!(first.pixel_snapshot.coverage.contains(&"text".to_string()));
    assert!(
        first
            .pixel_snapshot
            .coverage
            .contains(&"focus_ring".to_string())
    );
}
