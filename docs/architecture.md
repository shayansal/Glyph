# Architecture

Glyphspace is split into independent layers. The current repository implements the reference kernel and contract surfaces for these layers; some host implementations are still headless or scaffolded rather than product-grade.

1. Capability layer: typed actions, permissions, risk, confirmation, and audit.
2. Semantic world graph: glyphs and semantic edges.
3. Spatial semantics: configurable meaning for axes, depth, focus, periphery, orbiting, urgency, and policy locks.
4. Layout: deterministic constraints, responsive placement, collision avoidance, focus and accessibility order.
5. Renderer: host-neutral command frames, scene diffs, GPU pipeline plans, and headless pixel output.
6. Personalization: reversible patches layered over base manifests and lenses.
7. Policy: mandatory validation for patches, capabilities, trust surfaces, and accessibility.
8. AI contract: model-agnostic patch proposal interface.
9. Accessibility: semantic tree and web DOM mirror.
10. Rust frontend kernel: semantic components, signal state, typed capability handlers, app runtime, audit stream, and host contract.
11. CLI, SDK, and conformance: developer tools, browser host glue, SSR routes, and executable standard checks.

## Current Maturity

- Canonical kernel: implemented and tested. `GlyphWorld`, capabilities, policy, patches, layout, accessibility, serialization, semantic diffs, and CLI validation are stable enough to be treated as the reference implementation surface.
- Rust frontend kernel: implemented as a usable framework layer. Apps can be authored in Rust, rendered into semantic glyph worlds, invoke typed capabilities, emit audit events, and export portable `.glyph.json`.
- Renderer: contract-rich and CI-real with first native hardware binding. Command frames, scene patches, GPU pipeline plans, WGSL contracts, text atlas state, deterministic screenshots, headless pixel output, a real `winit` + `wgpu::Surface` presenter, product-loop routing, GPU upload plans, and command-frame raster snapshots exist. Deeper hardware primitive execution and full font rendering are next.
- Web: Rust/WASM-preferred for canonical validation and patch operations, with TypeScript kept as SDK/demo/browser glue.
- Native/mobile: host contracts, window-runner hooks, runtime state, and mobile bridge frames exist. Full desktop/mobile shell maturity remains future work.

## Rust Frontend Layer

`glyphspace-app` is the framework-facing layer. It deliberately does not create a virtual DOM. Components render `Glyph` values into the canonical `GlyphWorld`; state updates rebuild the semantic world and emit `SemanticDiff`; glyph input invokes typed capability handlers; policy validates authority before handlers can mutate state or patches can apply.

Hosts implement `SemanticHost`. A host renders the world, performs hit-testing, stores accepted patches, emits audit events, and maintains the accessibility mirror. The current headless and contract hosts compose layout, render-core batching/diffs, render patches, accessibility-tree validation, and runtime state bridges so the framework contract is testable without a product GPU window.

## Dioxus-Parity Platform Layer

Glyphspace now has the first APIs for the platform features expected from a modern Rust app framework:

- `gx` workflow commands for scaffolding, development preflight, policy inspection, export, and conformance.
- `glyph!(...)` semantic authoring and `ComponentKit` primitives.
- `SemanticRouter` for URL/deep-link routing into worlds, lenses, glyph focus, camera, and accessibility landmarks.
- `CapabilityFunctionRegistry` for policy-audited server-function-style capability execution.
- `SemanticSsrSnapshot` for world/accessibility/policy hydration.
- `MobileHostAdapter` for iOS/Android host contracts.
- `DevtoolsSnapshot` for world, capability, policy, audit, and accessibility inspection.

## True-SOTA Execution Layer

The latest execution layer deepens those contracts:

- `ProductionRenderer` and `RenderSnapshot` certify deterministic render frames.
- `HotReloadEngine` reloads manifests and patches while preserving state and emitting devtools events.
- `SemanticSsrServer` exposes accessibility HTML, capability HTTP responses, and world update streams.
- `TypedSignal`, `Memo`, `ReactiveEffect`, and `SuspenseBoundary` support typed reactive invalidation.
- `NativeHostRuntime` models input events, focus traversal, offline patch storage, and mobile lens profiles.
- `PatchTimeline` and `DevtoolsReplay` make unsafe AI proposal replay inspectable.
- Domain kits for CRM, finance, workflows, admin/security, agents, and dashboards provide semantic primitives.
- `SemanticConformanceSuite` certifies renderer determinism, policy, accessibility, host, and patch compatibility invariants.

## Product Gaps

The architecture intentionally separates contracts from host polish. The main remaining product gaps are deeper hardware rendering of every glyph primitive, full font shaping/rasterization with a mature shaper, OS-backed file watcher streams and multi-child hot reload orchestration, authenticated SSR sessions, generated mobile projects, native accessibility bridge implementations, and a polished live devtools UI.
