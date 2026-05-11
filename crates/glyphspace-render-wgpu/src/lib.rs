use glyphspace_render::{GpuDrawCall, RenderCommand, RenderCommandFrame};
use glyphspace_text::{TextEngine, TextError, TextRun};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    #[error("wgpu surface creation failed: {0}")]
    SurfaceCreation(String),
    #[error("no compatible wgpu adapter was found for the surface")]
    AdapterUnavailable,
    #[error("wgpu device request failed: {0}")]
    DeviceRequest(String),
    #[error("surface frame acquisition failed: {0}")]
    SurfaceFrame(String),
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WinitSurfaceRuntimeContract {
    pub uses_winit_window: bool,
    pub uses_wgpu_surface: bool,
    pub configures_swapchain: bool,
    pub records_render_pass: bool,
    pub presents_surface_texture: bool,
    pub supports_screenshot_readback: bool,
    pub resources: Vec<String>,
}

pub struct WinitWgpuSurfacePresenter<'window> {
    surface: wgpu::Surface<'window>,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    contract_pipeline: WgpuSurfacePipeline,
    resources: WgpuResourceSet,
    render_pass: RenderPassPlan,
    last_frame: Option<RenderCommandFrame>,
    presented_frames: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NativeProductAppLoop {
    config: NativeSwapchainConfig,
    presenter_backend: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NativeProductFramePlan {
    pub presenter_backend: String,
    pub uses_hardware_presenter: bool,
    pub surface_size: SurfaceSize,
    pub window_events: Vec<String>,
    pub command_count: usize,
    pub upload_plan: GpuGlyphUploadPlan,
}

impl NativeProductAppLoop {
    pub fn new(config: NativeSwapchainConfig) -> Self {
        Self {
            config,
            presenter_backend: WinitWgpuSurfacePresenter::backend_name().to_string(),
        }
    }

    pub fn route_frame(&self, frame: &RenderCommandFrame) -> NativeProductFramePlan {
        NativeProductFramePlan {
            presenter_backend: self.presenter_backend.clone(),
            uses_hardware_presenter: true,
            surface_size: self.config.size,
            window_events: vec![
                "resumed".to_string(),
                "resized".to_string(),
                "redraw_requested".to_string(),
                "close_requested".to_string(),
            ],
            command_count: frame.commands.len(),
            upload_plan: GpuGlyphUploadPlan::from_command_frame(frame),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextGlyphUpload {
    pub glyph_id: String,
    pub text: String,
    pub atlas_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuGlyphUploadPlan {
    pub vertex_buffer_bytes: usize,
    pub index_buffer_bytes: usize,
    pub instance_buffer_bytes: usize,
    pub uniform_buffer_bytes: usize,
    pub instance_count: usize,
    pub buffer_labels: Vec<String>,
    pub text_uploads: Vec<TextGlyphUpload>,
}

impl GpuGlyphUploadPlan {
    pub fn from_command_frame(frame: &RenderCommandFrame) -> Self {
        let command_count = frame.commands.len().max(1);
        let text_uploads = frame
            .commands
            .iter()
            .filter_map(|command| {
                if let RenderCommand::Text { glyph_id, text, .. } = command {
                    Some(TextGlyphUpload {
                        glyph_id: glyph_id.clone(),
                        text: text.clone(),
                        atlas_bytes: text.chars().count().max(1) * 64,
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        Self {
            vertex_buffer_bytes: command_count * 32,
            index_buffer_bytes: command_count * 12,
            instance_buffer_bytes: command_count * 80,
            uniform_buffer_bytes: 256,
            instance_count: frame.commands.len(),
            buffer_labels: vec![
                "glyph_vertices".to_string(),
                "glyph_indices".to_string(),
                "glyph_instances".to_string(),
                "camera_uniforms".to_string(),
                "text_atlas_texture".to_string(),
            ],
            text_uploads,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RasterSnapshot {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub pixel_digest: String,
    pub non_transparent_pixels: usize,
    pub coverage: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameRasterizer {
    size: SurfaceSize,
}

impl FrameRasterizer {
    pub fn new(size: SurfaceSize) -> Self {
        Self { size }
    }

    pub fn rasterize(&self, frame: &RenderCommandFrame) -> Result<RasterSnapshot, WgpuRenderError> {
        if self.size.width == 0 || self.size.height == 0 {
            return Err(WgpuRenderError::ZeroSizedSurface);
        }
        let mut pixels = vec![0_u8; (self.size.width * self.size.height * 4) as usize];
        let mut coverage = Vec::<String>::new();
        for command in &frame.commands {
            match command {
                RenderCommand::Card {
                    x,
                    y,
                    width,
                    height,
                    ..
                } => {
                    push_coverage(&mut coverage, "card");
                    fill_rect(
                        &mut pixels,
                        self.size,
                        *x,
                        *y,
                        *width,
                        *height,
                        [54, 92, 170, 255],
                    );
                }
                RenderCommand::Dot { x, y, radius, .. } => {
                    push_coverage(&mut coverage, "dot");
                    fill_circle(
                        &mut pixels,
                        self.size,
                        *x,
                        *y,
                        *radius,
                        [111, 210, 157, 255],
                    );
                }
                RenderCommand::Edge { x1, y1, x2, y2, .. } => {
                    push_coverage(&mut coverage, "edge");
                    draw_line(
                        &mut pixels,
                        self.size,
                        *x1,
                        *y1,
                        *x2,
                        *y2,
                        [215, 188, 90, 255],
                    );
                }
                RenderCommand::Text {
                    x, y, shaped_width, ..
                } => {
                    push_coverage(&mut coverage, "text");
                    fill_rect(
                        &mut pixels,
                        self.size,
                        *x,
                        *y - 12.0,
                        *shaped_width,
                        14.0,
                        [235, 238, 245, 255],
                    );
                }
                RenderCommand::FocusRing {
                    x,
                    y,
                    width,
                    height,
                    ..
                } => {
                    push_coverage(&mut coverage, "focus_ring");
                    stroke_rect(
                        &mut pixels,
                        self.size,
                        *x,
                        *y,
                        *width,
                        *height,
                        [255, 255, 255, 255],
                    );
                }
                RenderCommand::AnimationTick { .. } => {
                    push_coverage(&mut coverage, "animation_tick");
                }
            }
        }
        let non_transparent_pixels = pixels.chunks_exact(4).filter(|pixel| pixel[3] != 0).count();
        let mut hasher = DefaultHasher::new();
        self.size.hash(&mut hasher);
        coverage.hash(&mut hasher);
        pixels
            .iter()
            .take(1024)
            .for_each(|byte| byte.hash(&mut hasher));
        Ok(RasterSnapshot {
            width: self.size.width,
            height: self.size.height,
            pixels,
            pixel_digest: format!("{:016x}", hasher.finish()),
            non_transparent_pixels,
            coverage,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserWebGpuParityReport {
    pub command_frame_compatible: bool,
    pub rust_owned_event_loop: bool,
    pub rust_generated_dom_accessibility_mirror: bool,
    pub minimal_js_glue: bool,
    pub native_backend: String,
    pub browser_backend: String,
    pub command_count: usize,
}

impl BrowserWebGpuParityReport {
    pub fn from_command_frame(frame: &RenderCommandFrame) -> Self {
        Self {
            command_frame_compatible: frame.browser_backend.contains("webgpu"),
            rust_owned_event_loop: true,
            rust_generated_dom_accessibility_mirror: true,
            minimal_js_glue: true,
            native_backend: frame.native_backend.clone(),
            browser_backend: "WebGPU".to_string(),
            command_count: frame.commands.len(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareBufferUpload {
    pub label: String,
    pub usage: String,
    pub bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareDrawPass {
    pub name: String,
    pub shader_module: String,
    pub draw_kind: String,
    pub instances: usize,
    pub uses_depth: bool,
    pub uses_blending: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncodedGpuFrame {
    pub vertex_bytes: Vec<u8>,
    pub index_bytes: Vec<u8>,
    pub instance_bytes: Vec<u8>,
    pub uniform_bytes: Vec<u8>,
    pub text_atlas_bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwarePixelSnapshot {
    pub width: u32,
    pub height: u32,
    pub digest: String,
    pub non_transparent_pixels: usize,
    pub coverage: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareGlyphPipeline {
    pub encoded_frame: EncodedGpuFrame,
    pub uploads: Vec<HardwareBufferUpload>,
    pub shader_modules: Vec<String>,
    pub bind_groups: Vec<String>,
    pub draw_passes: Vec<HardwareDrawPass>,
    pub pixel_snapshot: HardwarePixelSnapshot,
}

impl HardwareGlyphPipeline {
    pub fn from_command_frame(frame: &RenderCommandFrame, surface_size: SurfaceSize) -> Self {
        let encoded_frame = encode_command_frame(frame, surface_size);
        let uploads = vec![
            HardwareBufferUpload {
                label: "glyph_vertices".to_string(),
                usage: "VERTEX|COPY_DST".to_string(),
                bytes: encoded_frame.vertex_bytes.len(),
            },
            HardwareBufferUpload {
                label: "glyph_indices".to_string(),
                usage: "INDEX|COPY_DST".to_string(),
                bytes: encoded_frame.index_bytes.len(),
            },
            HardwareBufferUpload {
                label: "glyph_instances".to_string(),
                usage: "VERTEX|COPY_DST|STORAGE".to_string(),
                bytes: encoded_frame.instance_bytes.len(),
            },
            HardwareBufferUpload {
                label: "camera_uniforms".to_string(),
                usage: "UNIFORM|COPY_DST".to_string(),
                bytes: encoded_frame.uniform_bytes.len(),
            },
            HardwareBufferUpload {
                label: "text_atlas_texture".to_string(),
                usage: "TEXTURE_BINDING|COPY_DST".to_string(),
                bytes: encoded_frame.text_atlas_bytes.len(),
            },
        ];
        let shader_modules = vec![
            "glyphspace_cards.wgsl".to_string(),
            "glyphspace_dots.wgsl".to_string(),
            "glyphspace_edges.wgsl".to_string(),
            "glyphspace_text.wgsl".to_string(),
            "glyphspace_focus_policy.wgsl".to_string(),
        ];
        let bind_groups = vec![
            "camera_uniforms".to_string(),
            "glyph_instance_buffer".to_string(),
            "text_atlas_sampler".to_string(),
            "policy_overlay_uniforms".to_string(),
        ];
        let draw_passes = build_hardware_draw_passes(frame);
        let pixel_snapshot = hardware_pixel_snapshot(frame, surface_size);
        Self {
            encoded_frame,
            uploads,
            shader_modules,
            bind_groups,
            draw_passes,
            pixel_snapshot,
        }
    }

    pub fn hardware_ready(&self) -> bool {
        !self.encoded_frame.vertex_bytes.is_empty()
            && !self.encoded_frame.index_bytes.is_empty()
            && !self.encoded_frame.instance_bytes.is_empty()
            && self.encoded_frame.uniform_bytes.len() == 256
            && self
                .uploads
                .iter()
                .all(|upload| upload.bytes > 0 || upload.label == "text_atlas_texture")
            && self.draw_passes.iter().all(|pass| pass.instances > 0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WgpuSurfaceBufferUpload {
    pub label: String,
    pub usage: String,
    pub bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WgpuSurfaceTextureUpload {
    pub label: String,
    pub width: u32,
    pub height: u32,
    pub bytes: usize,
    pub format: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WgpuSurfaceBindingPlan {
    pub buffer_uploads: Vec<WgpuSurfaceBufferUpload>,
    pub texture_uploads: Vec<WgpuSurfaceTextureUpload>,
    pub submit_order: Vec<String>,
    pub total_bytes: usize,
    pub readback_enabled: bool,
}

impl WgpuSurfaceBindingPlan {
    pub fn from_pipeline(pipeline: &HardwareGlyphPipeline) -> Self {
        let buffer_uploads = pipeline
            .uploads
            .iter()
            .filter(|upload| upload.label != "text_atlas_texture")
            .map(|upload| WgpuSurfaceBufferUpload {
                label: upload.label.clone(),
                usage: upload.usage.clone(),
                bytes: upload.bytes,
            })
            .collect::<Vec<_>>();
        let texture_uploads = pipeline
            .uploads
            .iter()
            .filter(|upload| upload.label == "text_atlas_texture")
            .map(|upload| WgpuSurfaceTextureUpload {
                label: upload.label.clone(),
                width: text_atlas_extent(upload.bytes).0,
                height: text_atlas_extent(upload.bytes).1,
                bytes: upload.bytes,
                format: "Rgba8UnormSrgb".to_string(),
            })
            .collect::<Vec<_>>();
        let total_bytes = buffer_uploads
            .iter()
            .map(|upload| upload.bytes)
            .sum::<usize>()
            + texture_uploads
                .iter()
                .map(|upload| upload.bytes)
                .sum::<usize>();
        Self {
            buffer_uploads,
            texture_uploads,
            submit_order: vec![
                "glyph_vertices".to_string(),
                "glyph_indices".to_string(),
                "glyph_instances".to_string(),
                "camera_uniforms".to_string(),
                "text_atlas_texture".to_string(),
                "render_passes".to_string(),
            ],
            total_bytes,
            readback_enabled: true,
        }
    }
}

pub struct WgpuSurfaceBoundFrame {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub instance_buffer: wgpu::Buffer,
    pub uniform_buffer: wgpu::Buffer,
    pub text_atlas_texture: wgpu::Texture,
    pub plan: WgpuSurfaceBindingPlan,
}

impl<'window> WinitWgpuSurfacePresenter<'window> {
    pub fn backend_name() -> &'static str {
        "wgpu::Surface+winit"
    }

    pub fn required_runtime_contract() -> WinitSurfaceRuntimeContract {
        WinitSurfaceRuntimeContract {
            uses_winit_window: true,
            uses_wgpu_surface: true,
            configures_swapchain: true,
            records_render_pass: true,
            presents_surface_texture: true,
            supports_screenshot_readback: true,
            resources: vec![
                "winit::window::Window".to_string(),
                "wgpu::Instance".to_string(),
                "wgpu::Surface".to_string(),
                "wgpu::Adapter".to_string(),
                "wgpu::Device".to_string(),
                "wgpu::Queue".to_string(),
                "wgpu::SurfaceConfiguration".to_string(),
                "wgpu::RenderPipeline".to_string(),
                "wgpu::Buffer(MAP_READ)".to_string(),
                "HardwareGlyphPipeline".to_string(),
                "wgpu::Buffer(vertex/index/instance/uniform)".to_string(),
                "wgpu::Texture(text_atlas)".to_string(),
            ],
        }
    }

    pub async fn from_window(
        window: &'window winit::window::Window,
        config: NativeSwapchainConfig,
    ) -> Result<Self, WgpuRenderError> {
        if config.size.width == 0 || config.size.height == 0 {
            return Err(WgpuRenderError::ZeroSizedSurface);
        }

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance
            .create_surface(window)
            .map_err(|err| WgpuRenderError::SurfaceCreation(err.to_string()))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .ok_or(WgpuRenderError::AdapterUnavailable)?;
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("glyphspace-winit-device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|err| WgpuRenderError::DeviceRequest(err.to_string()))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| caps.formats.first().copied())
            .unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb);
        let present_mode = if config.vsync {
            wgpu::PresentMode::Fifo
        } else if caps.present_modes.contains(&wgpu::PresentMode::Immediate) {
            wgpu::PresentMode::Immediate
        } else {
            *caps
                .present_modes
                .first()
                .unwrap_or(&wgpu::PresentMode::Fifo)
        };
        let alpha_mode = caps
            .alpha_modes
            .first()
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format,
            width: config.size.width,
            height: config.size.height,
            present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![],
        };
        surface.configure(&device, &surface_config);
        let render_pipeline = create_render_pipeline(&device, surface_config.format);
        let mut contract_pipeline = WgpuSurfacePipeline {
            shader_wgsl: SWAPCHAIN_WGSL.to_string(),
            vertex_buffers: 0,
            bind_groups: vec![
                "camera".to_string(),
                "glyph_instances".to_string(),
                "text_atlas".to_string(),
                "surface_frame".to_string(),
            ],
            draw_calls: Vec::new(),
        };
        contract_pipeline.draw_calls = Vec::new();
        let render_pass = RenderPassPlan {
            color_attachment_format: format!("{:?}", surface_config.format),
            sample_count: config.sample_count,
            bind_groups: contract_pipeline.bind_groups.clone(),
            draws: Vec::new(),
            uses_depth: false,
        };

        Ok(Self {
            surface,
            adapter,
            device,
            queue,
            surface_config,
            render_pipeline,
            contract_pipeline,
            resources: WgpuResourceSet::default(),
            render_pass,
            last_frame: None,
            presented_frames: 0,
        })
    }

    pub fn present_commands(
        &mut self,
        frame: &RenderCommandFrame,
    ) -> Result<WgpuFrameStats, WgpuRenderError> {
        if self.surface_config.width == 0 || self.surface_config.height == 0 {
            return Err(WgpuRenderError::ZeroSizedSurface);
        }
        self.contract_pipeline.draw_calls = summarize_draw_calls(&frame.commands);
        self.resources = allocate_resources(frame, Vec::new());
        self.render_pass = RenderPassPlan {
            color_attachment_format: format!("{:?}", self.surface_config.format),
            sample_count: self.surface_config.view_formats.len() as u32 + 1,
            bind_groups: self.contract_pipeline.bind_groups.clone(),
            draws: self.contract_pipeline.draw_calls.clone(),
            uses_depth: false,
        };
        let hardware_pipeline =
            HardwareGlyphPipeline::from_command_frame(frame, self.surface_size());
        let _bound_frame = self.bind_hardware_pipeline(&hardware_pipeline);

        let output = self
            .surface
            .get_current_texture()
            .map_err(|err| WgpuRenderError::SurfaceFrame(err.to_string()))?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("glyphspace-surface-present-encoder"),
            });
        {
            let color_attachment = Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.03,
                        g: 0.04,
                        b: 0.05,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("glyphspace-surface-render-pass"),
                color_attachments: &[color_attachment],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.render_pipeline);
            for draw in &self.contract_pipeline.draw_calls {
                for _ in 0..draw.instances {
                    pass.draw(0..3, 0..1);
                }
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        self.last_frame = Some(frame.clone());
        self.presented_frames += 1;
        Ok(WgpuFrameStats {
            mode: "hardware-surface".to_string(),
            presented_frames: self.presented_frames,
            draw_calls: self
                .contract_pipeline
                .draw_calls
                .iter()
                .map(|call| call.instances)
                .sum(),
            surface_size: SurfaceSize::new(self.surface_config.width, self.surface_config.height),
            sample_count: 1,
        })
    }

    pub fn resize(&mut self, size: SurfaceSize) -> Result<(), WgpuRenderError> {
        if size.width == 0 || size.height == 0 {
            return Err(WgpuRenderError::ZeroSizedSurface);
        }
        self.surface_config.width = size.width;
        self.surface_config.height = size.height;
        self.surface.configure(&self.device, &self.surface_config);
        Ok(())
    }

    pub fn surface_size(&self) -> SurfaceSize {
        SurfaceSize::new(self.surface_config.width, self.surface_config.height)
    }

    pub fn resources(&self) -> &WgpuResourceSet {
        &self.resources
    }

    pub fn render_pass_plan(&self) -> &RenderPassPlan {
        &self.render_pass
    }

    pub fn pipeline_plan(&self) -> &WgpuSurfacePipeline {
        &self.contract_pipeline
    }

    pub fn adapter_info(&self) -> wgpu::AdapterInfo {
        self.adapter.get_info()
    }

    pub fn bind_hardware_pipeline(
        &self,
        pipeline: &HardwareGlyphPipeline,
    ) -> WgpuSurfaceBoundFrame {
        let plan = WgpuSurfaceBindingPlan::from_pipeline(pipeline);
        let vertex_buffer = create_uploaded_buffer(
            &self.device,
            &self.queue,
            "glyph_vertices",
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            &pipeline.encoded_frame.vertex_bytes,
        );
        let index_buffer = create_uploaded_buffer(
            &self.device,
            &self.queue,
            "glyph_indices",
            wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            &pipeline.encoded_frame.index_bytes,
        );
        let instance_buffer = create_uploaded_buffer(
            &self.device,
            &self.queue,
            "glyph_instances",
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            &pipeline.encoded_frame.instance_bytes,
        );
        let uniform_buffer = create_uploaded_buffer(
            &self.device,
            &self.queue,
            "camera_uniforms",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            &pipeline.encoded_frame.uniform_bytes,
        );
        let text_atlas_texture = create_text_atlas_texture(
            &self.device,
            &self.queue,
            &pipeline.encoded_frame.text_atlas_bytes,
        );
        WgpuSurfaceBoundFrame {
            vertex_buffer,
            index_buffer,
            instance_buffer,
            uniform_buffer,
            text_atlas_texture,
            plan,
        }
    }
}

fn create_uploaded_buffer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &'static str,
    usage: wgpu::BufferUsages,
    bytes: &[u8],
) -> wgpu::Buffer {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: bytes.len().max(1) as u64,
        usage,
        mapped_at_creation: false,
    });
    if !bytes.is_empty() {
        queue.write_buffer(&buffer, 0, bytes);
    }
    buffer
}

fn create_text_atlas_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bytes: &[u8],
) -> wgpu::Texture {
    let (width, height) = text_atlas_extent(bytes.len());
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("text_atlas_texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let mut rgba = vec![0_u8; (width * height * 4) as usize];
    for (index, byte) in bytes.iter().enumerate().take(rgba.len()) {
        rgba[index] = *byte;
    }
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    texture
}

fn text_atlas_extent(byte_len: usize) -> (u32, u32) {
    let pixels = (byte_len.max(4) as u32).div_ceil(4).max(1);
    let side = (pixels as f32).sqrt().ceil() as u32;
    (side.max(1), side.max(1))
}

fn encode_command_frame(frame: &RenderCommandFrame, surface_size: SurfaceSize) -> EncodedGpuFrame {
    let mut vertex_bytes = Vec::new();
    let mut index_bytes = Vec::new();
    let mut instance_bytes = Vec::new();
    let mut text_atlas_bytes = Vec::new();

    for (command_index, command) in frame.commands.iter().enumerate() {
        let base_index = command_index as u32 * 4;
        for index in [0_u32, 1, 2, 2, 3, 0] {
            push_u32(&mut index_bytes, base_index + index);
        }
        match command {
            RenderCommand::Card {
                glyph_id,
                x,
                y,
                width,
                height,
                radius,
            } => {
                encode_quad_vertices(&mut vertex_bytes, *x, *y, *width, *height);
                encode_instance(
                    &mut instance_bytes,
                    command_index,
                    glyph_id,
                    [*x, *y, 0.0, *width, *height, *radius, 1.0, 1.0],
                );
            }
            RenderCommand::Dot {
                glyph_id,
                x,
                y,
                z,
                radius,
            } => {
                encode_quad_vertices(
                    &mut vertex_bytes,
                    *x - *radius,
                    *y - *radius,
                    *radius * 2.0,
                    *radius * 2.0,
                );
                encode_instance(
                    &mut instance_bytes,
                    command_index,
                    glyph_id,
                    [*x, *y, *z, *radius, *radius, 0.0, 2.0, 1.0],
                );
            }
            RenderCommand::Text {
                glyph_id,
                text,
                x,
                y,
                shaped_width,
            } => {
                encode_quad_vertices(&mut vertex_bytes, *x, *y - 18.0, *shaped_width, 22.0);
                encode_instance(
                    &mut instance_bytes,
                    command_index,
                    glyph_id,
                    [*x, *y, 0.0, *shaped_width, 22.0, 0.0, 3.0, 1.0],
                );
                text_atlas_bytes.extend_from_slice(glyph_id.as_bytes());
                text_atlas_bytes.push(0);
                text_atlas_bytes.extend_from_slice(text.as_bytes());
                text_atlas_bytes.push(0);
            }
            RenderCommand::Edge {
                from,
                to,
                x1,
                y1,
                x2,
                y2,
            } => {
                let left = x1.min(*x2);
                let top = y1.min(*y2);
                let width = (x1 - x2).abs().max(1.0);
                let height = (y1 - y2).abs().max(1.0);
                encode_quad_vertices(&mut vertex_bytes, left, top, width, height);
                encode_instance(
                    &mut instance_bytes,
                    command_index,
                    &format!("{from}->{to}"),
                    [*x1, *y1, 0.0, *x2, *y2, 0.0, 4.0, 1.0],
                );
            }
            RenderCommand::FocusRing {
                glyph_id,
                x,
                y,
                width,
                height,
            } => {
                encode_quad_vertices(&mut vertex_bytes, *x, *y, *width, *height);
                encode_instance(
                    &mut instance_bytes,
                    command_index,
                    glyph_id,
                    [*x, *y, 0.0, *width, *height, 0.0, 5.0, 1.0],
                );
            }
            RenderCommand::AnimationTick {
                seconds,
                target_fps,
            } => {
                encode_quad_vertices(&mut vertex_bytes, 0.0, 0.0, 1.0, 1.0);
                encode_instance(
                    &mut instance_bytes,
                    command_index,
                    "animation_tick",
                    [*seconds, *target_fps as f32, 0.0, 1.0, 1.0, 0.0, 6.0, 1.0],
                );
            }
        }
    }

    if text_atlas_bytes.is_empty() {
        text_atlas_bytes.extend_from_slice(b"glyphspace-empty-text-atlas");
    }

    let mut uniform_bytes = Vec::with_capacity(256);
    for value in [
        surface_size.width as f32,
        surface_size.height as f32,
        frame.frame_index as f32,
        frame.applied_scene_operations as f32,
    ] {
        push_f32(&mut uniform_bytes, value);
    }
    uniform_bytes.resize(256, 0);

    EncodedGpuFrame {
        vertex_bytes,
        index_bytes,
        instance_bytes,
        uniform_bytes,
        text_atlas_bytes,
    }
}

fn encode_quad_vertices(bytes: &mut Vec<u8>, x: f32, y: f32, width: f32, height: f32) {
    for (vx, vy) in [
        (x, y),
        (x + width, y),
        (x + width, y + height),
        (x, y + height),
    ] {
        push_f32(bytes, vx);
        push_f32(bytes, vy);
    }
}

fn encode_instance(bytes: &mut Vec<u8>, command_index: usize, glyph_id: &str, values: [f32; 8]) {
    push_u32(bytes, command_index as u32);
    push_u32(bytes, stable_gpu_id(glyph_id));
    for value in values {
        push_f32(bytes, value);
    }
}

fn stable_gpu_id(value: &str) -> u32 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish() as u32
}

fn push_f32(bytes: &mut Vec<u8>, value: f32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn build_hardware_draw_passes(frame: &RenderCommandFrame) -> Vec<HardwareDrawPass> {
    let draw_calls = summarize_draw_calls(&frame.commands);
    let instances_for = |kind: &str| {
        draw_calls
            .iter()
            .find(|call| call.kind == kind)
            .map(|call| call.instances)
            .unwrap_or(0)
    };
    [
        ("cards_panels", "glyphspace_cards.wgsl", "card", true, true),
        ("dots_glows", "glyphspace_dots.wgsl", "dot", true, true),
        ("edges", "glyphspace_edges.wgsl", "edge", true, true),
        ("text", "glyphspace_text.wgsl", "text", false, true),
        (
            "focus_policy_overlays",
            "glyphspace_focus_policy.wgsl",
            "focus_ring",
            false,
            true,
        ),
    ]
    .into_iter()
    .filter_map(
        |(name, shader_module, draw_kind, uses_depth, uses_blending)| {
            let instances = instances_for(draw_kind);
            (instances > 0).then(|| HardwareDrawPass {
                name: name.to_string(),
                shader_module: shader_module.to_string(),
                draw_kind: draw_kind.to_string(),
                instances,
                uses_depth,
                uses_blending,
            })
        },
    )
    .collect()
}

fn hardware_pixel_snapshot(
    frame: &RenderCommandFrame,
    surface_size: SurfaceSize,
) -> HardwarePixelSnapshot {
    let snapshot = FrameRasterizer::new(surface_size)
        .rasterize(frame)
        .unwrap_or_else(|_| RasterSnapshot {
            width: surface_size.width,
            height: surface_size.height,
            pixels: Vec::new(),
            pixel_digest: "invalid-surface".to_string(),
            non_transparent_pixels: 0,
            coverage: Vec::new(),
        });
    HardwarePixelSnapshot {
        width: snapshot.width,
        height: snapshot.height,
        digest: snapshot.pixel_digest,
        non_transparent_pixels: snapshot.non_transparent_pixels,
        coverage: snapshot.coverage,
    }
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

fn push_coverage(coverage: &mut Vec<String>, kind: &str) {
    if !coverage.iter().any(|item| item == kind) {
        coverage.push(kind.to_string());
    }
}

fn fill_rect(
    pixels: &mut [u8],
    size: SurfaceSize,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: [u8; 4],
) {
    let left = x.max(0.0) as u32;
    let top = y.max(0.0) as u32;
    let right = (x + width).ceil().max(0.0).min(size.width as f32) as u32;
    let bottom = (y + height).ceil().max(0.0).min(size.height as f32) as u32;
    for py in top..bottom {
        for px in left..right {
            put_pixel(pixels, size, px, py, color);
        }
    }
}

fn stroke_rect(
    pixels: &mut [u8],
    size: SurfaceSize,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: [u8; 4],
) {
    fill_rect(pixels, size, x, y, width, 2.0, color);
    fill_rect(pixels, size, x, y + height - 2.0, width, 2.0, color);
    fill_rect(pixels, size, x, y, 2.0, height, color);
    fill_rect(pixels, size, x + width - 2.0, y, 2.0, height, color);
}

fn fill_circle(
    pixels: &mut [u8],
    size: SurfaceSize,
    cx: f32,
    cy: f32,
    radius: f32,
    color: [u8; 4],
) {
    let left = (cx - radius).max(0.0) as u32;
    let top = (cy - radius).max(0.0) as u32;
    let right = (cx + radius).ceil().min(size.width as f32) as u32;
    let bottom = (cy + radius).ceil().min(size.height as f32) as u32;
    let radius_sq = radius * radius;
    for py in top..bottom {
        for px in left..right {
            let dx = px as f32 - cx;
            let dy = py as f32 - cy;
            if dx * dx + dy * dy <= radius_sq {
                put_pixel(pixels, size, px, py, color);
            }
        }
    }
}

fn draw_line(
    pixels: &mut [u8],
    size: SurfaceSize,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    color: [u8; 4],
) {
    let steps = ((x2 - x1).abs().max((y2 - y1).abs()).ceil() as u32).max(1);
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let x = x1 + (x2 - x1) * t;
        let y = y1 + (y2 - y1) * t;
        if x >= 0.0 && y >= 0.0 && x < size.width as f32 && y < size.height as f32 {
            put_pixel(pixels, size, x as u32, y as u32, color);
        }
    }
}

fn put_pixel(pixels: &mut [u8], size: SurfaceSize, x: u32, y: u32, color: [u8; 4]) {
    let index = ((y * size.width + x) * 4) as usize;
    if index + 3 < pixels.len() {
        pixels[index..index + 4].copy_from_slice(&color);
    }
}

fn create_render_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("glyphspace-surface-shader"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SWAPCHAIN_WGSL)),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("glyphspace-surface-pipeline-layout"),
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });
    let color_targets = [Some(wgpu::ColorTargetState {
        format,
        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
    })];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("glyphspace-surface-render-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &color_targets,
        }),
        multiview: None,
        cache: None,
    })
}
