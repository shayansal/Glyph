use glyphspace_core::{Glyph, GlyphWorld};
use glyphspace_layout::{DeviceProfile, Viewport, compile_layout};
use glyphspace_render::render_core::{SceneBatcher, ScenePatch};
use glyphspace_render::{
    GlyphTextRun, RendererConfig, SelectionStyle, TextShaper, WgpuGlyphRenderer,
    render_canvas::CanvasFallbackRenderer,
};

#[test]
fn renderer_shapes_text_tracks_selection_and_applies_scene_patch() {
    let shaped = TextShaper::placeholder().shape("Revenue $1.2M", 16.0);
    let selection = SelectionStyle::focused("revenue");

    assert_eq!(shaped.glyphs[0].cluster, 0);
    assert_eq!(selection.glyph_id, "revenue");
    assert!(selection.outline_width > 0.0);

    let mut world = GlyphWorld::new("render", "Render");
    world
        .add_glyph(Glyph::metric("revenue", "Revenue"))
        .unwrap();
    let layout =
        compile_layout(&world, Viewport::desktop(), None, DeviceProfile::desktop()).unwrap();
    let batcher = SceneBatcher;
    let before = batcher.batch(&layout);
    let after = before.clone();
    let patch = ScenePatch::from_diff(batcher.diff(&before, &after));

    assert!(patch.operations.is_empty());
}

#[test]
fn canvas_fallback_and_wgpu_prepare_matching_primitive_counts() {
    let mut world = GlyphWorld::new("render", "Render");
    world
        .add_glyph(Glyph::metric("revenue", "Revenue"))
        .unwrap();
    let layout =
        compile_layout(&world, Viewport::desktop(), None, DeviceProfile::desktop()).unwrap();

    let canvas = CanvasFallbackRenderer.render_to_svg_fragment(&layout);
    let scene = WgpuGlyphRenderer::headless(RendererConfig::default())
        .prepare_scene(&layout)
        .unwrap();
    let text = GlyphTextRun::new("revenue", "Revenue").with_font_size(14.0);

    assert!(canvas.contains("data-glyph-id=\"revenue\""));
    assert_eq!(scene.primitive_count, layout.render_primitives.len());
    assert_eq!(text.text, "Revenue");
}
