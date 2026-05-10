use glyphspace_layout::{LayoutResult, RenderPrimitive};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod render_core {
    use super::*;
    use indexmap::IndexMap;

    #[derive(Clone, Debug, Default)]
    pub struct SceneBatcher;

    impl SceneBatcher {
        pub fn batch(&self, layout: &LayoutResult) -> SceneBatch {
            let mut primitives = IndexMap::new();
            for (index, primitive) in layout.render_primitives.iter().enumerate() {
                primitives.insert(primitive_key(index, primitive), primitive.clone());
            }
            SceneBatch {
                primitive_count: primitives.len(),
                primitives,
            }
        }

        pub fn diff(&self, before: &SceneBatch, after: &SceneBatch) -> SceneDiff {
            let added = after
                .primitives
                .keys()
                .filter(|key| !before.primitives.contains_key(*key))
                .cloned()
                .collect();
            let removed = before
                .primitives
                .keys()
                .filter(|key| !after.primitives.contains_key(*key))
                .cloned()
                .collect();
            let changed = after
                .primitives
                .iter()
                .filter_map(|(key, primitive)| {
                    before
                        .primitives
                        .get(key)
                        .filter(|old| *old != primitive)
                        .map(|_| key.clone())
                })
                .collect();
            SceneDiff {
                added,
                removed,
                changed,
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct SceneBatch {
        pub primitive_count: usize,
        pub primitives: IndexMap<String, RenderPrimitive>,
    }

    #[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    pub struct SceneDiff {
        pub added: Vec<String>,
        pub removed: Vec<String>,
        pub changed: Vec<String>,
    }

    fn primitive_key(index: usize, primitive: &RenderPrimitive) -> String {
        match primitive {
            RenderPrimitive::Dot { glyph_id, .. } => format!("{glyph_id}:dot:{index}"),
            RenderPrimitive::RoundedRect { glyph_id, .. } => format!("{glyph_id}:rect:{index}"),
            RenderPrimitive::TextRun { glyph_id, .. } => format!("{glyph_id}:text:{index}"),
        }
    }
}

pub mod render_canvas {
    use super::*;

    #[derive(Clone, Debug, Default)]
    pub struct CanvasFallbackRenderer;

    impl CanvasFallbackRenderer {
        pub fn render_to_svg_fragment(&self, layout: &LayoutResult) -> String {
            let mut output = String::new();
            output.push_str("<g data-renderer=\"glyphspace-canvas-fallback\">");
            for primitive in &layout.render_primitives {
                match primitive {
                    RenderPrimitive::Dot {
                        glyph_id,
                        x,
                        y,
                        radius,
                        ..
                    } => output.push_str(&format!(
                        "<circle data-glyph-id=\"{glyph_id}\" cx=\"{x}\" cy=\"{y}\" r=\"{radius}\" />"
                    )),
                    RenderPrimitive::RoundedRect { glyph_id, bounds, .. } => output.push_str(
                        &format!(
                            "<rect data-glyph-id=\"{glyph_id}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" />",
                            bounds.x, bounds.y, bounds.width, bounds.height
                        ),
                    ),
                    RenderPrimitive::TextRun {
                        glyph_id,
                        text,
                        x,
                        y,
                    } => output.push_str(&format!(
                        "<text data-glyph-id=\"{glyph_id}\" x=\"{x}\" y=\"{y}\">{}</text>",
                        escape(text)
                    )),
                }
            }
            output.push_str("</g>");
            output
        }
    }

    fn escape(input: &str) -> String {
        input
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }
}

pub mod render_wgpu {
    pub use super::WgpuGlyphRenderer;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RendererConfig {
    pub use_perspective_camera: bool,
    pub device_pixel_ratio: f32,
    pub clear_color: [f64; 4],
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            use_perspective_camera: false,
            device_pixel_ratio: 1.0,
            clear_color: [0.03, 0.035, 0.04, 1.0],
        }
    }
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("renderer has no primitives to draw")]
    EmptyScene,
}

#[derive(Clone, Debug)]
pub struct WgpuGlyphRenderer {
    pub config: RendererConfig,
    pub backend_name: &'static str,
}

impl WgpuGlyphRenderer {
    pub fn headless(config: RendererConfig) -> Self {
        Self {
            config,
            backend_name: "wgpu-headless-reference",
        }
    }

    pub fn prepare_scene(&self, layout: &LayoutResult) -> Result<PreparedScene, RenderError> {
        if layout.render_primitives.is_empty() {
            return Err(RenderError::EmptyScene);
        }
        let color = wgpu::Color {
            r: self.config.clear_color[0],
            g: self.config.clear_color[1],
            b: self.config.clear_color[2],
            a: self.config.clear_color[3],
        };
        Ok(PreparedScene {
            primitive_count: layout.render_primitives.len(),
            dot_count: layout
                .render_primitives
                .iter()
                .filter(|primitive| matches!(primitive, RenderPrimitive::Dot { .. }))
                .count(),
            panel_count: layout
                .render_primitives
                .iter()
                .filter(|primitive| matches!(primitive, RenderPrimitive::RoundedRect { .. }))
                .count(),
            clear_color: [color.r, color.g, color.b, color.a],
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreparedScene {
    pub primitive_count: usize,
    pub dot_count: usize,
    pub panel_count: usize,
    pub clear_color: [f64; 4],
}
