use glyphspace_core::{
    AccessibilityNode, Glyph, GlyphKind, GlyphPatch, GlyphPose, GlyphWorld, PatchOp, PolicyContext,
    Priority, SemanticRole,
};
use glyphspace_policy::PolicyEngine;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PatchError {
    #[error("policy rejected patch: {0}")]
    PolicyRejected(String),
    #[error("missing glyph: {0}")]
    MissingGlyph(String),
    #[error("cannot invert patch against world: {0}")]
    CannotInvert(String),
}

pub fn validate_patch(
    world: &GlyphWorld,
    patch: &GlyphPatch,
    policy_context: &PolicyContext,
) -> glyphspace_core::ValidationReport {
    PolicyEngine.validate_patch(world, patch, policy_context)
}

pub fn apply_patch(
    world: &GlyphWorld,
    patch: &GlyphPatch,
    policy_context: &PolicyContext,
) -> Result<GlyphWorld, PatchError> {
    let report = validate_patch(world, patch, policy_context);
    if !report.allowed {
        return Err(PatchError::PolicyRejected(
            report
                .violations
                .iter()
                .map(|violation| violation.message.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        ));
    }

    let mut next = world.clone();
    for op in &patch.ops {
        apply_op(&mut next, op)?;
    }
    Ok(next)
}

pub fn invert_patch(patch: &GlyphPatch) -> GlyphPatch {
    let mut ops = Vec::new();
    for op in patch.ops.iter().rev() {
        let inverse = match op {
            PatchOp::Hide { glyph_id } => PatchOp::Show {
                glyph_id: glyph_id.clone(),
            },
            PatchOp::Show { glyph_id } => PatchOp::Hide {
                glyph_id: glyph_id.clone(),
            },
            PatchOp::Collapse { glyph_id } => PatchOp::Expand {
                glyph_id: glyph_id.clone(),
            },
            PatchOp::Expand { glyph_id } => PatchOp::Collapse {
                glyph_id: glyph_id.clone(),
            },
            PatchOp::Resize { glyph_id, .. } => PatchOp::Resize {
                glyph_id: glyph_id.clone(),
                scale: 1.0,
            },
            PatchOp::SetDepth { glyph_id, .. } => PatchOp::SetDepth {
                glyph_id: glyph_id.clone(),
                z: 0.0,
            },
            PatchOp::SetPriority { glyph_id, .. } => PatchOp::SetPriority {
                glyph_id: glyph_id.clone(),
                priority: Priority::Normal,
            },
            PatchOp::Move { glyph_id, .. } => PatchOp::Move {
                glyph_id: glyph_id.clone(),
                pose: GlyphPose::default(),
            },
            PatchOp::CreateSummaryGlyph { id, .. } | PatchOp::CreateAgentGlyph { id, .. } => {
                PatchOp::Hide {
                    glyph_id: id.clone(),
                }
            }
            PatchOp::Pin { glyph_id } => PatchOp::SetStyleToken {
                glyph_id: glyph_id.clone(),
                key: "pinned".to_string(),
                value: "false".to_string(),
            },
            PatchOp::Group { group_id, .. } => PatchOp::Ungroup {
                group_id: group_id.clone(),
            },
            PatchOp::Ungroup { group_id } => PatchOp::Show {
                glyph_id: group_id.clone(),
            },
            PatchOp::SetStyleToken { glyph_id, key, .. } => PatchOp::SetStyleToken {
                glyph_id: glyph_id.clone(),
                key: key.clone(),
                value: String::new(),
            },
            PatchOp::SetDensity { glyph_id, .. } => PatchOp::SetDensity {
                glyph_id: glyph_id.clone(),
                density: glyphspace_core::Density::Comfortable,
            },
            PatchOp::ReorderFocus { ordered_glyph_ids } => PatchOp::ReorderFocus {
                ordered_glyph_ids: ordered_glyph_ids.clone(),
            },
            PatchOp::SetAccessibilityPreference { glyph_id, .. } => {
                PatchOp::SetAccessibilityPreference {
                    glyph_id: glyph_id.clone(),
                    reduced_motion: Some(false),
                    high_contrast: Some(false),
                }
            }
            PatchOp::BindCapability {
                glyph_id,
                capability_id,
            } => PatchOp::UnbindOptionalCapability {
                glyph_id: glyph_id.clone(),
                capability_id: capability_id.clone(),
            },
            PatchOp::UnbindOptionalCapability {
                glyph_id,
                capability_id,
            } => PatchOp::BindCapability {
                glyph_id: glyph_id.clone(),
                capability_id: capability_id.clone(),
            },
        };
        ops.push(inverse);
    }
    GlyphPatch::new(
        format!("{}_inverse", patch.id),
        format!("Undo {}", patch.description),
        ops,
    )
}

pub fn invert_patch_against_world(
    world: &GlyphWorld,
    patch: &GlyphPatch,
) -> Result<GlyphPatch, PatchError> {
    let mut ops = Vec::new();
    for op in patch.ops.iter().rev() {
        let inverse = match op {
            PatchOp::Move { glyph_id, .. } => PatchOp::Move {
                glyph_id: glyph_id.clone(),
                pose: world
                    .glyphs
                    .get(glyph_id)
                    .ok_or_else(|| PatchError::CannotInvert(format!("missing glyph {glyph_id}")))?
                    .pose,
            },
            PatchOp::Resize { glyph_id, .. } => PatchOp::Resize {
                glyph_id: glyph_id.clone(),
                scale: world
                    .glyphs
                    .get(glyph_id)
                    .ok_or_else(|| PatchError::CannotInvert(format!("missing glyph {glyph_id}")))?
                    .pose
                    .scale,
            },
            PatchOp::SetPriority { glyph_id, .. } => PatchOp::SetPriority {
                glyph_id: glyph_id.clone(),
                priority: world
                    .glyphs
                    .get(glyph_id)
                    .ok_or_else(|| PatchError::CannotInvert(format!("missing glyph {glyph_id}")))?
                    .priority
                    .clone(),
            },
            PatchOp::Collapse { glyph_id } | PatchOp::Expand { glyph_id } => {
                if world
                    .glyphs
                    .get(glyph_id)
                    .ok_or_else(|| PatchError::CannotInvert(format!("missing glyph {glyph_id}")))?
                    .state
                    .collapsed
                {
                    PatchOp::Collapse {
                        glyph_id: glyph_id.clone(),
                    }
                } else {
                    PatchOp::Expand {
                        glyph_id: glyph_id.clone(),
                    }
                }
            }
            PatchOp::Hide { glyph_id } | PatchOp::Show { glyph_id } => {
                if world
                    .glyphs
                    .get(glyph_id)
                    .ok_or_else(|| PatchError::CannotInvert(format!("missing glyph {glyph_id}")))?
                    .state
                    .hidden
                {
                    PatchOp::Hide {
                        glyph_id: glyph_id.clone(),
                    }
                } else {
                    PatchOp::Show {
                        glyph_id: glyph_id.clone(),
                    }
                }
            }
            PatchOp::SetDepth { glyph_id, .. } => PatchOp::SetDepth {
                glyph_id: glyph_id.clone(),
                z: world
                    .glyphs
                    .get(glyph_id)
                    .ok_or_else(|| PatchError::CannotInvert(format!("missing glyph {glyph_id}")))?
                    .pose
                    .z,
            },
            PatchOp::SetDensity { glyph_id, .. } => PatchOp::SetDensity {
                glyph_id: glyph_id.clone(),
                density: world
                    .glyphs
                    .get(glyph_id)
                    .ok_or_else(|| PatchError::CannotInvert(format!("missing glyph {glyph_id}")))?
                    .style
                    .density
                    .clone(),
            },
            PatchOp::CreateSummaryGlyph { id, .. } | PatchOp::CreateAgentGlyph { id, .. } => {
                PatchOp::Hide {
                    glyph_id: id.clone(),
                }
            }
            other => invert_patch(&GlyphPatch::new("single", "single", vec![other.clone()]))
                .ops
                .into_iter()
                .next()
                .ok_or_else(|| PatchError::CannotInvert("empty inverse".to_string()))?,
        };
        ops.push(inverse);
    }
    Ok(GlyphPatch::new(
        format!("{}_inverse", patch.id),
        format!("Undo {}", patch.description),
        ops,
    ))
}

pub fn explain_patch(patch: &GlyphPatch) -> String {
    let mut lines = vec![patch.description.clone()];
    for op in &patch.ops {
        lines.push(match op {
            PatchOp::Move { glyph_id, pose } => {
                format!(
                    "Move {glyph_id} to ({:.1}, {:.1}, {:.1}).",
                    pose.x, pose.y, pose.z
                )
            }
            PatchOp::Resize { glyph_id, scale } => format!("Resize {glyph_id} to {scale:.1}x."),
            PatchOp::SetPriority { glyph_id, priority } => {
                format!("Set {glyph_id} priority to {priority:?}.")
            }
            PatchOp::Collapse { glyph_id } => format!("Collapse {glyph_id}."),
            PatchOp::Expand { glyph_id } => format!("Expand {glyph_id}."),
            PatchOp::Hide { glyph_id } => format!("Hide {glyph_id}."),
            PatchOp::Show { glyph_id } => format!("Show {glyph_id}."),
            PatchOp::CreateSummaryGlyph { id, label, .. } => {
                format!("Create summary glyph {id} labeled {label}.")
            }
            PatchOp::CreateAgentGlyph { id, label, .. } => {
                format!("Create agent glyph {id} labeled {label}.")
            }
            other => format!("Apply {other:?}."),
        });
    }
    lines.join("\n")
}

fn apply_op(world: &mut GlyphWorld, op: &PatchOp) -> Result<(), PatchError> {
    match op {
        PatchOp::Move { glyph_id, pose } => glyph_mut(world, glyph_id)?.pose = *pose,
        PatchOp::Resize { glyph_id, scale } => glyph_mut(world, glyph_id)?.pose.scale = *scale,
        PatchOp::SetPriority { glyph_id, priority } => {
            glyph_mut(world, glyph_id)?.priority = priority.clone();
        }
        PatchOp::Collapse { glyph_id } => glyph_mut(world, glyph_id)?.state.collapsed = true,
        PatchOp::Expand { glyph_id } => glyph_mut(world, glyph_id)?.state.collapsed = false,
        PatchOp::Hide { glyph_id } => glyph_mut(world, glyph_id)?.state.hidden = true,
        PatchOp::Show { glyph_id } => glyph_mut(world, glyph_id)?.state.hidden = false,
        PatchOp::Pin { glyph_id } => glyph_mut(world, glyph_id)?.state.pinned = true,
        PatchOp::SetStyleToken {
            glyph_id,
            key,
            value,
        } => {
            glyph_mut(world, glyph_id)?
                .style
                .tokens
                .insert(key.clone(), value.clone());
        }
        PatchOp::SetDensity { glyph_id, density } => {
            glyph_mut(world, glyph_id)?.style.density = density.clone();
        }
        PatchOp::SetDepth { glyph_id, z } => glyph_mut(world, glyph_id)?.pose.z = *z,
        PatchOp::CreateSummaryGlyph {
            id,
            source_glyphs,
            label,
        } => {
            let mut glyph = Glyph::new(id, GlyphKind::Card, label)
                .with_role(SemanticRole::Summary)
                .with_accessibility(AccessibilityNode::static_text(label));
            glyph.metadata.insert(
                "source_glyphs".to_string(),
                serde_json::json!(source_glyphs),
            );
            world
                .add_glyph(glyph)
                .map_err(|err| PatchError::MissingGlyph(err.to_string()))?;
        }
        PatchOp::CreateAgentGlyph {
            id,
            label,
            allowed_capabilities,
        } => {
            let mut glyph = Glyph::new(id, GlyphKind::Agent, label).with_role(SemanticRole::Agent);
            glyph.metadata.insert(
                "allowed_capabilities".to_string(),
                serde_json::json!(allowed_capabilities),
            );
            world
                .add_glyph(glyph)
                .map_err(|err| PatchError::MissingGlyph(err.to_string()))?;
        }
        PatchOp::ReorderFocus { ordered_glyph_ids } => {
            for (index, glyph_id) in ordered_glyph_ids.iter().enumerate() {
                glyph_mut(world, glyph_id)?.accessibility.focus_index = Some(index as u32);
            }
        }
        PatchOp::SetAccessibilityPreference {
            glyph_id,
            reduced_motion,
            high_contrast,
        } => {
            let glyph = glyph_mut(world, glyph_id)?;
            if let Some(value) = reduced_motion {
                glyph.accessibility.reduced_motion = *value;
            }
            if let Some(value) = high_contrast {
                glyph.accessibility.high_contrast = *value;
                glyph.style.high_contrast = *value;
            }
        }
        PatchOp::BindCapability {
            glyph_id,
            capability_id,
        } => {
            glyph_mut(world, glyph_id)?
                .capability_bindings
                .push(glyphspace_core::CapabilityBinding::new(capability_id));
        }
        PatchOp::UnbindOptionalCapability {
            glyph_id,
            capability_id,
        } => {
            glyph_mut(world, glyph_id)?
                .capability_bindings
                .retain(|binding| !binding.optional || binding.capability_id != *capability_id);
        }
        PatchOp::Group { .. } | PatchOp::Ungroup { .. } => {}
    }
    Ok(())
}

fn glyph_mut<'a>(world: &'a mut GlyphWorld, glyph_id: &str) -> Result<&'a mut Glyph, PatchError> {
    world
        .glyphs
        .get_mut(glyph_id)
        .ok_or_else(|| PatchError::MissingGlyph(glyph_id.to_string()))
}
