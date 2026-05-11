# Web/WASM App Guide

The web target keeps the semantic world in Rust/WASM and mirrors accessibility into DOM.

Recommended host shape:

1. Load `.glyph.json` or compile a Rust-authored app.
2. Use WASM for canonical policy and patch validation.
3. Render visuals through WebGPU when available.
4. Maintain a DOM accessibility mirror for keyboard and screen reader users.
5. Stream server state changes through semantic diffs.
