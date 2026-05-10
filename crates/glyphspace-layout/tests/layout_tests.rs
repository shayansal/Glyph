use glyphspace_core::{Glyph, GlyphKind, Priority};
use glyphspace_layout::{DeviceProfile, LayoutMode, Viewport, compile_layout};

#[test]
fn deterministic_layout_is_stable() {
    let mut world = glyphspace_core::GlyphWorld::new("world", "CRM");
    for id in ["revenue", "runway", "risk", "admin"] {
        world.add_glyph(Glyph::new(id, GlyphKind::Dot, id)).unwrap();
    }

    let a = compile_layout(&world, Viewport::desktop(), None, DeviceProfile::desktop()).unwrap();
    let b = compile_layout(&world, Viewport::desktop(), None, DeviceProfile::desktop()).unwrap();

    assert_eq!(a.layout_hash, b.layout_hash);
    assert_eq!(a.resolved_poses, b.resolved_poses);
}

#[test]
fn priority_controls_z_depth() {
    let mut world = glyphspace_core::GlyphWorld::new("world", "CRM");
    world
        .add_glyph(
            Glyph::new("revenue", GlyphKind::Metric, "Revenue").with_priority(Priority::Critical),
        )
        .unwrap();
    world
        .add_glyph(Glyph::new("archive", GlyphKind::Panel, "Archive").with_priority(Priority::Low))
        .unwrap();

    let result = compile_layout(
        &world,
        Viewport::desktop(),
        Some(LayoutMode::TwoPointFiveD),
        DeviceProfile::desktop(),
    )
    .unwrap();

    assert!(result.resolved_poses["revenue"].z < result.resolved_poses["archive"].z);
}
