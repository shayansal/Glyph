# Glyphspace Implementation Plan

## Checkpoints

1. Scaffold workspace, crates, docs, licenses, and progress log.
2. Implement core semantic world graph, glyphs, capabilities, patches, and serialization.
3. Implement schema validation and JSON schema export.
4. Implement policy validation for world, patch, capability, accessibility, focus order, and trust surfaces.
5. Implement personalization patch apply, invert, explain, and validation wrapper.
6. Implement deterministic layout for 2D, 2.5D, and 3D depth with stable hashing.
7. Implement CLI commands.
8. Implement mock/rule-based AI patch generator.
9. Implement wgpu renderer facade with headless scene preparation.
10. Implement WASM bridge and TypeScript SDK.
11. Implement web demo with lenses, NL patch proposals, policy warnings, inspectors, and accessibility mirror.
12. Implement CRM dashboard examples.
13. Add accessibility mirror and tests.
14. Add conformance fixtures and standard docs.
15. Run validation and record results.

## Priorities

The prototype favors simple, modular, testable code over advanced visuals. GPU-specific rendering is isolated so validation and policy behavior remain testable without a GPU.

