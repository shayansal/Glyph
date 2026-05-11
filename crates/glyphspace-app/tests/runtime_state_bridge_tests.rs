use glyphspace_app::{RuntimeStateBridge, RuntimeStateChange};
use glyphspace_core::{Glyph, GlyphWorld, PolicyContext};

fn world() -> GlyphWorld {
    let mut world = GlyphWorld::new("bridge", "Bridge");
    world
        .add_glyph(Glyph::metric("revenue", "Revenue"))
        .unwrap();
    world
}

#[test]
fn runtime_state_bridge_turns_server_changes_into_render_accessibility_and_audit_diffs() {
    let mut bridge = RuntimeStateBridge::new(world(), PolicyContext::demo_user());

    let update = bridge
        .apply_server_change(RuntimeStateChange::SetMetricLabel {
            glyph_id: "revenue".to_string(),
            label: "Revenue $2.4M".to_string(),
        })
        .expect("state bridge update");

    assert!(update.semantic_diff.has_changes());
    assert!(!update.layout_diff.changed_glyphs.is_empty());
    assert!(!update.render_diff.operations.is_empty());
    assert!(
        update
            .accessibility_diff
            .changed_nodes
            .contains(&"revenue".to_string())
    );
    assert_eq!(update.audit_event.action, "server.state_changed");
}
