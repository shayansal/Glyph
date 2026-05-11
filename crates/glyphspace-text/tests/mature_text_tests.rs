use glyphspace_text::{
    FontDescriptor, TextDirection, TextEngine, TextRun, TextWrap, UnicodeScript,
};

#[test]
fn mature_text_shape_handles_fallback_emoji_rtl_ligatures_and_wrapping() {
    let engine = TextEngine::new()
        .with_font(FontDescriptor::system("Inter"))
        .with_fallback(FontDescriptor::system("Noto Color Emoji"))
        .with_fallback(FontDescriptor::system("Noto Sans Arabic"));
    let run = TextRun::new("office revenue مرحبا 🚀 pipeline forecast", 18.0)
        .with_direction(TextDirection::Auto)
        .with_wrap(TextWrap::Word)
        .with_max_width(150.0);

    let shaped = engine.shape_rich(&run).expect("rich shape");

    assert!(shaped.lines.len() > 1);
    assert!(shaped.contains_rtl);
    assert!(shaped.contains_emoji);
    assert!(shaped.ligature_count >= 1);
    assert!(
        shaped
            .glyphs
            .iter()
            .any(|glyph| glyph.script == UnicodeScript::Arabic)
    );
    assert!(
        shaped
            .glyphs
            .iter()
            .any(|glyph| glyph.font_family == "Noto Color Emoji")
    );
    assert!(shaped.lines.iter().all(|line| line.width <= 150.0));
}
