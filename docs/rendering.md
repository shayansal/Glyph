# Rendering

The renderer crates are currently contract-rich and headless-real, with the first real native surface binding in place. `glyphspace-render` exposes host-neutral render primitives, deterministic command frames, scene diffs, scene patches, GPU pipeline plans, WGSL shader contracts, text atlas state, screenshot conformance, and nonblank pixel output for CI. `glyphspace-render-wgpu` now preserves that command-frame path across both the headless contract presenter and a real `winit` + `wgpu::Surface` presenter. The project does not yet provide a finished hardware-backed product renderer.

The SOTA renderer tranche adds:

- `TextShaper::placeholder()` and `GlyphTextRun` as the explicit text shaping seam before adopting a full shaper.
- `SelectionStyle` for focus/selection outlines.
- `ScenePatch` derived from scene diffs so hosts can apply incremental updates.
- Stable render primitive keys based on glyph id and primitive kind.
- `NativeRendererHost::run_winit_window(...)`, which creates a `winit` event loop/window and drives redraw/resize through the same world/layout/render path. CI still uses headless rendering.

The production renderer contract now includes `ProductionRenderer` and `RenderSnapshot`. A production frame carries layout, prepared wgpu scene metadata, scene batch, scene patch, and accessibility node counts. Render snapshots are deterministic digests used by `gx conformance` to certify renderer stability across frames.

The beyond-SOTA render loop adds `RenderCommandFrame`, which is the host-neutral command buffer emitted after layout. It contains deterministic commands for semantic dot anchors, cards, text, graph edges, focus rings, and animation ticks. Native hosts consume the frame as a wgpu-oriented command stream, while browser hosts can consume the same frame as a WebGPU command buffer contract. `RenderSnapshot` now records both primitive and command digests so conformance can detect renderer drift even when the higher-level layout hash is stable.

The GPU renderer layer now exposes `GpuPipelinePlan`, `GpuDrawCall`, and `ScreenshotConformance`. The plan carries WGSL shader source, vertex buffer requirements, bind group names, draw-call categories, and a browser parity report that maps native `wgpu` to browser WebGPU. Screenshot conformance is deterministic and command-buffer based, so CI can certify coverage of dots, cards, edges, text, focus rings, and animation without requiring a physical GPU.

The actual drawing tranche adds `ActualGpuRenderer`, `GpuSurfaceConfig`, `GpuBufferSet`, `TextAtlas`, and `GpuPixelOutput`. In headless mode it allocates deterministic wgpu-style buffers, maintains a text atlas, honors MSAA/resizing surface configuration, and produces nonblank pixel output for CI. Native and browser hosts can replace the headless pixel sink with real swapchain presentation while preserving the same command-frame, buffer, text-atlas, and conformance contracts.

The native presentation tranche adds `glyphspace-render-wgpu` with `NativeSwapchainPresenter`, `NativeSwapchainConfig`, `SurfaceSize`, and `WgpuFrameStats`. Its headless contract mode records the same facts a hardware presenter must preserve: surface size, sample count, present mode, WGSL pipeline contract, draw-call categories, frame count, and resize behavior.

The hardware presentation tranche adds `WinitWgpuSurfacePresenter`. It creates a `wgpu::Instance`, binds a `wgpu::Surface` from a `winit::window::Window`, requests an adapter/device/queue, configures the swapchain with render-attachment plus `COPY_SRC` usage, builds a real render pipeline, records a render pass, presents `SurfaceTexture` frames, and exposes the same resource/pass/readback contract used by CI. Hardware readback is now represented in the surface contract; full visual snapshot capture from native presented frames is still upcoming.

The product renderer tranche adds `NativeProductAppLoop`, `GpuGlyphUploadPlan`, `FrameRasterizer`, and `BrowserWebGpuParityReport`. These route command frames into the native surface presenter contract, describe per-frame vertex/index/instance/uniform/text uploads, rasterize cards/dots/edges/text/focus rings into deterministic pixels for snapshots, and certify that browser WebGPU consumes the same command-frame shape with a Rust-generated accessibility mirror.

The hardware encoding tranche adds `HardwareGlyphPipeline`. It converts every `RenderCommandFrame` into deterministic vertex, index, instance, uniform, and text-atlas byte payloads; names the GPU uploads; partitions draw passes for cards/panels, dots/glows, edges, text, and focus/policy overlays; and emits a deterministic pixel snapshot contract for native and browser parity tests.

The hardware binding tranche adds `WgpuSurfaceBindingPlan` and `WinitWgpuSurfacePresenter::bind_hardware_pipeline`. The presenter can now allocate and write real vertex/index/instance/uniform `wgpu::Buffer` resources plus a text-atlas `wgpu::Texture` from the encoded command-frame payloads.

The shader input tranche adds `HardwareShaderInputPlan`, explicit vertex/instance layouts, indexed draw ranges, and a WGSL surface shader that consumes `position`, `command_index`, `glyph_hash`, `geometry`, and `kind_and_opacity` attributes. The native presenter now sets uploaded vertex and instance buffers and issues indexed draws from the hardware plan.

The primitive pipeline tranche adds `PrimitivePipelineSet`. It maps indexed hardware draws into explicit pipeline descriptors for cards/panels, dots/glows, edges, text-atlas rendering, and focus/policy overlays, including topology, bind groups, blending/depth requirements, text-atlas usage, and policy overlay flags.

The primitive shader tranche adds `PrimitiveShaderRegistry` and `PrimitivePipelineCompilationPlan`. The registry owns specialized WGSL modules for cards/panels, dots/glows, edges, text-atlas sampling, and focus/policy overlays; compilation plans validate bind groups, color formats, MSAA sample counts, shader entry points, draw routes, depth, blending, text-atlas usage, and policy overlay requirements. This is still one step short of full product rendering: the next renderer task is creating actual per-primitive `wgpu::RenderPipeline` objects from these plans and using them in `WinitWgpuSurfacePresenter`.

## What Is Real Today

- Command frames for dots, cards, text, graph edges, focus rings, and animation ticks.
- Stable primitive and command digests for renderer determinism checks.
- Scene batching and scene patch generation so hosts can apply incremental updates.
- GPU pipeline planning with WGSL shader contracts, vertex/index/instance buffer requirements, bind group names, and draw-call categories.
- Browser parity metadata mapping the native `wgpu` plan to browser WebGPU expectations.
- Deterministic screenshot conformance without requiring a physical GPU in CI.
- Headless pixel output that is nonblank, resizable, MSAA-aware, and digestible.
- Native swapchain presentation contract with resize, MSAA, present-mode metadata, WGSL pipeline contract, and draw-call stats.
- Real `winit` + `wgpu::Surface` presenter with swapchain configuration, real render pipeline creation, surface texture presentation, and screenshot readback bindings.
- Product frame routing to the native presenter, GPU upload plans, deterministic command-frame raster snapshots, and browser WebGPU parity reports.
- Hardware command-frame encoding into deterministic vertex/index/instance/uniform/text-atlas byte payloads, draw-pass partitions, real surface buffer/texture upload binding, shader input layouts, indexed draw plans, primitive pipeline routing, and per-primitive shader compilation plans.
- Production renderer resource plans for vertex/index/instance/uniform buffers, bind groups, render passes, and texture uploads.
- Text atlas uploads from `glyphspace-text`, including DPI-aware clipped raster output.
- Deterministic screenshot readback, command-frame hit testing, browser WebGPU parity presenter, draw state for clip/scroll/z/transform/opacity/masks, and benchmark reports for 1k, 10k, and 100k glyph scenarios.

## What Is Next

- Bind `PrimitivePipelineCompilationPlan` to actual per-primitive `wgpu::RenderPipeline` objects for cards/dots/edges/text/focus through `WinitWgpuSurfacePresenter`.
- Browser WebGPU renderer consuming the same command-frame contract.
- Actual GPU atlas upload using the new `glyphspace-text` shaping/rasterization abstraction.
- GPU clipping, scrolling, z-order, transforms, and selection/focus outlines rendered as pixels.
- Frame animation scheduler integrated with host event loops.
- GPU texture readback screenshots for renderer snapshot tests.
