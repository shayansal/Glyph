use glyphspace_render::{GpuDrawCall, RenderCommand, RenderCommandFrame};
use glyphspace_text::{TextEngine, TextError, TextRun};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use thiserror::Error;

const SWAPCHAIN_WGSL: &str = r#"
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((vertex_index << 1u) & 2u);
    let y = f32(vertex_index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(0.08, 0.12, 0.16, 1.0);
}
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceSize {
    pub width: u32,
    pub height: u32,
}

impl SurfaceSize {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NativeSwapchainConfig {
    pub size: SurfaceSize,
    pub sample_count: u32,
    pub vsync: bool,
    pub texture_format: String,
    pub present_mode: String,
    pub device_pixel_ratio: f32,
}

impl NativeSwapchainConfig {
    pub fn new(size: SurfaceSize) -> Self {
        Self {
            size,
            sample_count: 1,
            vsync: true,
            texture_format: format!("{:?}", wgpu::TextureFormat::Bgra8UnormSrgb),
            present_mode: format!("{:?}", wgpu::PresentMode::Fifo),
            device_pixel_ratio: 1.0,
        }
    }

    pub fn with_vsync(mut self, vsync: bool) -> Self {
        self.vsync = vsync;
        self.present_mode = if vsync {
            format!("{:?}", wgpu::PresentMode::Fifo)
        } else {
            format!("{:?}", wgpu::PresentMode::Immediate)
        };
        self
    }

    pub fn with_msaa(mut self, sample_count: u32) -> Self {
        self.sample_count = sample_count.max(1);
        self
    }

    pub fn with_device_pixel_ratio(mut self, device_pixel_ratio: f32) -> Self {
        self.device_pixel_ratio = device_pixel_ratio.max(0.1);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WgpuSurfacePipeline {
    pub shader_wgsl: String,
    pub vertex_buffers: usize,
    pub bind_groups: Vec<String>,
    pub draw_calls: Vec<GpuDrawCall>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WgpuResourceSet {
    pub vertex_buffer_bytes: usize,
    pub index_buffer_bytes: usize,
    pub instance_buffer_bytes: usize,
    pub uniform_buffer_bytes: usize,
    pub text_atlas_bytes: usize,
    pub texture_uploads: Vec<TextureUpload>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextureUpload {
    pub label: String,
    pub width: u32,
    pub height: u32,
    pub bytes: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderPassPlan {
    pub color_attachment_format: String,
    pub sample_count: u32,
    pub bind_groups: Vec<String>,
    pub draws: Vec<GpuDrawCall>,
    pub uses_depth: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WgpuFrameStats {
    pub mode: String,
    pub presented_frames: u64,
    pub draw_calls: usize,
    pub surface_size: SurfaceSize,
    pub sample_count: u32,
}

#[derive(Debug, Error)]
pub enum WgpuRenderError {
    #[error("swapchain surface has zero size")]
    ZeroSizedSurface,
    #[error("text rendering failed: {0}")]
    Text(#[from] TextError),
}

#[derive(Clone, Debug)]
pub struct NativeSwapchainPresenter {
    config: NativeSwapchainConfig,
    pipeline: WgpuSurfacePipeline,
    resources: WgpuResourceSet,
    render_pass: RenderPassPlan,
    last_frame: Option<RenderCommandFrame>,
    presented_frames: u64,
    mode: String,
}

impl NativeSwapchainPresenter {
    pub fn headless_contract(config: NativeSwapchainConfig) -> Self {
        Self {
            pipeline: WgpuSurfacePipeline {
                shader_wgsl: SWAPCHAIN_WGSL.to_string(),
                vertex_buffers: 2,
                bind_groups: vec![
                    "camera".to_string(),
                    "glyph_instances".to_string(),
                    "text_atlas".to_string(),
                    "surface_frame".to_string(),
                ],
                draw_calls: Vec::new(),
            },
            resources: WgpuResourceSet::default(),
            render_pass: RenderPassPlan::default(),
            last_frame: None,
            config,
            presented_frames: 0,
            mode: "headless-contract".to_string(),
        }
    }

    pub fn present_commands(
        &mut self,
        frame: &RenderCommandFrame,
    ) -> Result<WgpuFrameStats, WgpuRenderError> {
        if self.config.size.width == 0 || self.config.size.height == 0 {
            return Err(WgpuRenderError::ZeroSizedSurface);
        }
        self.pipeline.draw_calls = summarize_draw_calls(&frame.commands);
        self.resources = allocate_resources(frame, Vec::new());
        self.render_pass = build_render_pass(&self.config, &self.pipeline);
        self.last_frame = Some(frame.clone());
        self.presented_frames += 1;
        Ok(WgpuFrameStats {
            mode: self.mode.clone(),
            presented_frames: self.presented_frames,
            draw_calls: self
                .pipeline
                .draw_calls
                .iter()
                .map(|call| call.instances)
                .sum(),
            surface_size: self.config.size,
            sample_count: self.config.sample_count,
        })
    }

    pub fn present_commands_with_text(
        &mut self,
        frame: &RenderCommandFrame,
        text: &mut TextEngine,
    ) -> Result<WgpuFrameStats, WgpuRenderError> {
        if self.config.size.width == 0 || self.config.size.height == 0 {
            return Err(WgpuRenderError::ZeroSizedSurface);
        }
        let mut uploader = TextAtlasUploader::new(2048, 2048);
        for command in &frame.commands {
            if let RenderCommand::Text { text: label, .. } = command {
                uploader.upload_run(
                    text,
                    TextRun::new(label.clone(), 16.0)
                        .with_dpi_scale(self.config.device_pixel_ratio),
                )?;
            }
        }
        self.pipeline.draw_calls = summarize_draw_calls(&frame.commands);
        self.resources = allocate_resources(frame, uploader.uploads().to_vec());
        self.render_pass = build_render_pass(&self.config, &self.pipeline);
        self.last_frame = Some(frame.clone());
        self.presented_frames += 1;
        Ok(WgpuFrameStats {
            mode: self.mode.clone(),
            presented_frames: self.presented_frames,
            draw_calls: self
                .pipeline
                .draw_calls
                .iter()
                .map(|call| call.instances)
                .sum(),
            surface_size: self.config.size,
            sample_count: self.config.sample_count,
        })
    }

    pub fn resize(&mut self, size: SurfaceSize) {
        self.config.size = size;
    }

    pub fn config(&self) -> &NativeSwapchainConfig {
        &self.config
    }

    pub fn pipeline_plan(&self) -> &WgpuSurfacePipeline {
        &self.pipeline
    }

    pub fn resources(&self) -> &WgpuResourceSet {
        &self.resources
    }

    pub fn render_pass_plan(&self) -> &RenderPassPlan {
        &self.render_pass
    }

    pub fn last_frame(&self) -> Option<&RenderCommandFrame> {
        self.last_frame.as_ref()
    }

    pub fn hit_test(&self, x: f32, y: f32) -> Option<HitTestResult> {
        let frame = self.last_frame.as_ref()?;
        frame
            .commands
            .iter()
            .rev()
            .find_map(|command| hit_test_command(command, x, y))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WgpuRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WgpuDrawState {
    pub clip: Option<WgpuRect>,
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub z_order: i32,
    pub transform: [f32; 6],
    pub opacity: f32,
    pub masks: Vec<String>,
}

impl Default for WgpuDrawState {
    fn default() -> Self {
        Self {
            clip: None,
            scroll_x: 0.0,
            scroll_y: 0.0,
            z_order: 0,
            transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            opacity: 1.0,
            masks: Vec::new(),
        }
    }
}

impl WgpuDrawState {
    pub fn with_clip(mut self, x: f32, y: f32, width: f32, height: f32) -> Self {
        self.clip = Some(WgpuRect {
            x,
            y,
            width,
            height,
        });
        self
    }

    pub fn with_scroll(mut self, scroll_x: f32, scroll_y: f32) -> Self {
        self.scroll_x = scroll_x;
        self.scroll_y = scroll_y;
        self
    }

    pub fn with_z_order(mut self, z_order: i32) -> Self {
        self.z_order = z_order;
        self
    }

    pub fn with_transform(mut self, transform: [f32; 6]) -> Self {
        self.transform = transform;
        self
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn with_mask(mut self, mask: impl Into<String>) -> Self {
        self.masks.push(mask.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenshotReadback {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub pixel_digest: String,
    pub coverage: Vec<String>,
}

impl ScreenshotReadback {
    pub fn read(presenter: &NativeSwapchainPresenter) -> Result<Self, WgpuRenderError> {
        if presenter.config.size.width == 0 || presenter.config.size.height == 0 {
            return Err(WgpuRenderError::ZeroSizedSurface);
        }
        let width = presenter.config.size.width;
        let height = presenter.config.size.height;
        let coverage = presenter
            .pipeline
            .draw_calls
            .iter()
            .map(|call| call.kind.clone())
            .collect::<Vec<_>>();
        let mut pixels = vec![0_u8; (width * height * 4) as usize];
        let pixel_len = pixels.len();
        for (index, call) in presenter.pipeline.draw_calls.iter().enumerate() {
            let offset = (index * 17) % pixel_len.max(1);
            let value = match call.kind.as_str() {
                "card" => 80,
                "dot" => 140,
                "text" => 220,
                "edge" => 120,
                "focus_ring" => 255,
                _ => 48,
            };
            pixels[offset] = value;
            pixels[(offset + 1) % pixel_len] = value.saturating_add(10);
            pixels[(offset + 2) % pixel_len] = value.saturating_add(20);
            pixels[(offset + 3) % pixel_len] = 255;
        }
        let mut hasher = DefaultHasher::new();
        width.hash(&mut hasher);
        height.hash(&mut hasher);
        coverage.hash(&mut hasher);
        pixels
            .iter()
            .take(256)
            .for_each(|byte| byte.hash(&mut hasher));
        Ok(Self {
            width,
            height,
            pixels,
            pixel_digest: format!("{:016x}", hasher.finish()),
            coverage,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HitTestResult {
    pub glyph_id: String,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug)]
pub struct TextAtlasUploader {
    atlas_width: u32,
    atlas_height: u32,
    uploads: Vec<TextureUpload>,
}

impl TextAtlasUploader {
    pub fn new(atlas_width: u32, atlas_height: u32) -> Self {
        Self {
            atlas_width,
            atlas_height,
            uploads: Vec::new(),
        }
    }

    pub fn upload_run(
        &mut self,
        text: &mut TextEngine,
        run: TextRun,
    ) -> Result<TextureUpload, WgpuRenderError> {
        let shaped = text.shape(&run)?;
        let raster = text.rasterize(&shaped)?;
        let upload = TextureUpload {
            label: "text_atlas".to_string(),
            width: raster.atlas_width.min(self.atlas_width),
            height: raster.atlas_height.min(self.atlas_height),
            bytes: raster.alpha_pixels.len(),
        };
        self.uploads.push(upload.clone());
        Ok(upload)
    }

    pub fn upload_count(&self) -> usize {
        self.uploads.len()
    }

    pub fn uploads(&self) -> &[TextureUpload] {
        &self.uploads
    }
}

#[derive(Clone, Debug)]
pub struct BrowserWebGpuPresenter {
    config: NativeSwapchainConfig,
    pipeline: WgpuSurfacePipeline,
    presented_frames: u64,
    dom_accessibility_overlay: bool,
}

impl BrowserWebGpuPresenter {
    pub fn new(size: SurfaceSize) -> Self {
        Self {
            config: NativeSwapchainConfig::new(size),
            pipeline: WgpuSurfacePipeline {
                shader_wgsl: SWAPCHAIN_WGSL.to_string(),
                vertex_buffers: 2,
                bind_groups: vec![
                    "camera".to_string(),
                    "glyph_instances".to_string(),
                    "text_atlas".to_string(),
                    "webgpu_canvas".to_string(),
                ],
                draw_calls: Vec::new(),
            },
            presented_frames: 0,
            dom_accessibility_overlay: true,
        }
    }

    pub fn present_commands(
        &mut self,
        frame: &RenderCommandFrame,
    ) -> Result<WgpuFrameStats, WgpuRenderError> {
        if self.config.size.width == 0 || self.config.size.height == 0 {
            return Err(WgpuRenderError::ZeroSizedSurface);
        }
        self.pipeline.draw_calls = summarize_draw_calls(&frame.commands);
        self.presented_frames += 1;
        Ok(WgpuFrameStats {
            mode: "browser-webgpu-contract".to_string(),
            presented_frames: self.presented_frames,
            draw_calls: self
                .pipeline
                .draw_calls
                .iter()
                .map(|call| call.instances)
                .sum(),
            surface_size: self.config.size,
            sample_count: self.config.sample_count,
        })
    }

    pub fn backend(&self) -> &'static str {
        "webgpu"
    }

    pub fn pipeline_plan(&self) -> &WgpuSurfacePipeline {
        &self.pipeline
    }

    pub fn dom_accessibility_overlay_enabled(&self) -> bool {
        self.dom_accessibility_overlay
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RenderBenchmarkSuite {
    pub target_frame_ms: f32,
}

impl RenderBenchmarkSuite {
    pub fn prototype() -> Self {
        Self {
            target_frame_ms: 16.67,
        }
    }

    pub fn run<const N: usize>(&self, glyph_counts: [usize; N]) -> RenderBenchmarkReport {
        let scenarios = glyph_counts
            .into_iter()
            .map(|glyphs| RenderBenchmarkScenario {
                glyphs,
                commands: glyphs * 3,
                estimated_frame_ms: estimate_frame_ms(glyphs),
                within_budget: estimate_frame_ms(glyphs) <= self.target_frame_ms,
            })
            .collect::<Vec<_>>();
        let summary = scenarios
            .iter()
            .map(|scenario| {
                format!(
                    "{} glyphs: {:.2}ms",
                    scenario.glyphs, scenario.estimated_frame_ms
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        RenderBenchmarkReport { scenarios, summary }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RenderBenchmarkReport {
    pub scenarios: Vec<RenderBenchmarkScenario>,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RenderBenchmarkScenario {
    pub glyphs: usize,
    pub commands: usize,
    pub estimated_frame_ms: f32,
    pub within_budget: bool,
}

fn summarize_draw_calls(commands: &[RenderCommand]) -> Vec<GpuDrawCall> {
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for command in commands {
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
    counts
        .into_iter()
        .map(|(kind, instances)| GpuDrawCall { kind, instances })
        .collect()
}

fn allocate_resources(
    frame: &RenderCommandFrame,
    texture_uploads: Vec<TextureUpload>,
) -> WgpuResourceSet {
    let command_count = frame.commands.len();
    let text_atlas_bytes = texture_uploads.iter().map(|upload| upload.bytes).sum();
    WgpuResourceSet {
        vertex_buffer_bytes: command_count.max(1) * 32,
        index_buffer_bytes: command_count.max(1) * 12,
        instance_buffer_bytes: command_count.max(1) * 64,
        uniform_buffer_bytes: 256,
        text_atlas_bytes,
        texture_uploads,
    }
}

fn build_render_pass(
    config: &NativeSwapchainConfig,
    pipeline: &WgpuSurfacePipeline,
) -> RenderPassPlan {
    RenderPassPlan {
        color_attachment_format: config.texture_format.clone(),
        sample_count: config.sample_count,
        bind_groups: pipeline.bind_groups.clone(),
        draws: pipeline.draw_calls.clone(),
        uses_depth: true,
    }
}

fn hit_test_command(command: &RenderCommand, x: f32, y: f32) -> Option<HitTestResult> {
    match command {
        RenderCommand::Card {
            glyph_id,
            x: left,
            y: top,
            width,
            height,
            ..
        } if x >= *left && x <= *left + *width && y >= *top && y <= *top + *height => {
            Some(HitTestResult {
                glyph_id: glyph_id.clone(),
                x,
                y,
            })
        }
        RenderCommand::Dot {
            glyph_id,
            x: center_x,
            y: center_y,
            radius,
            ..
        } if (x - center_x).abs() <= *radius && (y - center_y).abs() <= *radius => {
            Some(HitTestResult {
                glyph_id: glyph_id.clone(),
                x,
                y,
            })
        }
        RenderCommand::Text {
            glyph_id,
            x: left,
            y: baseline,
            shaped_width,
            ..
        } if x >= *left
            && x <= *left + *shaped_width
            && y >= *baseline - 18.0
            && y <= *baseline + 4.0 =>
        {
            Some(HitTestResult {
                glyph_id: glyph_id.clone(),
                x,
                y,
            })
        }
        _ => None,
    }
}

fn estimate_frame_ms(glyphs: usize) -> f32 {
    1.0 + glyphs as f32 * 0.00042
}
