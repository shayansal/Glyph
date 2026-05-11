use glyphspace_core::{AccessibilityNode, GlyphId, GlyphWorld, PolicyViolation, ValidationReport};
use glyphspace_layout::LayoutResult;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeAccessibilityBridge {
    pub platforms: Vec<String>,
    pub supports_focus_order: bool,
    pub supports_spoken_spatial_descriptions: bool,
    pub supports_live_regions: bool,
}

impl NativeAccessibilityBridge {
    pub fn desktop_and_mobile() -> Self {
        Self {
            platforms: vec![
                "windows.uia".to_string(),
                "macos.ax".to_string(),
                "linux.atspi".to_string(),
                "ios.uiaccessibility".to_string(),
                "android.accessibility_node_provider".to_string(),
            ],
            supports_focus_order: true,
            supports_spoken_spatial_descriptions: true,
            supports_live_regions: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessibilitySnapshot {
    pub node_count: usize,
    pub focus_order: Vec<GlyphId>,
    pub labels: Vec<String>,
    pub digest: String,
}

impl AccessibilitySnapshot {
    pub fn from_world(world: &GlyphWorld) -> Self {
        let tree = build_accessibility_tree(world);
        let labels = tree
            .order
            .iter()
            .filter_map(|id| tree.nodes.get(id))
            .map(|node| node.label.clone())
            .collect::<Vec<_>>();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        tree.order.hash(&mut hasher);
        labels.hash(&mut hasher);
        Self {
            node_count: tree.nodes.len(),
            focus_order: tree.order,
            labels,
            digest: format!("{:016x}", hasher.finish()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenReaderHarness;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenReaderTranscript {
    pub utterances: Vec<String>,
}

impl ScreenReaderHarness {
    pub fn new() -> Self {
        Self
    }

    pub fn read_snapshot(&self, snapshot: &AccessibilitySnapshot) -> ScreenReaderTranscript {
        ScreenReaderTranscript {
            utterances: snapshot
                .labels
                .iter()
                .enumerate()
                .map(|(index, label)| {
                    format!("{} of {}: {}", index + 1, snapshot.node_count, label)
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessibilityInspector;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessibilityInspection {
    pub focus_order: Vec<GlyphId>,
    pub issues: Vec<String>,
    pub node_count: usize,
}

impl AccessibilityInspector {
    pub fn new() -> Self {
        Self
    }

    pub fn inspect(&self, snapshot: &AccessibilitySnapshot) -> AccessibilityInspection {
        let mut issues = Vec::new();
        if snapshot.node_count != snapshot.focus_order.len() {
            issues.push("focus order does not cover every node".to_string());
        }
        if snapshot.labels.iter().any(|label| label.trim().is_empty()) {
            issues.push("empty accessibility label".to_string());
        }
        AccessibilityInspection {
            focus_order: snapshot.focus_order.clone(),
            node_count: snapshot.node_count,
            issues,
        }
    }
}
