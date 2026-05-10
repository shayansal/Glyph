use glyphspace_core::{
    Capability, CapabilityBinding, Glyph, GlyphKind, GlyphPatch, PatchOp, PolicyContext,
    PolicyZone, RiskLevel,
};
use glyphspace_policy::{PolicyEngine, PolicyOutcome};

#[test]
fn policy_decision_explains_authority_boundary_and_records_audit() {
    let mut world = glyphspace_core::GlyphWorld::new("world", "CRM");
    world
        .add_glyph(
            Glyph::new(
                "payment_confirmation",
                GlyphKind::Panel,
                "Close deal confirmation",
            )
            .with_policy_zone(PolicyZone::Payment)
            .mandatory(),
        )
        .unwrap();
    let patch = GlyphPatch::new(
        "unsafe",
        "hide confirmation",
        vec![PatchOp::Hide {
            glyph_id: "payment_confirmation".into(),
        }],
    );

    let decision = PolicyEngine.evaluate_patch(&world, &world, &patch, &PolicyContext::demo_user());

    assert_eq!(decision.outcome, PolicyOutcome::Rejected);
    assert!(
        decision
            .explanation
            .contains("may rearrange UI but may not create authority")
    );
    assert_eq!(
        decision.safe_world.canonical_digest().unwrap(),
        world.canonical_digest().unwrap()
    );
    assert!(
        decision
            .audit_events
            .iter()
            .any(|event| event.action == "patch.rejected")
    );
}

#[test]
fn high_risk_capability_requires_confirmation_and_audit() {
    let mut world = glyphspace_core::GlyphWorld::new("world", "CRM");
    world.capabilities.insert(
        "deal.close_won".into(),
        Capability::new("deal.close_won", "Close Won").with_risk(RiskLevel::High),
    );
    world
        .add_glyph(
            Glyph::new("close", GlyphKind::Button, "Close")
                .with_capability(CapabilityBinding::new("deal.close_won")),
        )
        .unwrap();

    let report = PolicyEngine.validate_world(&world, &PolicyContext::demo_user());

    assert!(!report.allowed);
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.code == "unsafe_high_risk_capability")
    );
}

#[test]
fn capability_permission_gate_rejects_missing_permission() {
    let mut world = glyphspace_core::GlyphWorld::new("world", "CRM");
    world.capabilities.insert(
        "admin.delete".into(),
        Capability::new("admin.delete", "Delete").with_permission("admin.delete"),
    );
    world
        .add_glyph(
            Glyph::new("delete", GlyphKind::Button, "Delete")
                .with_capability(CapabilityBinding::new("admin.delete")),
        )
        .unwrap();

    let report = PolicyEngine.validate_world(&world, &PolicyContext::demo_user());

    assert!(!report.allowed);
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.code == "missing_capability_permission")
    );
}
