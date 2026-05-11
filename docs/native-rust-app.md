# Native Rust App Guide

Native hosts use the same canonical `GlyphWorld` as web and mobile hosts.

The current native path is:

1. Rust app state renders semantic glyphs.
2. Layout resolves poses, hit regions, focus order, and accessibility order.
3. The renderer emits command frames and GPU pipeline plans.
4. The native host can launch a `winit` window and use `wgpu` resources.
5. Accessibility and policy frames remain testable without a GPU.

## Current Stage

The native path is credible as a framework contract and test harness. Rust-authored apps can compile to `GlyphWorld`, invoke typed capabilities, emit semantic patches and audit events, produce layout/render/accessibility diffs, and run through headless host validation. Renderer output is deterministic and pixel-producing in CI, and `glyphspace-render-wgpu` now defines the swapchain presentation contract for surface size, MSAA, present mode, draw calls, and resize behavior. It also includes `WinitWgpuSurfacePresenter`, which creates a real `wgpu::Surface` from a `winit` window, configures a render/readback-capable swapchain, builds a render pipeline, and presents surface textures. The next milestone is routing the full product native app loop through that presenter.

## Product-Grade Native Work Remaining

- Product app-loop integration with the real `wgpu` surface presenter.
- Window lifecycle, menus, clipboard, drag/drop, dialogs, notifications, storage, and installer packaging.
- Text input, IME, focus traversal, keyboard navigation, screen reader bridges, and native accessibility APIs.
- GPU screenshot readback and visual snapshot conformance.
- `gx dev --native` as a long-running process with rebuilds, state preservation, diagnostics overlay, and devtools stream.
