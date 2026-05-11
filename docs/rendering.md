# Rendering

The renderer crates are currently contract-rich and headless-real. `glyphspace-render` exposes host-neutral render primitives, deterministic command frames, scene diffs, scene patches, GPU pipeline plans, WGSL shader contracts, text atlas state, screenshot conformance, and nonblank pixel output for CI. `glyphspace-render-wgpu` adds the native swapchain presentation contract while preserving that command-frame path. The project does not yet provide a finished hardware-backed product renderer.

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

## What Is Real Today

- Command frames for dots, cards, text, graph edges, focus rings, and animation ticks.
- Stable primitive and command digests for renderer determinism checks.
- Scene batching and scene patch generation so hosts can apply incremental updates.
- GPU pipeline planning with WGSL shader contracts, vertex/index/instance buffer requirements, bind group names, and draw-call categories.
- Browser parity metadata mapping the native `wgpu` plan to browser WebGPU expectations.
- Deterministic screenshot conformance without requiring a physical GPU in CI.
- Headless pixel output that is nonblank, resizable, MSAA-aware, and digestible.
- Native swapchain presentation contract with resize, MSAA, present-mode metadata, WGSL pipeline contract, and draw-call stats.

## What Is Next

- Real native `wgpu` surface creation and hardware-backed swapchain presentation.
- Browser WebGPU renderer consuming the same command-frame contract.
- GPU atlas upload using the new `glyphspace-text` shaping/rasterization abstraction.
- GPU clipping, scrolling, z-order, transforms, and selection/focus outlines rendered as pixels.
- Frame animation scheduler integrated with host event loops.
- GPU texture readback screenshots for renderer snapshot tests.
