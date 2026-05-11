use glyphspace_core::{Capability, Glyph, GlyphPatch, GlyphWorld, PatchOp};
use indexmap::IndexMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DslError {
    #[error("json parse failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("duplicate glyph id: {0}")]
    DuplicateGlyph(String),
    #[error("duplicate capability id: {0}")]
    DuplicateCapability(String),
}

pub fn parse_world_json(input: &str) -> Result<GlyphWorld, DslError> {
    Ok(serde_json::from_str(input)?)
}

#[derive(Clone, Debug)]
pub struct GlyphApp {
    id: String,
    name: String,
    spec_version: String,
    glyphs: Vec<Glyph>,
    capabilities: Vec<Capability>,
    lenses: Vec<Lens>,
    metadata: glyphspace_core::Metadata,
}

impl GlyphApp {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            spec_version: glyphspace_core::SPEC_VERSION.to_string(),
            glyphs: Vec::new(),
            capabilities: Vec::new(),
            lenses: Vec::new(),
            metadata: glyphspace_core::Metadata::new(),
        }
    }

    pub fn spec_version(mut self, spec_version: impl Into<String>) -> Self {
        self.spec_version = spec_version.into();
        self
    }

    pub fn capability(mut self, capability: Capability) -> Self {
        self.capabilities.push(capability);
        self
    }

    pub fn glyph(mut self, glyph: Glyph) -> Self {
        self.glyphs.push(glyph);
        self
    }

    pub fn lens(mut self, lens: Lens) -> Self {
        self.lenses.push(lens);
        self
    }

    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    pub fn compile(&self) -> Result<GlyphWorld, DslError> {
        let mut world = GlyphWorld::new(self.id.clone(), self.name.clone());
        world.spec_version = self.spec_version.clone();
        world.metadata = self.metadata.clone();

        let mut capabilities = IndexMap::new();
        for capability in &self.capabilities {
            if capabilities.contains_key(&capability.id) {
                return Err(DslError::DuplicateCapability(capability.id.clone()));
            }
            capabilities.insert(capability.id.clone(), capability.clone());
        }
        world.capabilities = capabilities;

        for glyph in &self.glyphs {
            world
                .add_glyph(glyph.clone())
                .map_err(|_| DslError::DuplicateGlyph(glyph.id.clone()))?;
        }

        if !self.lenses.is_empty() {
            world.metadata.insert(
                "lenses".to_string(),
                serde_json::to_value(self.lenses.iter().map(GlyphPatch::from).collect::<Vec<_>>())?,
            );
        }

        Ok(world)
    }

    pub fn to_glyph_json(&self) -> Result<String, DslError> {
        Ok(self.compile()?.to_canonical_json()?)
    }
}

impl From<glyphspace_core::CanonicalError> for DslError {
    fn from(error: glyphspace_core::CanonicalError) -> Self {
        match error {
            glyphspace_core::CanonicalError::Serde(error) => DslError::Json(error),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Lens {
    patch: GlyphPatch,
}

impl Lens {
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            patch: GlyphPatch::new(id, description, Vec::new()),
        }
    }

    pub fn op(mut self, op: PatchOp) -> Self {
        self.patch.ops.push(op);
        self
    }

    pub fn patch(&self) -> &GlyphPatch {
        &self.patch
    }

    pub fn to_json(&self) -> Result<String, DslError> {
        Ok(serde_json::to_string(&self.patch)?)
    }
}

impl From<Lens> for GlyphPatch {
    fn from(lens: Lens) -> Self {
        lens.patch
    }
}

impl From<&Lens> for GlyphPatch {
    fn from(lens: &Lens) -> Self {
        lens.patch.clone()
    }
}

#[macro_export]
macro_rules! glyph_app {
    ($id:expr, $name:expr $(,)?) => {
        $crate::GlyphApp::new($id, $name)
    };
}
