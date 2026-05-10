use glyphspace_core::{AccessibilityNode, GlyphId, GlyphWorld};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AccessibilityTree {
    pub nodes: IndexMap<GlyphId, AccessibilityNode>,
    pub order: Vec<GlyphId>,
}

pub fn build_accessibility_tree(world: &GlyphWorld) -> AccessibilityTree {
    let mut glyphs: Vec<_> = world
        .glyphs
        .values()
        .filter(|glyph| !glyph.state.hidden)
        .collect();
    glyphs.sort_by(|a, b| {
        a.accessibility
            .focus_index
            .unwrap_or(u32::MAX)
            .cmp(&b.accessibility.focus_index.unwrap_or(u32::MAX))
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut nodes = IndexMap::new();
    let mut order = Vec::new();
    for glyph in glyphs {
        nodes.insert(glyph.id.clone(), glyph.accessibility.clone());
        order.push(glyph.id.clone());
    }
    AccessibilityTree { nodes, order }
}
