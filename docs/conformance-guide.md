# Conformance Guide

Run:

```bash
gx conformance --world examples/crm-dashboard/app.glyph.json --out target/conformance.json
gx conformance --world examples/crm-dashboard/app.glyph.json --artifact-dir target/conformance-artifacts
```

The report covers canonical serialization, schema compatibility, policy invariants, accessibility invariants, renderer determinism, screenshot conformance, host certification, patch compatibility, kernel invalid-fixture coverage, formal error-code coverage, and the declared public API stability surface. The artifact directory form writes versioned JSON files for kernel fixtures, API stability, renderer snapshots, policy invariants, and the conformance manifest.
