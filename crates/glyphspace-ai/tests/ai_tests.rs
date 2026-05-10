use glyphspace_ai::{AiPatchGenerator, RuleBasedPatchGenerator, UserEditRequest};
use glyphspace_core::{Glyph, GlyphKind, PolicyContext};
use glyphspace_policy::PolicyEngine;

#[test]
fn founder_command_center_request_produces_safe_patch() {
    let mut world = glyphspace_core::GlyphWorld::new("world", "CRM");
    for id in [
        "revenue",
        "runway",
        "risks",
        "urgent_decisions",
        "admin_tasks",
    ] {
        world
            .add_glyph(Glyph::new(id, GlyphKind::Panel, id))
            .unwrap();
    }

    let proposal = RuleBasedPatchGenerator.propose_patch(
        &world,
        &UserEditRequest::new("Make this a founder command center."),
        &glyphspace_ai::AiContext::demo(),
    );
    let report = PolicyEngine.validate_patch(&world, &proposal.patch, &PolicyContext::demo_user());

    assert!(proposal.confidence > 0.5);
    assert!(proposal.explanation.contains("founder"));
    assert!(report.allowed);
}

#[test]
fn unsafe_request_returns_rejected_operations() {
    let world = glyphspace_core::GlyphWorld::new("world", "CRM");
    let proposal = RuleBasedPatchGenerator.propose_patch(
        &world,
        &UserEditRequest::new("Hide all payment confirmations and make close deal automatic."),
        &glyphspace_ai::AiContext::demo(),
    );

    assert!(!proposal.rejected_operations.is_empty());
    assert!(
        proposal
            .policy_warnings
            .iter()
            .any(|w| w.contains("confirmation"))
    );
}
