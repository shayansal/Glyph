# Rust Frontend Kernel

Glyphspace is Rust-first. JSON is the canonical portable format, not the primary way developers should author applications.

The `glyphspace-app` crate is the Rust frontend kernel that sits above `glyphspace-core`:

- `AppRuntime<State>` owns application state, the canonical `GlyphWorld`, policy context, typed capability handlers, patch history, and audit events.
- `component(|state| Vec<Glyph>)` renders semantic state into glyphs. Components do not produce DOM nodes; they produce canonical world graph objects.
- `typed_capability::<Input, Output>("capability.id")` wraps serde-typed Rust handlers around capability manifests.
- `Signal<T>` provides small reactive state primitives for framework and host integration.
- `SemanticHost` defines what a platform host must provide: render a world, hit-test input, store patches, and emit audit events.
- `HeadlessSemanticHost` uses the layout engine, renderer preparation path, scene batcher/diff, and accessibility tree so tests can exercise the same contract without a GPU window.

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
let mut runtime = AppRuntime::new(app, state, policy_context)
    .with_component(component(|state: &CrmState| {
        vec![Glyph::metric("stage_status", format!("Stage: {}", state.stage))]
    }))
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

This is still a kernel, not a polished app framework. It does not yet include proc macros, router primitives, async resources, SSR/hydration, or a full native event loop host. Those should come after the semantic runtime contract stays stable under conformance tests.
