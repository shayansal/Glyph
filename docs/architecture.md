# Architecture

Glyphspace is split into independent layers:

1. Capability layer: typed actions, permissions, risk, confirmation, and audit.
2. Semantic world graph: glyphs and semantic edges.
3. Spatial semantics: configurable meaning for axes, depth, focus, periphery, orbiting, urgency, and policy locks.
4. Layout: deterministic constraints, responsive placement, collision avoidance, focus and accessibility order.
5. Renderer: wgpu-oriented primitives with a headless preparation path.
6. Personalization: reversible patches layered over base manifests and lenses.
7. Policy: mandatory validation for patches, capabilities, trust surfaces, and accessibility.
8. AI contract: model-agnostic patch proposal interface.
9. Accessibility: semantic tree and web DOM mirror.
10. CLI and SDK: developer tools and browser host.

