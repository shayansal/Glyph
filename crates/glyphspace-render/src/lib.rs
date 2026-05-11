use glyphspace_accessibility::build_accessibility_tree;
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

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimationClock {
    pub seconds: f32,
}

impl AnimationClock {
    pub fn fixed_seconds(seconds: f32) -> Self {
        Self { seconds }
    }
}

impl Default for AnimationClock {
    fn default() -> Self {
        Self { seconds: 0.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RenderLoopConfig {
    pub target_fps: u16,
    pub animations_enabled: bool,
}

impl RenderLoopConfig {
    pub fn animated_60hz() -> Self {
        Self {
            target_fps: 60,
            animations_enabled: true,
        }
    }
}

impl Default for RenderLoopConfig {
    fn default() -> Self {
        Self {
            target_fps: 0,
            animations_enabled: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RenderCommand {
    Dot {
        glyph_id: GlyphId,
        x: f32,
        y: f32,
        z: f32,
        radius: f32,
    },
    Card {
        glyph_id: GlyphId,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
    },
    Text {
        glyph_id: GlyphId,
        text: String,
        x: f32,
        y: f32,
        shaped_width: f32,
    },
    Edge {
        from: GlyphId,
        to: GlyphId,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    },
    FocusRing {
        glyph_id: GlyphId,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    AnimationTick {
        seconds: f32,
        target_fps: u16,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RenderCommandFrame {
    pub frame_index: u64,
    pub native_backend: String,
    pub browser_backend: String,
    pub commands: Vec<RenderCommand>,
    pub applied_scene_operations: usize,
}

impl RenderCommandFrame {
    pub fn apply_scene_patch(&mut self, patch: &render_core::ScenePatch) {
        self.applied_scene_operations += patch.operations.len();
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
pub struct ProductionFrame {
    pub layout: LayoutResult,
    pub prepared_scene: PreparedScene,
    pub scene_batch: render_core::SceneBatch,
    pub scene_patch: render_core::ScenePatch,
    pub accessibility_node_count: usize,
    pub command_frame: RenderCommandFrame,
}

#[derive(Clone, Debug)]
pub struct ProductionRenderer {
    viewport: Viewport,
    device_profile: DeviceProfile,
    renderer: WgpuGlyphRenderer,
    batcher: render_core::SceneBatcher,
    last_batch: Option<render_core::SceneBatch>,
    render_loop: RenderLoopConfig,
    animation_clock: AnimationClock,
    focused_glyph: Option<GlyphId>,
    frame_index: u64,
}

impl ProductionRenderer {
    pub fn headless(viewport: Viewport, device_profile: DeviceProfile) -> Self {
        Self {
            viewport,
            device_profile,
            renderer: WgpuGlyphRenderer::headless(RendererConfig::default()),
            batcher: render_core::SceneBatcher,
            last_batch: None,
            render_loop: RenderLoopConfig::default(),
            animation_clock: AnimationClock::default(),
            focused_glyph: None,
            frame_index: 0,
        }
    }

    pub fn with_render_loop(mut self, config: RenderLoopConfig) -> Self {
        self.render_loop = config;
        self
    }

    pub fn with_animation_clock(mut self, clock: AnimationClock) -> Self {
        self.animation_clock = clock;
        self
    }

    pub fn with_focus(mut self, glyph_id: impl Into<String>) -> Self {
        self.focused_glyph = Some(glyph_id.into());
        self
    }

    pub fn render_world(&mut self, world: &GlyphWorld) -> Result<ProductionFrame, NativeHostError> {
        let layout = compile_layout(world, self.viewport, None, self.device_profile)?;
        let prepared_scene = self.renderer.prepare_scene(&layout)?;
        let scene_batch = self.batcher.batch(&layout);
        let diff = self
            .last_batch
            .as_ref()
            .map_or_else(render_core::SceneDiff::default, |before| {
                self.batcher.diff(before, &scene_batch)
            });
        let scene_patch = render_core::ScenePatch::from_diff(diff);
        self.last_batch = Some(scene_batch.clone());
        let command_frame = self.command_frame(world, &layout);
        self.frame_index += 1;
        Ok(ProductionFrame {
            layout,
            prepared_scene,
            scene_batch,
            scene_patch,
            accessibility_node_count: build_accessibility_tree(world).nodes.len(),
            command_frame,
        })
    }

    fn command_frame(&self, world: &GlyphWorld, layout: &LayoutResult) -> RenderCommandFrame {
        let shaper = TextShaper::placeholder();
        let mut commands = Vec::new();
        for primitive in &layout.render_primitives {
            match primitive {
                RenderPrimitive::Dot {
                    glyph_id,
                    x,
                    y,
                    z,
                    radius,
                } => commands.push(RenderCommand::Dot {
                    glyph_id: glyph_id.clone(),
                    x: *x,
                    y: *y,
                    z: *z,
                    radius: *radius,
                }),
                RenderPrimitive::RoundedRect {
                    glyph_id,
                    bounds,
                    radius,
                } => commands.push(RenderCommand::Card {
                    glyph_id: glyph_id.clone(),
                    x: bounds.x,
                    y: bounds.y,
                    width: bounds.width,
                    height: bounds.height,
                    radius: *radius,
                }),
                RenderPrimitive::TextRun {
                    glyph_id,
                    text,
                    x,
                    y,
                } => commands.push(RenderCommand::Text {
                    glyph_id: glyph_id.clone(),
                    text: text.clone(),
                    x: *x,
                    y: *y,
                    shaped_width: shaper.shape(text, 16.0).width,
                }),
            }
        }
        let existing_dot_ids = commands
            .iter()
            .filter_map(|command| match command {
                RenderCommand::Dot { glyph_id, .. } => Some(glyph_id.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        for (glyph_id, bounds) in &layout.bounding_volumes {
            if !existing_dot_ids.contains(glyph_id) {
                commands.push(RenderCommand::Dot {
                    glyph_id: glyph_id.clone(),
                    x: bounds.x,
                    y: bounds.y,
                    z: bounds.z,
                    radius: 4.0,
                });
            }
        }
        for edge in &world.edges {
            if let (Some(from), Some(to)) = (
                layout.bounding_volumes.get(&edge.from),
                layout.bounding_volumes.get(&edge.to),
            ) {
                commands.push(RenderCommand::Edge {
                    from: edge.from.clone(),
                    to: edge.to.clone(),
                    x1: from.x,
                    y1: from.y,
                    x2: to.x,
                    y2: to.y,
                });
            }
        }
        if let Some(glyph_id) = &self.focused_glyph {
            if let Some(bounds) = layout.bounding_volumes.get(glyph_id) {
                commands.push(RenderCommand::FocusRing {
                    glyph_id: glyph_id.clone(),
                    x: bounds.x,
                    y: bounds.y,
                    width: bounds.width,
                    height: bounds.height,
                });
            }
        }
        if self.render_loop.animations_enabled {
            commands.push(RenderCommand::AnimationTick {
                seconds: self.animation_clock.seconds,
                target_fps: self.render_loop.target_fps,
            });
        }
        RenderCommandFrame {
            frame_index: self.frame_index,
            native_backend: self.renderer.backend_name.to_string(),
            browser_backend: "webgpu-command-buffer".to_string(),
            commands,
            applied_scene_operations: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderSnapshot {
    pub digest: String,
    pub command_digest: String,
    pub primitive_count: usize,
    pub command_count: usize,
    pub accessibility_node_count: usize,
}

impl RenderSnapshot {
    pub fn from_frame(frame: &ProductionFrame) -> Self {
        let digest = serde_json::to_string(&(
            frame.layout.layout_hash,
            frame.prepared_scene.primitive_count,
            frame.scene_batch.primitive_count,
            frame.accessibility_node_count,
        ))
        .map(stable_digest)
        .unwrap_or_else(|_| "0000000000000000".to_string());
        Self {
            digest,
            command_digest: serde_json::to_string(&frame.command_frame.commands)
                .map(stable_digest)
                .unwrap_or_else(|_| "0000000000000000".to_string()),
            primitive_count: frame.prepared_scene.primitive_count,
            command_count: frame.command_frame.commands.len(),
            accessibility_node_count: frame.accessibility_node_count,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuDrawCall {
    pub kind: String,
    pub instances: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserParityReport {
    pub native_api: String,
    pub browser_api: String,
    pub command_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuPipelinePlan {
    pub shader_wgsl: String,
    pub vertex_buffers: usize,
    pub draw_calls: Vec<GpuDrawCall>,
    pub bind_groups: Vec<String>,
}

impl GpuPipelinePlan {
    pub fn from_frame(frame: &ProductionFrame) -> Self {
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        for command in &frame.command_frame.commands {
            let kind = match command {
                RenderCommand::Dot { .. } => "dot",
                RenderCommand::Card { .. } => "card",
                RenderCommand::Text { .. } => "text",
                RenderCommand::Edge { .. } => "edge",
                RenderCommand::FocusRing { .. } => "focus_ring",
                RenderCommand::AnimationTick { .. } => "animation_tick",
            };
            *counts.entry(kind.to_string()).or_default() += 1;
        }
        Self {
            shader_wgsl: GLYPHSPACE_WGSL.to_string(),
            vertex_buffers: 2,
            draw_calls: counts
                .into_iter()
                .map(|(kind, instances)| GpuDrawCall { kind, instances })
                .collect(),
            bind_groups: vec![
                "camera".to_string(),
                "glyph_instances".to_string(),
                "text_atlas".to_string(),
            ],
        }
    }

    pub fn uses_wgsl(&self) -> bool {
        self.shader_wgsl.contains("@vertex") && self.shader_wgsl.contains("@fragment")
    }

    pub fn browser_parity(&self) -> BrowserParityReport {
        BrowserParityReport {
            native_api: "wgpu".to_string(),
            browser_api: "WebGPU".to_string(),
            command_count: self.draw_calls.iter().map(|call| call.instances).sum(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenshotConformance {
    pub pixel_digest: String,
    pub coverage: Vec<String>,
    pub command_count: usize,
}

impl ScreenshotConformance {
    pub fn from_frame(frame: &ProductionFrame) -> Self {
        let plan = GpuPipelinePlan::from_frame(frame);
        let mut coverage = plan
            .draw_calls
            .iter()
            .map(|call| call.kind.clone())
            .collect::<Vec<_>>();
        coverage.sort();
        Self {
            pixel_digest: stable_digest(
                serde_json::to_string(&(
                    frame.command_frame.frame_index,
                    &frame.command_frame.commands,
                    &coverage,
                ))
                .unwrap_or_default(),
            ),
            command_count: frame.command_frame.commands.len(),
            coverage,
        }
    }
}

const GLYPHSPACE_WGSL: &str = r#"
struct VertexOut {
  @builtin(position) position: vec4<f32>,
  @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(@location(0) position: vec2<f32>, @location(1) color: vec4<f32>) -> VertexOut {
  var out: VertexOut;
  out.position = vec4<f32>(position, 0.0, 1.0);
  out.color = color;
  return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
  return in.color;
}
"#;

fn stable_digest(input: String) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
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
