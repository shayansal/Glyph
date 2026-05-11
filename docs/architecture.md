# Architecture

Glyphspace is split into independent layers:

1. Capability layer: typed actions, permissions, risk, confirmation, and audit.
2. Semantic world graph: glyphs and semantic edges.
3. Spatial semantics: configurable meaning for axes, depth, focus, periphery, orbiting, urgency, and policy locks.
4. Layout: deterministic constraints, responsive placement, collision avoidance, focus and accessibility order.
5. Renderer: wgpu-oriented primitives with a headless preparation path.
6. Personalization: reversible patches layered over base manifests and lenses.
7. Policy: mandatory validation for patches, capabilities, trust surfaces, and accessibility.
8. AI contract: model-agnostic patch proposal interface.
9. Accessibility: semantic tree and web DOM mirror.
10. Rust frontend kernel: semantic components, signal state, typed capability handlers, app runtime, audit stream, and host contract.
11. CLI and SDK: developer tools and browser host.

## Rust Frontend Layer

`glyphspace-app` is the framework-facing layer. It deliberately does not create a virtual DOM. Components render `Glyph` values into the canonical `GlyphWorld`; state updates rebuild the semantic world and emit `SemanticDiff`; glyph input invokes typed capability handlers; policy validates authority before handlers can mutate state or patches can apply.

Hosts implement `SemanticHost`. A host renders the world, performs hit-testing, stores accepted patches, emits audit events, and maintains the accessibility mirror. The current `HeadlessSemanticHost` composes layout, render-core batching/diffs, and accessibility-tree validation so the framework contract is testable without a GPU.

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
