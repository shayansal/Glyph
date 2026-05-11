# Host Adapter Guide

A host adapter must provide:

- render surface
- input events
- accessibility mirror
- patch storage
- policy context
- capability invocation
- device profile
- audit sink

Host certification checks web/WASM/WebGPU/DOM, native `winit`/`wgpu`, iOS, and Android profiles.

## Current Stage

Glyphspace has explicit host contracts, conformance checks, and scaffolded bridge frames for web, native, iOS, and Android. The reference implementation can validate semantic host behavior headlessly: render frames, scene patches, hit maps, accessibility frames, policy decisions, patch storage, audit events, and state-preserving reload batches.

## Host Maturity Levels

- Contract host: validates the required host surface without requiring a product UI.
- Headless host: runs layout, render preparation, accessibility, hit-testing, policy, and audit flows in CI.
- Window host: opens a native or browser surface and processes resize/input/redraw events.
- Product host: includes OS integrations such as accessibility APIs, IME, menus, clipboard, storage, notifications, installers, mobile lifecycle, and offline patch queues.

The repository is strongest at the contract/headless levels today. Window/product hosts are the next platform milestone.
