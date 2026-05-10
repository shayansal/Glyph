use glyphspace_core::{Glyph, GlyphKind, GlyphWorld};
use glyphspace_layout::{DeviceProfile, Viewport, compile_layout};
use glyphspace_render::{
    RendererConfig, render_canvas::CanvasFallbackRenderer, render_core::SceneBatcher,
    render_wgpu::WgpuGlyphRenderer,
};

#[test]
fn renderer_core_batches_and_diffs_scene_primitives() {
    let mut world = GlyphWorld::new("world", "CRM");
    world
        .add_glyph(Glyph::new("revenue", GlyphKind::Metric, "Revenue"))
        .unwrap();
    let layout =
        compile_layout(&world, Viewport::desktop(), None, DeviceProfile::desktop()).unwrap();

    let batch = SceneBatcher.batch(&layout);
    let diff = SceneBatcher.diff(&batch, &batch);

    assert!(batch.primitive_count > 0);
    assert!(diff.added.is_empty());
    assert!(diff.removed.is_empty());
}

#[test]
fn canvas_fallback_exports_svg_like_accessible_shapes() {
    let mut world = GlyphWorld::new("world", "CRM");
    world
        .add_glyph(Glyph::new("revenue", GlyphKind::Metric, "Revenue"))
        .unwrap();
    let layout =
        compile_layout(&world, Viewport::desktop(), None, DeviceProfile::desktop()).unwrap();

    let output = CanvasFallbackRenderer.render_to_svg_fragment(&layout);

    assert!(output.contains("data-glyph-id=\"revenue\""));
}

#[test]
fn wgpu_tier_prepares_headless_scene() {
    let mut world = GlyphWorld::new("world", "CRM");
    world
        .add_glyph(Glyph::new("revenue", GlyphKind::Metric, "Revenue"))
        .unwrap();
    let layout =
        compile_layout(&world, Viewport::desktop(), None, DeviceProfile::desktop()).unwrap();

    let scene = WgpuGlyphRenderer::headless(RendererConfig::default())
        .prepare_scene(&layout)
        .unwrap();

    assert!(scene.primitive_count > 0);
}
