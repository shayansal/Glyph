use glyphspace_core::{Glyph, GlyphKind, GlyphPatch, PatchOp, PolicyContext, PolicyZone};
use glyphspace_policy::PolicyEngine;

#[test]
fn cannot_hide_security_warning() {
    let mut world = glyphspace_core::GlyphWorld::new("world", "CRM");
    world
        .add_glyph(
            Glyph::new("security_warning", GlyphKind::Panel, "Security warning")
                .with_policy_zone(PolicyZone::Security)
                .mandatory(),
        )
        .unwrap();
    let patch = GlyphPatch::new(
        "hide_security",
        "hide security",
        vec![PatchOp::Hide {
            glyph_id: "security_warning".into(),
        }],
    );

    let report = PolicyEngine.validate_patch(&world, &patch, &PolicyContext::demo_user());

    assert!(!report.allowed);
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.code == "mandatory_trust_surface")
    );
}

#[test]
fn can_move_optional_glyph() {
    let mut world = glyphspace_core::GlyphWorld::new("world", "CRM");
    world
        .add_glyph(Glyph::new("admin", GlyphKind::Panel, "Admin"))
        .unwrap();
    let patch = GlyphPatch::new(
        "move_admin",
        "move admin",
        vec![PatchOp::Move {
            glyph_id: "admin".into(),
            pose: glyphspace_core::GlyphPose::at(1.0, 2.0, 0.5),
        }],
    );

    let report = PolicyEngine.validate_patch(&world, &patch, &PolicyContext::demo_user());

    assert!(report.allowed);
}
