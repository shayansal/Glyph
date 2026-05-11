use glyphspace_core::{Glyph, GlyphWorld};
use glyphspace_layout::{DeviceProfile, Viewport};
use glyphspace_render::NativeRendererHost;

#[test]
fn native_renderer_host_prepares_scene_and_hit_tests_headlessly() {
    let mut world = GlyphWorld::new("native_host", "Native Host");
    world.add_glyph(Glyph::button("deal", "Deal")).unwrap();

    let mut host = NativeRendererHost::headless(Viewport::desktop(), DeviceProfile::desktop());
    let frame = host.render_world(&world).unwrap();
    let hit = host.hit_test(frame.hit_regions[0].center_x, frame.hit_regions[0].center_y);

    assert_eq!(frame.prepared_scene.primitive_count, 2);
    assert_eq!(hit.as_deref(), Some("deal"));
    assert_eq!(host.backend_name(), "wgpu-headless-reference");
}
