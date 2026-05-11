use glyphspace_core::{Glyph, GlyphWorld};
use glyphspace_layout::{DeviceProfile, Viewport};
use glyphspace_render::{ProductionRenderer, RenderCommand, RenderCommandFrame};
use glyphspace_render_wgpu::{
    BrowserWebGpuParityReport, FrameRasterizer, GpuGlyphUploadPlan, NativeProductAppLoop,
    NativeSwapchainConfig, SurfaceSize,
};

fn product_frame() -> RenderCommandFrame {
    RenderCommandFrame {
        frame_index: 42,
        native_backend: "wgpu::Surface+winit".to_string(),
        browser_backend: "webgpu-command-buffer".to_string(),
        applied_scene_operations: 0,
        commands: vec![
            RenderCommand::Card {
                glyph_id: "card".to_string(),
                x: 12.0,
                y: 12.0,
                width: 120.0,
                height: 48.0,
                radius: 8.0,
            },
            RenderCommand::Dot {
                glyph_id: "dot".to_string(),
                x: 48.0,
                y: 88.0,
                z: 0.2,
                radius: 10.0,
            },
            RenderCommand::Edge {
                from: "card".to_string(),
                to: "dot".to_string(),
                x1: 72.0,
                y1: 60.0,
                x2: 48.0,
                y2: 88.0,
            },
            RenderCommand::Text {
                glyph_id: "label".to_string(),
                text: "Revenue closed".to_string(),
                x: 20.0,
                y: 36.0,
                shaped_width: 96.0,
            },
            RenderCommand::FocusRing {
                glyph_id: "card".to_string(),
                x: 10.0,
                y: 10.0,
                width: 124.0,
                height: 52.0,
            },
        ],
    }
}

#[test]
fn native_product_app_loop_routes_frames_to_winit_surface_presenter() {
    let mut world = GlyphWorld::new("native-product", "Native Product");
    world.add_glyph(Glyph::card("revenue", "Revenue")).unwrap();
    let mut renderer = ProductionRenderer::headless(Viewport::desktop(), DeviceProfile::desktop());
    let frame = renderer.render_world(&world).expect("frame");

    let loop_plan = NativeProductAppLoop::new(
        NativeSwapchainConfig::new(SurfaceSize::new(1280, 720)).with_device_pixel_ratio(2.0),
    )
    .route_frame(&frame.command_frame);

    assert_eq!(loop_plan.presenter_backend, "wgpu::Surface+winit");
    assert!(loop_plan.uses_hardware_presenter);
    assert!(loop_plan.window_events.contains(&"resized".to_string()));
    assert!(
        loop_plan
            .window_events
            .contains(&"redraw_requested".to_string())
    );
    assert_eq!(loop_plan.surface_size, SurfaceSize::new(1280, 720));
    assert!(loop_plan.command_count >= 2);
}

#[test]
fn glyph_upload_plan_uses_real_vertex_instance_uniform_and_text_uploads() {
    let frame = product_frame();
    let upload = GpuGlyphUploadPlan::from_command_frame(&frame);

    assert!(upload.vertex_buffer_bytes >= frame.commands.len() * 16);
    assert!(upload.instance_buffer_bytes >= frame.commands.len() * 64);
    assert!(upload.uniform_buffer_bytes >= 256);
    assert_eq!(upload.instance_count, frame.commands.len());
    assert!(upload.buffer_labels.contains(&"glyph_vertices".to_string()));
    assert!(
        upload
            .buffer_labels
            .contains(&"glyph_instances".to_string())
    );
    assert!(
        upload
            .buffer_labels
            .contains(&"camera_uniforms".to_string())
    );
    assert_eq!(upload.text_uploads.len(), 1);
    assert_eq!(upload.text_uploads[0].glyph_id, "label");
}

#[test]
fn frame_rasterizer_draws_cards_dots_edges_text_and_focus_pixels() {
    let frame = product_frame();
    let snapshot = FrameRasterizer::new(SurfaceSize::new(180, 120))
        .rasterize(&frame)
        .expect("raster snapshot");

    assert_eq!(snapshot.width, 180);
    assert_eq!(snapshot.height, 120);
    assert!(snapshot.non_transparent_pixels > 0);
    assert!(snapshot.coverage.contains(&"card".to_string()));
    assert!(snapshot.coverage.contains(&"dot".to_string()));
    assert!(snapshot.coverage.contains(&"edge".to_string()));
    assert!(snapshot.coverage.contains(&"text".to_string()));
    assert!(snapshot.coverage.contains(&"focus_ring".to_string()));
    assert_ne!(snapshot.pixel_digest, "0000000000000000");
}

#[test]
fn browser_webgpu_parity_matches_native_command_and_accessibility_contract() {
    let frame = product_frame();
    let report = BrowserWebGpuParityReport::from_command_frame(&frame);

    assert!(report.command_frame_compatible);
    assert!(report.rust_owned_event_loop);
    assert!(report.rust_generated_dom_accessibility_mirror);
    assert_eq!(report.native_backend, "wgpu::Surface+winit");
    assert_eq!(report.browser_backend, "WebGPU");
    assert_eq!(report.command_count, frame.commands.len());
    assert!(report.minimal_js_glue);
}
