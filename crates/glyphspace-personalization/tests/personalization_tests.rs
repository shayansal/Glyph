use glyphspace_core::{Glyph, GlyphKind, GlyphPatch, GlyphPose, PatchOp, PolicyContext};
use glyphspace_personalization::{apply_patch, explain_patch, invert_patch};

#[test]
fn applies_move_and_collapse_patch() {
    let mut world = glyphspace_core::GlyphWorld::new("world", "CRM");
    world
        .add_glyph(Glyph::new("admin", GlyphKind::Panel, "Admin"))
        .unwrap();
    let patch = GlyphPatch::new(
        "p",
        "Move and collapse admin",
        vec![
            PatchOp::Move {
                glyph_id: "admin".into(),
                pose: GlyphPose::at(2.0, 0.0, 1.0),
            },
            PatchOp::Collapse {
                glyph_id: "admin".into(),
            },
        ],
    );

    let updated = apply_patch(&world, &patch, &PolicyContext::demo_user()).unwrap();

    assert_eq!(updated.glyphs["admin"].pose.x, 2.0);
    assert!(updated.glyphs["admin"].state.collapsed);
}

#[test]
fn invert_patch_reverses_order_and_explains() {
    let patch = GlyphPatch::new(
        "p",
        "Hide admin",
        vec![PatchOp::Hide {
            glyph_id: "admin".into(),
        }],
    );
    let inverse = invert_patch(&patch);

    assert!(matches!(inverse.ops[0], PatchOp::Show { .. }));
    assert!(explain_patch(&patch).contains("Hide admin"));
}
