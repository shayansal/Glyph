use glyphspace_app::{SemanticSsrServer, SsrAuthSession, SsrCapabilityRequest};
use glyphspace_core::{
    Capability, Glyph, GlyphPatch, GlyphWorld, PatchOp, PolicyContext, Priority, RiskLevel,
};

fn crm_world() -> GlyphWorld {
    let mut world = GlyphWorld::new("crm", "CRM");
    world.capabilities.insert(
        "deal.update_stage".to_string(),
        Capability::builder("deal.update_stage", "Update Deal Stage")
            .permission("crm.deal.write")
            .risk(RiskLevel::Medium)
            .build(),
    );
    world
        .add_glyph(Glyph::button("deal", "Deal").binds("deal.update_stage"))
        .unwrap();
    world
}

#[test]
fn secure_ssr_capability_request_uses_session_policy_and_emits_audit_metadata() {
    let mut server = SemanticSsrServer::new(crm_world(), PolicyContext::default());
    server.register_capability("deal.update_stage", |_input| {
        Ok(GlyphPatch::new(
            "stage_secure",
            "Stage update",
            vec![PatchOp::SetPriority {
                glyph_id: "deal".to_string(),
                priority: Priority::High,
            }],
        ))
    });
    let session = SsrAuthSession::new("session-1", "rep-1")
        .with_tenant("acme")
        .with_permission("crm.deal.write")
        .with_csrf_token("csrf-1");
    let request = SsrCapabilityRequest::new(
        "deal.update_stage",
        serde_json::json!({"deal_id": "d1", "stage": "proposal"}),
    )
    .with_session(session)
    .with_csrf_token("csrf-1")
    .with_request_id("req-1");

    let response = server
        .handle_secure_capability_http(request)
        .expect("session policy permits capability");

    assert_eq!(response.status, 200);
    assert_eq!(response.patch.id, "stage_secure");
    assert_eq!(response.body["actor"], "rep-1");
    assert_eq!(response.body["audit"]["session_id"], "session-1");
    assert_eq!(response.body["audit"]["tenant_id"], "acme");
    assert_eq!(response.body["audit"]["request_id"], "req-1");
}

#[test]
fn secure_ssr_capability_request_rejects_missing_permission_and_bad_csrf() {
    let mut server = SemanticSsrServer::new(crm_world(), PolicyContext::default());
    server.register_capability("deal.update_stage", |_input| {
        Ok(GlyphPatch::new(
            "stage_should_not_run",
            "Stage update",
            vec![PatchOp::SetPriority {
                glyph_id: "deal".to_string(),
                priority: Priority::High,
            }],
        ))
    });

    let missing_permission = SsrAuthSession::new("session-2", "viewer")
        .with_tenant("acme")
        .with_csrf_token("csrf-2");
    let denied = server.handle_secure_capability_http(
        SsrCapabilityRequest::new("deal.update_stage", serde_json::json!({}))
            .with_session(missing_permission)
            .with_csrf_token("csrf-2"),
    );
    assert!(
        denied
            .unwrap_err()
            .to_string()
            .contains("missing permission")
    );

    let csrf_session = SsrAuthSession::new("session-3", "rep-2")
        .with_tenant("acme")
        .with_permission("crm.deal.write")
        .with_csrf_token("csrf-good");
    let csrf_denied = server.handle_secure_capability_http(
        SsrCapabilityRequest::new("deal.update_stage", serde_json::json!({}))
            .with_session(csrf_session)
            .with_csrf_token("csrf-bad"),
    );
    assert!(csrf_denied.unwrap_err().to_string().contains("csrf"));
}
