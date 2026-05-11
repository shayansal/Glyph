# Glyphspace

Glyphspace is an open-source reference prototype for AI-native spatial UI. Applications expose capabilities and semantic state; the engine turns that into a user-editable world of glyphs, validates personalization patches against policy, and renders a portable spatial surface.

This repository is not a React clone and the DOM is not the source of truth. The source of truth is a versioned semantic world graph made of glyphs, edges, capabilities, policy zones, lenses, and reversible patches.

## Why This Exists

Traditional software asks developers to ship fixed UI and asks users to adapt. Glyphspace flips the contract:

1. Developers expose typed capabilities and semantic data.
2. The engine compiles a semantic spatial UI.
3. Users personalize the surface with language, direct manipulation, and lenses.
4. Personalization is stored as reversible, inspectable patches.
5. Policy validation prevents unsafe or unauthorized changes.

The prototype is model-agnostic. The AI layer is an adapter contract plus a local rule-based implementation. It does not call proprietary LLM APIs.

## What Is Included

- Rust workspace with modular crates for app authoring, core, schema, policy, personalization, layout, AI, renderer, accessibility, CLI, and WASM.
- Versioned semantic schema types and JSON schema export.
- Dot/glyph world graph with semantic edges.
- Capability manifest and policy-safe binding validation.
- Reversible personalization patch system.
- Deterministic 2D, 2.5D, and basic 3D layout compiler.
- Headless wgpu renderer facade for testable render preparation.
- WASM bridge and TypeScript SDK/demo.
- Rust-first frontend kernel with semantic components, typed capability handlers, signal state, policy-gated runtime invocation, scene diffs, and a headless semantic host.
- TypeScript app integration layer with `defineGlyphApp`, `defineCapability`, `defineGlyph`, `defineLens`, host adapters, a runtime bridge, patch storage, and audit streaming.
- CRM/founder dashboard example with lenses.
- Accessibility semantic tree and DOM mirror in the web SDK.
- CLI for validate, compile, patch, explain, inspect, export-schema, and snapshot.

## Run

```bash
cargo test --workspace
cargo run -p glyphspace-cli -- validate examples/crm-dashboard/app.glyph.json
cargo run -p glyphspace-cli -- explain examples/crm-dashboard/founder.lens.glyph.json
cargo run -p crm-dashboard-rust
cargo run -p crm-dashboard-rust -- --export > /tmp/crm-dashboard-rust.glyph.json
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.121 --locked
./scripts/build-wasm.sh
cd web
npm install
npm test
npm run build
npm run dev
```

On Windows environments with the schannel revocation issue, set `CARGO_HTTP_CHECK_REVOKE=false` before Cargo commands.

The web SDK prefers the generated Rust/WASM kernel at `web/src/wasm/glyphspace_wasm.js` for policy, patch, and AI proposal operations. If the generated package is absent, the demo falls back to the local TypeScript policy adapter so frontend work can continue.

## App Integration Layer

Glyphspace is Rust-first. Apps can be authored directly in Rust and compile to `GlyphWorld`; `.glyph.json` is the portable export format, not the primary authoring experience. The `glyphspace-app` crate adds the framework layer above the DSL: semantic components, typed capability handlers, policy-gated runtime invocation, audit trails, scene diffs, and a host contract where accessibility is rendered alongside visuals.

```rust
use glyphspace_core::{Capability, Glyph, Priority, RiskLevel};
use glyphspace_dsl::GlyphApp;

let app = GlyphApp::new("crm_dashboard_rust", "Rust CRM Dashboard")
    .capability(
        Capability::builder("deal.update_stage", "Update Deal Stage")
            .permission("crm.deal.write")
            .risk(RiskLevel::Medium)
            .build(),
    )
    .glyph(Glyph::metric("revenue", "Revenue").priority(Priority::High))
    .glyph(Glyph::button("deal_northstar", "Northstar Health").binds("deal.update_stage"));

let world = app.compile()?;
```

For a live Rust frontend, mount the app with semantic components and typed capability handlers:

```rust
use glyphspace_app::{AppRuntime, CapabilityOutput, component, typed_capability};

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

The native Rust CRM example uses this path end to end: state renders glyphs, glyph clicks invoke typed capabilities, policy gates the invocation, results apply semantic patches, audit events are recorded, and a headless host renders both the visual scene and accessibility tree. See `examples/crm-dashboard-rust`.

Web apps can also be authored with the TypeScript DSL and compiled to `.glyph.json`-compatible world data:

```ts
import { defineCapability, defineGlyphApp, jsonSchema } from "@glyphspace/web";

const updateStage = defineCapability<{ deal_id: string; stage: string }, { deal_id: string; stage: string }>({
  id: "deal.update_stage",
  name: "Update Deal Stage",
  intent: "move a sales opportunity to a new pipeline stage",
  input_schema: jsonSchema({ type: "object" }),
  required_permissions: ["crm.deal.write"],
  risk: "medium",
});

const app = defineGlyphApp({
  id: "crm_dashboard",
  name: "CRM Dashboard",
  capabilities: [updateStage],
  glyphs: [{ id: "pipeline", kind: "surface", label: "Pipeline stages" }],
});
```

At runtime, a host adapter provides the render surface, input events, accessibility mirror, patch storage, policy context, capability invocation, device profile, and audit sink. The demo CRM data source invokes `deal.update_stage`, mutates local CRM state, returns a semantic patch, and streams an audit event into devtools. The developer-kernel conformance tests verify that DSL output is stable canonical `.glyph.json`, Rust CLI schema validation accepts it, WASM and local policy agree, permission gates block unauthorized capability invocation, audit is emitted, and accessibility mirror semantics survive personalization.

## Create A Capability

Capabilities describe what can be done, not how to draw a button:

```json
{
  "id": "deal.update_stage",
  "name": "Update Deal Stage",
  "intent": "move a sales opportunity to a new pipeline stage",
  "required_permissions": ["crm.deal.write"],
  "risk": "medium",
  "reversible": true,
  "requires_confirmation": false,
  "audit": true
}
```

## Create A Lens Or Patch

Lenses are normal patches layered over the base world:

```json
{
  "spec_version": "0.1.0",
  "id": "founder_lens",
  "description": "Prioritize revenue, runway, risk, and urgent decisions.",
  "ops": [
    { "type": "set_priority", "glyph_id": "revenue", "priority": "critical" },
    { "type": "collapse", "glyph_id": "admin_tasks" }
  ]
}
```

Validate and apply it:

```bash
cargo run -p glyphspace-cli -- validate examples/crm-dashboard/founder.lens.glyph.json
cargo run -p glyphspace-cli -- patch examples/crm-dashboard/app.glyph.json examples/crm-dashboard/founder.lens.glyph.json --out /tmp/founder-world.json
```

## Contributing

Glyphspace is dual licensed under MIT or Apache-2.0. Contributions should keep the core model portable, renderer separable, policy mandatory, AI model-agnostic, and accessibility semantics intact.
