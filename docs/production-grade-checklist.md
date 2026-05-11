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
- Fuzz/property tests for invalid worlds/patches/policies/layout: `next`.
- Version every schema and migration path: `implemented` for the first registry and `0.0.9 -> 0.1.0` migration path.
- Backwards/forwards compatibility tests: `implemented` for known old and unsupported future versions.
- Stabilize public Rust APIs: `contract`.
- Feature flags and extension namespace rules: `implemented`.
- Formal error codes: `implemented` with `FormalErrorCode`.
- Performance budgets: `implemented` with `KernelPerformanceBudget`.

## 2. Real Renderer

- Real `wgpu::Surface` presentation: `contract`.
- Native window renderer end to end: `contract`.
- Browser WebGPU command-frame renderer: `contract`.
- GPU buffers, bind groups, pipelines, uniforms, texture uploads: `contract`.
- Cards, panels, dots, edges, focus rings, glows, overlays as pixels: `contract/headless`.
- `glyphspace-text` atlas integration: `contract`.
- Mature font shaping: `next`.
- Font fallback, emoji, RTL, ligatures, line breaking, wrapping: `contract` for fallback and raster cache; `next` for full text engine.
- Clipping, scrolling, z-order, transforms, opacity, masks: `next`.
- Resize, DPI, MSAA, frame pacing, animation scheduler: `contract`.
- Render-state hit testing: `contract`.
- Screenshot readback and visual snapshots: `headless`.
- Renderer benchmarks: `next`.

## 3. `gx dev`

- Real supervisor: `contract` with long-running manager and finite CI paths.
- Watch Rust, glyph, lens, policy, schema, assets: `contract`.
- Rebuild native/WASM, restart SSR: `next`.
- Preserve state: `implemented` at manager/session level.
- Diagnostics/devtools stream: `implemented` at event/report level.
- Auto-open browser/native window: `contract`.
- Friendly errors: `implemented` for new DX commands; needs compiler integration.
- Crash recovery/incremental reload: `next`.
- `--native`, `--web`, `--mobile`, `--ssr`, `--all`: `implemented` at process-manager target selection level.
- Project config/logs/traces/profiling/health: `partial`.

## 4. Developer Experience

- `glyph!` grammar and macro diagnostics: `contract`.
- Templates for dashboard/CRM/mobile/SSR/blank: `partial`.
- `gx add component/capability/lens`: `implemented`.
- `gx doctor`: `implemented`.
- `gx fmt`: `implemented` for JSON glyph/lens/policy files.
- `gx schema check`: `implemented`.
- `gx conformance --certify-host`: `implemented`.
- VS Code extension/language server/syntax/inline diagnostics: `contract`.
- Teachable Rust examples/docs/migrations/changelog: `partial`.

## 5. Component System

- Stable semantic component APIs: `contract`.
- Props/slots/children/typed events/lifecycle: `next`.
- Async resources, suspense, error boundaries: `contract`.
- Forms, tables, lists, menus, dialogs, command palette, tabs, nav: `contract`.
- Accessible defaults and keyboard behavior: `partial`.
- Layout primitives and domain kits: `contract`.

## 6. State And Runtime

- Fine-grained reactive graph: `contract`.
- Derived memos/effects/resources/suspense/error boundaries: `contract`.
- Glyph-level invalidation: `contract`.
- World diff -> layout diff -> render diff -> accessibility diff: `implemented`.
- Transactions, undo/redo, patch persistence, offline queues: `partial`.
- Audit streaming, typed capability RPC, server sync: `partial`.
- Conflict resolution for server/client/user/AI patches: `partial`.

## 7. Policy

- Formal policy language: `partial`.
- Mandatory trust surfaces, permission gates, risk/confirmation/audit: `implemented`.
- Enterprise policy contexts and layered policy: `partial`.
- Human explanations, simulator, Policy Studio, unsafe replay: `contract`.
- Last-known-safe fallback: `contract`.
- Security review and invariant fixtures: `partial`.

## 8. Accessibility

- Accessibility as renderer: `implemented` at semantic-frame level.
- Native Windows/macOS/Linux, iOS, Android bridges: `contract`.
- Web DOM mirror from Rust/WASM: `partial`.
- Keyboard nav/focus/spoken descriptions/preferences: `partial`.
- Screen reader and snapshot tests: `partial`.
- Personalization cannot remove labels/roles/focus: `implemented` in policy/accessibility checks.
- Devtools accessibility inspector: `contract`.

## 9. Web Without JS App Logic

- Rust/WASM bootloader and minimal JS: `partial`.
- Rust-owned routing/event/state/capability/runtime: `contract`.
- WebGPU renderer from Rust: `contract`.
- DOM accessibility mirror from Rust: `partial`.
- SSR hydration and streaming diffs: `partial`.
- Bundle optimization, panic reporting, deployment templates: `next`.

## 10. Native Desktop

- Real `winit` app loop and `wgpu` renderer: `contract`.
- Window lifecycle, menus, clipboard, drag/drop, dialogs, notifications, storage: `next`.
- IME, multi-window, native accessibility, packaging, auto-update, crash hooks: `next`.

## 11. Mobile

- iOS Swift Package and Android Gradle starter: `implemented`.
- Xcode project, Rust library build, Swift/Kotlin FFI, native accessibility: `next`.
- Touch gestures, mobile layouts, offline store, push/deep links, lifecycle, renderer, examples, CI: `partial/next`.

## 12. Server And Fullstack

- Axum-native server path: `implemented`.
- Typed capability RPC, auth/session policy, database examples, deployment: `partial/next`.
- Streaming diffs and SSR accessibility HTML: `implemented`.
- Observability, rate limits, secure audit storage: `next`.

## 13. Devtools

- World/glyph/layout/render/reactive/policy/accessibility inspectors: `contract`.
- Patch timeline, audit stream, unsafe replay, capability trace: `contract`.
- Performance flamegraph, hot reload timeline, diagnostic bundle: `next`.

## 14. Conformance And Standards

- Formal `gx conformance`: `implemented`.
- Fixture corpus, schema, patch, renderer, accessibility, policy, host, SSR reports: `partial`.
- Versioned reports, public spec docs, RFC, governance, extension registry: `partial`.

## 15. Ecosystem And Distribution

- Publish crates/npm/schemas: `next`.
- Docs site/examples gallery/CI matrix/release automation: `partial`.
- Semver, compatibility, security, contributing, issue templates, benchmarks, roadmap board: `partial/next`.
