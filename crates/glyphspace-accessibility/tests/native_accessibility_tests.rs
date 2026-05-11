use glyphspace_accessibility::{
    AccessibilityInspector, AccessibilitySnapshot, NativeAccessibilityBridge, ScreenReaderHarness,
};
use glyphspace_core::{Glyph, GlyphWorld};

fn world() -> GlyphWorld {
    let mut world = GlyphWorld::new("a11y-native", "Accessibility Native");
    world.add_glyph(Glyph::button("deal", "Open Deal")).unwrap();
    world
}

#[test]
fn native_accessibility_bridges_cover_desktop_and_mobile_platforms() {
    let bridge = NativeAccessibilityBridge::desktop_and_mobile();

    assert!(bridge.platforms.contains(&"windows.uia".to_string()));
    assert!(bridge.platforms.contains(&"macos.ax".to_string()));
    assert!(bridge.platforms.contains(&"linux.atspi".to_string()));
    assert!(
        bridge
            .platforms
            .contains(&"ios.uiaccessibility".to_string())
    );
    assert!(
        bridge
            .platforms
            .contains(&"android.accessibility_node_provider".to_string())
    );
    assert!(bridge.supports_focus_order);
    assert!(bridge.supports_spoken_spatial_descriptions);
}

#[test]
fn screen_reader_snapshots_and_inspector_expose_accessibility_frame() {
    let world = world();
    let snapshot = AccessibilitySnapshot::from_world(&world);
    let harness = ScreenReaderHarness::new().read_snapshot(&snapshot);
    let inspector = AccessibilityInspector::new().inspect(&snapshot);

    assert_eq!(snapshot.node_count, 1);
    assert!(snapshot.digest.len() >= 8);
    assert!(
        harness
            .utterances
            .iter()
            .any(|line| line.contains("Open Deal"))
    );
    assert!(inspector.focus_order.contains(&"deal".to_string()));
    assert!(inspector.issues.is_empty());
}
