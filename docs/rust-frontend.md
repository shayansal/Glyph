# Rust Frontend Kernel

Glyphspace is Rust-first. JSON is the canonical portable format, not the primary way developers should author applications.

The `glyphspace-app` crate is the Rust frontend kernel that sits above `glyphspace-core`:

- `AppRuntime<State>` owns application state, the canonical `GlyphWorld`, policy context, typed capability handlers, patch history, and audit events.
- `#[glyph_component]` and `component(|state| Vec<Glyph>)` render semantic state into glyphs. Components do not produce DOM nodes; they produce canonical world graph objects.
- `#[capability(...)]` keeps Rust handlers and capability manifests together. The macro emits a `*_manifest()` function that compiles into the canonical world.
- `#[glyph_app]` and `#[lens]` reserve the app/lens annotation surface for natural Rust authoring.
- `glyph!(...)` provides the first semantic markup layer for common glyphs, such as metrics and capability-bound buttons.
- `ComponentKit` provides reusable semantic primitives such as risk, confirmation, metric, and agent glyphs.
- `typed_capability::<Input, Output>("capability.id")` wraps serde-typed Rust handlers around capability manifests.
- `CapabilityFunctionRegistry` provides policy-audited semantic server functions that return patches instead of mutating UI directly.
- `SemanticRouter` routes URLs and host navigation events to worlds, lenses, glyph focus targets, camera positions, and accessibility landmarks.
- `SemanticSsrSnapshot` serializes the canonical world, accessibility tree, policy context, and digest for semantic hydration.
- `SemanticSsrServer` provides the first server-side path for accessibility HTML, capability-over-HTTP responses, and streaming world update events.
- `AxumSsrAdapter` exposes a real Axum/Tokio SSR router for world JSON, accessibility HTML, capability invocation, and server-sent world update streams. `serve_localhost()` starts an ephemeral local server for tests and development smoke checks.
- `MobileHostAdapter` describes native accessibility bridges, offline patch stores, and mobile lens profiles.
- `MobileShell` models concrete iOS/Android shells with native accessibility bridge metadata, offline patch queues, mobile lens profiles, and push/update channel hints.
- `NativeHostRuntime` tracks desktop/native input, focus traversal, mobile lens profiles, and offline patch storage.
- `Signal<T>` provides small reactive state primitives for framework and host integration.
- `ReactiveGraph` adds dependency-tracked computed values, dirty component tracking, and `AsyncResource` adds pending/ready/failed/canceled states for host-managed async work.
- `TypedSignal`, `Memo`, `ReactiveEffect`, and `SuspenseBoundary` provide the next typed reactive layer for fine-grained glyph invalidation.
- `FineGrainedRuntime` adds Leptos-style signal/memo/effect invalidation, suspense resources, error boundaries, and glyph-level world diffs that only mark changed semantic objects.
- `SemanticHost` defines what a platform host must provide: render a world, hit-test input, store patches, and emit audit events.
- `HeadlessSemanticHost` uses the layout engine, renderer preparation path, scene batcher/diff, and accessibility tree so tests can exercise the same contract without a GPU window.
- `HostAdapterSpec`, `ConformanceHarness`, and `interop::FrameworkBridge` make host and framework integration explicit and testable.
- `PolicyStudio` explains accepted and rejected patch operations for devtools surfaces.
- `DevtoolsStudio` captures a live world graph inspector, glyph inspector, policy explanation, accessibility frame, layout debugger, and audit stream into one replayable frame.
- `AiPersonalizationSession` provides a rule-based local personalization preview with accepted safe patches, undo patches, rejected operations, and policy explanations.
- `HostCertificationSuite` and `InteropEmbedSurface` certify web/native/mobile hosts and describe how Dioxus/Yew/Leptos-style DOM hosts can embed Glyphspace while Glyphspace owns semantic UI and accessibility mirrors.
- `accessibility_frame()` turns each rendered frame into a verified accessibility frame with focus order and spatial descriptions.
- `HotReloadEngine`, `PatchTimeline`, `DevtoolsReplay`, and `SemanticConformanceSuite` turn development, unsafe proposal replay, and standard certification into executable contracts. Hot reload can now watch manifest and patch files, reload changed content, preserve runtime state, emit semantic diffs, and stream devtools batch events.

## Why This Can Beat DOM-First Rust Frameworks

Yew and similar Rust web frameworks are excellent at compiling Rust components to DOM UI. Glyphspace is aiming at a different layer: a semantic UI runtime that can target web, native, mobile, AR, and VR from the same world graph.

The source of truth is:

```text
Rust app state -> semantic components -> GlyphWorld -> policy/layout -> visual renderer + accessibility renderer
```

That lets Glyphspace offer capabilities a virtual-DOM framework cannot make native to its model:

- AI personalization can rearrange UI while policy prevents new authority.
- Capability invocation is a first-class typed contract, not an event callback convention.
- Accessibility is rendered from the same semantic graph as visuals.
- Layout, scene diffs, patch diffs, and audit events are stable conformance surfaces.
- The same app can export `.glyph.json` for web/WASM compatibility without making JSON the authoring language.

## Minimal Pattern

```rust
let deal = glyph!(button("deal", "Deal").binds("deal.update_stage"));

#[glyph_component]
fn stage_component(state: &CrmState) -> Vec<Glyph> {
    vec![Glyph::metric("stage_status", format!("Stage: {}", state.stage))]
}

let mut runtime = AppRuntime::new(app, state, policy_context)
    .with_component(component(stage_component))
    .mount()?;

runtime.register_typed(
    typed_capability::<UpdateStageInput, UpdateStageOutput>("deal.update_stage"),
    |state, input, _world| {
        state.stage = input.stage;
        Ok(CapabilityOutput::new(UpdateStageOutput { stage: state.stage.clone() }))
    },
);
```

## Dioxus Parity Target

Glyphspace should match Dioxus on ergonomics and tooling while moving the source of truth above DOM nodes:

- Dioxus has `rsx!`; Glyphspace needs `glyph!` and semantic component macros.
- Dioxus has typed routing; Glyphspace routes to lenses, glyph focus, camera positions, and accessibility landmarks.
- Dioxus has server functions; Glyphspace has policy-audited capability functions that return semantic patches.
- Dioxus has SSR/hydration; Glyphspace hydrates canonical worlds, accessibility frames, policy context, and patch digests.
- Dioxus has `dx`; Glyphspace has `gx` for semantic scaffolding, dev preflight, policy inspection, export, and conformance.

## Current Limits

This is still a kernel, not a polished app framework. It now has macro, fine-grained reactive, host, policy studio, conformance, interop, accessibility-frame, watched hot-reload, live Axum/Tokio SSR, production renderer command frames, GPU pipeline plans, screenshot conformance, devtools replay, AI personalization previews, and mobile shell bridge frames. The next layer should add platform file notification backends, real GPU text rasterization, authenticated SSR capability sessions, and native iOS/Android project templates.
