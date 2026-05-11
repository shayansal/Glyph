use glyphspace_core::{Glyph, GlyphPatch, GlyphWorld, PatchOp, PolicyContext};
use glyphspace_policy::{
    EnterprisePolicyContext, LastKnownSafeFallback, PolicyEngine, PolicyLanguage, PolicyLayerKind,
    PolicySimulator,
};

#[test]
fn policy_language_layers_enterprise_contexts_and_explains_authority_denials() {
    let policy = PolicyLanguage::parse(
        "require trust_surface visible; deny ai create_authority; require risk high confirmation audit",
    )
    .expect("policy parses");
    let context = EnterprisePolicyContext::new("tenant-a")
        .layer(PolicyLayerKind::Organization, policy.clone())
        .layer(
            PolicyLayerKind::Role,
            PolicyLanguage::parse("allow visual_rearrange").unwrap(),
        )
        .with_user_context(PolicyContext::demo_user());

    assert_eq!(policy.rules.len(), 3);
    assert!(
        context
            .effective_rules()
            .iter()
            .any(|rule| rule.contains("trust_surface"))
    );
    assert!(
        context
            .human_explanation()
            .contains("AI may personalize layout but cannot create authority")
    );
}

#[test]
fn last_known_safe_fallback_recovers_from_rejected_patch() {
    let mut world = GlyphWorld::new("safe", "Safe World");
    world
        .add_glyph(Glyph::button("confirm", "Confirm").mandatory())
        .unwrap();
    let unsafe_patch = GlyphPatch::new(
        "hide_confirm",
        "Hide confirmation",
        vec![PatchOp::Hide {
            glyph_id: "confirm".to_string(),
        }],
    );
    let decision =
        PolicyEngine.evaluate_patch(&world, &world, &unsafe_patch, &PolicyContext::demo_user());

    let fallback = LastKnownSafeFallback::new(world.clone()).recover(&decision);

    assert!(!decision.report.allowed);
    assert_eq!(fallback.world.id, world.id);
    assert!(fallback.recovered);
    assert!(fallback.explanation.contains("last known safe"));
}

#[test]
fn policy_simulator_reports_invariants_for_security_fixtures() {
    let mut world = GlyphWorld::new("sim", "Simulator");
    world
        .add_glyph(Glyph::button("confirm", "Confirm").mandatory())
        .unwrap();
    let patch = GlyphPatch::new(
        "hide_confirm",
        "Hide confirmation",
        vec![PatchOp::Hide {
            glyph_id: "confirm".to_string(),
        }],
    );

    let report = PolicySimulator::new(PolicyContext::demo_user()).simulate(&world, &patch);

    assert!(!report.allowed);
    assert!(
        report
            .invariants_checked
            .contains(&"mandatory_trust_surfaces".to_string())
    );
    assert!(report.explanation.contains("cannot hide"));
}
