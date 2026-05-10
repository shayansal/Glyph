use glyphspace_core::{
    AccessibilityNode, Capability, CapabilityBinding, EdgeKind, Glyph, GlyphEdge, GlyphId,
    GlyphKind, GlyphPatch, GlyphPose, GlyphWorld, PatchOp, PolicyZone, Priority, RiskLevel,
    SemanticRole,
};

#[test]
fn create_world_adds_glyphs_edges_and_serializes() {
    let mut world = GlyphWorld::new("world_crm", "CRM");
    let revenue = Glyph::new("revenue", GlyphKind::Metric, "Revenue")
        .with_role(SemanticRole::Metric)
        .with_priority(Priority::Critical);
    let risk = Glyph::new("risk", GlyphKind::Panel, "Risk");

    world.add_glyph(revenue).unwrap();
    world.add_glyph(risk).unwrap();
    world
        .add_edge(GlyphEdge::new("revenue", "risk", EdgeKind::RelatedTo))
        .unwrap();

    let json = serde_json::to_string(&world).unwrap();
    let restored: GlyphWorld = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.glyphs.len(), 2);
    assert_eq!(restored.edges[0].kind, EdgeKind::RelatedTo);
    assert_eq!(restored.glyphs["revenue"].priority, Priority::Critical);
}

#[test]
fn stable_layout_hash_is_independent_of_insertion_order() {
    let mut a = GlyphWorld::new("world", "A");
    a.add_glyph(Glyph::new("b", GlyphKind::Dot, "B")).unwrap();
    a.add_glyph(Glyph::new("a", GlyphKind::Dot, "A")).unwrap();

    let mut b = GlyphWorld::new("world", "A");
    b.add_glyph(Glyph::new("a", GlyphKind::Dot, "A")).unwrap();
    b.add_glyph(Glyph::new("b", GlyphKind::Dot, "B")).unwrap();

    assert_eq!(a.stable_layout_hash(), b.stable_layout_hash());
}

#[test]
fn patch_ops_round_trip_with_snake_case_tags() {
    let patch = GlyphPatch::new(
        "patch_founder",
        "Prioritize revenue",
        vec![
            PatchOp::SetPriority {
                glyph_id: GlyphId::from("revenue"),
                priority: Priority::Critical,
            },
            PatchOp::Move {
                glyph_id: GlyphId::from("revenue"),
                pose: GlyphPose {
                    x: 0.0,
                    y: 1.2,
                    z: 0.1,
                    scale: 1.4,
                    ..GlyphPose::default()
                },
            },
        ],
    );

    let value = serde_json::to_value(&patch).unwrap();
    assert_eq!(value["ops"][0]["type"], "set_priority");
    let restored: GlyphPatch = serde_json::from_value(value).unwrap();
    assert_eq!(restored.ops.len(), 2);
}

#[test]
fn capability_model_carries_risk_permissions_and_bindings() {
    let capability = Capability::new("deal.update_stage", "Update Deal Stage")
        .with_risk(RiskLevel::Medium)
        .with_permission("crm.deal.write");

    let glyph = Glyph::new("deal_stage", GlyphKind::Button, "Update stage")
        .with_capability(CapabilityBinding::new("deal.update_stage"));

    assert!(
        capability
            .required_permissions
            .contains(&"crm.deal.write".to_string())
    );
    assert_eq!(
        glyph.capability_bindings[0].capability_id,
        "deal.update_stage"
    );
}

#[test]
fn trust_glyphs_keep_accessibility_metadata() {
    let glyph = Glyph::new(
        "payment_confirmation",
        GlyphKind::Panel,
        "Payment confirmation",
    )
    .with_policy_zone(PolicyZone::Payment)
    .with_accessibility(AccessibilityNode::button("Confirm payment"));

    assert_eq!(glyph.policy_zone, PolicyZone::Payment);
    assert_eq!(glyph.accessibility.label, "Confirm payment");
}
