# Glyphspace

Glyphspace is a Rust-first, AI-native UI substrate. Applications expose typed capabilities and semantic state; Glyphspace turns that into a canonical world graph of glyphs, validates user or AI personalization through policy, lays it out spatially, and renders both visual and accessibility frames.

This is not a React clone and the DOM is not the source of truth. The source of truth is a versioned `GlyphWorld`: glyphs, semantic edges, capabilities, lenses, policy zones, accessibility nodes, and reversible patches.

## Current Stage

Glyphspace is currently a reference-kernel prototype with strong executable contracts. The core model, policy engine, patch system, layout engine, CLI, Rust app runtime, conformance checks, SSR adapter, accessibility mirror, and headless renderer paths are implemented and tested. The project is now past "JSON demo" territory: Rust apps can author semantic UIs directly and export `.glyph.json` only as a portable interchange format.

The renderer is not yet a finished product GPU host. It has real command frames, scene diffs, GPU pipeline plans, WGSL contracts, MSAA/resizing configuration, text-atlas state, deterministic nonblank pixel output, screenshot conformance, and wgpu-style buffers in CI/headless mode. The native `glyphspace-render-wgpu` path now also binds that resource/pass/readback contract to an actual `wgpu::Surface` created from a `winit` window. The next credibility jump is routing the full native app loop through that hardware presenter, then adding full font shaping/rasterization, polished browser WebGPU parity, and production devtools UI.

The web target keeps JavaScript as distribution glue only where the browser requires it today. Canonical policy, patch validation, AI patch proposal, and semantic world operations prefer the Rust/WASM kernel. Long term, Glyphspace is aiming for Rust-authored web apps with WebGPU rendering, Rust event/capability/state flow, Rust SSR hydration, and a DOM accessibility mirror generated from Rust.

## Why This Exists

Traditional software asks developers to ship fixed UI and asks users to adapt. Glyphspace flips the contract:

1. Developers expose typed capabilities and semantic data.
2. The engine compiles a semantic spatial UI.
3. Users personalize the surface with language, direct manipulation, and lenses.
4. Personalization is stored as reversible, inspectable patches.
5. Policy validation prevents unsafe or unauthorized changes.

The AI layer is model-agnostic. The prototype includes a local rule-based adapter and does not call proprietary LLM APIs.

## What Works Today

- Rust workspace with modular crates for core, schema, policy, personalization, layout, AI, renderer, accessibility, app runtime, CLI, WASM, and macros.
- Canonical `GlyphWorld` model with stable serialization, semantic diffs, schema export, stable layout hashes, and conformance fixtures.
- Capability manifests with permissions, risk levels, confirmation requirements, audit requirements, and policy-safe binding validation.
- Reversible personalization patches with validation, explanation, inversion, merge/conflict surfaces, and policy enforcement.
- Deterministic 2D, 2.5D, and basic 3D layout with hit maps, focus order, accessibility order, reduced-motion and mobile profiles.
- Rust-first app kernel with builders, `glyph!(...)`, proc macros, semantic components, typed capability handlers, reactive state, async resource states, policy-gated runtime invocation, scene diffs, audit trails, and host contracts.
- Runtime state bridge from server/app data changes to semantic diffs, layout diffs, render patches, accessibility diffs, and audit events.
- Renderer contracts for command frames, scene patches, GPU pipeline planning, text atlas state, deterministic screenshots, and headless pixel output.
- `glyphspace-render-wgpu` with a native swapchain presentation contract plus an actual `winit` + `wgpu::Surface` presenter that configures swapchain usage, render passes, resources, presentation, and screenshot readback bindings.
- `glyphspace-text` with shaping, fallback selection, clipped raster atlas output, DPI scaling, and cache statistics.
- Renderer production contracts for render-pass/resource allocation, text atlas uploads, browser WebGPU parity, deterministic screenshot readback, command hit testing, draw state, and 1k/10k/100k benchmark reports.
- `glyphspace-dev` with a long-running `gx dev` process manager model, project config parsing, supervisor health reports, polling fingerprint watcher, incremental reload plans, crash recovery diagnostics, devtools heartbeat, and state preservation.
- Axum/Tokio SSR adapter for world JSON, accessibility HTML, capability POST, and streamed world updates.
- WASM bridge and web SDK/demo with Rust/WASM-preferred policy and patch operations.
- `gx` developer workflow for scaffolding, dev preflight/reporting, policy explanation, target export, and conformance reports.
- Production-kernel contracts for frozen `GlyphWorld` fields, schema migrations, feature flags, extension namespace validation, formal error codes, compatibility reports, and performance budgets.
- Developer experience commands for `gx add component`, `gx add capability`, `gx add lens`, `gx doctor`, `gx fmt`, `gx schema check`, and host certification reports.
- CRM/founder dashboard examples in portable JSON and Rust-authored form.
- Accessibility tree generation, accessibility frame verification, and DOM mirror support in the web SDK.
- CI metadata, tutorial docs, host adapter docs, conformance docs, and governance/standard drafts.

## Maturity Map

| Area | Current status | Next product-grade step |
| --- | --- | --- |
| Core world graph | Implemented and tested | Broaden compatibility fixtures and semantic diff corpus |
| Policy and patches | Implemented and tested | Richer enterprise policy language and visual Policy Studio |
| Rust authoring | Usable kernel APIs and macros | Polish macro grammar, diagnostics, component library |
| Reactivity | Fine-grained kernel exists | Executor-integrated async resources and ergonomic suspense |
| Layout | Deterministic and testable | More constraints, virtualization, advanced responsive policies |
| Renderer | Command-frame/headless pixel plus real `winit`/`wgpu::Surface` binding | Drive the product native app loop through the hardware presenter |
| Web/WASM | Rust/WASM kernel plus thin JS glue | No-JS app authoring, Rust bootloader, WebGPU host parity |
| SSR | Axum adapter and tested routes | Auth/session policy context and deployment templates |
| Native desktop | Host contracts, window-runner hooks, and surface presenter | Product window lifecycle, IME, menus, clipboard, installers |
| Mobile | iOS/Android project templates and runtime bridge stubs | Native accessibility bridge and Rust FFI packaging |
| Devtools | Snapshot/replay/report data models | Polished live inspector UI and performance flamegraph |
| Ecosystem | CRM/finance/workflow/admin/agent/dashboard kits | Published registries and compatibility guarantees |

## Run

```bash
cargo test --workspace
cargo run -p glyphspace-cli -- validate examples/crm-dashboard/app.glyph.json
cargo run -p glyphspace-cli -- explain examples/crm-dashboard/founder.lens.glyph.json
cargo run -p glyphspace-cli --bin gx -- conformance --world examples/crm-dashboard/app.glyph.json --out target/conformance.json
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

The web SDK prefers the generated Rust/WASM kernel at `web/src/wasm/glyphspace_wasm.js` for policy, patch, and AI proposal operations. If the generated package is absent, the demo falls back to the local TypeScript adapter so frontend work can continue.

## Rust App Authoring

Glyphspace is Rust-first. Apps can be authored directly in Rust and compiled to `GlyphWorld`; `.glyph.json` is the portable export format, not the primary authoring experience.

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

For a live Rust frontend, mount semantic components and typed capability handlers:

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

Proc macro authoring is available for the ergonomic layer:

```rust
#[glyph_component]
fn stage_component(state: &CrmState) -> Vec<Glyph> {
    vec![Glyph::metric("stage", format!("Stage: {}", state.stage))]
}

#[capability(id = "deal.update_stage", name = "Update Deal Stage", permission = "crm.deal.write", risk = "medium")]
fn update_stage(state: &mut CrmState, input: UpdateStageInput) -> UpdateStageOutput {
    state.stage = input.stage;
    UpdateStageOutput { stage: state.stage.clone() }
}
```

Start a new semantic Rust app:

```bash
cargo run -p glyphspace-cli --bin gx -- new crm_semantic
cargo run -p glyphspace-cli --bin gx -- dev --native --web --watch --ssr --browser --report target/gx-dev.json
cargo run -p glyphspace-cli --bin gx -- dev --native --web --watch --ssr --browser
cargo run -p glyphspace-cli --bin gx -- conformance --world examples/crm-dashboard/app.glyph.json --out target/conformance.json
```

## Web And TypeScript

The long-term direction is Rust-authored, no-JS application logic. The current repository still includes a TypeScript SDK/demo because browsers require some glue and because it is useful for distribution and inspection. That layer is intentionally thin: it loads world data, mounts canvas/WebGPU surfaces, mirrors accessibility into DOM, and delegates canonical validation to Rust/WASM when available.

Web apps can also be authored with the TypeScript DSL and compiled to `.glyph.json`-compatible world data, but this is no longer the strategic center of the framework.

## Capabilities And Patches

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

Validate and apply a patch:

```bash
cargo run -p glyphspace-cli -- validate examples/crm-dashboard/founder.lens.glyph.json
cargo run -p glyphspace-cli -- patch examples/crm-dashboard/app.glyph.json examples/crm-dashboard/founder.lens.glyph.json --out /tmp/founder-world.json
```

## Documentation

- [Current stage](docs/current-stage.md)
- [Architecture](docs/architecture.md)
- [Rust frontend kernel](docs/rust-frontend.md)
- [Rendering](docs/rendering.md)
- [Native Rust app guide](docs/native-rust-app.md)
- [Web/WASM app guide](docs/web-wasm-app.md)
- [Host adapter guide](docs/host-adapter-guide.md)
- [Conformance guide](docs/conformance-guide.md)
- [Policy-safe AI personalization](docs/policy-safe-ai-personalization.md)
- [Roadmap](docs/roadmap.md)
- [Production grade checklist](docs/production-grade-checklist.md)
- [Standard draft](docs/standard.md)
- [Governance](docs/governance.md)

## Contributing

Glyphspace is dual licensed under MIT or Apache-2.0. Contributions should keep the core model portable, renderer separable, policy mandatory, AI model-agnostic, Rust authoring first-class, and accessibility semantics intact.
