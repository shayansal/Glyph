# Build A CRM In 30 Minutes

1. Create a Rust app with `gx new crm-demo`.
2. Define capabilities such as `deal.update_stage` and `deal.close`.
3. Render metrics, deals, risks, and confirmation surfaces as glyphs.
4. Run `gx dev --web --watch --ssr --browser --report target/gx-dev.json`.
5. Try an AI personalization request and inspect the policy explanation.
6. Run `gx conformance --world examples/crm-dashboard/app.glyph.json --out target/conformance.json`.

The important idea: the CRM owns data and capabilities, while Glyphspace owns semantic layout, personalization patches, policy validation, renderer snapshots, and accessibility frames.
