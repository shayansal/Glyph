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
            for primitive in &layout.render_primitives {
                primitives.insert(primitive_key(primitive), primitive.clone());
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

    #[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ScenePatch {
        pub operations: Vec<ScenePatchOp>,
    }

    impl ScenePatch {
        pub fn from_diff(diff: SceneDiff) -> Self {
            let mut operations = Vec::new();
            operations.extend(diff.added.into_iter().map(ScenePatchOp::Add));
            operations.extend(diff.removed.into_iter().map(ScenePatchOp::Remove));
            operations.extend(diff.changed.into_iter().map(ScenePatchOp::Update));
            Self { operations }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub enum ScenePatchOp {
        Add(String),
        Remove(String),
        Update(String),
    }

    fn primitive_key(primitive: &RenderPrimitive) -> String {
        match primitive {
            RenderPrimitive::Dot { glyph_id, .. } => format!("{glyph_id}:dot"),
            RenderPrimitive::RoundedRect { glyph_id, .. } => format!("{glyph_id}:rect"),
            RenderPrimitive::TextRun { glyph_id, .. } => format!("{glyph_id}:text"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GlyphTextRun {
    pub glyph_id: GlyphId,
    pub text: String,
    pub font_size: f32,
}

impl GlyphTextRun {
    pub fn new(glyph_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            glyph_id: glyph_id.into(),
            text: text.into(),
            font_size: 16.0,
        }
    }

    pub fn with_font_size(mut self, font_size: f32) -> Self {
        self.font_size = font_size;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShapedText {
    pub glyphs: Vec<ShapedGlyph>,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShapedGlyph {
    pub cluster: usize,
    pub advance: f32,
}

#[derive(Clone, Debug, Default)]
pub struct TextShaper;

impl TextShaper {
    pub fn placeholder() -> Self {
        Self
    }

    pub fn shape(&self, text: &str, font_size: f32) -> ShapedText {
        let advance = font_size * 0.58;
        let glyphs = text
            .chars()
            .enumerate()
            .map(|(cluster, _)| ShapedGlyph { cluster, advance })
            .collect::<Vec<_>>();
        ShapedText {
            width: advance * glyphs.len() as f32,
            height: font_size * 1.25,
            glyphs,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SelectionStyle {
    pub glyph_id: GlyphId,
    pub outline_width: f32,
    pub color_token: String,
}

impl SelectionStyle {
    pub fn focused(glyph_id: impl Into<String>) -> Self {
        Self {
            glyph_id: glyph_id.into(),
            outline_width: 2.0,
            color_token: "focus-ring".to_string(),
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

    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
    }

    pub fn run_winit_window(
        world: GlyphWorld,
        config: NativeWindowConfig,
    ) -> Result<(), NativeHostError> {
        let event_loop = winit::event_loop::EventLoop::new()
            .map_err(|error| NativeHostError::Winit(error.to_string()))?;
        let mut app = GlyphspaceWinitApp {
            title: config.title,
            world,
            host: Self::winit_wgpu(config.viewport, config.device_profile),
            window: None,
            last_frame: None,
        };
        event_loop
            .run_app(&mut app)
            .map_err(|error| NativeHostError::Winit(error.to_string()))
    }
}

pub struct WinitWgpuHost {
    pub host: NativeRendererHost,
    pub event_loop: winit::event_loop::EventLoop<()>,
    pub instance: wgpu::Instance,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeWindowConfig {
    pub title: String,
    pub viewport: Viewport,
    pub device_profile: DeviceProfile,
}

impl NativeWindowConfig {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            viewport: Viewport::desktop(),
            device_profile: DeviceProfile::desktop(),
        }
    }

    pub fn with_viewport(mut self, viewport: Viewport) -> Self {
        self.viewport = viewport;
        self
    }

    pub fn with_device_profile(mut self, device_profile: DeviceProfile) -> Self {
        self.device_profile = device_profile;
        self
    }
}

struct GlyphspaceWinitApp {
    title: String,
    world: GlyphWorld,
    host: NativeRendererHost,
    window: Option<winit::window::Window>,
    last_frame: Option<NativeFrame>,
}

impl winit::application::ApplicationHandler for GlyphspaceWinitApp {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = winit::window::Window::default_attributes().with_title(self.title.clone());
        match event_loop.create_window(attrs) {
            Ok(window) => {
                self.last_frame = self.host.render_world(&self.world).ok();
                window.request_redraw();
                self.window = Some(window);
            }
            Err(error) => {
                eprintln!("failed to create glyphspace window: {error}");
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            winit::event::WindowEvent::CloseRequested => event_loop.exit(),
            winit::event::WindowEvent::Resized(size) => {
                self.host.set_viewport(Viewport {
                    width: size.width as f32,
                    height: size.height as f32,
                    device_pixel_ratio: self
                        .window
                        .as_ref()
                        .map_or(1.0, |window| window.scale_factor() as f32),
                });
                self.last_frame = self.host.render_world(&self.world).ok();
            }
            winit::event::WindowEvent::RedrawRequested => {
                self.last_frame = self.host.render_world(&self.world).ok();
            }
            _ => {}
        }
    }
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
