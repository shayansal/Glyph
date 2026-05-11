use glyphspace_core::{EdgeKind, Glyph, GlyphEdge, GlyphWorld};
use glyphspace_layout::{DeviceProfile, Viewport};
use glyphspace_render::{
    AnimationClock, ProductionRenderer, RenderCommand, RenderLoopConfig, RenderSnapshot,
};

fn edge_world() -> GlyphWorld {
    let mut world = GlyphWorld::new("render-loop", "Render Loop");
    world
        .add_glyph(Glyph::metric("revenue", "Revenue"))
        .unwrap();
    world
        .add_glyph(Glyph::card("risk", "Pipeline Risk"))
        .unwrap();
    world
        .add_edge(GlyphEdge::new("revenue", "risk", EdgeKind::RelatedTo))
        .unwrap();
    world
}

#[test]
fn production_renderer_emits_full_command_frame_for_text_cards_edges_and_animation() {
    let world = edge_world();
    let mut renderer = ProductionRenderer::headless(Viewport::desktop(), DeviceProfile::desktop())
        .with_render_loop(RenderLoopConfig::animated_60hz())
        .with_animation_clock(AnimationClock::fixed_seconds(0.25))
        .with_focus("revenue");

    let frame = renderer.render_world(&world).expect("frame renders");
    let commands = &frame.command_frame.commands;

    assert!(
        commands
            .iter()
            .any(|command| matches!(command, RenderCommand::Dot { .. }))
    );
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, RenderCommand::Card { .. }))
    );
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, RenderCommand::Text { .. }))
    );
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, RenderCommand::Edge { .. }))
    );
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, RenderCommand::FocusRing { glyph_id, .. } if glyph_id == "revenue"))
    );
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, RenderCommand::AnimationTick { seconds, .. } if *seconds == 0.25))
    );
    assert_eq!(
        frame.command_frame.native_backend,
        "wgpu-headless-reference"
    );
    assert_eq!(frame.command_frame.browser_backend, "webgpu-command-buffer");
}

#[test]
fn render_snapshots_include_command_digest_and_scene_patch_application() {
    let world = edge_world();
    let mut renderer = ProductionRenderer::headless(Viewport::desktop(), DeviceProfile::desktop());

    let first = renderer.render_world(&world).expect("first frame");
    let second = renderer.render_world(&world).expect("second frame");
    let mut applied = first.command_frame.clone();
    applied.apply_scene_patch(&second.scene_patch);

    let snapshot = RenderSnapshot::from_frame(&second);

    assert_eq!(applied.commands.len(), second.command_frame.commands.len());
    assert_ne!(snapshot.command_digest, "0000000000000000");
    assert_eq!(snapshot.command_count, second.command_frame.commands.len());
}
