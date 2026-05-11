# Native Rust App Guide

Native hosts use the same canonical `GlyphWorld` as web and mobile hosts.

The current native path is:

1. Rust app state renders semantic glyphs.
2. Layout resolves poses, hit regions, focus order, and accessibility order.
3. The renderer emits command frames and GPU pipeline plans.
4. The native host can launch a `winit` window and use `wgpu` resources.
5. Accessibility and policy frames remain testable without a GPU.
