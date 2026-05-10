use glyphspace_accessibility::{build_accessibility_tree, validate_accessibility_render};
use glyphspace_core::{AccessibilityNode, Glyph, GlyphKind, GlyphWorld};
use glyphspace_layout::{DeviceProfile, Viewport, compile_layout};

#[test]
fn every_visual_glyph_has_accessibility_node_and_spatial_description() {
    let mut world = GlyphWorld::new("world", "CRM");
    world
        .add_glyph(
            Glyph::new("revenue", GlyphKind::Metric, "Revenue").with_accessibility(
                AccessibilityNode {
                    spatial_description: "Important metric near the front.".into(),
                    ..AccessibilityNode::static_text("Revenue")
                },
            ),
        )
        .unwrap();
    let layout =
        compile_layout(&world, Viewport::desktop(), None, DeviceProfile::desktop()).unwrap();
    let tree = build_accessibility_tree(&world);

    let report = validate_accessibility_render(&world, &layout, &tree);

    assert!(report.allowed);
    assert_eq!(tree.order, layout.accessibility_order);
}

#[test]
fn accessibility_renderer_rejects_missing_visual_node() {
    let mut world = GlyphWorld::new("world", "CRM");
    world
        .add_glyph(Glyph::new("revenue", GlyphKind::Metric, "Revenue"))
        .unwrap();
    let mut layout =
        compile_layout(&world, Viewport::desktop(), None, DeviceProfile::desktop()).unwrap();
    layout.accessibility_order.clear();
    let tree = build_accessibility_tree(&world);

    let report = validate_accessibility_render(&world, &layout, &tree);

    assert!(!report.allowed);
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.code == "missing_accessibility_order")
    );
}
