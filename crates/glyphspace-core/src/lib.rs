use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use thiserror::Error;

pub const SPEC_VERSION: &str = "0.1.0";
pub const SCHEMA_VERSION: &str = "0.1.0";

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FormalErrorCode {
    SchemaVersionUnsupported,
    FeatureFlagUnsupported,
    ExtensionNamespaceInvalid,
    PerformanceBudgetExceeded,
    MigrationUnavailable,
    InvalidWorld,
    InvalidPatch,
    InvalidPolicy,
    InvalidLayout,
    PublicApiUnstable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FormalKernelError {
    pub code: FormalErrorCode,
    pub path: String,
    pub message: String,
}

impl FormalKernelError {
    pub fn new(code: FormalErrorCode, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code,
            path: path.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for FormalKernelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{:?} at {}: {}",
            self.code, self.path, self.message
        )
    }
}

impl std::error::Error for FormalKernelError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionNamespace {
    pub value: String,
}

impl ExtensionNamespace {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CompatibilityReport {
    pub compatible: bool,
    pub errors: Vec<FormalKernelError>,
    pub warnings: Vec<String>,
}

impl CompatibilityReport {
    pub fn allow() -> Self {
        Self {
            compatible: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn push_error(&mut self, error: FormalKernelError) {
        self.compatible = false;
        self.errors.push(error);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProductionKernelContract {
    pub runtime_model: String,
    pub spec_version: String,
    pub schema_version: String,
    pub frozen_fields: Vec<String>,
    pub supported_feature_flags: Vec<String>,
    pub allowed_extension_roots: Vec<String>,
}

impl ProductionKernelContract {
    pub fn v0_1() -> Self {
        Self {
            runtime_model: "GlyphWorld".to_string(),
            spec_version: SPEC_VERSION.to_string(),
            schema_version: SCHEMA_VERSION.to_string(),
            frozen_fields: vec![
                "spec_version".to_string(),
                "id".to_string(),
                "name".to_string(),
                "glyphs".to_string(),
                "edges".to_string(),
                "capabilities".to_string(),
                "policies".to_string(),
                "spatial_semantics".to_string(),
                "metadata".to_string(),
            ],
            supported_feature_flags: vec![
                "org.glyphspace.core.v1".to_string(),
                "org.glyphspace.policy.v1".to_string(),
                "org.glyphspace.accessibility.v1".to_string(),
                "org.glyphspace.layout.v1".to_string(),
                "org.glyphspace.patches.v1".to_string(),
            ],
            allowed_extension_roots: vec![
                "com.".to_string(),
                "org.".to_string(),
                "net.".to_string(),
                "io.".to_string(),
            ],
        }
    }

    pub fn supports_feature(&self, feature: &str) -> bool {
        self.supported_feature_flags
            .iter()
            .any(|supported| supported == feature)
    }

    pub fn validate_extension_namespace(
        &self,
        namespace: &ExtensionNamespace,
    ) -> Result<(), FormalKernelError> {
        let value = namespace.value.trim();
        let has_allowed_root = self
            .allowed_extension_roots
            .iter()
            .any(|root| value.starts_with(root));
        let has_multiple_segments = value.split('.').filter(|part| !part.is_empty()).count() >= 3;
        if has_allowed_root && has_multiple_segments && !value.contains(' ') {
            Ok(())
        } else {
            Err(FormalKernelError::new(
                FormalErrorCode::ExtensionNamespaceInvalid,
                "extension_namespace",
                format!(
                    "extension namespace `{value}` must look like com.company.product or org.project.feature"
                ),
            ))
        }
    }

    pub fn compatibility_report(&self, world: &GlyphWorld) -> CompatibilityReport {
        let mut report = CompatibilityReport::allow();
        if world.spec_version != self.spec_version {
            report.push_error(FormalKernelError::new(
                FormalErrorCode::SchemaVersionUnsupported,
                "spec_version",
                format!(
                    "world spec_version {} is not supported by contract {}",
                    world.spec_version, self.spec_version
                ),
            ));
        }
        if let Some(flags) = world
            .metadata
            .get("feature_flags")
            .and_then(Value::as_array)
        {
            for (index, flag) in flags.iter().enumerate() {
                let Some(flag) = flag.as_str() else {
                    report.push_error(FormalKernelError::new(
                        FormalErrorCode::FeatureFlagUnsupported,
                        format!("metadata.feature_flags[{index}]"),
                        "feature flag must be a string",
                    ));
                    continue;
                };
                if !self.supports_feature(flag) {
                    report.push_error(FormalKernelError::new(
                        FormalErrorCode::FeatureFlagUnsupported,
                        format!("metadata.feature_flags[{index}]"),
                        format!("feature flag `{flag}` is not supported"),
                    ));
                }
            }
        }
        if let Some(extensions) = world.metadata.get("extensions").and_then(Value::as_object) {
            for namespace in extensions.keys() {
                if let Err(error) =
                    self.validate_extension_namespace(&ExtensionNamespace::new(namespace))
                {
                    report.push_error(FormalKernelError {
                        path: format!("metadata.extensions.{namespace}"),
                        ..error
                    });
                }
            }
        }
        report
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SchemaMigration {
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SchemaMigrationRegistry {
    pub current_version: String,
    pub migrations: Vec<SchemaMigration>,
}

impl SchemaMigrationRegistry {
    pub fn reference() -> Self {
        Self {
            current_version: SPEC_VERSION.to_string(),
            migrations: vec![SchemaMigration {
                from: "0.0.9".to_string(),
                to: SPEC_VERSION.to_string(),
            }],
        }
    }

    pub fn migrate_world(&self, mut world: GlyphWorld) -> Result<GlyphWorld, FormalKernelError> {
        if world.spec_version == self.current_version {
            return Ok(world);
        }
        let Some(migration) = self
            .migrations
            .iter()
            .find(|migration| migration.from == world.spec_version)
        else {
            return Err(FormalKernelError::new(
                FormalErrorCode::SchemaVersionUnsupported,
                "spec_version",
                format!("no migration path from {}", world.spec_version),
            ));
        };
        let from = world.spec_version.clone();
        world.spec_version = migration.to.clone();
        let entry = serde_json::json!({
            "from": from,
            "to": migration.to,
            "runtime_model": "GlyphWorld",
        });
        match world.metadata.get_mut("migration_history") {
            Some(Value::Array(history)) => history.push(entry),
            _ => {
                world
                    .metadata
                    .insert("migration_history".to_string(), Value::Array(vec![entry]));
            }
        }
        Ok(world)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KernelPerformanceBudget {
    pub max_validation_ms: u64,
    pub max_layout_ms: u64,
    pub max_patch_ms: u64,
    pub target_glyph_count: usize,
}

impl KernelPerformanceBudget {
    pub fn prototype() -> Self {
        Self {
            max_validation_ms: 20,
            max_layout_ms: 50,
            max_patch_ms: 20,
            target_glyph_count: 1_000,
        }
    }

    pub fn with_validation_ms(mut self, max_validation_ms: u64) -> Self {
        self.max_validation_ms = max_validation_ms;
        self
    }

    pub fn with_layout_ms(mut self, max_layout_ms: u64) -> Self {
        self.max_layout_ms = max_layout_ms;
        self
    }

    pub fn with_patch_ms(mut self, max_patch_ms: u64) -> Self {
        self.max_patch_ms = max_patch_ms;
        self
    }

    pub fn evaluate(&self, sample: KernelPerformanceSample) -> KernelPerformanceReport {
        let mut report = KernelPerformanceReport {
            within_budget: true,
            sample,
            budget: *self,
            violations: Vec::new(),
        };
        if sample.validation_ms > self.max_validation_ms {
            report.push_violation(
                "validation_ms",
                sample.validation_ms,
                self.max_validation_ms,
            );
        }
        if sample.layout_ms > self.max_layout_ms {
            report.push_violation("layout_ms", sample.layout_ms, self.max_layout_ms);
        }
        if sample.patch_ms > self.max_patch_ms {
            report.push_violation("patch_ms", sample.patch_ms, self.max_patch_ms);
        }
        report
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KernelPerformanceSample {
    pub validation_ms: u64,
    pub layout_ms: u64,
    pub patch_ms: u64,
    pub glyph_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KernelPerformanceReport {
    pub within_budget: bool,
    pub sample: KernelPerformanceSample,
    pub budget: KernelPerformanceBudget,
    pub violations: Vec<FormalKernelError>,
}

impl KernelPerformanceReport {
    fn push_violation(&mut self, metric: &str, actual: u64, expected: u64) {
        self.within_budget = false;
        self.violations.push(FormalKernelError::new(
            FormalErrorCode::PerformanceBudgetExceeded,
            metric,
            format!("{metric}={actual}ms exceeded budget {expected}ms"),
        ));
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum InvalidFixtureKind {
    World,
    Patch,
    Policy,
    Layout,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InvalidFixture {
    pub name: String,
    pub kind: InvalidFixtureKind,
    pub payload: Value,
    pub expected_error_code: FormalErrorCode,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InvalidFixtureCorpus {
    pub spec_version: String,
    pub cases: Vec<InvalidFixture>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InvalidFixtureReport {
    pub passed: bool,
    pub total_cases: usize,
    pub covered_kinds: Vec<InvalidFixtureKind>,
    pub expected_error_codes: Vec<FormalErrorCode>,
    pub missing_coverage: Vec<String>,
}

impl InvalidFixtureCorpus {
    pub fn production() -> Self {
        Self {
            spec_version: SPEC_VERSION.to_string(),
            cases: vec![
                invalid_fixture(
                    "world_unsupported_spec",
                    InvalidFixtureKind::World,
                    serde_json::json!({"spec_version": "99.0.0", "id": "bad"}),
                    FormalErrorCode::SchemaVersionUnsupported,
                ),
                invalid_fixture(
                    "world_duplicate_glyph_ids",
                    InvalidFixtureKind::World,
                    serde_json::json!({"glyphs": [{"id": "a"}, {"id": "a"}]}),
                    FormalErrorCode::InvalidWorld,
                ),
                invalid_fixture(
                    "world_invalid_extension_namespace",
                    InvalidFixtureKind::World,
                    serde_json::json!({"metadata": {"extensions": {"vendor only": {}}}}),
                    FormalErrorCode::ExtensionNamespaceInvalid,
                ),
                invalid_fixture(
                    "patch_unknown_op",
                    InvalidFixtureKind::Patch,
                    serde_json::json!({"ops": [{"type": "change_permissions"}]}),
                    FormalErrorCode::InvalidPatch,
                ),
                invalid_fixture(
                    "patch_missing_target",
                    InvalidFixtureKind::Patch,
                    serde_json::json!({"ops": [{"type": "move", "glyph_id": "missing"}]}),
                    FormalErrorCode::InvalidPatch,
                ),
                invalid_fixture(
                    "patch_mutates_audit",
                    InvalidFixtureKind::Patch,
                    serde_json::json!({"ops": [{"type": "set_audit", "enabled": false}]}),
                    FormalErrorCode::InvalidPatch,
                ),
                invalid_fixture(
                    "policy_missing_trust_surface",
                    InvalidFixtureKind::Policy,
                    serde_json::json!({"mandatory_trust_surfaces": []}),
                    FormalErrorCode::InvalidPolicy,
                ),
                invalid_fixture(
                    "policy_bypass_confirmation",
                    InvalidFixtureKind::Policy,
                    serde_json::json!({"risk": "critical", "requires_confirmation": false}),
                    FormalErrorCode::InvalidPolicy,
                ),
                invalid_fixture(
                    "policy_bad_feature_flag",
                    InvalidFixtureKind::Policy,
                    serde_json::json!({"feature_flags": [42]}),
                    FormalErrorCode::FeatureFlagUnsupported,
                ),
                invalid_fixture(
                    "layout_negative_viewport",
                    InvalidFixtureKind::Layout,
                    serde_json::json!({"viewport": {"width": -1, "height": 480}}),
                    FormalErrorCode::InvalidLayout,
                ),
                invalid_fixture(
                    "layout_nan_depth",
                    InvalidFixtureKind::Layout,
                    serde_json::json!({"glyph": "a", "z": "NaN"}),
                    FormalErrorCode::InvalidLayout,
                ),
                invalid_fixture(
                    "layout_performance_budget",
                    InvalidFixtureKind::Layout,
                    serde_json::json!({"glyph_count": 1000000}),
                    FormalErrorCode::PerformanceBudgetExceeded,
                ),
            ],
        }
    }

    pub fn validate_against(&self, contract: &ProductionKernelContract) -> InvalidFixtureReport {
        let mut covered_kinds = self.cases.iter().map(|case| case.kind).collect::<Vec<_>>();
        covered_kinds.sort();
        covered_kinds.dedup();

        let mut expected_error_codes = self
            .cases
            .iter()
            .map(|case| case.expected_error_code.clone())
            .collect::<Vec<_>>();
        expected_error_codes.dedup();

        let mut missing_coverage = Vec::new();
        for kind in [
            InvalidFixtureKind::World,
            InvalidFixtureKind::Patch,
            InvalidFixtureKind::Policy,
            InvalidFixtureKind::Layout,
        ] {
            if !covered_kinds.contains(&kind) {
                missing_coverage.push(format!("missing {kind:?} invalid fixtures"));
            }
        }
        if self.spec_version != contract.spec_version {
            missing_coverage.push(format!(
                "fixture corpus spec_version {} does not match contract {}",
                self.spec_version, contract.spec_version
            ));
        }
        if self.cases.len() < 12 {
            missing_coverage.push("production corpus needs at least twelve fixtures".to_string());
        }

        InvalidFixtureReport {
            passed: missing_coverage.is_empty(),
            total_cases: self.cases.len(),
            covered_kinds,
            expected_error_codes,
            missing_coverage,
        }
    }
}

fn invalid_fixture(
    name: &str,
    kind: InvalidFixtureKind,
    payload: Value,
    expected_error_code: FormalErrorCode,
) -> InvalidFixture {
    InvalidFixture {
        name: name.to_string(),
        kind,
        payload,
        expected_error_code,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ApiStabilityReport {
    pub spec_version: String,
    pub public_types: Vec<String>,
    pub public_functions: Vec<String>,
    pub feature_flags: Vec<String>,
    pub allowed_extension_roots: Vec<String>,
    pub semver_guarantees: Vec<String>,
    pub error_codes: Vec<FormalErrorCode>,
}

impl ApiStabilityReport {
    pub fn v0_1() -> Self {
        let contract = ProductionKernelContract::v0_1();
        Self {
            spec_version: contract.spec_version,
            public_types: vec![
                "GlyphWorld".to_string(),
                "Glyph".to_string(),
                "GlyphPose".to_string(),
                "GlyphPatch".to_string(),
                "PatchOp".to_string(),
                "PolicyContext".to_string(),
                "ValidationReport".to_string(),
                "Capability".to_string(),
                "AccessibilityNode".to_string(),
                "ProductionKernelContract".to_string(),
            ],
            public_functions: vec![
                "canonicalize_world".to_string(),
                "semantic_diff".to_string(),
                "detect_patch_conflicts".to_string(),
                "GlyphWorld::to_canonical_json".to_string(),
                "GlyphWorld::stable_layout_hash".to_string(),
                "SchemaMigrationRegistry::migrate_world".to_string(),
            ],
            feature_flags: contract.supported_feature_flags,
            allowed_extension_roots: contract.allowed_extension_roots,
            semver_guarantees: vec![
                "patch releases may add fixtures but cannot break canonical serialization".to_string(),
                "minor releases may add optional fields behind feature flags".to_string(),
                "major releases are required for removing public fields or changing patch semantics"
                    .to_string(),
            ],
            error_codes: vec![
                FormalErrorCode::SchemaVersionUnsupported,
                FormalErrorCode::FeatureFlagUnsupported,
                FormalErrorCode::ExtensionNamespaceInvalid,
                FormalErrorCode::PerformanceBudgetExceeded,
                FormalErrorCode::MigrationUnavailable,
                FormalErrorCode::InvalidWorld,
                FormalErrorCode::InvalidPatch,
                FormalErrorCode::InvalidPolicy,
                FormalErrorCode::InvalidLayout,
                FormalErrorCode::PublicApiUnstable,
            ],
        }
    }
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

impl PolicyRule {
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
        }
    }
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
    if before.spec_version != after.spec_version {
        changes.push(SemanticChange {
            kind: SemanticChangeKind::MetadataChanged,
            path: "spec_version".to_string(),
            before: Some(before.spec_version.clone()),
            after: Some(after.spec_version.clone()),
        });
    }
    if before.name != after.name {
        changes.push(SemanticChange {
            kind: SemanticChangeKind::MetadataChanged,
            path: "name".to_string(),
            before: Some(before.name.clone()),
            after: Some(after.name.clone()),
        });
    }
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
    diff_edges(before, after, &mut changes);
    if before.policies != after.policies {
        changes.push(SemanticChange {
            kind: SemanticChangeKind::MetadataChanged,
            path: "policies".to_string(),
            before: serde_json::to_string(&before.policies).ok(),
            after: serde_json::to_string(&after.policies).ok(),
        });
    }
    if before.spatial_semantics != after.spatial_semantics {
        changes.push(SemanticChange {
            kind: SemanticChangeKind::MetadataChanged,
            path: "spatial_semantics".to_string(),
            before: serde_json::to_string(&before.spatial_semantics).ok(),
            after: serde_json::to_string(&after.spatial_semantics).ok(),
        });
    }
    diff_metadata(&before.metadata, &after.metadata, &mut changes);
    SemanticDiff { changes }
}

fn diff_glyph(id: &str, before: &Glyph, after: &Glyph, changes: &mut Vec<SemanticChange>) {
    if before.kind != after.kind {
        changes.push(SemanticChange {
            kind: SemanticChangeKind::GlyphChanged,
            path: format!("glyphs.{id}.kind"),
            before: Some(format!("{:?}", before.kind)),
            after: Some(format!("{:?}", after.kind)),
        });
    }
    if before.label != after.label {
        changes.push(SemanticChange {
            kind: SemanticChangeKind::GlyphChanged,
            path: format!("glyphs.{id}.label"),
            before: Some(before.label.clone()),
            after: Some(after.label.clone()),
        });
    }
    if before.semantic_role != after.semantic_role {
        changes.push(SemanticChange {
            kind: SemanticChangeKind::GlyphChanged,
            path: format!("glyphs.{id}.semantic_role"),
            before: Some(format!("{:?}", before.semantic_role)),
            after: Some(format!("{:?}", after.semantic_role)),
        });
    }
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
    if before.style != after.style {
        changes.push(SemanticChange {
            kind: SemanticChangeKind::GlyphChanged,
            path: format!("glyphs.{id}.style"),
            before: serde_json::to_string(&before.style).ok(),
            after: serde_json::to_string(&after.style).ok(),
        });
    }
    if before.mass != after.mass {
        changes.push(SemanticChange {
            kind: SemanticChangeKind::GlyphChanged,
            path: format!("glyphs.{id}.mass"),
            before: Some(before.mass.to_string()),
            after: Some(after.mass.to_string()),
        });
    }
    if before.data_binding != after.data_binding {
        changes.push(SemanticChange {
            kind: SemanticChangeKind::GlyphChanged,
            path: format!("glyphs.{id}.data_binding"),
            before: serde_json::to_string(&before.data_binding).ok(),
            after: serde_json::to_string(&after.data_binding).ok(),
        });
    }
    if before.capability_bindings != after.capability_bindings {
        changes.push(SemanticChange {
            kind: SemanticChangeKind::GlyphChanged,
            path: format!("glyphs.{id}.capability_bindings"),
            before: serde_json::to_string(&before.capability_bindings).ok(),
            after: serde_json::to_string(&after.capability_bindings).ok(),
        });
    }
    if before.constraints != after.constraints {
        changes.push(SemanticChange {
            kind: SemanticChangeKind::GlyphChanged,
            path: format!("glyphs.{id}.constraints"),
            before: serde_json::to_string(&before.constraints).ok(),
            after: serde_json::to_string(&after.constraints).ok(),
        });
    }
    if before.affordances != after.affordances {
        changes.push(SemanticChange {
            kind: SemanticChangeKind::GlyphChanged,
            path: format!("glyphs.{id}.affordances"),
            before: serde_json::to_string(&before.affordances).ok(),
            after: serde_json::to_string(&after.affordances).ok(),
        });
    }
    if before.policy_zone != after.policy_zone {
        changes.push(SemanticChange {
            kind: SemanticChangeKind::GlyphChanged,
            path: format!("glyphs.{id}.policy_zone"),
            before: Some(format!("{:?}", before.policy_zone)),
            after: Some(format!("{:?}", after.policy_zone)),
        });
    }
    if before.mandatory != after.mandatory {
        changes.push(SemanticChange {
            kind: SemanticChangeKind::GlyphChanged,
            path: format!("glyphs.{id}.mandatory"),
            before: Some(before.mandatory.to_string()),
            after: Some(after.mandatory.to_string()),
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
    diff_metadata_paths(
        &before.metadata,
        &after.metadata,
        &format!("glyphs.{id}.metadata"),
        changes,
    );
}

fn diff_edges(before: &GlyphWorld, after: &GlyphWorld, changes: &mut Vec<SemanticChange>) {
    if before.edges == after.edges {
        return;
    }
    let kind = if after.edges.len() < before.edges.len() {
        SemanticChangeKind::EdgeRemoved
    } else {
        SemanticChangeKind::EdgeAdded
    };
    changes.push(SemanticChange {
        kind,
        path: "edges".to_string(),
        before: serde_json::to_string(&before.edges).ok(),
        after: serde_json::to_string(&after.edges).ok(),
    });
}

fn diff_metadata(before: &Metadata, after: &Metadata, changes: &mut Vec<SemanticChange>) {
    diff_metadata_paths(before, after, "metadata", changes);
}

fn diff_metadata_paths(
    before: &Metadata,
    after: &Metadata,
    prefix: &str,
    changes: &mut Vec<SemanticChange>,
) {
    let mut keys = before.keys().chain(after.keys()).collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    for key in keys {
        if before.get(key) != after.get(key) {
            changes.push(SemanticChange {
                kind: SemanticChangeKind::MetadataChanged,
                path: format!("{prefix}.{key}"),
                before: before
                    .get(key)
                    .and_then(|value| serde_json::to_string(value).ok()),
                after: after
                    .get(key)
                    .and_then(|value| serde_json::to_string(value).ok()),
            });
        }
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
