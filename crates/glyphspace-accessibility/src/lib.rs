use glyphspace_core::{AccessibilityNode, GlyphId, GlyphWorld, PolicyViolation, ValidationReport};
use glyphspace_layout::LayoutResult;
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

pub fn validate_accessibility_render(
    world: &GlyphWorld,
    layout: &LayoutResult,
    tree: &AccessibilityTree,
) -> ValidationReport {
    let mut report = ValidationReport::allow();
    for glyph_id in world
        .glyphs
        .values()
        .filter(|glyph| !glyph.state.hidden)
        .map(|glyph| &glyph.id)
    {
        if !tree.nodes.contains_key(glyph_id) {
            report.push_violation(
                "missing_accessibility_node",
                format!("visible glyph {glyph_id} has no accessibility node"),
                Some(glyph_id.clone()),
            );
        }
        if !layout.accessibility_order.contains(glyph_id) {
            report.push_violation(
                "missing_accessibility_order",
                format!("visible glyph {glyph_id} is missing from accessibility order"),
                Some(glyph_id.clone()),
            );
        }
    }
    if tree.order != layout.accessibility_order {
        report.warnings.push(
            "accessibility tree order differs from resolved layout accessibility order".to_string(),
        );
    }
    for (glyph_id, node) in &tree.nodes {
        if node.label.trim().is_empty() {
            report.violations.push(PolicyViolation {
                code: "missing_accessibility_label".to_string(),
                message: format!("accessibility node {glyph_id} has no label"),
                glyph_id: Some(glyph_id.clone()),
            });
            report.allowed = false;
        }
    }
    report
}
