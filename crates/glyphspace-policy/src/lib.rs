use glyphspace_core::{
    Capability, Glyph, GlyphPatch, GlyphWorld, PatchOp, PolicyContext, PolicyZone, RiskLevel,
    ValidationReport,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default)]
pub struct PolicyEngine;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyOutcome {
    Accepted,
    AcceptedWithWarnings,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub outcome: PolicyOutcome,
    pub report: ValidationReport,
    pub explanation: String,
    pub audit_events: Vec<AuditEvent>,
    pub safe_world: GlyphWorld,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    pub action: String,
    pub actor: String,
    pub subject: String,
    pub outcome: PolicyOutcome,
    pub explanation: String,
}

impl PolicyEngine {
    pub fn validate_world(&self, world: &GlyphWorld, context: &PolicyContext) -> ValidationReport {
        let mut report = ValidationReport::allow();
        for glyph in world.glyphs.values() {
            self.validate_accessibility(glyph, &mut report);
            for binding in &glyph.capability_bindings {
                match world.capabilities.get(&binding.capability_id) {
                    Some(capability) => {
                        self.validate_capability_invocation(capability, context, &mut report);
                    }
                    None => report.push_violation(
                        "missing_capability",
                        format!("glyph {} binds unknown capability", glyph.id),
                        Some(glyph.id.clone()),
                    ),
                }
            }
        }
        report
    }

    pub fn validate_patch(
        &self,
        world: &GlyphWorld,
        patch: &GlyphPatch,
        context: &PolicyContext,
    ) -> ValidationReport {
        let mut report = if context.can_personalize && context.has_permission("ui.personalize") {
            ValidationReport::allow()
        } else {
            ValidationReport::reject("missing_permission", "user cannot personalize this world")
        };

        for op in &patch.ops {
            self.validate_patch_op(world, op, context, &mut report);
        }
        report
    }

    pub fn evaluate_patch(
        &self,
        current_world: &GlyphWorld,
        last_safe_world: &GlyphWorld,
        patch: &GlyphPatch,
        context: &PolicyContext,
    ) -> PolicyDecision {
        let report = self.validate_patch(current_world, patch, context);
        let outcome = if !report.allowed {
            PolicyOutcome::Rejected
        } else if report.warnings.is_empty() {
            PolicyOutcome::Accepted
        } else {
            PolicyOutcome::AcceptedWithWarnings
        };
        let explanation = explain_policy_boundary(&report);
        let safe_world = if matches!(outcome, PolicyOutcome::Rejected) {
            last_safe_world.clone()
        } else {
            current_world.clone()
        };
        let audit_events = vec![AuditEvent {
            action: match outcome {
                PolicyOutcome::Accepted => "patch.accepted",
                PolicyOutcome::AcceptedWithWarnings => "patch.accepted_with_warnings",
                PolicyOutcome::Rejected => "patch.rejected",
            }
            .to_string(),
            actor: context.user_id.clone(),
            subject: patch.id.clone(),
            outcome: outcome.clone(),
            explanation: explanation.clone(),
        }];

        PolicyDecision {
            outcome,
            report,
            explanation,
            audit_events,
            safe_world,
        }
    }

    pub fn validate_capability_invocation(
        &self,
        capability: &Capability,
        context: &PolicyContext,
        report: &mut ValidationReport,
    ) {
        for permission in &capability.required_permissions {
            if !context.has_permission(permission) {
                report.push_violation(
                    "missing_capability_permission",
                    format!("missing permission {permission}"),
                    None,
                );
            }
        }
        if matches!(capability.risk, RiskLevel::High | RiskLevel::Critical)
            && (!capability.requires_confirmation || !capability.audit)
        {
            report.push_violation(
                "unsafe_high_risk_capability",
                "high risk capabilities require confirmation and audit",
                None,
            );
        }
    }

    pub fn validate_accessibility(&self, glyph: &Glyph, report: &mut ValidationReport) {
        if (glyph.capability_bindings.is_empty() && glyph.affordances.is_empty())
            || glyph.accessibility.is_valid_for_interactive()
        {
            return;
        }
        report.push_violation(
            "missing_accessibility_label",
            format!("interactive glyph {} needs role and label", glyph.id),
            Some(glyph.id.clone()),
        );
    }

    pub fn validate_focus_order(&self, world: &GlyphWorld) -> ValidationReport {
        let mut report = ValidationReport::allow();
        let mut seen = std::collections::BTreeSet::new();
        for glyph in world.glyphs.values() {
            if let Some(index) = glyph.accessibility.focus_index
                && !seen.insert(index)
            {
                report
                    .warnings
                    .push(format!("duplicate focus index {index}"));
            }
        }
        report
    }

    pub fn validate_trust_surface_visibility(&self, world: &GlyphWorld) -> ValidationReport {
        let mut report = ValidationReport::allow();
        for glyph in world.glyphs.values() {
            if is_protected(glyph) && glyph.state.hidden {
                report.push_violation(
                    "hidden_trust_surface",
                    format!("protected glyph {} is hidden", glyph.id),
                    Some(glyph.id.clone()),
                );
            }
        }
        report
    }

    fn validate_patch_op(
        &self,
        world: &GlyphWorld,
        op: &PatchOp,
        context: &PolicyContext,
        report: &mut ValidationReport,
    ) {
        match op {
            PatchOp::Hide { glyph_id }
            | PatchOp::Collapse { glyph_id }
            | PatchOp::Move { glyph_id, .. }
            | PatchOp::Resize { glyph_id, .. }
            | PatchOp::SetPriority { glyph_id, .. }
            | PatchOp::Expand { glyph_id }
            | PatchOp::Show { glyph_id }
            | PatchOp::Pin { glyph_id }
            | PatchOp::SetStyleToken { glyph_id, .. }
            | PatchOp::SetDensity { glyph_id, .. }
            | PatchOp::SetDepth { glyph_id, .. }
            | PatchOp::SetAccessibilityPreference { glyph_id, .. }
            | PatchOp::BindCapability { glyph_id, .. }
            | PatchOp::UnbindOptionalCapability { glyph_id, .. } => {
                let Some(glyph) = world.glyphs.get(glyph_id) else {
                    report.push_violation(
                        "missing_glyph",
                        format!("patch targets missing glyph {glyph_id}"),
                        Some(glyph_id.clone()),
                    );
                    return;
                };
                if matches!(op, PatchOp::Hide { .. }) && is_protected(glyph) {
                    report.push_violation(
                        "mandatory_trust_surface",
                        "mandatory trust, security, legal, payment, and compliance surfaces cannot be hidden",
                        Some(glyph_id.clone()),
                    );
                }
                if let PatchOp::BindCapability { capability_id, .. } = op {
                    match world.capabilities.get(capability_id) {
                        Some(capability) => {
                            self.validate_capability_invocation(capability, context, report);
                        }
                        None => report.push_violation(
                            "fake_capability",
                            format!("cannot bind unknown capability {capability_id}"),
                            Some(glyph_id.clone()),
                        ),
                    }
                }
            }
            PatchOp::CreateAgentGlyph {
                allowed_capabilities,
                ..
            } => {
                for capability_id in allowed_capabilities {
                    if !world.capabilities.contains_key(capability_id) {
                        report.push_violation(
                            "fake_capability",
                            format!("agent cannot claim unknown capability {capability_id}"),
                            None,
                        );
                    }
                }
            }
            PatchOp::CreateSummaryGlyph { source_glyphs, .. } => {
                for glyph_id in source_glyphs {
                    if !world.glyphs.contains_key(glyph_id) {
                        report.push_violation(
                            "missing_summary_source",
                            format!("summary source {glyph_id} does not exist"),
                            Some(glyph_id.clone()),
                        );
                    }
                }
            }
            PatchOp::Group { glyph_ids, .. }
            | PatchOp::ReorderFocus {
                ordered_glyph_ids: glyph_ids,
            } => {
                for glyph_id in glyph_ids {
                    if !world.glyphs.contains_key(glyph_id) {
                        report.push_violation(
                            "missing_glyph",
                            format!("patch targets missing glyph {glyph_id}"),
                            Some(glyph_id.clone()),
                        );
                    }
                }
            }
            PatchOp::Ungroup { group_id } => {
                if !world.glyphs.contains_key(group_id) {
                    report
                        .warnings
                        .push(format!("ungroup target {group_id} does not exist yet"));
                }
            }
        }
    }
}

fn is_protected(glyph: &Glyph) -> bool {
    glyph.mandatory
        || matches!(
            glyph.policy_zone,
            PolicyZone::Trust
                | PolicyZone::Security
                | PolicyZone::Legal
                | PolicyZone::Payment
                | PolicyZone::Compliance
                | PolicyZone::Mandatory
        )
}

fn explain_policy_boundary(report: &ValidationReport) -> String {
    if report.allowed {
        return "Patch accepted: personalization may rearrange and restyle UI while preserving authority, permissions, confirmations, audit, and accessibility.".to_string();
    }
    let reasons = report
        .violations
        .iter()
        .map(|violation| format!("{}: {}", violation.code, violation.message))
        .collect::<Vec<_>>()
        .join("; ");
    format!(
        "Patch rejected: AI may rearrange UI but may not create authority, bypass confirmations, hide mandatory trust surfaces, or remove accessibility semantics. {reasons}"
    )
}
