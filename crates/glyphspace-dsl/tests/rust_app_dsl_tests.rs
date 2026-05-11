use glyphspace_core::{Capability, Glyph, GlyphKind, GlyphPatch, PatchOp, Priority, RiskLevel};
use glyphspace_dsl::{GlyphApp, Lens};

#[test]
fn rust_builders_compile_directly_to_world_and_export_json() {
    let update_stage = Capability::builder("deal.update_stage", "Update Deal Stage")
        .intent("move a sales opportunity to a new pipeline stage")
        .permission("crm.deal.write")
        .risk(RiskLevel::Medium)
        .build();

    let founder = Lens::new("founder", "Founder command center").op(PatchOp::SetPriority {
        glyph_id: "revenue".into(),
        priority: Priority::Critical,
    });

    let app = GlyphApp::new("crm_rust", "Rust CRM")
        .capability(update_stage)
        .glyph(Glyph::metric("revenue", "Revenue").priority(Priority::High))
        .glyph(Glyph::button("advance_deal", "Advance deal").binds("deal.update_stage"))
        .lens(founder);

    let world = app.compile().unwrap();
    assert_eq!(world.id, "crm_rust");
    assert_eq!(
        world.capabilities["deal.update_stage"].risk,
        RiskLevel::Medium
    );
    assert_eq!(world.glyphs["revenue"].kind, GlyphKind::Metric);
    assert_eq!(world.glyphs["advance_deal"].accessibility.role, "button");
    assert_eq!(
        world.glyphs["advance_deal"].capability_bindings[0].capability_id,
        "deal.update_stage"
    );

    let exported = app.to_glyph_json().unwrap();
    let round_trip: glyphspace_core::GlyphWorld = serde_json::from_str(&exported).unwrap();
    assert_eq!(round_trip.stable_layout_hash(), world.stable_layout_hash());
}

#[test]
fn lens_builder_exports_a_patch() {
    let lens: GlyphPatch = Lens::new("sales", "Sales focus")
        .op(PatchOp::Collapse {
            glyph_id: "admin".into(),
        })
        .into();

    assert_eq!(lens.id, "sales");
    assert_eq!(lens.ops.len(), 1);
}
