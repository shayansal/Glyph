use glyphspace_core::{GlyphId, GlyphWorld};
use glyphspace_layout::{
    DeviceProfile, HitTestEntry, LayoutError, LayoutResult, RenderPrimitive, Viewport,
    compile_layout,
};
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NativeFrame {
    pub layout: LayoutResult,
    pub prepared_scene: PreparedScene,
    pub hit_regions: Vec<NativeHitRegion>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NativeHitRegion {
    pub glyph_id: GlyphId,
    pub center_x: f32,
    pub center_y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Error)]
pub enum NativeHostError {
    #[error(transparent)]
    Layout(#[from] LayoutError),
    #[error(transparent)]
    Render(#[from] RenderError),
    #[error("winit host failed: {0}")]
    Winit(String),
}

#[derive(Clone, Debug)]
pub struct NativeRendererHost {
    viewport: Viewport,
    device_profile: DeviceProfile,
    renderer: WgpuGlyphRenderer,
    last_hit_regions: Vec<NativeHitRegion>,
}

impl NativeRendererHost {
    pub fn headless(viewport: Viewport, device_profile: DeviceProfile) -> Self {
        Self {
            viewport,
            device_profile,
            renderer: WgpuGlyphRenderer::headless(RendererConfig::default()),
            last_hit_regions: Vec::new(),
        }
    }

    pub fn winit_wgpu(viewport: Viewport, device_profile: DeviceProfile) -> Self {
        // The prototype exposes the native host contract while tests use the same
        // headless path to avoid requiring a GPU surface in CI.
        Self::headless(viewport, device_profile)
    }

    pub fn create_winit_wgpu(
        viewport: Viewport,
        device_profile: DeviceProfile,
    ) -> Result<WinitWgpuHost, NativeHostError> {
        let event_loop = winit::event_loop::EventLoop::new()
            .map_err(|error| NativeHostError::Winit(error.to_string()))?;
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        Ok(WinitWgpuHost {
            host: Self::headless(viewport, device_profile),
            event_loop,
            instance,
        })
    }

    pub fn render_world(&mut self, world: &GlyphWorld) -> Result<NativeFrame, NativeHostError> {
        let layout = compile_layout(world, self.viewport, None, self.device_profile)?;
        let prepared_scene = self.renderer.prepare_scene(&layout)?;
        let hit_regions = layout
            .hit_test_map
            .iter()
            .map(hit_region_from_entry)
            .collect::<Vec<_>>();
        self.last_hit_regions = hit_regions.clone();
        Ok(NativeFrame {
            layout,
            prepared_scene,
            hit_regions,
        })
    }

    pub fn hit_test(&self, x: f32, y: f32) -> Option<GlyphId> {
        self.last_hit_regions
            .iter()
            .find(|region| {
                let left = region.center_x - region.width / 2.0;
                let right = region.center_x + region.width / 2.0;
                let top = region.center_y - region.height / 2.0;
                let bottom = region.center_y + region.height / 2.0;
                x >= left && x <= right && y >= top && y <= bottom
            })
            .map(|region| region.glyph_id.clone())
    }

    pub fn backend_name(&self) -> &'static str {
        self.renderer.backend_name
    }
}

pub struct WinitWgpuHost {
    pub host: NativeRendererHost,
    pub event_loop: winit::event_loop::EventLoop<()>,
    pub instance: wgpu::Instance,
}

fn hit_region_from_entry(entry: &HitTestEntry) -> NativeHitRegion {
    NativeHitRegion {
        glyph_id: entry.glyph_id.clone(),
        center_x: entry.bounds.x,
        center_y: entry.bounds.y,
        width: entry.bounds.width,
        height: entry.bounds.height,
    }
}
