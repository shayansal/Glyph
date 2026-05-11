use glyphspace_app::{
    AiPersonalizationSession, DeveloperExperienceKit, DevtoolsStudio, FineGrainedRuntime,
    HostCertificationSuite, InteropEmbedSurface,
};
use glyphspace_core::{
    Capability, Glyph, GlyphPatch, GlyphWorld, PatchOp, PolicyContext, PolicyZone, RiskLevel,
};

fn platform_world() -> GlyphWorld {
    let mut world = GlyphWorld::new("platform", "Platform");
    world.capabilities.insert(
        "deal.close".to_string(),
        Capability::builder("deal.close", "Close Deal")
            .permission("crm.deal.write")
            .risk(RiskLevel::High)
            .requires_confirmation(true)
            .build(),
    );
    world
        .add_glyph(Glyph::metric("revenue", "Revenue"))
        .unwrap();
    world
        .add_glyph(Glyph::button("close_deal", "Close Deal").binds("deal.close"))
        .unwrap();
    world
        .add_glyph(
            Glyph::button("confirmation", "Confirm Close")
                .with_policy_zone(PolicyZone::Trust)
                .mandatory(),
        )
        .unwrap();
    world
}

#[test]
fn fine_grained_runtime_invalidates_only_changed_glyphs_and_tracks_boundaries() {
    let mut runtime = FineGrainedRuntime::new()
        .signal("revenue", 100)
        .memo("forecast", &["revenue"], |values| values[0] * 2)
        .effect("render_revenue", &["forecast"], "revenue")
        .suspense("deals")
        .error_boundary("deals_boundary");

    runtime.set_signal("revenue", 120).unwrap();
    runtime.reject_resource("deals", "network timeout");
    let diff = runtime.flush();

    assert_eq!(runtime.value("forecast"), Some(240));
    assert_eq!(diff.invalidated_glyphs, vec!["revenue".to_string()]);
    assert!(
        diff.world_diff
            .changed_glyphs
            .contains(&"revenue".to_string())
    );
    assert!(
        runtime
            .error("deals_boundary")
            .unwrap()
            .contains("network timeout")
    );
}

#[test]
fn devtools_studio_captures_graph_policy_accessibility_layout_and_audit() {
    let world = platform_world();
    let patch = GlyphPatch::new(
        "unsafe",
        "Hide confirmation",
        vec![PatchOp::Hide {
            glyph_id: "confirmation".to_string(),
        }],
    );

    let frame = DevtoolsStudio::new(PolicyContext::demo_user())
        .with_audit_event("capability.invoked", "deal.close")
        .capture(&world, &patch)
        .expect("studio frame");

    assert!(frame.graph.nodes.contains(&"revenue".to_string()));
    assert_eq!(frame.glyph.id, "confirmation");
    assert!(!frame.policy.allowed);
    assert!(frame.accessibility.verified);
    assert!(frame.layout.render_primitive_count > 0);
    assert_eq!(frame.audit_stream.len(), 1);
}

#[test]
fn ai_personalization_session_previews_policy_safe_magic_without_authority() {
    let world = platform_world();
    let session = AiPersonalizationSession::rule_based(world.clone(), PolicyContext::demo_user());

    let preview = session.propose("cannot hide confirmation, but can move it");

    assert!(!preview.accepted_patch.ops.is_empty());
    assert!(
        preview
            .policy_explanation
            .summary
            .contains("cannot hide or bypass")
    );
    assert!(
        preview
            .rejected_operations
            .iter()
            .any(|op| op.contains("hide"))
    );
    assert!(preview.undo_patch.ops.len() >= preview.accepted_patch.ops.len());
}

#[test]
fn host_certification_and_interop_embedding_cover_web_native_mobile_and_dom_hosts() {
    let report = HostCertificationSuite::new()
        .web_wasm_webgpu_dom()
        .native_winit_wgpu()
        .ios_shell()
        .android_shell()
        .certify();
    let dioxus_surface = InteropEmbedSurface::dioxus("crm")
        .imports_state("deals")
        .exports_accessibility_mirror()
        .owns_semantic_ui();

    assert!(report.passed);
    assert!(
        report
            .certifications
            .contains(&"web_wasm_webgpu_dom".to_string())
    );
    assert!(
        report
            .certifications
            .contains(&"native_winit_wgpu".to_string())
    );
    assert!(report.certifications.contains(&"ios_shell".to_string()));
    assert!(report.certifications.contains(&"android_shell".to_string()));
    assert_eq!(dioxus_surface.framework, "dioxus");
    assert!(dioxus_surface.semantic_ui_owner);
}

#[test]
fn developer_experience_kit_exports_templates_docs_errors_and_vscode_language_support() {
    let kit = DeveloperExperienceKit::crm_30_minute();

    assert!(kit.templates.iter().any(|template| template.name == "crm"));
    assert!(kit.commands.contains(&"gx new".to_string()));
    assert!(kit.commands.contains(&"gx dev".to_string()));
    assert!(
        kit.docs
            .iter()
            .any(|doc| doc.contains("build a CRM in 30 minutes"))
    );
    assert!(kit.vscode.file_extensions.contains(&".glyph".to_string()));
    assert!(
        kit.vscode
            .file_extensions
            .contains(&".lens.glyph".to_string())
    );
    assert!(
        kit.error_examples
            .iter()
            .any(|error| error.contains("policy"))
    );
}
