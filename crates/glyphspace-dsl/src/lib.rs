use glyphspace_core::GlyphWorld;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DslError {
    #[error("json parse failed: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn parse_world_json(input: &str) -> Result<GlyphWorld, DslError> {
    Ok(serde_json::from_str(input)?)
}
