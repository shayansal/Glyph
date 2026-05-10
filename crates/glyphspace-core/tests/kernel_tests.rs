use glyphspace_core::{
    CapabilityBinding, Glyph, GlyphKind, GlyphPatch, GlyphPose, GlyphWorld, PatchOp, Priority,
    SemanticChangeKind, detect_patch_conflicts, semantic_diff,
};

#[test]
fn canonical_serialization_is_stable_across_insertion_order() {
    let mut a = GlyphWorld::new("world", "CRM");
    a.add_glyph(Glyph::new("b", GlyphKind::Panel, "B")).unwrap();
    a.add_glyph(Glyph::new("a", GlyphKind::Metric, "A"))
        .unwrap();

    let mut b = GlyphWorld::new("world", "CRM");
    b.add_glyph(Glyph::new("a", GlyphKind::Metric, "A"))
        .unwrap();
    b.add_glyph(Glyph::new("b", GlyphKind::Panel, "B")).unwrap();

    assert_eq!(
        a.to_canonical_json().unwrap(),
        b.to_canonical_json().unwrap()
    );
    assert_eq!(a.canonical_digest().unwrap(), b.canonical_digest().unwrap());
}

#[test]
fn semantic_diff_reports_added_removed_and_changed_glyphs() {
    let mut before = GlyphWorld::new("world", "CRM");
    before
        .add_glyph(Glyph::new("revenue", GlyphKind::Metric, "Revenue"))
        .unwrap();
    before
        .add_glyph(Glyph::new("admin", GlyphKind::Panel, "Admin"))
        .unwrap();

    let mut after = GlyphWorld::new("world", "CRM");
    after
        .add_glyph(
            Glyph::new("revenue", GlyphKind::Metric, "Revenue").with_priority(Priority::Critical),
        )
        .unwrap();
    after
        .add_glyph(Glyph::new("risk", GlyphKind::Warning, "Risk"))
        .unwrap();

    let diff = semantic_diff(&before, &after);

    assert!(diff.has_changes());
    assert!(
        diff.changes
            .iter()
            .any(|change| change.kind == SemanticChangeKind::GlyphChanged
                && change.path == "glyphs.revenue.priority")
    );
    assert!(
        diff.changes
            .iter()
            .any(|change| change.kind == SemanticChangeKind::GlyphRemoved
                && change.path == "glyphs.admin")
    );
    assert!(diff.changes.iter().any(
        |change| change.kind == SemanticChangeKind::GlyphAdded && change.path == "glyphs.risk"
    ));
}

#[test]
fn patch_conflict_detection_finds_same_field_different_values() {
    let left = GlyphPatch::new(
        "left",
        "left",
        vec![PatchOp::SetPriority {
            glyph_id: "revenue".into(),
            priority: Priority::High,
        }],
    );
    let right = GlyphPatch::new(
        "right",
        "right",
        vec![PatchOp::SetPriority {
            glyph_id: "revenue".into(),
            priority: Priority::Critical,
        }],
    );

    let report = detect_patch_conflicts(&left, &right);

    assert!(report.has_conflicts());
    assert_eq!(report.conflicts[0].path, "glyphs.revenue.priority");
}

#[test]
fn patch_conflict_detection_allows_identical_capability_bindings() {
    let left = GlyphPatch::new(
        "left",
        "left",
        vec![PatchOp::BindCapability {
            glyph_id: "follow_ups".into(),
            capability_id: "task.create_followup".into(),
        }],
    );
    let right = GlyphPatch::new(
        "right",
        "right",
        vec![PatchOp::BindCapability {
            glyph_id: "follow_ups".into(),
            capability_id: "task.create_followup".into(),
        }],
    );

    assert!(!detect_patch_conflicts(&left, &right).has_conflicts());
}

#[test]
fn capability_binding_normalizes_to_conflict_key() {
    let binding = CapabilityBinding::new("deal.update_stage");
    assert_eq!(
        binding.conflict_key("deal_stage"),
        "glyphs.deal_stage.capability.deal.update_stage"
    );
}

#[test]
fn move_patch_conflict_key_is_pose() {
    let op = PatchOp::Move {
        glyph_id: "revenue".into(),
        pose: GlyphPose::at(1.0, 2.0, 0.0),
    };

    assert_eq!(op.conflict_key(), Some("glyphs.revenue.pose".to_string()));
}
