use glyphspace_core::{Glyph, GlyphKind, GlyphPatch, GlyphPose, GlyphWorld, PatchOp, Priority};
use glyphspace_personalization::{apply_patch, invert_patch_against_world};
use proptest::prelude::*;

fn base_world() -> GlyphWorld {
    let mut world = GlyphWorld::new("world", "CRM");
    world
        .add_glyph(Glyph::new("revenue", GlyphKind::Metric, "Revenue"))
        .unwrap();
    world
}

proptest! {
    #[test]
    fn patch_then_world_aware_inverse_returns_equivalent_world(x in -10.0f32..10.0, y in -10.0f32..10.0, z in 0.0f32..3.0, scale in 0.5f32..2.0) {
        let world = base_world();
        let patch = GlyphPatch::new(
            "personalize",
            "move and resize",
            vec![
                PatchOp::Move {
                    glyph_id: "revenue".into(),
                    pose: GlyphPose { x, y, z, scale, ..GlyphPose::default() },
                },
                PatchOp::SetPriority {
                    glyph_id: "revenue".into(),
                    priority: Priority::Critical,
                },
            ],
        );
        let context = glyphspace_core::PolicyContext::demo_user();
        let updated = apply_patch(&world, &patch, &context).unwrap();
        let inverse = invert_patch_against_world(&world, &patch).unwrap();
        let restored = apply_patch(&updated, &inverse, &context).unwrap();

        prop_assert_eq!(world.to_canonical_json().unwrap(), restored.to_canonical_json().unwrap());
    }
}
