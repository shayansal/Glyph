use anyhow::{Result, anyhow};
use glyphspace_core::{Capability, GlyphPatch, GlyphWorld};
use schemars::{JsonSchema, schema_for};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaValidation {
    pub valid: bool,
    pub warnings: Vec<String>,
}

pub fn validate_world_json(value: &Value) -> Result<SchemaValidation> {
    let world: GlyphWorld = serde_json::from_value(value.clone())?;
    if world.id.trim().is_empty() {
        return Err(anyhow!("manifest id is required"));
    }
    if world.spec_version.trim().is_empty() {
        return Err(anyhow!("spec_version is required"));
    }
    Ok(SchemaValidation {
        valid: true,
        warnings: extension_warnings(value),
    })
}

pub fn validate_patch_json(value: &Value) -> Result<SchemaValidation> {
    let patch: GlyphPatch = serde_json::from_value(value.clone())?;
    if patch.id.trim().is_empty() {
        return Err(anyhow!("patch id is required"));
    }
    Ok(SchemaValidation {
        valid: true,
        warnings: extension_warnings(value),
    })
}

pub fn export_schema<T: JsonSchema>() -> Result<Value> {
    Ok(serde_json::to_value(schema_for!(T))?)
}

pub fn export_named_schema(name: &str) -> Result<Value> {
    match name {
        "glyphspace" | "world" => export_schema::<GlyphWorld>(),
        "capability" => export_schema::<Capability>(),
        "patch" | "lens" => export_schema::<GlyphPatch>(),
        "policy" => Ok(serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "Glyphspace Policy",
            "type": "object",
            "required": ["spec_version", "rules"],
            "properties": {
                "spec_version": { "type": "string" },
                "rules": { "type": "array" }
            }
        })),
        other => Err(anyhow!("unknown schema {other}")),
    }
}

fn extension_warnings(value: &Value) -> Vec<String> {
    let Some(object) = value.as_object() else {
        return Vec::new();
    };
    object
        .keys()
        .filter(|key| key.starts_with("x_") || key.contains(':'))
        .map(|key| format!("preserved unknown extension field {key}"))
        .collect()
}
