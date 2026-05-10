use glyphspace_core::{GlyphId, GlyphPose, GlyphWorld, PolicyViolation, Priority};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
    pub device_pixel_ratio: f32,
}

impl Viewport {
    pub fn desktop() -> Self {
        Self {
            width: 1440.0,
            height: 900.0,
            device_pixel_ratio: 1.0,
        }
    }

    pub fn mobile() -> Self {
        Self {
            width: 390.0,
            height: 844.0,
            device_pixel_ratio: 2.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutMode {
    TwoD,
    TwoPointFiveD,
    ThreeD,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceProfile {
    pub mode: LayoutMode,
    pub reduced_motion: bool,
    pub maximum_depth: bool,
}

impl DeviceProfile {
    pub fn desktop() -> Self {
        Self {
            mode: LayoutMode::TwoPointFiveD,
            reduced_motion: false,
            maximum_depth: false,
        }
    }

    pub fn accessible() -> Self {
        Self {
            mode: LayoutMode::TwoD,
            reduced_motion: true,
            maximum_depth: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutResult {
    pub resolved_poses: IndexMap<GlyphId, GlyphPose>,
    pub bounding_volumes: IndexMap<GlyphId, BoundingVolume>,
    pub focus_order: Vec<GlyphId>,
    pub accessibility_order: Vec<GlyphId>,
    pub render_primitives: Vec<RenderPrimitive>,
    pub hit_test_map: Vec<HitTestEntry>,
    pub warnings: Vec<String>,
    pub policy_violations: Vec<PolicyViolation>,
    pub layout_hash: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoundingVolume {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub width: f32,
    pub height: f32,
    pub depth: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RenderPrimitive {
    Dot {
        glyph_id: GlyphId,
        x: f32,
        y: f32,
        z: f32,
        radius: f32,
    },
    RoundedRect {
        glyph_id: GlyphId,
        bounds: BoundingVolume,
        radius: f32,
    },
    TextRun {
        glyph_id: GlyphId,
        text: String,
        x: f32,
        y: f32,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HitTestEntry {
    pub glyph_id: GlyphId,
    pub bounds: BoundingVolume,
}

#[derive(Debug, Error)]
pub enum LayoutError {
    #[error("world has no glyphs")]
    EmptyWorld,
}

pub fn compile_layout(
    world: &GlyphWorld,
    viewport: Viewport,
    user_lens: Option<LayoutMode>,
    device_profile: DeviceProfile,
) -> Result<LayoutResult, LayoutError> {
    if world.glyphs.is_empty() {
        return Err(LayoutError::EmptyWorld);
    }
    let mode = user_lens.unwrap_or(device_profile.mode);
    let mut glyphs: Vec<_> = world.glyphs.values().filter(|g| !g.state.hidden).collect();
    glyphs.sort_by(|a, b| {
        priority_rank(&b.priority)
            .cmp(&priority_rank(&a.priority))
            .then_with(|| a.id.cmp(&b.id))
    });

    let columns = if viewport.width < 600.0 { 1 } else { 4 };
    let spacing_x = (viewport.width / columns as f32).max(160.0);
    let spacing_y = 140.0;
    let mut resolved_poses = IndexMap::new();
    let mut bounding_volumes = IndexMap::new();
    let mut render_primitives = Vec::new();
    let mut hit_test_map = Vec::new();
    let mut focus_order = Vec::new();
    let mut warnings = Vec::new();

    for (index, glyph) in glyphs.iter().enumerate() {
        let col = index % columns;
        let row = index / columns;
        let mut pose = glyph.pose;
        pose.x = (col as f32 + 0.5) * spacing_x - viewport.width / 2.0;
        pose.y = viewport.height / 2.0 - (row as f32 + 1.0) * spacing_y;
        pose.z = if device_profile.maximum_depth || matches!(mode, LayoutMode::TwoD) {
            0.0
        } else {
            depth_for_priority(&glyph.priority)
        };
        if matches!(mode, LayoutMode::ThreeD) {
            pose.z *= 1.8;
        }
        if glyph.pose.scale != 1.0 {
            pose.scale = glyph.pose.scale;
        }
        let width = if glyph.state.collapsed { 96.0 } else { 180.0 } * pose.scale;
        let height = if glyph.state.collapsed { 42.0 } else { 88.0 } * pose.scale;
        let bounds = BoundingVolume {
            x: pose.x,
            y: pose.y,
            z: pose.z,
            width,
            height,
            depth: 12.0,
        };
        resolved_poses.insert(glyph.id.clone(), pose);
        bounding_volumes.insert(glyph.id.clone(), bounds);
        hit_test_map.push(HitTestEntry {
            glyph_id: glyph.id.clone(),
            bounds,
        });
        focus_order.push(glyph.id.clone());
        render_primitives.push(RenderPrimitive::RoundedRect {
            glyph_id: glyph.id.clone(),
            bounds,
            radius: 8.0,
        });
        render_primitives.push(RenderPrimitive::TextRun {
            glyph_id: glyph.id.clone(),
            text: glyph.label.clone(),
            x: pose.x - width / 2.0 + 12.0,
            y: pose.y,
        });
    }

    if device_profile.reduced_motion {
        warnings.push("reduced motion enabled; pulse/glow animation should be static".to_string());
    }
    let accessibility_order = focus_order.clone();
    Ok(LayoutResult {
        resolved_poses,
        bounding_volumes,
        focus_order,
        accessibility_order,
        render_primitives,
        hit_test_map,
        warnings,
        policy_violations: Vec::new(),
        layout_hash: world.stable_layout_hash(),
    })
}

fn priority_rank(priority: &Priority) -> u8 {
    match priority {
        Priority::Low => 0,
        Priority::Normal => 1,
        Priority::High => 2,
        Priority::Critical => 3,
    }
}

fn depth_for_priority(priority: &Priority) -> f32 {
    match priority {
        Priority::Critical => 0.05,
        Priority::High => 0.2,
        Priority::Normal => 0.6,
        Priority::Low => 1.0,
    }
}
