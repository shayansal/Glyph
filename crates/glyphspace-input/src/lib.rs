use glyphspace_core::GlyphId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputEvent {
    PointerMove { x: f32, y: f32 },
    GlyphClick { glyph_id: GlyphId },
    KeyboardActivate { glyph_id: GlyphId },
    NaturalLanguageEdit { text: String },
}
