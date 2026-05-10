use glyphspace_ai::{AiContext, AiPatchGenerator, RuleBasedPatchGenerator, UserEditRequest};
use glyphspace_core::{Capability, GlyphPatch, GlyphWorld, PolicyContext};
use glyphspace_personalization::apply_patch;
use glyphspace_policy::PolicyEngine;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmGlyphspaceEngine {
    world_json: String,
}

#[wasm_bindgen]
impl WasmGlyphspaceEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();
        Self {
            world_json: String::new(),
        }
    }

    pub fn load_world(&mut self, world_json: &str) -> Result<(), JsValue> {
        let _: GlyphWorld = serde_json::from_str(world_json).map_err(js_err)?;
        self.world_json = world_json.to_string();
        Ok(())
    }

    pub fn propose_patch(&self, request: &str) -> Result<String, JsValue> {
        let world = self.world()?;
        let proposal = RuleBasedPatchGenerator.propose_patch(
            &world,
            &UserEditRequest::new(request),
            &AiContext::demo(),
        );
        serde_json::to_string(&proposal).map_err(js_err)
    }

    pub fn apply_patch(&mut self, patch_json: &str) -> Result<String, JsValue> {
        let world = self.world()?;
        let patch: GlyphPatch = serde_json::from_str(patch_json).map_err(js_err)?;
        let updated = apply_patch(&world, &patch, &PolicyContext::demo_user()).map_err(js_err)?;
        self.world_json = serde_json::to_string(&updated).map_err(js_err)?;
        Ok(self.world_json.clone())
    }

    pub fn validate_patch(&self, patch_json: &str) -> Result<String, JsValue> {
        let world = self.world()?;
        let patch: GlyphPatch = serde_json::from_str(patch_json).map_err(js_err)?;
        let report = PolicyEngine.validate_patch(&world, &patch, &PolicyContext::demo_user());
        serde_json::to_string(&report).map_err(js_err)
    }

    pub fn validate_capability_invocation(
        &self,
        capability_json: &str,
        policy_context_json: &str,
    ) -> Result<String, JsValue> {
        let capability: Capability = serde_json::from_str(capability_json).map_err(js_err)?;
        let context: PolicyContext = serde_json::from_str(policy_context_json).map_err(js_err)?;
        let mut report = glyphspace_core::ValidationReport::allow();
        PolicyEngine.validate_capability_invocation(&capability, &context, &mut report);
        serde_json::to_string(&report).map_err(js_err)
    }
}

impl Default for WasmGlyphspaceEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmGlyphspaceEngine {
    fn world(&self) -> Result<GlyphWorld, JsValue> {
        serde_json::from_str(&self.world_json).map_err(js_err)
    }
}

fn js_err(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}
