use glyphspace_core::{EdgeKind, Glyph, GlyphEdge, GlyphWorld};
use glyphspace_layout::{DeviceProfile, Viewport};
use glyphspace_render::{
    GpuPipelinePlan, ProductionRenderer, RenderLoopConfig, ScreenshotConformance,
};

fn dashboard_world() -> GlyphWorld {
    let mut world = GlyphWorld::new("gpu-dashboard", "GPU Dashboard");
    world
        .add_glyph(Glyph::metric("revenue", "Revenue"))
        .unwrap();
    world.add_glyph(Glyph::card("risk", "Risk")).unwrap();
    world
        .add_edge(GlyphEdge::new("revenue", "risk", EdgeKind::RelatedTo))
        .unwrap();
    world
}

#[test]
fn gpu_renderer_builds_real_wgpu_pipeline_plan_for_cards_edges_dots_and_text() {
    let world = dashboard_world();
    let mut renderer = ProductionRenderer::headless(Viewport::desktop(), DeviceProfile::desktop())
        .with_render_loop(RenderLoopConfig::animated_60hz())
        .with_focus("revenue");
    let frame = renderer.render_world(&world).expect("frame renders");

    let plan = GpuPipelinePlan::from_frame(&frame);

    assert!(plan.uses_wgsl());
    assert!(plan.vertex_buffers >= 1);
    assert!(plan.draw_calls.iter().any(|call| call.kind == "dot"));
    assert!(plan.draw_calls.iter().any(|call| call.kind == "card"));
    assert!(plan.draw_calls.iter().any(|call| call.kind == "edge"));
    assert!(plan.draw_calls.iter().any(|call| call.kind == "text"));
    assert!(plan.draw_calls.iter().any(|call| call.kind == "focus_ring"));
    assert_eq!(plan.browser_parity().browser_api, "WebGPU");
    assert_eq!(plan.browser_parity().native_api, "wgpu");
}

#[test]
fn screenshot_conformance_produces_deterministic_render_snapshots() {
    let world = dashboard_world();
    let mut renderer = ProductionRenderer::headless(Viewport::desktop(), DeviceProfile::desktop());
    let frame = renderer.render_world(&world).expect("frame renders");

    let first = ScreenshotConformance::from_frame(&frame);
    let second = ScreenshotConformance::from_frame(&frame);

    assert_eq!(first.pixel_digest, second.pixel_digest);
    assert!(first.coverage.contains(&"dot".to_string()));
    assert!(first.coverage.contains(&"card".to_string()));
    assert!(first.coverage.contains(&"edge".to_string()));
    assert!(first.coverage.contains(&"text".to_string()));
}
