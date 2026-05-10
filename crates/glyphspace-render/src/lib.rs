use glyphspace_layout::{LayoutResult, RenderPrimitive};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
