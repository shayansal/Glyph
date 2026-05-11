# Web/WASM App Guide

The web target keeps the semantic world in Rust/WASM and mirrors accessibility into DOM. The current implementation still uses a thin TypeScript SDK/demo layer for browser loading and developer ergonomics, but canonical policy, patch validation, and AI proposal operations prefer the generated Rust/WASM package.

Recommended host shape:

1. Load `.glyph.json` or compile a Rust-authored app.
2. Use WASM for canonical policy and patch validation.
3. Render visuals through WebGPU when available.
4. Maintain a DOM accessibility mirror for keyboard and screen reader users.
5. Stream server state changes through semantic diffs.

## Current Stage

- Rust/WASM bridge exists for world loading, patch validation/application, and local AI proposal flows.
- The Vite demo imports generated wasm-bindgen output from `web/src/wasm` when present.
- The DOM accessibility mirror exists as a semantic companion to the canvas/WebGPU visual surface.
- The TypeScript layer remains distribution glue and demo infrastructure, not the intended long-term source of application truth.

## Next Stage

- Rust/WASM bootloader with minimal JavaScript.
- Browser WebGPU host consuming the same command frames as native `wgpu`.
- Rust-owned event loop, routing, state, capability invocation, and SSR hydration.
- Production auth provider adapters, typed capability RPC transport, and streaming semantic diffs from the Axum server.
