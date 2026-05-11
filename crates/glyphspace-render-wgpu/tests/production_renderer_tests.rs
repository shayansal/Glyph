use glyphspace_render::{RenderCommand, RenderCommandFrame};
use glyphspace_render_wgpu::{
    BrowserWebGpuPresenter, NativeSwapchainConfig, NativeSwapchainPresenter, RenderBenchmarkSuite,
    ScreenshotReadback, SurfaceSize, TextAtlasUploader, WgpuDrawState,
};
use glyphspace_text::{FontDescriptor, TextEngine, TextRun};

fn command_frame(count: usize) -> RenderCommandFrame {
    let mut commands = Vec::with_capacity(count * 3);
    for index in 0..count {
        let glyph_id = format!("glyph_{index}");
        commands.push(RenderCommand::Card {
            glyph_id: glyph_id.clone(),
            x: (index % 80) as f32 * 12.0,
            y: (index / 80) as f32 * 10.0,
            width: 80.0,
            height: 32.0,
            radius: 6.0,
        });
        commands.push(RenderCommand::Dot {
            glyph_id: glyph_id.clone(),
            x: (index % 80) as f32 * 12.0,
            y: (index / 80) as f32 * 10.0,
            z: (index % 16) as f32 * 0.01,
            radius: 4.0,
        });
        commands.push(RenderCommand::Text {
            glyph_id,
            text: format!("Metric {index}"),
            x: (index % 80) as f32 * 12.0,
            y: (index / 80) as f32 * 10.0 + 14.0,
            shaped_width: 64.0,
        });
    }
    RenderCommandFrame {
        frame_index: 7,
        native_backend: "wgpu".into(),
        browser_backend: "webgpu".into(),
        commands,
        applied_scene_operations: 0,
    }
}

#[test]
fn native_presenter_builds_gpu_resources_render_pass_and_text_atlas_uploads() {
    let frame = command_frame(12);
    let config = NativeSwapchainConfig::new(SurfaceSize::new(1024, 768))
        .with_msaa(4)
        .with_device_pixel_ratio(2.0);
    let mut presenter = NativeSwapchainPresenter::headless_contract(config);
    let mut text = TextEngine::new()
        .with_font(FontDescriptor::system("Inter"))
        .with_fallback(FontDescriptor::system("Noto Sans"));

    let stats = presenter
        .present_commands_with_text(&frame, &mut text)
        .expect("present with text");

    let resources = presenter.resources();
    assert!(resources.vertex_buffer_bytes > 0);
    assert!(resources.index_buffer_bytes > 0);
    assert!(resources.instance_buffer_bytes >= frame.commands.len() * 64);
    assert!(resources.uniform_buffer_bytes >= 128);
    assert!(resources.text_atlas_bytes > 0);
    assert!(
        resources
            .texture_uploads
            .iter()
            .any(|upload| upload.label == "text_atlas")
    );

    let pass = presenter.render_pass_plan();
    assert_eq!(pass.color_attachment_format, "Bgra8UnormSrgb");
    assert_eq!(pass.sample_count, 4);
    assert!(pass.draws.iter().any(|draw| draw.kind == "card"));
    assert!(pass.draws.iter().any(|draw| draw.kind == "dot"));
    assert!(pass.draws.iter().any(|draw| draw.kind == "text"));
    assert!(pass.bind_groups.contains(&"text_atlas".to_string()));
    assert_eq!(stats.mode, "headless-contract");
}

#[test]
fn screenshot_readback_is_deterministic_nonblank_and_supports_hit_testing() {
    let frame = command_frame(4);
    let mut presenter = NativeSwapchainPresenter::headless_contract(NativeSwapchainConfig::new(
        SurfaceSize::new(320, 180),
    ));
    presenter.present_commands(&frame).expect("present");

    let first = ScreenshotReadback::read(&presenter).expect("screenshot");
    let second = ScreenshotReadback::read(&presenter).expect("screenshot");

    assert_eq!(first.pixel_digest, second.pixel_digest);
    assert_eq!(first.width, 320);
    assert_eq!(first.height, 180);
    assert!(first.pixels.iter().any(|pixel| *pixel != 0));
    assert!(first.coverage.contains(&"card".to_string()));
    assert!(first.coverage.contains(&"text".to_string()));

    let hit = presenter
        .hit_test(6.0, 6.0)
        .expect("hit map returns front-most glyph");
    assert_eq!(hit.glyph_id, "glyph_0");
}

#[test]
fn browser_webgpu_presenter_consumes_same_command_frame_contract() {
    let frame = command_frame(6);
    let mut native = NativeSwapchainPresenter::headless_contract(NativeSwapchainConfig::new(
        SurfaceSize::new(640, 360),
    ));
    let mut browser = BrowserWebGpuPresenter::new(SurfaceSize::new(640, 360));

    let native_stats = native.present_commands(&frame).expect("native present");
    let browser_stats = browser.present_commands(&frame).expect("browser present");

    assert_eq!(native_stats.draw_calls, browser_stats.draw_calls);
    assert_eq!(browser.backend(), "webgpu");
    assert!(browser.pipeline_plan().shader_wgsl.contains("@fragment"));
    assert!(browser.dom_accessibility_overlay_enabled());
}

#[test]
fn draw_state_tracks_clipping_scrolling_z_order_transforms_opacity_and_masks() {
    let state = WgpuDrawState::default()
        .with_clip(0.0, 0.0, 400.0, 240.0)
        .with_scroll(12.0, 18.0)
        .with_z_order(42)
        .with_transform([1.0, 0.0, 0.0, 1.0, 24.0, 32.0])
        .with_opacity(0.72)
        .with_mask("rounded-panel-mask");

    assert_eq!(state.clip.unwrap().width, 400.0);
    assert_eq!(state.scroll_x, 12.0);
    assert_eq!(state.z_order, 42);
    assert_eq!(state.transform[4], 24.0);
    assert_eq!(state.opacity, 0.72);
    assert_eq!(state.masks, vec!["rounded-panel-mask"]);
}

#[test]
fn text_atlas_uploader_shapes_rasterizes_and_batches_uploads() {
    let mut text = TextEngine::new().with_font(FontDescriptor::system("Inter"));
    let mut uploader = TextAtlasUploader::new(1024, 1024);
    let upload = uploader
        .upload_run(
            &mut text,
            TextRun::new("Revenue مرحبا 🚀", 18.0)
                .with_dpi_scale(2.0)
                .with_clip(0.0, 0.0, 256.0, 64.0),
        )
        .expect("text upload");

    assert_eq!(upload.label, "text_atlas");
    assert!(upload.bytes > 0);
    assert_eq!(upload.width, 256);
    assert_eq!(upload.height, 64);
    assert_eq!(uploader.upload_count(), 1);
}

#[test]
fn renderer_benchmarks_cover_1k_10k_and_100k_glyph_scenarios() {
    let suite = RenderBenchmarkSuite::prototype();
    let report = suite.run([1_000, 10_000, 100_000]);

    assert_eq!(report.scenarios.len(), 3);
    assert!(
        report
            .scenarios
            .iter()
            .any(|scenario| scenario.glyphs == 1_000)
    );
    assert!(
        report
            .scenarios
            .iter()
            .any(|scenario| scenario.glyphs == 10_000)
    );
    assert!(
        report
            .scenarios
            .iter()
            .any(|scenario| scenario.glyphs == 100_000)
    );
    assert!(
        report
            .scenarios
            .iter()
            .all(|scenario| scenario.estimated_frame_ms > 0.0)
    );
    assert!(report.summary.contains("100000 glyphs"));
}
