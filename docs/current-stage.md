# Current Stage

Glyphspace is currently a reference-kernel prototype with a Rust-first framework surface. It is no longer just a set of JSON fixtures or a canvas demo: the repository contains executable Rust contracts for semantic app authoring, policy-safe personalization, layout, rendering preparation, accessibility, SSR, conformance, and developer workflows.

## Implemented And Tested

- Canonical `GlyphWorld` runtime model with glyphs, edges, capabilities, policies, lenses, accessibility nodes, patches, serialization, layout hashing, and semantic diffs.
- Policy engine for world validation, patch validation, capability invocation, trust-surface visibility, focus order, and accessibility preservation.
- Personalization system with reversible patches, explanations, inversion, merge/conflict surfaces, and policy rejection paths.
- Deterministic layout compiler for 2D, 2.5D, and basic 3D placement with focus order, hit-test maps, accessibility order, reduced-motion mode, and mobile profiles.
- Rust app kernel with builders, macros, semantic components, typed capability handlers, reactive primitives, async resource states, audit events, host contracts, and runtime state bridges.
- Renderer command-frame stack with render primitives, scene batches, scene diffs, scene patches, GPU pipeline plans, WGSL shader contracts, screenshot conformance, and deterministic headless pixel output.
- Native `wgpu` swapchain presentation contract in `glyphspace-render-wgpu`, preserving command frames while exposing surface configuration, MSAA, draw-call summaries, resize, and presentation stats in headless contract mode.
- Real `WinitWgpuSurfacePresenter` binding from `winit::window::Window` to `wgpu::Surface`, adapter/device/queue setup, swapchain configuration, render-pipeline creation, surface texture presentation, and readback-capable surface usage.
- Product renderer contracts for routing command frames through the native surface presenter, GPU glyph upload plans, command-frame raster snapshots for cards/dots/edges/text/focus rings, and browser WebGPU parity reports.
- Hardware renderer encoding contracts that turn command frames into deterministic vertex, index, instance, uniform, and text-atlas byte payloads, plus draw-pass partitions for cards/panels, dots/glows, edges, text, and focus/policy overlays.
- Text shaping/rasterization abstraction in `glyphspace-text`, including font fallback selection, DPI-aware shaping, clipped atlas output, glyph cache keys, and cache hit/miss stats.
- Rich text shaping metadata for fallback fonts, emoji, RTL scripts, ligature detection, and word wrapping.
- Long-running development process model in `glyphspace-dev`, wired into `gx dev` for target orchestration, watcher/SSR/browser/native status, diagnostics, devtools heartbeat, and state preservation.
- Development supervisor and polling file watcher that parse project config, classify Rust/glyph/lens/policy/schema/asset changes, plan incremental reloads, preserve state, report process health, and generate crash recovery diagnostics.
- Real dev command execution for rebuild commands, safe SSR restart with preserved state snapshots, native notification backend contracts, live watcher stream batches, process orchestration reports, and compiler diagnostic parsing.
- Production kernel conformance scaffolding for invalid world, patch, policy, and layout fixtures, plus an API stability report for public Rust types/functions, feature flags, extension roots, semver guarantees, and error-code coverage.
- Accessibility renderer that turns semantic worlds and render frames into accessible nodes, focus order, spatial descriptions, and web DOM mirror data.
- Axum/Tokio SSR adapter for world JSON, accessibility HTML, capability POST, and server-sent world update routes.
- `gx` CLI for scaffolding, dev preflight/report artifacts, policy explanation, export, and conformance reports.
- CRM examples and conformance tests covering policy rejection, capability gates, audit events, accessibility preservation, and render determinism.

## CI-Real / Headless-Real

Several subsystems are intentionally product-shaped but still headless or contract-first:

- `ActualGpuRenderer` produces deterministic nonblank pixels, tracks text atlas state, honors MSAA/resizing, and allocates wgpu-style buffers. The native presenter can now present command frames through a real `wgpu::Surface`, `NativeProductAppLoop` routes product frames to that presenter contract, and `HardwareGlyphPipeline` encodes command frames into deterministic GPU byte payloads. Binding those payloads into per-glyph hardware shader draws is still maturing.
- `gx dev` now uses a long-running process manager model plus a concrete polling fingerprint watcher, native notification backend contract, live watcher stream, command executor, process orchestrator, and compiler diagnostic parser. Normal `gx dev` stays alive and emits heartbeat/devtools events; `--report` and `--once` provide finite bootstrap paths for CI. The next step is wiring the native notification backend to a real OS event source and managing long-running child restarts.
- Mobile host work now includes generated iOS Swift Package and Android Gradle project files with runtime bridge stubs, but not Rust FFI packaging or native accessibility adapters.
- Devtools have frame models, replay data, policy explanations, and timelines, but not a finished inspector UI.

## Not Yet Product-Real

- Product native app loop using the hardware `wgpu` surface presenter, plus browser WebGPU parity.
- Full hardware-backed text rendering with real font engines, GPU atlas upload, scrolling, IME, and advanced text input.
- Production native window lifecycle: menus, clipboard, drag/drop, file dialogs, notifications, packaging, and installers.
- Authenticated SSR sessions, database-backed examples, deployment templates, and production capability RPC.
- Generated native iOS/Android projects with native accessibility bridges and push/deep-link integration.
- Published crate/npm release pipeline, registry story, and CI matrix across platforms.

## Strategic Direction

Glyphspace is trying to eliminate JavaScript as the application authoring layer, not by pretending browsers need no glue, but by moving the source of truth into Rust:

```text
Rust state -> semantic components -> GlyphWorld -> policy/layout -> visual renderer + accessibility renderer
```

The durable differentiator is policy-safe AI personalization: AI can rearrange a user's UI, suggest lenses, collapse noise, or emphasize urgent work, but it cannot create authority, bypass confirmation, hide mandatory trust surfaces, or remove accessibility semantics.
