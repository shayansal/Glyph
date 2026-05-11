# Production Grade Checklist

This checklist tracks the full production-readiness list. Status values:

- `implemented`: executable code/tests exist in the repository.
- `contract`: stable contract or scaffold exists, but product host/platform work remains.
- `next`: not yet implemented beyond docs/plans.

## 1. Kernel Hardening

- Freeze canonical `GlyphWorld` runtime model: `implemented` via `ProductionKernelContract::v0_1`.
- Canonical serialization across versions/platforms: `implemented` via canonical JSON and digest tests.
- Expanded semantic diff coverage: `implemented` for glyph fields, capabilities, edges, policies, spatial semantics, and metadata.
- Exhaustive patch merge/conflict detection: `implemented` for all current `PatchOp` conflict keys; needs more fixtures as new ops are added.
- Property tests for reversible patch ops: `implemented` for world-aware inverse core path; needs expansion to every op variant.
- Fuzz/property tests for invalid worlds/patches/policies/layout: `partial`; `InvalidFixtureCorpus::production` now covers invalid world, patch, policy, and layout fixtures with expected formal error codes. Property/fuzz runners are still next.
- Version every schema and migration path: `implemented` for the first registry and `0.0.9 -> 0.1.0` migration path.
- Backwards/forwards compatibility tests: `implemented` for known old and unsupported future versions.
- Stabilize public Rust APIs: `partial`; `ApiStabilityReport::v0_1` declares the first public type/function surface, feature flags, extension roots, semver promises, and formal error-code coverage.
- Feature flags and extension namespace rules: `implemented`.
- Formal error codes: `implemented` with `FormalErrorCode`.
- Performance budgets: `implemented` with `KernelPerformanceBudget`.

## 2. Real Renderer

- Real `wgpu::Surface` presentation: `partial`; `WinitWgpuSurfacePresenter` creates a real surface from `winit`, configures swapchain usage, builds a render pipeline, presents surface textures, and exposes readback bindings. `NativeProductAppLoop` now routes command frames to that presenter contract; full interactive product window polish remains.
- Native window renderer end to end: `partial`; window-runner hooks, hardware presenter, and product-loop routing exist, but production interaction and OS lifecycle work remains.
- Browser WebGPU command-frame renderer: `partial`; browser presenter and `BrowserWebGpuParityReport` consume the same command-frame contract and track Rust-owned DOM accessibility mirror expectations.
- GPU buffers, bind groups, pipelines, uniforms, texture uploads: `partial`; resource accounting, upload plans, encoded vertex/index/instance/uniform/text-atlas byte payloads, render-pass plans, a real surface render pipeline, `WinitWgpuSurfacePresenter::bind_hardware_pipeline`, shader input layouts, and indexed draw plans now exist. Mature per-primitive hardware shader pipelines are next.
- Cards, panels, dots, edges, focus rings, glows, overlays as pixels: `partial`; `FrameRasterizer` now draws command-frame cards/dots/edges/text/focus pixels for deterministic snapshots, and `HardwareGlyphPipeline` partitions hardware draw passes for cards/panels, dots/glows, edges, text, and focus/policy overlays. Hardware shader parity still remains.
- `glyphspace-text` atlas integration: `implemented` at upload-contract level.
- Mature font shaping: `partial`; rich shaping now tracks fallback, emoji, RTL, ligature, and wrapping metadata using a deterministic prototype engine.
- Font fallback, emoji, RTL, ligatures, line breaking, wrapping: `partial`; executable shaping contract exists, mature shaper integration remains.
- Clipping, scrolling, z-order, transforms, opacity, masks: `partial` through `WgpuDrawState`.
- Resize, DPI, MSAA, frame pacing, animation scheduler: `partial`.
- Render-state hit testing: `implemented` for command-frame hit regions.
- Screenshot readback and visual snapshots: `partial`; deterministic headless and command-frame raster snapshots exist and native surfaces are configured for readback-capable presentation.
- Renderer benchmarks: `implemented` for 1k, 10k, and 100k glyph scenarios.

## 3. `gx dev`

- Real supervisor: `partial`; long-running manager, project config parsing, health reports, reload planning, friendly diagnostics, crash recovery plans, command execution, and finite CI paths exist.
- Watch Rust, glyph, lens, policy, schema, assets: `partial`; polling fingerprint watcher detects/classifies changes, `DevNotificationBackend::native` defines the OS notification backend contract, `LiveWatcherStream` converts native notifications into semantic reload batches, and `NativeOsWatcherBridge` turns OS create/modify/remove/rename events into reload batches. Wiring this bridge to the long-running platform watcher event source is next.
- Rebuild native/WASM, restart SSR: `partial`; `DevCommandExecutor` runs rebuild commands, `DevProcessSupervisor` restarts SSR safely with preserved state snapshots, and `DevOrchestrator` bootstraps supervised native/WASM/SSR process reports.
- Preserve state: `implemented` at manager/session level.
- Diagnostics/devtools stream: `implemented` at event/report level.
- Auto-open browser/native window: `contract`.
- Friendly errors: `partial`; new DX commands emit friendly errors and `CompilerDiagnosticParser` extracts Rust/compiler-style errors into devtools diagnostics. Deeper schema/policy source maps remain.
- Crash recovery/incremental reload: `partial`; recovery/reload plans, SSR restart execution, live reload batches, orchestration reports, and `LongRunningDevSupervisor` restart/heartbeat reporting exist. Wiring this into the actual `gx dev` loop is next.
- `--native`, `--web`, `--mobile`, `--ssr`, `--all`: `implemented` at process-manager target selection level.
- Project config/logs/traces/profiling/health: `partial`; config parsing and health reports exist, live process telemetry next.

## 4. Developer Experience

- `glyph!` grammar and macro diagnostics: `partial`; component props/slots/events/lifecycle contracts now complement existing macro grammar.
- Templates for dashboard/CRM/mobile/SSR/blank: `partial`; release/docs surfaces are tracked, more runnable templates remain.
- `gx add component/capability/lens`: `implemented`.
- `gx doctor`: `implemented`.
- `gx fmt`: `implemented` for JSON glyph/lens/policy files.
- `gx schema check`: `implemented`.
- `gx conformance --certify-host`: `implemented`.
- VS Code extension/language server/syntax/inline diagnostics: `contract`.
- Teachable Rust examples/docs/migrations/changelog: `partial`.

## 5. Component System

- Stable semantic component APIs: `partial`.
- Props/slots/children/typed events/lifecycle: `implemented` for product component contracts.
- Async resources, suspense, error boundaries: `contract`.
- Forms, tables, lists, menus, dialogs, command palette, tabs, nav: `partial`; accessible form/table/list/menu/dialog/nav primitives exist, command palette/tabs need fuller behavior.
- Accessible defaults and keyboard behavior: `partial`; production default keyboard map exists.
- Layout primitives and domain kits: `contract`.

## 6. State And Runtime

- Fine-grained reactive graph: `contract`.
- Derived memos/effects/resources/suspense/error boundaries: `contract`.
- Glyph-level invalidation: `contract`.
- World diff -> layout diff -> render diff -> accessibility diff: `implemented`.
- Transactions, undo/redo, patch persistence, offline queues: `partial`; first-class transaction, undo/redo, and in-memory offline queue contracts exist.
- Audit streaming, typed capability RPC, server sync: `partial`.
- Conflict resolution for server/client/user/AI patches: `partial`; sync conflict reports and resolution options exist.

## 7. Policy

- Formal policy language: `partial`; a rule parser and enterprise layering model exist.
- Mandatory trust surfaces, permission gates, risk/confirmation/audit: `implemented`.
- Enterprise policy contexts and layered policy: `partial`; org/role/user/session layering is modeled.
- Human explanations, simulator, Policy Studio, unsafe replay: `partial`; simulator and explanations are executable.
- Last-known-safe fallback: `implemented` for rejected patch recovery.
- Security review and invariant fixtures: `partial`; simulator checks core invariants.

## 8. Accessibility

- Accessibility as renderer: `implemented` at semantic-frame level.
- Native Windows/macOS/Linux, iOS, Android bridges: `partial`; bridge descriptors cover UIA, AX, AT-SPI, UIAccessibility, and Android node providers.
- Web DOM mirror from Rust/WASM: `partial`.
- Keyboard nav/focus/spoken descriptions/preferences: `partial`.
- Screen reader and snapshot tests: `partial`; screen reader harness and accessibility snapshots exist.
- Personalization cannot remove labels/roles/focus: `implemented` in policy/accessibility checks.
- Devtools accessibility inspector: `partial`; accessibility inspector reports focus order and issues from snapshots.

## 9. Web Without JS App Logic

- Rust/WASM bootloader and minimal JS: `partial`.
- Rust-owned routing/event/state/capability/runtime: `partial`; `NoJsWebRuntime` tracks Rust-owned routing, events, state, hydration, and semantic diff streaming.
- WebGPU renderer from Rust: `partial`; WebGPU parity contract exists.
- DOM accessibility mirror from Rust: `partial`; Rust-generated mirror expectation is tracked.
- SSR hydration and streaming diffs: `partial`.
- Bundle optimization, panic reporting, deployment templates: `next`.

## 10. Native Desktop

- Real `winit` app loop and `wgpu` renderer: `partial`; app-loop hooks, real surface presenter, and frame routing exist, product interaction polish next.
- Window lifecycle, menus, clipboard, drag/drop, dialogs, notifications, storage: `partial`; desktop integration capability set exists, OS implementations next.
- IME, multi-window, native accessibility, packaging, auto-update, crash hooks: `partial/next`; IME and packaging are tracked, deeper OS integration remains.

## 11. Mobile

- iOS Swift Package and Android Gradle starter: `implemented`.
- Xcode project, Rust library build, Swift/Kotlin FFI, native accessibility: `partial/next`; mobile FFI build plan tracks Swift/Kotlin bindings.
- Touch gestures, mobile layouts, offline store, push/deep links, lifecycle, renderer, examples, CI: `partial/next`; gestures, deep links, and lifecycle hooks are modeled.

## 12. Server And Fullstack

- Axum-native server path: `implemented`.
- Typed capability RPC, auth/session policy, database examples, deployment: `partial/next`.
- Streaming diffs and SSR accessibility HTML: `implemented`.
- Observability, rate limits, secure audit storage: `next`.

## 13. Devtools

- World/glyph/layout/render/reactive/policy/accessibility inspectors: `partial`; product devtools app tracks visual inspectors including render frames.
- Patch timeline, audit stream, unsafe replay, capability trace: `contract`.
- Performance flamegraph, hot reload timeline, diagnostic bundle: `partial`; product devtools and diagnostic bundle contracts exist.

## 14. Conformance And Standards

- Formal `gx conformance`: `implemented`.
- Fixture corpus, schema, patch, renderer, accessibility, policy, host, SSR reports: `partial`; invalid fixture coverage and API stability reports now exist, `gx conformance --out` includes kernel fixture/API stability sections, and `gx conformance --artifact-dir` writes a versioned artifact bundle. The next step is filling every artifact with deeper certification payloads.
- Versioned reports, public spec docs, RFC, governance, extension registry: `partial`.

## 15. Ecosystem And Distribution

- Publish crates/npm/schemas: `partial`; distribution readiness tracks crates, npm wrapper, and schema package.
- Docs site/examples gallery/CI matrix/release automation: `partial`; docs site and CI matrix readiness are modeled.
- Semver, compatibility, security, contributing, issue templates, benchmarks, roadmap board: `partial/next`; security policy readiness is modeled.
