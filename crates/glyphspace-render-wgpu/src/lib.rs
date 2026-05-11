use glyphspace_render::{GpuDrawCall, RenderCommand, RenderCommandFrame};
use serde::{Deserialize, Serialize};
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeSwapchainConfig {
    pub size: SurfaceSize,
    pub sample_count: u32,
    pub vsync: bool,
    pub texture_format: String,
    pub present_mode: String,
}

impl NativeSwapchainConfig {
    pub fn new(size: SurfaceSize) -> Self {
        Self {
            size,
            sample_count: 1,
            vsync: true,
            texture_format: format!("{:?}", wgpu::TextureFormat::Bgra8UnormSrgb),
            present_mode: format!("{:?}", wgpu::PresentMode::Fifo),
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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WgpuSurfacePipeline {
    pub shader_wgsl: String,
    pub vertex_buffers: usize,
    pub bind_groups: Vec<String>,
    pub draw_calls: Vec<GpuDrawCall>,
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
}

#[derive(Clone, Debug)]
pub struct NativeSwapchainPresenter {
    config: NativeSwapchainConfig,
    pipeline: WgpuSurfacePipeline,
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
