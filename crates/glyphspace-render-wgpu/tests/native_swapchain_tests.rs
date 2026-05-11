use glyphspace_render::{RenderCommand, RenderCommandFrame};
use glyphspace_render_wgpu::{
    NativeSwapchainConfig, NativeSwapchainPresenter, SurfaceSize, WgpuFrameStats,
};

#[test]
fn native_swapchain_presenter_preserves_command_frame_contract() {
    let config = NativeSwapchainConfig::new(SurfaceSize::new(1280, 720))
        .with_vsync(true)
        .with_msaa(4);
    let mut presenter = NativeSwapchainPresenter::headless_contract(config);

    let frame = RenderCommandFrame {
        frame_index: 1,
        native_backend: "wgpu".into(),
        browser_backend: "webgpu".into(),
        commands: vec![
            RenderCommand::Dot {
                glyph_id: "revenue".into(),
                x: 10.0,
                y: 20.0,
                z: 0.0,
                radius: 8.0,
            },
            RenderCommand::Text {
                glyph_id: "revenue".into(),
                text: "Revenue".into(),
                x: 12.0,
                y: 24.0,
                shaped_width: 80.0,
            },
        ],
        applied_scene_operations: 0,
    };

    let stats = presenter
        .present_commands(&frame)
        .expect("present commands");

    assert_eq!(presenter.config().size.width, 1280);
    assert_eq!(presenter.config().sample_count, 4);
    assert!(presenter.pipeline_plan().shader_wgsl.contains("@vertex"));
    assert_eq!(stats.presented_frames, 1);
    assert_eq!(stats.draw_calls, 2);
    assert_eq!(stats.surface_size, SurfaceSize::new(1280, 720));
    assert_eq!(stats.mode, "headless-contract");

    presenter.resize(SurfaceSize::new(800, 600));
    let WgpuFrameStats { surface_size, .. } = presenter
        .present_commands(&frame)
        .expect("present after resize");
    assert_eq!(surface_size, SurfaceSize::new(800, 600));
}
