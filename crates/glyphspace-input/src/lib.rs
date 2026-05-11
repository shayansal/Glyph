use glyphspace_core::{
    Capability, GlyphId, GlyphPatch, GlyphWorld, PolicyContext, ValidationReport,
};
use glyphspace_personalization::{PatchError, apply_patch};
use glyphspace_policy::PolicyEngine;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputEvent {
    PointerMove {
        x: f32,
        y: f32,
    },
    GlyphClick {
        glyph_id: GlyphId,
        #[serde(default)]
        input: serde_json::Value,
    },
    KeyboardActivate {
        glyph_id: GlyphId,
    },
    NaturalLanguageEdit {
        text: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CapabilityResult {
    pub output: serde_json::Value,
    pub patch: Option<GlyphPatch>,
}

pub type CapabilityHandler<State> = Box<
    dyn FnMut(&mut State, serde_json::Value, &GlyphWorld) -> Result<CapabilityResult, RuntimeError>,
>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeAuditEvent {
    pub action: String,
    pub subject: String,
    pub detail: String,
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("missing glyph: {0}")]
    MissingGlyph(String),
    #[error("missing capability: {0}")]
    MissingCapability(String),
    #[error("missing capability handler: {0}")]
    MissingHandler(String),
    #[error("policy rejected capability: {0}")]
    PolicyRejected(String),
    #[error("patch failed: {0}")]
    Patch(#[from] PatchError),
}

pub struct GlyphspaceRuntime<State> {
    world: GlyphWorld,
    state: State,
    policy_context: PolicyContext,
    handlers: BTreeMap<String, CapabilityHandler<State>>,
    patch_store: Vec<GlyphPatch>,
    audit_log: Vec<RuntimeAuditEvent>,
}

impl<State> GlyphspaceRuntime<State> {
    pub fn new(world: GlyphWorld, state: State, policy_context: PolicyContext) -> Self {
        Self {
            world,
            state,
            policy_context,
            handlers: BTreeMap::new(),
            patch_store: Vec::new(),
            audit_log: Vec::new(),
        }
    }

    pub fn register(
        &mut self,
        capability_id: impl Into<String>,
        handler: impl FnMut(
            &mut State,
            serde_json::Value,
            &GlyphWorld,
        ) -> Result<CapabilityResult, RuntimeError>
        + 'static,
    ) {
        self.handlers
            .insert(capability_id.into(), Box::new(handler));
    }

    pub fn world(&self) -> &GlyphWorld {
        &self.world
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn patch_store(&self) -> &[GlyphPatch] {
        &self.patch_store
    }

    pub fn audit_log(&self) -> &[RuntimeAuditEvent] {
        &self.audit_log
    }

    pub fn handle_input(
        &mut self,
        event: InputEvent,
    ) -> Result<Option<CapabilityResult>, RuntimeError> {
        match event {
            InputEvent::GlyphClick { glyph_id, input } => self
                .invoke_first_glyph_capability(&glyph_id, input)
                .map(Some),
            InputEvent::KeyboardActivate { glyph_id } => self
                .invoke_first_glyph_capability(&glyph_id, serde_json::Value::Null)
                .map(Some),
            InputEvent::PointerMove { .. } | InputEvent::NaturalLanguageEdit { .. } => Ok(None),
        }
    }

    fn invoke_first_glyph_capability(
        &mut self,
        glyph_id: &str,
        input: serde_json::Value,
    ) -> Result<CapabilityResult, RuntimeError> {
        let glyph = self
            .world
            .glyphs
            .get(glyph_id)
            .ok_or_else(|| RuntimeError::MissingGlyph(glyph_id.to_string()))?;
        let capability_id = glyph
            .capability_bindings
            .first()
            .map(|binding| binding.capability_id.clone())
            .ok_or_else(|| RuntimeError::MissingCapability(glyph_id.to_string()))?;
        self.invoke_capability(&capability_id, input)
    }

    pub fn invoke_capability(
        &mut self,
        capability_id: &str,
        input: serde_json::Value,
    ) -> Result<CapabilityResult, RuntimeError> {
        let capability = self
            .world
            .capabilities
            .get(capability_id)
            .cloned()
            .ok_or_else(|| RuntimeError::MissingCapability(capability_id.to_string()))?;

        self.validate_capability(&capability)?;

        let handler = self
            .handlers
            .get_mut(capability_id)
            .ok_or_else(|| RuntimeError::MissingHandler(capability_id.to_string()))?;
        let result = handler(&mut self.state, input, &self.world)?;
        self.audit(
            "capability.invoked",
            capability_id,
            result.output.to_string(),
        );
        if let Some(patch) = &result.patch {
            self.world = apply_patch(&self.world, patch, &self.policy_context)?;
            self.patch_store.push(patch.clone());
            self.audit("patch.applied", &patch.id, patch.description.clone());
        }
        Ok(result)
    }

    fn validate_capability(&mut self, capability: &Capability) -> Result<(), RuntimeError> {
        let mut report = ValidationReport::allow();
        PolicyEngine.validate_capability_invocation(capability, &self.policy_context, &mut report);
        if report.allowed {
            return Ok(());
        }
        let detail = report
            .violations
            .iter()
            .map(|violation| violation.message.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        self.audit("capability.rejected", &capability.id, detail.clone());
        Err(RuntimeError::PolicyRejected(detail))
    }

    fn audit(
        &mut self,
        action: impl Into<String>,
        subject: impl Into<String>,
        detail: impl Into<String>,
    ) {
        self.audit_log.push(RuntimeAuditEvent {
            action: action.into(),
            subject: subject.into(),
            detail: detail.into(),
        });
    }
}
