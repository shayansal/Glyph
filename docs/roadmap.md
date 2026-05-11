# Roadmap

## Current Position

Glyphspace is between Phase 0 and Phase 1. The reference kernel exists and is tested across core schema/model, policy-safe patches, layout, accessibility, Rust app runtime, SSR contracts, renderer command frames, headless pixel output, a real native `winit` + `wgpu::Surface` presenter, text shaping/rasterization contracts, `gx dev` process-manager and polling watcher contracts, mobile project templates, and conformance reports. The project is not yet a product-ready native/web/mobile framework because the product host loops, polished devtools, package/release story, and mobile FFI/accessibility bridges are still incomplete.

## Phases

- Phase 0: Prototype. Mostly complete: canonical model, policy, layout, AI adapter, examples, CLI, WASM bridge, docs, and tests exist.
- Phase 1: Reference engine. In progress: harden canonical serialization, expand conformance fixtures, route native product frames through the surface presenter, improve Rust macro ergonomics, and publish crates.
- Phase 2: Product hosts. Build polished desktop `winit`/`wgpu`, browser WebGPU, no-JS Rust/WASM bootloader, and upgrade `gx dev` from polling/supervisor contracts to real child-process orchestration.
- Phase 3: Mobile host adapters. Generate iOS/Android projects, native accessibility bridges, touch gestures, offline patch store, push/deep-link hooks, and mobile layout profiles.
- Phase 4: XR host adapters. Add OpenXR/WebXR host contracts and spatial accessibility conventions.
- Phase 5: Conformance suite. Ship `gx conformance` as a formal certification tool for schemas, policy, patches, accessibility, host adapters, renderer determinism, and semantic SSR.
- Phase 6: Extension registry. Publish semantic component kits, domain kits, policy extensions, host adapters, and compatibility guarantees.
- Phase 7: Governance foundation. Establish RFC process, security/accessibility review, working group, extension registry governance, and reference/spec separation.
- Phase 8: Standards proposal. Move stable schemas, policies, accessibility rules, and host contracts toward an external standards process.

Future adapters include Web, desktop, iOS/macOS, Android, OpenXR, WebXR, game engines, and server-side semantic snapshot export.
