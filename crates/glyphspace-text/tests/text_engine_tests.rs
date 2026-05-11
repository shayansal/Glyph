use glyphspace_text::{FontDescriptor, TextEngine, TextRun};

#[test]
fn text_engine_shapes_rasterizes_caches_and_clips_runs() {
    let mut engine = TextEngine::new()
        .with_font(FontDescriptor::system("Inter"))
        .with_fallback(FontDescriptor::system("Noto Sans"));

    let run = TextRun::new("Revenue + risk", 16.0)
        .with_dpi_scale(2.0)
        .with_clip(0.0, 0.0, 96.0, 32.0);

    let shaped = engine.shape(&run).expect("shape run");
    assert!(shaped.glyphs.len() >= 12);
    assert!(shaped.width > 0.0);
    assert_eq!(shaped.dpi_scale, 2.0);
    assert!(shaped.glyphs.iter().all(|glyph| glyph.advance > 0.0));

    let raster = engine.rasterize(&shaped).expect("rasterize run");
    assert!(raster.atlas_width > 0);
    assert!(raster.atlas_height > 0);
    assert!(raster.alpha_pixels.iter().any(|alpha| *alpha > 0));
    assert_eq!(raster.clipped_bounds.width, 96.0);

    let cached = engine.rasterize(&shaped).expect("cached run");
    assert_eq!(cached.cache_key, raster.cache_key);
    assert!(engine.cache_stats().hits >= 1);
}
