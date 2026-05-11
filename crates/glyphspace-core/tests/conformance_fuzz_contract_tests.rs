use glyphspace_core::{
    ApiStabilityReport, FormalErrorCode, InvalidFixtureCorpus, InvalidFixtureKind,
    ProductionKernelContract, SPEC_VERSION,
};

#[test]
fn invalid_fixture_corpus_covers_world_patch_policy_and_layout_inputs() {
    let corpus = InvalidFixtureCorpus::production();
    let report = corpus.validate_against(&ProductionKernelContract::v0_1());

    assert!(report.passed);
    assert!(report.total_cases >= 12);
    assert!(report.covered_kinds.contains(&InvalidFixtureKind::World));
    assert!(report.covered_kinds.contains(&InvalidFixtureKind::Patch));
    assert!(report.covered_kinds.contains(&InvalidFixtureKind::Policy));
    assert!(report.covered_kinds.contains(&InvalidFixtureKind::Layout));
    assert!(
        report
            .expected_error_codes
            .contains(&FormalErrorCode::SchemaVersionUnsupported)
    );
    assert!(
        report
            .expected_error_codes
            .contains(&FormalErrorCode::ExtensionNamespaceInvalid)
    );
}

#[test]
fn api_stability_report_declares_public_surface_feature_flags_and_extension_rules() {
    let report = ApiStabilityReport::v0_1();

    assert_eq!(report.spec_version, SPEC_VERSION);
    assert!(report.public_types.contains(&"GlyphWorld".to_string()));
    assert!(report.public_types.contains(&"GlyphPatch".to_string()));
    assert!(report.public_types.contains(&"PolicyContext".to_string()));
    assert!(
        report
            .public_functions
            .contains(&"detect_patch_conflicts".to_string())
    );
    assert!(
        report
            .feature_flags
            .contains(&"org.glyphspace.core.v1".to_string())
    );
    assert!(report.allowed_extension_roots.contains(&"com.".to_string()));
    assert!(
        report
            .semver_guarantees
            .iter()
            .any(|item| item.contains("minor"))
    );
    assert!(report.error_codes.len() >= 6);
}
