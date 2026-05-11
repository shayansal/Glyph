# Rendering

The renderer crate exposes wgpu-oriented preparation APIs and render primitives: dots, rounded rectangles, panels, text runs, edges, glows, shadows, and policy overlays. The first prototype keeps rendering separable and headless-testable.

The SOTA renderer tranche adds:

- `TextShaper::placeholder()` and `GlyphTextRun` as the explicit text shaping seam before adopting a full shaper.
- `SelectionStyle` for focus/selection outlines.
- `ScenePatch` derived from scene diffs so hosts can apply incremental updates.
- Stable render primitive keys based on glyph id and primitive kind.
- `NativeRendererHost::run_winit_window(...)`, which creates a `winit` event loop/window and drives redraw/resize through the same world/layout/render path. CI still uses headless rendering.
