use glyphspace_core::{
    Capability, ExtensionNamespace, FormalErrorCode, Glyph, GlyphEdge, GlyphKind, GlyphWorld,
    KernelPerformanceBudget, KernelPerformanceSample, PatchOp, Priority, ProductionKernelContract,
    RiskLevel, SPEC_VERSION, SchemaMigrationRegistry, SemanticChangeKind, SemanticRole,
    detect_patch_conflicts, semantic_diff,
};

#[test]
fn production_contract_freezes_glyphworld_versions_features_and_namespaces() {
    let contract = ProductionKernelContract::v0_1();

    assert_eq!(contract.runtime_model, "GlyphWorld");
    assert_eq!(contract.spec_version, SPEC_VERSION);
    assert_eq!(contract.schema_version, "0.1.0");
    assert!(contract.frozen_fields.contains(&"glyphs".to_string()));
    assert!(contract.frozen_fields.contains(&"capabilities".to_string()));
    assert!(contract.supports_feature("org.glyphspace.policy.v1"));
    assert!(contract.supports_feature("org.glyphspace.accessibility.v1"));

    assert!(
        contract
            .validate_extension_namespace(&ExtensionNamespace::new("com.company.crm"))
            .is_ok()
    );
    let err = contract
        .validate_extension_namespace(&ExtensionNamespace::new("crm"))
        .expect_err("bare extension namespaces are rejected");
    assert_eq!(err.code, FormalErrorCode::ExtensionNamespaceInvalid);
}

#[test]
fn compatibility_report_rejects_future_versions_and_unknown_feature_flags() {
    let contract = ProductionKernelContract::v0_1();
    let mut world = GlyphWorld::new("compat", "Compatibility");
    world.metadata.insert(
        "feature_flags".into(),
        serde_json::json!(["org.glyphspace.policy.v1", "com.company.experimental"]),
    );

    let report = contract.compatibility_report(&world);
    assert!(!report.compatible);
    assert!(report.errors.iter().any(|err| {
        err.code == FormalErrorCode::FeatureFlagUnsupported
            && err.path == "metadata.feature_flags[1]"
    }));

    world.spec_version = "9.0.0".to_string();
    let report = contract.compatibility_report(&world);
    assert!(report.errors.iter().any(|err| {
        err.code == FormalErrorCode::SchemaVersionUnsupported && err.path == "spec_version"
    }));
}

#[test]
fn schema_migration_registry_upgrades_known_world_versions_and_records_history() {
    let registry = SchemaMigrationRegistry::reference();
    let mut world = GlyphWorld::new("old", "Old World");
    world.spec_version = "0.0.9".to_string();

    let migrated = registry
        .migrate_world(world)
        .expect("known old versions migrate");

    assert_eq!(migrated.spec_version, SPEC_VERSION);
    assert_eq!(
        migrated.metadata["migration_history"][0]["from"],
        serde_json::json!("0.0.9")
    );
    assert_eq!(
        migrated.metadata["migration_history"][0]["to"],
        serde_json::json!(SPEC_VERSION)
    );

    let mut future = GlyphWorld::new("future", "Future");
    future.spec_version = "2.0.0".to_string();
    let err = registry
        .migrate_world(future)
        .expect_err("unknown future versions do not silently migrate");
    assert_eq!(err.code, FormalErrorCode::SchemaVersionUnsupported);
}

#[test]
fn performance_budgets_return_formal_error_codes() {
    let budget = KernelPerformanceBudget::prototype()
        .with_validation_ms(10)
        .with_layout_ms(20)
        .with_patch_ms(5);
    let sample = KernelPerformanceSample {
        validation_ms: 11,
        layout_ms: 20,
        patch_ms: 8,
        glyph_count: 1_000,
    };

    let report = budget.evaluate(sample);

    assert!(!report.within_budget);
    assert_eq!(report.violations.len(), 2);
    assert!(
        report
            .violations
            .iter()
            .all(|err| err.code == FormalErrorCode::PerformanceBudgetExceeded)
    );
}

#[test]
fn semantic_diff_covers_edges_capabilities_policy_spatial_and_glyph_fields() {
    let mut before = GlyphWorld::new("diff", "Diff");
    before
        .add_glyph(Glyph::metric("revenue", "Revenue"))
        .unwrap();
    before.add_glyph(Glyph::card("risk", "Risk")).unwrap();
    before
        .add_edge(GlyphEdge::new(
            "revenue",
            "risk",
            glyphspace_core::EdgeKind::RelatedTo,
        ))
        .unwrap();

    let mut after = before.clone();
    after.glyphs["revenue"].label = "Revenue updated".to_string();
    after.glyphs["revenue"].kind = GlyphKind::Button;
    after.glyphs["revenue"].semantic_role = SemanticRole::Action;
    after.glyphs["revenue"]
        .style
        .tokens
        .insert("tone".into(), "urgent".into());
    after.glyphs["revenue"].mandatory = true;
    after.capabilities.insert(
        "deal.close".into(),
        Capability::builder("deal.close", "Close Deal")
            .risk(RiskLevel::High)
            .build(),
    );
    after.edges.clear();
    after
        .policies
        .push(glyphspace_core::PolicyRule::new("policy.trust", "Trust"));
    after.spatial_semantics.z_axis = "risk depth".into();
    after
        .metadata
        .insert("schema_version".into(), serde_json::json!("0.1.0"));

    let diff = semantic_diff(&before, &after);
    let paths = diff
        .changes
        .iter()
        .map(|change| change.path.as_str())
        .collect::<Vec<_>>();

    assert!(paths.contains(&"glyphs.revenue.label"));
    assert!(paths.contains(&"glyphs.revenue.kind"));
    assert!(paths.contains(&"glyphs.revenue.semantic_role"));
    assert!(paths.contains(&"glyphs.revenue.style"));
    assert!(paths.contains(&"glyphs.revenue.mandatory"));
    assert!(paths.contains(&"capabilities.deal.close"));
    assert!(paths.contains(&"edges"));
    assert!(paths.contains(&"policies"));
    assert!(paths.contains(&"spatial_semantics"));
    assert!(paths.contains(&"metadata.schema_version"));
    assert!(
        diff.changes
            .iter()
            .any(|change| change.kind == SemanticChangeKind::EdgeRemoved)
    );
}

#[test]
fn patch_conflict_detection_reports_every_same_target_divergence() {
    let left = glyphspace_core::GlyphPatch::new(
        "left",
        "left",
        vec![
            PatchOp::Move {
                glyph_id: "a".into(),
                pose: glyphspace_core::GlyphPose::at(1.0, 0.0, 0.0),
            },
            PatchOp::SetPriority {
                glyph_id: "a".into(),
                priority: Priority::High,
            },
            PatchOp::Hide {
                glyph_id: "a".into(),
            },
        ],
    );
    let right = glyphspace_core::GlyphPatch::new(
        "right",
        "right",
        vec![
            PatchOp::Move {
                glyph_id: "a".into(),
                pose: glyphspace_core::GlyphPose::at(2.0, 0.0, 0.0),
            },
            PatchOp::SetPriority {
                glyph_id: "a".into(),
                priority: Priority::Low,
            },
            PatchOp::Show {
                glyph_id: "a".into(),
            },
        ],
    );

    let report = detect_patch_conflicts(&left, &right);
    let paths = report
        .conflicts
        .iter()
        .map(|conflict| conflict.path.as_str())
        .collect::<Vec<_>>();

    assert_eq!(report.conflicts.len(), 3);
    assert!(paths.contains(&"glyphs.a.pose"));
    assert!(paths.contains(&"glyphs.a.priority"));
    assert!(paths.contains(&"glyphs.a.state.hidden"));
}
