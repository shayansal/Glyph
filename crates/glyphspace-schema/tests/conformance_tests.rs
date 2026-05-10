use glyphspace_schema::{validate_patch_json, validate_world_json};

#[test]
fn conformance_world_fixture_validates() {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/conformance/valid-world.glyph.json"
    ))
    .unwrap();

    assert!(validate_world_json(&value).unwrap().valid);
}

#[test]
fn conformance_lens_fixture_validates_as_patch() {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/conformance/valid-lens.glyph.json"
    ))
    .unwrap();

    assert!(validate_patch_json(&value).unwrap().valid);
}

#[test]
fn conformance_policy_invariant_fixtures_are_well_formed_patches() {
    for fixture in [
        include_str!("../../../tests/conformance/policy-hide-mandatory-trust.patch.glyph.json"),
        include_str!("../../../tests/conformance/policy-create-fake-capability.patch.glyph.json"),
        include_str!("../../../tests/conformance/policy-remove-accessibility.patch.glyph.json"),
    ] {
        let value: serde_json::Value = serde_json::from_str(fixture).unwrap();
        assert!(validate_patch_json(&value).unwrap().valid);
    }
}
