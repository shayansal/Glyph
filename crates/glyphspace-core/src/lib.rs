use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use thiserror::Error;

pub const SPEC_VERSION: &str = "0.1.0";

pub type Metadata = IndexMap<String, serde_json::Value>;
pub type WorldId = String;
pub type GlyphId = String;
pub type CapabilityId = String;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorldError {
    #[error("glyph already exists: {0}")]
    DuplicateGlyph(GlyphId),
    #[error("missing glyph: {0}")]
    MissingGlyph(GlyphId),
}

#[derive(Debug, Error)]
pub enum CanonicalError {
    #[error("serialization failed: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GlyphWorld {
    pub spec_version: String,
    pub id: WorldId,
    pub name: String,
    #[serde(default)]
    pub glyphs: IndexMap<GlyphId, Glyph>,
    #[serde(default)]
    pub edges: Vec<GlyphEdge>,
    #[serde(default)]
    pub capabilities: IndexMap<CapabilityId, Capability>,
    #[serde(default)]
    pub policies: Vec<PolicyRule>,
    #[serde(default)]
    pub spatial_semantics: SpatialSemantics,
    #[serde(default)]
    pub metadata: Metadata,
}

impl GlyphWorld {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            spec_version: SPEC_VERSION.to_string(),
            id: id.into(),
            name: name.into(),
            glyphs: IndexMap::new(),
            edges: Vec::new(),
            capabilities: IndexMap::new(),
            policies: Vec::new(),
            spatial_semantics: SpatialSemantics::default(),
            metadata: Metadata::new(),
        }
    }

    pub fn add_glyph(&mut self, glyph: Glyph) -> Result<(), WorldError> {
        if self.glyphs.contains_key(&glyph.id) {
            return Err(WorldError::DuplicateGlyph(glyph.id));
        }
        self.glyphs.insert(glyph.id.clone(), glyph);
        Ok(())
    }

    pub fn add_edge(&mut self, edge: GlyphEdge) -> Result<(), WorldError> {
        if !self.glyphs.contains_key(&edge.from) {
            return Err(WorldError::MissingGlyph(edge.from));
        }
        if !self.glyphs.contains_key(&edge.to) {
            return Err(WorldError::MissingGlyph(edge.to));
        }
        self.edges.push(edge);
        Ok(())
    }

    pub fn stable_layout_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.to_canonical_json()
            .unwrap_or_else(|_| format!("{}:{}", self.spec_version, self.id))
            .hash(&mut hasher);
        hasher.finish()
    }

    pub fn to_canonical_json(&self) -> Result<String, CanonicalError> {
        let value = serde_json::to_value(self)?;
        Ok(serde_json::to_string(&canonicalize_value(value))?)
    }

    pub fn canonical_digest(&self) -> Result<String, CanonicalError> {
        let mut hasher = DefaultHasher::new();
        self.to_canonical_json()?.hash(&mut hasher);
        Ok(format!("{:016x}", hasher.finish()))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Glyph {
    pub id: GlyphId,
    pub kind: GlyphKind,
    pub label: String,
    #[serde(default)]
    pub semantic_role: SemanticRole,
    #[serde(default)]
    pub pose: GlyphPose,
    #[serde(default)]
    pub style: GlyphStyle,
    #[serde(default)]
    pub state: GlyphState,
    #[serde(default)]
    pub priority: Priority,
    #[serde(default = "default_mass")]
    pub mass: f32,
    #[serde(default)]
    pub data_binding: Option<DataBinding>,
    #[serde(default)]
    pub capability_bindings: Vec<CapabilityBinding>,
    #[serde(default = "default_accessibility")]
    pub accessibility: AccessibilityNode,
    #[serde(default)]
    pub constraints: Vec<GlyphConstraint>,
    #[serde(default)]
    pub affordances: Vec<InteractionAffordance>,
    #[serde(default)]
    pub policy_zone: PolicyZone,
    #[serde(default)]
    pub mandatory: bool,
    #[serde(default)]
    pub metadata: Metadata,
}

impl Glyph {
    pub fn new(id: impl Into<String>, kind: GlyphKind, label: impl Into<String>) -> Self {
        let label = label.into();
        Self {
            id: id.into(),
            kind,
            accessibility: AccessibilityNode::static_text(label.clone()),
            label,
            semantic_role: SemanticRole::Content,
            pose: GlyphPose::default(),
            style: GlyphStyle::default(),
            state: GlyphState::default(),
            priority: Priority::Normal,
            mass: 1.0,
            data_binding: None,
            capability_bindings: Vec::new(),
            constraints: Vec::new(),
            affordances: Vec::new(),
            policy_zone: PolicyZone::Optional,
            mandatory: false,
            metadata: Metadata::new(),
        }
    }

    pub fn button(id: impl Into<String>, label: impl Into<String>) -> Self {
        let label = label.into();
        Self::new(id, GlyphKind::Button, label.clone())
            .with_role(SemanticRole::Action)
            .with_accessibility(AccessibilityNode::button(label))
    }

    pub fn metric(id: impl Into<String>, label: impl Into<String>) -> Self {
        let label = label.into();
        Self::new(id, GlyphKind::Metric, label.clone())
            .with_role(SemanticRole::Metric)
            .with_accessibility(AccessibilityNode::static_text(label))
    }

    pub fn card(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(id, GlyphKind::Card, label)
    }

    pub fn panel(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(id, GlyphKind::Panel, label)
    }

    pub fn with_role(mut self, role: SemanticRole) -> Self {
        self.semantic_role = role;
        self
    }

    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    pub fn priority(self, priority: Priority) -> Self {
        self.with_priority(priority)
    }

    pub fn with_policy_zone(mut self, zone: PolicyZone) -> Self {
        self.policy_zone = zone;
        self
    }

    pub fn with_accessibility(mut self, accessibility: AccessibilityNode) -> Self {
        self.accessibility = accessibility;
        self
    }

    pub fn with_capability(mut self, binding: CapabilityBinding) -> Self {
        self.capability_bindings.push(binding);
        self
    }

    pub fn binds(self, capability_id: impl Into<String>) -> Self {
        self.with_capability(CapabilityBinding::new(capability_id))
    }

    pub fn mandatory(mut self) -> Self {
        self.mandatory = true;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GlyphPose {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub scale: f32,
    pub rotation_x: f32,
    pub rotation_y: f32,
    pub rotation_z: f32,
}

impl Default for GlyphPose {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            scale: 1.0,
            rotation_x: 0.0,
            rotation_y: 0.0,
            rotation_z: 0.0,
        }
    }
}

impl GlyphPose {
    pub fn at(x: f32, y: f32, z: f32) -> Self {
        Self {
            x,
            y,
            z,
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GlyphKind {
    Dot,
    Cluster,
    Card,
    Button,
    Panel,
    Orb,
    Room,
    Surface,
    Agent,
    DataRegion,
    Metric,
    Warning,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SemanticRole {
    #[default]
    Content,
    Metric,
    Action,
    Navigation,
    Warning,
    TrustSurface,
    Summary,
    Agent,
    DataRegion,
}

#[derive(
    Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    #[default]
    Normal,
    High,
    Critical,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    None,
    #[default]
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PolicyZone {
    #[default]
    Optional,
    Trust,
    Security,
    Legal,
    Payment,
    Compliance,
    Mandatory,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Contains,
    Near,
    Orbits,
    DependsOn,
    DataSource,
    Invokes,
    Summarizes,
    ConflictsWith,
    RelatedTo,
    PermissionGate,
    TemporalBefore,
    TemporalAfter,
    SemanticSimilarity,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GlyphEdge {
    pub from: GlyphId,
    pub to: GlyphId,
    pub kind: EdgeKind,
    pub weight: f32,
    pub metadata: Metadata,
}

impl GlyphEdge {
    pub fn new(from: impl Into<String>, to: impl Into<String>, kind: EdgeKind) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            kind,
            weight: 1.0,
            metadata: Metadata::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GlyphStyle {
    pub tokens: IndexMap<String, String>,
    pub density: Density,
    pub high_contrast: bool,
}

impl Default for GlyphStyle {
    fn default() -> Self {
        Self {
            tokens: IndexMap::new(),
            density: Density::Comfortable,
            high_contrast: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    Calm,
    Comfortable,
    Dense,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GlyphState {
    pub hidden: bool,
    pub collapsed: bool,
    pub pinned: bool,
    pub selected: bool,
    pub urgent: bool,
    pub changed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DataBinding {
    pub source: String,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CapabilityBinding {
    pub capability_id: CapabilityId,
    pub optional: bool,
}

impl CapabilityBinding {
    pub fn new(capability_id: impl Into<String>) -> Self {
        Self {
            capability_id: capability_id.into(),
            optional: false,
        }
    }

    pub fn conflict_key(&self, glyph_id: &str) -> String {
        format!("glyphs.{glyph_id}.capability.{}", self.capability_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GlyphConstraint {
    pub kind: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InteractionAffordance {
    pub kind: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Capability {
    pub id: CapabilityId,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub intent: String,
    #[serde(default)]
    pub input_schema: serde_json::Value,
    #[serde(default)]
    pub output_schema: serde_json::Value,
    #[serde(default)]
    pub required_permissions: Vec<String>,
    #[serde(default)]
    pub risk: RiskLevel,
    #[serde(default = "default_true")]
    pub reversible: bool,
    #[serde(default)]
    pub requires_confirmation: bool,
    #[serde(default = "default_true")]
    pub audit: bool,
    #[serde(default)]
    pub domain_tags: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

impl Capability {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            intent: String::new(),
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            required_permissions: Vec::new(),
            risk: RiskLevel::Low,
            reversible: true,
            requires_confirmation: false,
            audit: true,
            domain_tags: Vec::new(),
            aliases: Vec::new(),
        }
    }

    pub fn builder(id: impl Into<String>, name: impl Into<String>) -> CapabilityBuilder {
        CapabilityBuilder::new(id, name)
    }

    pub fn with_risk(mut self, risk: RiskLevel) -> Self {
        self.risk = risk;
        self
    }

    pub fn with_permission(mut self, permission: impl Into<String>) -> Self {
        self.required_permissions.push(permission.into());
        self
    }
}

#[derive(Clone, Debug)]
pub struct CapabilityBuilder {
    capability: Capability,
}

impl CapabilityBuilder {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            capability: Capability::new(id, name),
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.capability.description = description.into();
        self
    }

    pub fn intent(mut self, intent: impl Into<String>) -> Self {
        self.capability.intent = intent.into();
        self
    }

    pub fn input_schema(mut self, schema: serde_json::Value) -> Self {
        self.capability.input_schema = schema;
        self
    }

    pub fn output_schema(mut self, schema: serde_json::Value) -> Self {
        self.capability.output_schema = schema;
        self
    }

    pub fn permission(mut self, permission: impl Into<String>) -> Self {
        self.capability.required_permissions.push(permission.into());
        self
    }

    pub fn risk(mut self, risk: RiskLevel) -> Self {
        self.capability.risk = risk;
        self
    }

    pub fn reversible(mut self, reversible: bool) -> Self {
        self.capability.reversible = reversible;
        self
    }

    pub fn requires_confirmation(mut self, requires_confirmation: bool) -> Self {
        self.capability.requires_confirmation = requires_confirmation;
        self
    }

    pub fn audit(mut self, audit: bool) -> Self {
        self.capability.audit = audit;
        self
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.capability.domain_tags.push(tag.into());
        self
    }

    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        self.capability.aliases.push(alias.into());
        self
    }

    pub fn build(self) -> Capability {
        self.capability
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AccessibilityNode {
    pub role: String,
    pub label: String,
    pub description: String,
    pub state: String,
    pub keyboard_action: Option<String>,
    pub focus_index: Option<u32>,
    pub bounding_rect: Option<[f32; 4]>,
    pub spatial_description: String,
    pub children: Vec<GlyphId>,
    pub live_region: bool,
    pub reduced_motion: bool,
    pub high_contrast: bool,
}

impl AccessibilityNode {
    pub fn static_text(label: impl Into<String>) -> Self {
        Self {
            role: "text".to_string(),
            label: label.into(),
            description: String::new(),
            state: String::new(),
            keyboard_action: None,
            focus_index: None,
            bounding_rect: None,
            spatial_description: String::new(),
            children: Vec::new(),
            live_region: false,
            reduced_motion: false,
            high_contrast: false,
        }
    }

    pub fn button(label: impl Into<String>) -> Self {
        Self {
            role: "button".to_string(),
            keyboard_action: Some("activate".to_string()),
            ..Self::static_text(label)
        }
    }

    pub fn is_valid_for_interactive(&self) -> bool {
        !self.role.trim().is_empty() && !self.label.trim().is_empty()
    }
}

impl Default for AccessibilityNode {
    fn default() -> Self {
        Self::static_text("")
    }
}

fn default_mass() -> f32 {
    1.0
}

fn default_true() -> bool {
    true
}

fn default_accessibility() -> AccessibilityNode {
    AccessibilityNode::default()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PolicyRule {
    pub id: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SpatialSemantics {
    pub x_axis: String,
    pub y_axis: String,
    pub z_axis: String,
    pub center: String,
    pub periphery: String,
}

impl Default for SpatialSemantics {
    fn default() -> Self {
        Self {
            x_axis: "lateral relationship".to_string(),
            y_axis: "hierarchy and flow".to_string(),
            z_axis: "attention depth".to_string(),
            center: "current focus".to_string(),
            periphery: "ambient context".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GlyphPatch {
    pub spec_version: String,
    pub id: String,
    pub description: String,
    pub ops: Vec<PatchOp>,
    #[serde(default)]
    pub metadata: Metadata,
}

impl GlyphPatch {
    pub fn new(id: impl Into<String>, description: impl Into<String>, ops: Vec<PatchOp>) -> Self {
        Self {
            spec_version: SPEC_VERSION.to_string(),
            id: id.into(),
            description: description.into(),
            ops,
            metadata: Metadata::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SemanticDiff {
    pub changes: Vec<SemanticChange>,
}

impl SemanticDiff {
    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SemanticChange {
    pub kind: SemanticChangeKind,
    pub path: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SemanticChangeKind {
    GlyphAdded,
    GlyphRemoved,
    GlyphChanged,
    CapabilityAdded,
    CapabilityRemoved,
    CapabilityChanged,
    EdgeAdded,
    EdgeRemoved,
    MetadataChanged,
}

pub fn semantic_diff(before: &GlyphWorld, after: &GlyphWorld) -> SemanticDiff {
    let mut changes = Vec::new();
    for id in before.glyphs.keys() {
        if !after.glyphs.contains_key(id) {
            changes.push(SemanticChange {
                kind: SemanticChangeKind::GlyphRemoved,
                path: format!("glyphs.{id}"),
                before: Some(before.glyphs[id].label.clone()),
                after: None,
            });
        }
    }
    for id in after.glyphs.keys() {
        match before.glyphs.get(id) {
            None => changes.push(SemanticChange {
                kind: SemanticChangeKind::GlyphAdded,
                path: format!("glyphs.{id}"),
                before: None,
                after: Some(after.glyphs[id].label.clone()),
            }),
            Some(before_glyph) => diff_glyph(id, before_glyph, &after.glyphs[id], &mut changes),
        }
    }
    for id in before.capabilities.keys() {
        if !after.capabilities.contains_key(id) {
            changes.push(SemanticChange {
                kind: SemanticChangeKind::CapabilityRemoved,
                path: format!("capabilities.{id}"),
                before: Some(before.capabilities[id].name.clone()),
                after: None,
            });
        }
    }
    for id in after.capabilities.keys() {
        match before.capabilities.get(id) {
            None => changes.push(SemanticChange {
                kind: SemanticChangeKind::CapabilityAdded,
                path: format!("capabilities.{id}"),
                before: None,
                after: Some(after.capabilities[id].name.clone()),
            }),
            Some(before_capability) if before_capability != &after.capabilities[id] => {
                changes.push(SemanticChange {
                    kind: SemanticChangeKind::CapabilityChanged,
                    path: format!("capabilities.{id}"),
                    before: Some(before_capability.name.clone()),
                    after: Some(after.capabilities[id].name.clone()),
                });
            }
            _ => {}
        }
    }
    SemanticDiff { changes }
}

fn diff_glyph(id: &str, before: &Glyph, after: &Glyph, changes: &mut Vec<SemanticChange>) {
    if before.priority != after.priority {
        changes.push(SemanticChange {
            kind: SemanticChangeKind::GlyphChanged,
            path: format!("glyphs.{id}.priority"),
            before: Some(format!("{:?}", before.priority)),
            after: Some(format!("{:?}", after.priority)),
        });
    }
    if before.pose != after.pose {
        changes.push(SemanticChange {
            kind: SemanticChangeKind::GlyphChanged,
            path: format!("glyphs.{id}.pose"),
            before: serde_json::to_string(&before.pose).ok(),
            after: serde_json::to_string(&after.pose).ok(),
        });
    }
    if before.state != after.state {
        changes.push(SemanticChange {
            kind: SemanticChangeKind::GlyphChanged,
            path: format!("glyphs.{id}.state"),
            before: serde_json::to_string(&before.state).ok(),
            after: serde_json::to_string(&after.state).ok(),
        });
    }
    if before.accessibility != after.accessibility {
        changes.push(SemanticChange {
            kind: SemanticChangeKind::GlyphChanged,
            path: format!("glyphs.{id}.accessibility"),
            before: Some(before.accessibility.label.clone()),
            after: Some(after.accessibility.label.clone()),
        });
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PatchOp {
    Move {
        glyph_id: GlyphId,
        pose: GlyphPose,
    },
    Resize {
        glyph_id: GlyphId,
        scale: f32,
    },
    SetPriority {
        glyph_id: GlyphId,
        priority: Priority,
    },
    Collapse {
        glyph_id: GlyphId,
    },
    Expand {
        glyph_id: GlyphId,
    },
    Hide {
        glyph_id: GlyphId,
    },
    Show {
        glyph_id: GlyphId,
    },
    Group {
        group_id: GlyphId,
        glyph_ids: Vec<GlyphId>,
    },
    Ungroup {
        group_id: GlyphId,
    },
    Pin {
        glyph_id: GlyphId,
    },
    SetStyleToken {
        glyph_id: GlyphId,
        key: String,
        value: String,
    },
    SetDensity {
        glyph_id: GlyphId,
        density: Density,
    },
    SetDepth {
        glyph_id: GlyphId,
        z: f32,
    },
    CreateSummaryGlyph {
        id: GlyphId,
        source_glyphs: Vec<GlyphId>,
        label: String,
    },
    CreateAgentGlyph {
        id: GlyphId,
        label: String,
        allowed_capabilities: Vec<CapabilityId>,
    },
    ReorderFocus {
        ordered_glyph_ids: Vec<GlyphId>,
    },
    SetAccessibilityPreference {
        glyph_id: GlyphId,
        reduced_motion: Option<bool>,
        high_contrast: Option<bool>,
    },
    BindCapability {
        glyph_id: GlyphId,
        capability_id: CapabilityId,
    },
    UnbindOptionalCapability {
        glyph_id: GlyphId,
        capability_id: CapabilityId,
    },
}

impl PatchOp {
    pub fn conflict_key(&self) -> Option<String> {
        Some(match self {
            PatchOp::Move { glyph_id, .. } => format!("glyphs.{glyph_id}.pose"),
            PatchOp::Resize { glyph_id, .. } => format!("glyphs.{glyph_id}.pose.scale"),
            PatchOp::SetPriority { glyph_id, .. } => format!("glyphs.{glyph_id}.priority"),
            PatchOp::Collapse { glyph_id } | PatchOp::Expand { glyph_id } => {
                format!("glyphs.{glyph_id}.state.collapsed")
            }
            PatchOp::Hide { glyph_id } | PatchOp::Show { glyph_id } => {
                format!("glyphs.{glyph_id}.state.hidden")
            }
            PatchOp::Pin { glyph_id } => format!("glyphs.{glyph_id}.state.pinned"),
            PatchOp::SetStyleToken { glyph_id, key, .. } => {
                format!("glyphs.{glyph_id}.style.tokens.{key}")
            }
            PatchOp::SetDensity { glyph_id, .. } => format!("glyphs.{glyph_id}.style.density"),
            PatchOp::SetDepth { glyph_id, .. } => format!("glyphs.{glyph_id}.pose.z"),
            PatchOp::CreateSummaryGlyph { id, .. } | PatchOp::CreateAgentGlyph { id, .. } => {
                format!("glyphs.{id}")
            }
            PatchOp::ReorderFocus { .. } => "accessibility.focus_order".to_string(),
            PatchOp::SetAccessibilityPreference { glyph_id, .. } => {
                format!("glyphs.{glyph_id}.accessibility.preferences")
            }
            PatchOp::BindCapability {
                glyph_id,
                capability_id,
            }
            | PatchOp::UnbindOptionalCapability {
                glyph_id,
                capability_id,
            } => format!("glyphs.{glyph_id}.capability.{capability_id}"),
            PatchOp::Group { group_id, .. } | PatchOp::Ungroup { group_id } => {
                format!("glyphs.{group_id}.grouping")
            }
        })
    }

    pub fn conflict_value(&self) -> String {
        serde_json::to_string(&canonicalize_value(
            serde_json::to_value(self).unwrap_or(Value::Null),
        ))
        .unwrap_or_else(|_| format!("{self:?}"))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatchConflictReport {
    pub conflicts: Vec<PatchConflict>,
}

impl PatchConflictReport {
    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PatchConflict {
    pub path: String,
    pub left_patch: String,
    pub right_patch: String,
    pub left_value: String,
    pub right_value: String,
}

pub fn detect_patch_conflicts(left: &GlyphPatch, right: &GlyphPatch) -> PatchConflictReport {
    let mut seen = BTreeMap::new();
    for op in &left.ops {
        if let Some(path) = op.conflict_key() {
            seen.insert(path, op.conflict_value());
        }
    }

    let mut conflicts = Vec::new();
    for op in &right.ops {
        let Some(path) = op.conflict_key() else {
            continue;
        };
        let right_value = op.conflict_value();
        if let Some(left_value) = seen.get(&path)
            && left_value != &right_value
        {
            conflicts.push(PatchConflict {
                path,
                left_patch: left.id.clone(),
                right_patch: right.id.clone(),
                left_value: left_value.clone(),
                right_value,
            });
        }
    }
    PatchConflictReport { conflicts }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PolicyContext {
    pub user_id: String,
    pub permissions: Vec<String>,
    pub can_personalize: bool,
    pub allow_low_risk_ai_auto_apply: bool,
}

impl PolicyContext {
    pub fn demo_user() -> Self {
        Self {
            user_id: "demo_user".to_string(),
            permissions: vec![
                "ui.personalize".to_string(),
                "crm.deal.read".to_string(),
                "crm.deal.write".to_string(),
            ],
            can_personalize: true,
            allow_low_risk_ai_auto_apply: false,
        }
    }

    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.iter().any(|p| p == permission)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ValidationReport {
    pub allowed: bool,
    pub violations: Vec<PolicyViolation>,
    pub warnings: Vec<String>,
}

impl ValidationReport {
    pub fn allow() -> Self {
        Self {
            allowed: true,
            violations: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn reject(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            allowed: false,
            violations: vec![PolicyViolation {
                code: code.into(),
                message: message.into(),
                glyph_id: None,
            }],
            warnings: Vec::new(),
        }
    }

    pub fn push_violation(
        &mut self,
        code: impl Into<String>,
        message: impl Into<String>,
        glyph_id: Option<GlyphId>,
    ) {
        self.allowed = false;
        self.violations.push(PolicyViolation {
            code: code.into(),
            message: message.into(),
            glyph_id,
        });
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PolicyViolation {
    pub code: String,
    pub message: String,
    pub glyph_id: Option<GlyphId>,
}

pub type GlyphCluster = Glyph;
pub type GlyphField = Glyph;
pub type GlyphCapabilityBinding = CapabilityBinding;
pub type GlyphDataBinding = DataBinding;
pub type GlyphPolicyZone = PolicyZone;
pub type GlyphAccessibilityNode = AccessibilityNode;
pub type GlyphLens = GlyphPatch;

fn canonicalize_value(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut sorted = Map::new();
            let mut entries: Vec<_> = object.into_iter().collect();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            for (key, value) in entries {
                sorted.insert(key, canonicalize_value(value));
            }
            Value::Object(sorted)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_value).collect()),
        other => other,
    }
}
