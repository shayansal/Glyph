# Rendering

The renderer crate exposes wgpu-oriented preparation APIs and render primitives: dots, rounded rectangles, panels, text runs, edges, glows, shadows, and policy overlays. The first prototype keeps rendering separable and headless-testable.

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
