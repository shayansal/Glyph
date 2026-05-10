use glyphspace_core::{Density, GlyphPatch, GlyphPose, GlyphWorld, PatchOp, Priority};
use serde::{Deserialize, Serialize};

pub trait AiPatchGenerator {
    fn propose_patch(
        &self,
        world: &GlyphWorld,
        request: &UserEditRequest,
        context: &AiContext,
    ) -> PatchProposal;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserEditRequest {
    pub text: String,
}

impl UserEditRequest {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiContext {
    pub user_id: String,
    pub role: String,
}

impl AiContext {
    pub fn demo() -> Self {
        Self {
            user_id: "demo_user".to_string(),
            role: "founder".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatchProposal {
    pub patch: GlyphPatch,
    pub explanation: String,
    pub confidence: f32,
    pub rejected_operations: Vec<String>,
    pub policy_warnings: Vec<String>,
    pub before_summary: String,
    pub after_summary: String,
}

#[derive(Clone, Debug, Default)]
pub struct MockAiPatchGenerator;

impl AiPatchGenerator for MockAiPatchGenerator {
    fn propose_patch(
        &self,
        world: &GlyphWorld,
        request: &UserEditRequest,
        context: &AiContext,
    ) -> PatchProposal {
        RuleBasedPatchGenerator.propose_patch(world, request, context)
    }
}

#[derive(Clone, Debug, Default)]
pub struct RuleBasedPatchGenerator;

impl AiPatchGenerator for RuleBasedPatchGenerator {
    fn propose_patch(
        &self,
        world: &GlyphWorld,
        request: &UserEditRequest,
        _context: &AiContext,
    ) -> PatchProposal {
        let text = request.text.to_lowercase();
        if text.contains("payment confirmation") || text.contains("automatic") {
            return PatchProposal {
                patch: GlyphPatch::new(
                    "unsafe_request_safe_subset",
                    "Rejected unsafe authority changes",
                    vec![],
                ),
                explanation:
                    "Policy rejected requests to hide confirmations or auto-run high-risk actions."
                        .to_string(),
                confidence: 0.95,
                rejected_operations: vec![
                    "hide mandatory payment confirmations".to_string(),
                    "invoke close deal automatically without capability confirmation".to_string(),
                ],
                policy_warnings: vec![
                    "confirmation surfaces must remain visible".to_string(),
                    "AI may rearrange UI but may not create authority".to_string(),
                ],
                before_summary: summarize(world),
                after_summary: "No unsafe authority change applied.".to_string(),
            };
        }

        let mut ops = Vec::new();
        if text.contains("founder") || text.contains("revenue") || text.contains("risk") {
            for (id, x, y, z, scale) in [
                ("revenue", 0.0, 1.2, 0.05, 1.4),
                ("runway", 1.2, 0.8, 0.1, 1.25),
                ("risks", -1.2, 0.35, 0.15, 1.2),
                ("urgent_decisions", 0.0, -0.15, 0.05, 1.25),
            ] {
                if world.glyphs.contains_key(id) {
                    ops.push(PatchOp::SetPriority {
                        glyph_id: id.to_string(),
                        priority: Priority::Critical,
                    });
                    ops.push(PatchOp::Move {
                        glyph_id: id.to_string(),
                        pose: GlyphPose {
                            x,
                            y,
                            z,
                            scale,
                            ..GlyphPose::default()
                        },
                    });
                }
            }
            if world.glyphs.contains_key("admin_tasks") {
                ops.push(PatchOp::Collapse {
                    glyph_id: "admin_tasks".to_string(),
                });
            }
        } else if text.contains("low vision") || text.contains("accessible") {
            for id in world.glyphs.keys() {
                ops.push(PatchOp::Resize {
                    glyph_id: id.clone(),
                    scale: 1.25,
                });
                ops.push(PatchOp::SetAccessibilityPreference {
                    glyph_id: id.clone(),
                    reduced_motion: Some(true),
                    high_contrast: Some(true),
                });
            }
        } else if text.contains("hide") || text.contains("noise") || text.contains("calmer") {
            for (id, glyph) in &world.glyphs {
                if glyph.priority <= Priority::Normal && !glyph.mandatory {
                    ops.push(PatchOp::Collapse {
                        glyph_id: id.clone(),
                    });
                }
            }
            for id in world.glyphs.keys() {
                ops.push(PatchOp::SetDensity {
                    glyph_id: id.clone(),
                    density: Density::Calm,
                });
            }
        } else if text.contains("mobile") {
            for id in world.glyphs.keys() {
                ops.push(PatchOp::SetDepth {
                    glyph_id: id.clone(),
                    z: 0.0,
                });
            }
        } else if text.contains("act on") {
            for (id, glyph) in &world.glyphs {
                if glyph.capability_bindings.is_empty() && !glyph.mandatory {
                    ops.push(PatchOp::Collapse {
                        glyph_id: id.clone(),
                    });
                }
            }
        }

        if ops.is_empty() {
            ops.push(PatchOp::CreateSummaryGlyph {
                id: "ai_summary".to_string(),
                source_glyphs: world.glyphs.keys().take(3).cloned().collect(),
                label: "AI summary".to_string(),
            });
        }

        PatchProposal {
            patch: GlyphPatch::new("rule_based_personalization", request.text.clone(), ops),
            explanation: format!(
                "Rule-based adapter interpreted the request for a {} lens.",
                lens_name(&text)
            ),
            confidence: 0.76,
            rejected_operations: Vec::new(),
            policy_warnings: Vec::new(),
            before_summary: summarize(world),
            after_summary:
                "Patch changes priority, position, density, or accessibility preferences only."
                    .to_string(),
        }
    }
}

fn summarize(world: &GlyphWorld) -> String {
    format!(
        "{} glyphs, {} capabilities",
        world.glyphs.len(),
        world.capabilities.len()
    )
}

fn lens_name(text: &str) -> &'static str {
    if text.contains("founder") {
        "founder command center"
    } else if text.contains("sales rep") {
        "sales rep"
    } else if text.contains("vp sales") {
        "VP sales"
    } else {
        "personalized"
    }
}
