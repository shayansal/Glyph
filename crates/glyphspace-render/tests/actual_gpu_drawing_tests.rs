use glyphspace_core::{EdgeKind, Glyph, GlyphEdge, GlyphWorld};
use glyphspace_layout::{DeviceProfile, Viewport};
use glyphspace_render::{ActualGpuRenderer, GpuSurfaceConfig, ProductionRenderer, TextAtlas};

fn world() -> GlyphWorld {
    let mut world = GlyphWorld::new("pixels", "Pixels");
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
fn actual_gpu_renderer_allocates_buffers_pipeline_text_atlas_msaa_and_pixels() {
    let world = world();
    let mut renderer = ProductionRenderer::headless(Viewport::desktop(), DeviceProfile::desktop())
        .with_focus("revenue");
    let frame = renderer.render_world(&world).expect("frame renders");

    let mut gpu = ActualGpuRenderer::headless(GpuSurfaceConfig::new(320, 180).with_msaa(4));
    let output = gpu.render_frame(&frame).expect("gpu render");

    assert_eq!(gpu.surface_config().sample_count, 4);
    assert!(gpu.pipeline().shader_wgsl.contains("@fragment"));
    assert!(gpu.buffers().vertex_bytes > 0);
    assert!(gpu.buffers().index_bytes > 0);
    assert!(gpu.text_atlas().glyph_count() >= 4);
    assert_eq!(output.width, 320);
    assert_eq!(output.height, 180);
    assert!(output.pixels.iter().any(|pixel| *pixel != 0));
    assert_ne!(output.pixel_digest, "0000000000000000");
}

#[test]
fn actual_gpu_renderer_resizes_and_preserves_snapshot_conformance() {
    let world = world();
    let mut renderer = ProductionRenderer::headless(Viewport::desktop(), DeviceProfile::desktop());
    let frame = renderer.render_world(&world).expect("frame renders");
    let mut gpu = ActualGpuRenderer::headless(GpuSurfaceConfig::new(160, 90));

    let before = gpu.render_frame(&frame).expect("first render");
    gpu.resize(400, 240);
    let after = gpu.render_frame(&frame).expect("second render");

    assert_eq!(before.width, 160);
    assert_eq!(after.width, 400);
    assert_eq!(after.height, 240);
    assert_eq!(after.coverage, before.coverage);
}

#[test]
fn text_atlas_shapes_and_caches_runs_for_gpu_upload() {
    let mut atlas = TextAtlas::new(512, 512);
    let first = atlas.cache_run("revenue", "Revenue", 16.0);
    let second = atlas.cache_run("revenue", "Revenue", 16.0);

    assert_eq!(first.cache_key, second.cache_key);
    assert_eq!(atlas.glyph_count(), first.glyph_count);
    assert!(atlas.texture_bytes() > 0);
}
