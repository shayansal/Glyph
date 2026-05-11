# Rust Frontend Kernel

Glyphspace is Rust-first. JSON is the canonical portable format, not the primary way developers should author applications.

The `glyphspace-app` crate is the Rust frontend kernel that sits above `glyphspace-core`:

- `AppRuntime<State>` owns application state, the canonical `GlyphWorld`, policy context, typed capability handlers, patch history, and audit events.
- `#[glyph_component]` and `component(|state| Vec<Glyph>)` render semantic state into glyphs. Components do not produce DOM nodes; they produce canonical world graph objects.
- `#[capability(...)]` keeps Rust handlers and capability manifests together. The macro emits a `*_manifest()` function that compiles into the canonical world.
- `#[glyph_app]` and `#[lens]` reserve the app/lens annotation surface for natural Rust authoring.
- `typed_capability::<Input, Output>("capability.id")` wraps serde-typed Rust handlers around capability manifests.
- `Signal<T>` provides small reactive state primitives for framework and host integration.
- `ReactiveGraph` adds dependency-tracked computed values, dirty component tracking, and `AsyncResource` adds pending/ready/failed/canceled states for host-managed async work.
- `SemanticHost` defines what a platform host must provide: render a world, hit-test input, store patches, and emit audit events.
- `HeadlessSemanticHost` uses the layout engine, renderer preparation path, scene batcher/diff, and accessibility tree so tests can exercise the same contract without a GPU window.
- `HostAdapterSpec`, `ConformanceHarness`, and `interop::FrameworkBridge` make host and framework integration explicit and testable.
- `PolicyStudio` explains accepted and rejected patch operations for devtools surfaces.
- `accessibility_frame()` turns each rendered frame into a verified accessibility frame with focus order and spatial descriptions.

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

## Current Limits

This is still a kernel, not a polished app framework. It now has the first macro, reactive, host, policy studio, conformance, interop, and accessibility-frame surfaces. The next layer should add router primitives, richer component composition, async executors, SSR/hydration for web hosts, and production text/layout/rendering.
