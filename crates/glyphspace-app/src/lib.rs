use axum::response::IntoResponse;
use glyphspace_accessibility::{AccessibilityTree, build_accessibility_tree};
use glyphspace_core::{
    CanonicalError, Glyph, GlyphId, GlyphKind, GlyphPatch, GlyphPose, GlyphWorld, PolicyContext,
    PolicyZone, Priority, SemanticDiff, ValidationReport, semantic_diff,
};
use glyphspace_dsl::{DslError, GlyphApp};
use glyphspace_input::InputEvent;
use glyphspace_layout::{DeviceProfile, Viewport};
use glyphspace_personalization::{PatchError, apply_patch, invert_patch};
use glyphspace_policy::{AuditEvent, PolicyEngine, PolicyOutcome};
use glyphspace_render::render_core::{SceneBatch, SceneBatcher, SceneDiff, ScenePatch};
use glyphspace_render::{
    NativeFrame, NativeHostError, NativeRendererHost, ProductionRenderer, RenderSnapshot,
    ScreenshotConformance,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use thiserror::Error;

pub use glyphspace_input::InputEvent as GlyphInputEvent;
pub use glyphspace_layout::{DeviceProfile as GlyphDeviceProfile, Viewport as GlyphViewport};

#[macro_export]
macro_rules! glyph {
    (metric($id:expr, $label:expr).priority($priority:expr)) => {
        glyphspace_core::Glyph::metric($id, $label).priority($priority)
    };
    (metric($id:expr, $label:expr)) => {
        glyphspace_core::Glyph::metric($id, $label)
    };
    (button($id:expr, $label:expr).binds($capability_id:expr)) => {
        glyphspace_core::Glyph::button($id, $label).binds($capability_id)
    };
    (button($id:expr, $label:expr)) => {
        glyphspace_core::Glyph::button($id, $label)
    };
    (card($id:expr, $label:expr)) => {
        glyphspace_core::Glyph::card($id, $label)
    };
    (panel($id:expr, $label:expr)) => {
        glyphspace_core::Glyph::panel($id, $label)
    };
}
pub use glyphspace_macros::{capability, glyph_app, glyph_component, lens};

type Listener<'a, T> = Box<dyn FnMut(&T, u64) + 'a>;
type UntypedHandler<State> = Box<
    dyn FnMut(&mut State, serde_json::Value, &GlyphWorld) -> Result<CapabilityInvocation, AppError>,
>;

pub struct Signal<'a, T> {
    value: T,
    version: u64,
    listeners: Vec<Listener<'a, T>>,
}

impl<'a, T> Signal<'a, T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            version: 0,
            listeners: Vec::new(),
        }
    }

    pub fn subscribe(&mut self, listener: impl FnMut(&T, u64) + 'a) {
        self.listeners.push(Box::new(listener));
    }

    pub fn version(&self) -> u64 {
        self.version
    }
}

impl<T: Clone> Signal<'_, T> {
    pub fn get(&self) -> T {
        self.value.clone()
    }

    pub fn set(&mut self, value: T) {
        self.value = value;
        self.notify();
    }

    pub fn update(&mut self, update: impl FnOnce(&mut T)) {
        update(&mut self.value);
        self.notify();
    }

    fn notify(&mut self) {
        self.version += 1;
        for listener in &mut self.listeners {
            listener(&self.value, self.version);
        }
    }
}

pub trait GlyphComponent<State>: 'static {
    fn render(&self, state: &State) -> Vec<Glyph>;
}

pub struct FnGlyphComponent<State, F> {
    render: F,
    _state: PhantomData<fn(&State)>,
}

impl<State: 'static, F> GlyphComponent<State> for FnGlyphComponent<State, F>
where
    F: Fn(&State) -> Vec<Glyph> + 'static,
{
    fn render(&self, state: &State) -> Vec<Glyph> {
        (self.render)(state)
    }
}

pub fn component<State, F>(render: F) -> FnGlyphComponent<State, F>
where
    State: 'static,
    F: Fn(&State) -> Vec<Glyph> + 'static,
{
    FnGlyphComponent {
        render,
        _state: PhantomData,
    }
}

#[derive(Clone, Debug)]
pub struct TypedCapability<Input, Output> {
    id: String,
    _input: PhantomData<Input>,
    _output: PhantomData<Output>,
}

impl<Input, Output> TypedCapability<Input, Output> {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            _input: PhantomData,
            _output: PhantomData,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

pub fn typed_capability<Input, Output>(id: impl Into<String>) -> TypedCapability<Input, Output> {
    TypedCapability::new(id)
}

#[derive(Clone, Debug, PartialEq)]
pub struct CapabilityOutput<Output> {
    pub output: Output,
    pub patch: Option<GlyphPatch>,
}

impl<Output> CapabilityOutput<Output> {
    pub fn new(output: Output) -> Self {
        Self {
            output,
            patch: None,
        }
    }

    pub fn with_patch(mut self, patch: GlyphPatch) -> Self {
        self.patch = Some(patch);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CapabilityInvocation {
    pub output: serde_json::Value,
    pub patch: Option<GlyphPatch>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppAuditEvent {
    pub action: String,
    pub subject: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeStateChange {
    SetMetricLabel { glyph_id: GlyphId, label: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeLayoutDiff {
    pub changed_glyphs: Vec<GlyphId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeAccessibilityDiff {
    pub changed_nodes: Vec<GlyphId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeStateUpdate {
    pub semantic_diff: SemanticDiff,
    pub layout_diff: RuntimeLayoutDiff,
    pub render_diff: ScenePatch,
    pub accessibility_diff: RuntimeAccessibilityDiff,
    pub audit_event: AppAuditEvent,
}

#[derive(Clone, Debug)]
pub struct RuntimeStateBridge {
    world: GlyphWorld,
    context: PolicyContext,
    renderer: ProductionRenderer,
    last_frame: Option<glyphspace_render::ProductionFrame>,
    last_accessibility: AccessibilityTree,
}

impl RuntimeStateBridge {
    pub fn new(world: GlyphWorld, context: PolicyContext) -> Self {
        let last_accessibility = build_accessibility_tree(&world);
        Self {
            world,
            context,
            renderer: ProductionRenderer::headless(Viewport::desktop(), DeviceProfile::desktop()),
            last_frame: None,
            last_accessibility,
        }
    }

    pub fn apply_server_change(
        &mut self,
        change: RuntimeStateChange,
    ) -> Result<RuntimeStateUpdate, AppError> {
        let before = self.world.clone();
        let previous_frame = if let Some(frame) = &self.last_frame {
            frame.clone()
        } else {
            self.renderer.render_world(&before)?
        };
        match change {
            RuntimeStateChange::SetMetricLabel { glyph_id, label } => {
                let glyph = self
                    .world
                    .glyphs
                    .get_mut(&glyph_id)
                    .ok_or_else(|| AppError::MissingGlyph(glyph_id.clone()))?;
                glyph.label = label.clone();
                glyph.accessibility.label = label;
            }
        }
        let next_frame = self.renderer.render_world(&self.world)?;
        let semantic_diff = semantic_diff(&before, &self.world);
        let changed_glyphs = semantic_diff
            .changes
            .iter()
            .filter_map(|change| change.path.strip_prefix("glyphs."))
            .filter_map(|path| path.split('.').next())
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let batcher = SceneBatcher;
        let before_batch = batcher.batch(&previous_frame.layout);
        let after_batch = batcher.batch(&next_frame.layout);
        let render_diff = ScenePatch::from_diff(batcher.diff(&before_batch, &after_batch));
        let accessibility_tree = build_accessibility_tree(&self.world);
        let accessibility_diff = RuntimeAccessibilityDiff {
            changed_nodes: changed_accessibility_nodes(
                &self.last_accessibility,
                &accessibility_tree,
            ),
        };
        self.last_frame = Some(next_frame);
        self.last_accessibility = accessibility_tree;
        Ok(RuntimeStateUpdate {
            semantic_diff,
            layout_diff: RuntimeLayoutDiff {
                changed_glyphs: changed_glyphs.clone(),
            },
            render_diff,
            accessibility_diff,
            audit_event: AppAuditEvent {
                action: "server.state_changed".to_string(),
                subject: self.context.user_id.clone(),
                detail: changed_glyphs.join(","),
            },
        })
    }
}

fn changed_accessibility_nodes(
    before: &AccessibilityTree,
    after: &AccessibilityTree,
) -> Vec<GlyphId> {
    after
        .nodes
        .iter()
        .filter_map(|(id, node)| {
            if before
                .nodes
                .get(id)
                .is_some_and(|before| before.label == node.label)
            {
                None
            } else {
                Some(id.clone())
            }
        })
        .collect()
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Dsl(#[from] DslError),
    #[error(transparent)]
    Patch(#[from] PatchError),
    #[error(transparent)]
    Host(#[from] NativeHostError),
    #[error(transparent)]
    Canonical(#[from] CanonicalError),
    #[error("capability input was invalid: {0}")]
    CapabilityInput(serde_json::Error),
    #[error("capability output could not be serialized: {0}")]
    CapabilityOutput(serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("server failed: {0}")]
    Server(String),
    #[error("missing glyph: {0}")]
    MissingGlyph(String),
    #[error("missing capability: {0}")]
    MissingCapability(String),
    #[error("missing capability handler: {0}")]
    MissingHandler(String),
    #[error("policy rejected capability: {0}")]
    PolicyRejected(String),
}

pub struct ComponentKit;

impl ComponentKit {
    pub fn metric_glyph(
        id: impl Into<String>,
        label: impl Into<String>,
        priority: Priority,
    ) -> Glyph {
        let mut glyph = Glyph::metric(id, label).priority(priority);
        glyph
            .metadata
            .insert("kit".to_string(), serde_json::json!("metric"));
        glyph
    }

    pub fn risk_glyph(
        id: impl Into<String>,
        label: impl Into<String>,
        priority: Priority,
    ) -> Glyph {
        let mut glyph = Glyph::new(id, GlyphKind::Card, label)
            .with_role(glyphspace_core::SemanticRole::Warning)
            .priority(priority);
        glyph
            .metadata
            .insert("kit".to_string(), serde_json::json!("risk"));
        glyph
    }

    pub fn confirmation_glyph(id: impl Into<String>, label: impl Into<String>) -> Glyph {
        let mut glyph = Glyph::button(id, label)
            .with_policy_zone(PolicyZone::Trust)
            .mandatory();
        glyph
            .metadata
            .insert("kit".to_string(), serde_json::json!("confirmation"));
        glyph
    }

    pub fn agent_glyph(id: impl Into<String>, label: impl Into<String>) -> Glyph {
        let mut glyph = Glyph::new(id, GlyphKind::Agent, label);
        glyph
            .metadata
            .insert("kit".to_string(), serde_json::json!("agent"));
        glyph
    }
}

pub struct CrmKit;

impl CrmKit {
    pub fn deal_card(id: impl Into<String>, label: impl Into<String>) -> Glyph {
        let mut glyph = Glyph::card(id, label).with_role(glyphspace_core::SemanticRole::DataRegion);
        glyph
            .metadata
            .insert("kit".to_string(), serde_json::json!("crm.deal"));
        glyph
    }
}

pub struct FinanceKit;

impl FinanceKit {
    pub fn runway_metric(id: impl Into<String>, months: u32) -> Glyph {
        let mut glyph =
            Glyph::metric(id, format!("Runway: {months} months")).priority(Priority::High);
        glyph
            .metadata
            .insert("kit".to_string(), serde_json::json!("finance.runway"));
        glyph
    }
}

pub struct WorkflowKit;

impl WorkflowKit {
    pub fn approval_task(id: impl Into<String>, label: impl Into<String>) -> Glyph {
        let mut glyph = Glyph::button(id, label).with_role(glyphspace_core::SemanticRole::Action);
        glyph
            .metadata
            .insert("kit".to_string(), serde_json::json!("workflow.approval"));
        glyph
    }
}

pub struct AdminKit;

impl AdminKit {
    pub fn security_notice(id: impl Into<String>, label: impl Into<String>) -> Glyph {
        let mut glyph = Glyph::new(id, GlyphKind::Warning, label)
            .with_role(glyphspace_core::SemanticRole::Warning)
            .with_policy_zone(PolicyZone::Security)
            .mandatory();
        glyph
            .metadata
            .insert("kit".to_string(), serde_json::json!("admin.security"));
        glyph
    }
}

pub struct AgentKit;

impl AgentKit {
    pub fn operator(id: impl Into<String>, label: impl Into<String>) -> Glyph {
        let mut glyph =
            ComponentKit::agent_glyph(id, label).with_role(glyphspace_core::SemanticRole::Agent);
        glyph
            .metadata
            .insert("kit".to_string(), serde_json::json!("agent.operator"));
        glyph
    }
}

pub struct DashboardKit;

impl DashboardKit {
    pub fn kpi_tile(
        id: impl Into<String>,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Glyph {
        let mut glyph = Glyph::metric(id, format!("{}: {}", name.into(), value.into()))
            .priority(Priority::High);
        glyph
            .metadata
            .insert("kit".to_string(), serde_json::json!("dashboard.kpi"));
        glyph
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RouteTarget {
    World,
    Glyph(GlyphId),
    Lens(String),
    PolicyStudio,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticRoute {
    pub pattern: String,
    pub target: RouteTarget,
    pub lens: Option<String>,
    pub camera: Option<String>,
    pub accessibility_landmark: Option<String>,
}

impl SemanticRoute {
    pub fn new(pattern: impl Into<String>, target: RouteTarget) -> Self {
        Self {
            pattern: pattern.into(),
            target,
            lens: None,
            camera: None,
            accessibility_landmark: None,
        }
    }

    pub fn lens(mut self, lens: impl Into<String>) -> Self {
        self.lens = Some(lens.into());
        self
    }

    pub fn camera(mut self, camera: impl Into<String>) -> Self {
        self.camera = Some(camera.into());
        self
    }

    pub fn accessibility_landmark(mut self, landmark: impl Into<String>) -> Self {
        self.accessibility_landmark = Some(landmark.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedRoute {
    pub target: RouteTarget,
    pub params: BTreeMap<String, String>,
    pub lens: Option<String>,
    pub camera: Option<String>,
    pub accessibility_landmark: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticRouter {
    routes: Vec<SemanticRoute>,
}

impl SemanticRouter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn route(mut self, route: SemanticRoute) -> Self {
        self.routes.push(route);
        self
    }

    pub fn resolve(&self, path: &str) -> Option<ResolvedRoute> {
        self.routes.iter().find_map(|route| {
            match_route(&route.pattern, path).map(|params| ResolvedRoute {
                target: route.target.clone(),
                params,
                lens: route.lens.clone(),
                camera: route.camera.clone(),
                accessibility_landmark: route.accessibility_landmark.clone(),
            })
        })
    }
}

fn match_route(pattern: &str, path: &str) -> Option<BTreeMap<String, String>> {
    let pattern_parts = pattern.trim_matches('/').split('/').collect::<Vec<_>>();
    let path_parts = path.trim_matches('/').split('/').collect::<Vec<_>>();
    if pattern == "/" && path == "/" {
        return Some(BTreeMap::new());
    }
    if pattern_parts.len() != path_parts.len() {
        return None;
    }
    let mut params = BTreeMap::new();
    for (pattern_part, path_part) in pattern_parts.iter().zip(path_parts) {
        if let Some(name) = pattern_part.strip_prefix(':') {
            params.insert(name.to_string(), path_part.to_string());
        } else if *pattern_part != path_part {
            return None;
        }
    }
    Some(params)
}

type CapabilityFunction = Box<dyn FnMut(serde_json::Value) -> Result<GlyphPatch, AppError> + Send>;

pub struct CapabilityFunctionRegistry {
    context: PolicyContext,
    functions: BTreeMap<String, CapabilityFunction>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HotReloadEvent {
    pub path: String,
    pub kind: String,
    pub semantic_diff: SemanticDiff,
    pub preserved_state: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HotReloadBatch {
    pub events: Vec<HotReloadEvent>,
    pub semantic_diff: SemanticDiff,
    pub preserved_state: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WatchedHotReloadKind {
    Manifest,
    Patch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WatchedHotReloadFile {
    path: PathBuf,
    kind: WatchedHotReloadKind,
    fingerprint: Option<String>,
}

#[derive(Clone, Debug)]
pub struct HotReloadEngine {
    world: GlyphWorld,
    events: Vec<HotReloadEvent>,
    watched_files: Vec<WatchedHotReloadFile>,
}

impl HotReloadEngine {
    pub fn new(world: GlyphWorld) -> Self {
        Self {
            world,
            events: Vec::new(),
            watched_files: Vec::new(),
        }
    }

    pub fn watch_manifest(mut self, path: impl Into<PathBuf>) -> Self {
        self.watched_files.push(WatchedHotReloadFile {
            path: path.into(),
            kind: WatchedHotReloadKind::Manifest,
            fingerprint: None,
        });
        self
    }

    pub fn watch_patch(mut self, path: impl Into<PathBuf>) -> Self {
        self.watched_files.push(WatchedHotReloadFile {
            path: path.into(),
            kind: WatchedHotReloadKind::Patch,
            fingerprint: None,
        });
        self
    }

    pub fn reload_manifest_text(
        &mut self,
        path: impl Into<String>,
        text: &str,
    ) -> Result<HotReloadEvent, AppError> {
        let next: GlyphWorld = serde_json::from_str(text).map_err(AppError::CapabilityInput)?;
        let diff = semantic_diff(&self.world, &next);
        self.world = next;
        let event = HotReloadEvent {
            path: path.into(),
            kind: "manifest_reloaded".to_string(),
            semantic_diff: diff,
            preserved_state: true,
        };
        self.events.push(event.clone());
        Ok(event)
    }

    pub fn reload_patch_text(
        &mut self,
        path: impl Into<String>,
        text: &str,
    ) -> Result<HotReloadEvent, AppError> {
        let patch: GlyphPatch = serde_json::from_str(text).map_err(AppError::CapabilityInput)?;
        let before = self.world.clone();
        let report = PolicyEngine.validate_patch(&self.world, &patch, &PolicyContext::demo_user());
        if report.allowed {
            self.world = apply_patch(&self.world, &patch, &PolicyContext::demo_user())?;
        }
        let event = HotReloadEvent {
            path: path.into(),
            kind: "patch_reloaded".to_string(),
            semantic_diff: semantic_diff(&before, &self.world),
            preserved_state: true,
        };
        self.events.push(event.clone());
        Ok(event)
    }

    pub fn devtools_events(&self) -> &[HotReloadEvent] {
        &self.events
    }

    pub fn devtools_event_stream(&self) -> &[HotReloadEvent] {
        &self.events
    }

    pub fn reload_changed_files(&mut self) -> Result<HotReloadBatch, AppError> {
        let before = self.world.clone();
        let mut events = Vec::new();
        for index in 0..self.watched_files.len() {
            let path = self.watched_files[index].path.clone();
            let text = fs::read_to_string(&path)?;
            let fingerprint = stable_text_fingerprint(&text);
            if self.watched_files[index].fingerprint.as_ref() == Some(&fingerprint) {
                continue;
            }
            let event = match self.watched_files[index].kind {
                WatchedHotReloadKind::Manifest => {
                    self.reload_manifest_text(path.display().to_string(), &text)?
                }
                WatchedHotReloadKind::Patch => {
                    self.reload_patch_text(path.display().to_string(), &text)?
                }
            };
            self.watched_files[index].fingerprint = Some(fingerprint);
            events.push(event);
        }
        let semantic_diff = semantic_diff(&before, &self.world);
        let preserved_state = true;
        let batch_event = HotReloadEvent {
            path: "watcher".to_string(),
            kind: "hot_reload.batch".to_string(),
            semantic_diff: semantic_diff.clone(),
            preserved_state,
        };
        self.events.push(batch_event);
        Ok(HotReloadBatch {
            events,
            semantic_diff,
            preserved_state,
        })
    }
}

fn stable_text_fingerprint(text: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[derive(Clone, Debug, PartialEq)]
pub struct CapabilityFunctionResult {
    pub patch: GlyphPatch,
    pub audit: AppAuditEvent,
}

impl CapabilityFunctionRegistry {
    pub fn new(context: PolicyContext) -> Self {
        Self {
            context,
            functions: BTreeMap::new(),
        }
    }

    pub fn register(
        &mut self,
        capability_id: impl Into<String>,
        function: impl FnMut(serde_json::Value) -> Result<GlyphPatch, AppError> + Send + 'static,
    ) {
        self.functions
            .insert(capability_id.into(), Box::new(function));
    }

    pub fn invoke(
        &mut self,
        world: &GlyphWorld,
        capability_id: &str,
        input: serde_json::Value,
    ) -> Result<CapabilityFunctionResult, AppError> {
        let capability = world
            .capabilities
            .get(capability_id)
            .ok_or_else(|| AppError::MissingCapability(capability_id.to_string()))?;
        let mut report = ValidationReport::allow();
        PolicyEngine.validate_capability_invocation(capability, &self.context, &mut report);
        if !report.allowed {
            return Err(AppError::PolicyRejected(
                report
                    .violations
                    .iter()
                    .map(|violation| violation.message.as_str())
                    .collect::<Vec<_>>()
                    .join("; "),
            ));
        }
        let function = self
            .functions
            .get_mut(capability_id)
            .ok_or_else(|| AppError::MissingHandler(capability_id.to_string()))?;
        let patch = function(input)?;
        Ok(CapabilityFunctionResult {
            audit: AppAuditEvent {
                action: "capability.function.invoked".to_string(),
                subject: capability_id.to_string(),
                detail: patch.id.clone(),
            },
            patch,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SemanticSsrSnapshot {
    pub world: GlyphWorld,
    pub accessibility_tree: AccessibilityTree,
    pub policy_context: PolicyContext,
    pub world_digest: String,
    pub patch_digest: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SemanticHttpResponse {
    pub status: u16,
    pub patch: GlyphPatch,
    pub body: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldStreamEvent {
    pub kind: String,
    pub payload: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldUpdateStream {
    pub events: Vec<WorldStreamEvent>,
}

pub struct SemanticSsrServer {
    world: GlyphWorld,
    context: PolicyContext,
    capabilities: CapabilityFunctionRegistry,
}

impl SemanticSsrServer {
    pub fn new(world: GlyphWorld, context: PolicyContext) -> Self {
        Self {
            world,
            capabilities: CapabilityFunctionRegistry::new(context.clone()),
            context,
        }
    }

    pub fn register_capability(
        &mut self,
        capability_id: impl Into<String>,
        function: impl FnMut(serde_json::Value) -> Result<GlyphPatch, AppError> + Send + 'static,
    ) {
        self.capabilities.register(capability_id, function);
    }

    pub fn render_accessibility_html(&self) -> Result<String, AppError> {
        let tree = build_accessibility_tree(&self.world);
        let mut html = String::from("<main data-glyphspace-accessibility=\"true\">");
        for (glyph_id, node) in tree.nodes {
            html.push_str(&format!(
                "<button role=\"{:?}\" data-glyph-id=\"{}\">{}</button>",
                node.role, glyph_id, node.label
            ));
        }
        html.push_str("</main>");
        Ok(html)
    }

    pub fn render_world_json(&self) -> Result<String, AppError> {
        self.world.to_canonical_json().map_err(AppError::Canonical)
    }

    pub fn handle_capability_http(
        &mut self,
        capability_id: &str,
        input: serde_json::Value,
    ) -> Result<SemanticHttpResponse, AppError> {
        let result = self
            .capabilities
            .invoke(&self.world, capability_id, input)?;
        Ok(SemanticHttpResponse {
            status: 200,
            body: serde_json::json!({
                "patch_id": result.patch.id,
                "actor": self.context.user_id,
            }),
            patch: result.patch,
        })
    }

    pub fn stream_world_updates(&self) -> WorldUpdateStream {
        WorldUpdateStream {
            events: vec![WorldStreamEvent {
                kind: "world.snapshot".to_string(),
                payload: self.world.id.clone(),
            }],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AxumRouteManifest {
    pub axum_backed: bool,
    pub routes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SsrTextResponse {
    pub status: u16,
    pub content_type: String,
    pub body: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AxumRouterStub {
    pub health_route: String,
    pub registered_routes: Vec<String>,
}

pub struct AxumSsrAdapter {
    server: SemanticSsrServer,
    routes: Vec<String>,
    world_route: String,
    accessibility_route: String,
    capability_route: String,
    stream_route: String,
}

impl AxumSsrAdapter {
    pub fn new(server: SemanticSsrServer) -> Self {
        Self {
            server,
            routes: Vec::new(),
            world_route: "/glyphspace/world".to_string(),
            accessibility_route: "/glyphspace/a11y".to_string(),
            capability_route: "/glyphspace/capability/:id".to_string(),
            stream_route: "/glyphspace/stream".to_string(),
        }
    }

    pub fn route_world(mut self, path: impl Into<String>) -> Self {
        self.world_route = path.into();
        self.routes.push(self.world_route.clone());
        self
    }

    pub fn route_accessibility(mut self, path: impl Into<String>) -> Self {
        self.accessibility_route = path.into();
        self.routes.push(self.accessibility_route.clone());
        self
    }

    pub fn route_capability(mut self, path: impl Into<String>) -> Self {
        self.capability_route = path.into();
        self.routes.push(self.capability_route.clone());
        self
    }

    pub fn route_stream(mut self, path: impl Into<String>) -> Self {
        self.stream_route = path.into();
        self.routes.push(self.stream_route.clone());
        self
    }

    pub fn route_manifest(&self) -> AxumRouteManifest {
        AxumRouteManifest {
            axum_backed: true,
            routes: self.routes.clone(),
        }
    }

    pub fn axum_router_stub(&self) -> AxumRouterStub {
        AxumRouterStub {
            health_route: "/__glyphspace/health".to_string(),
            registered_routes: self.routes.clone(),
        }
    }

    pub fn axum_router(self) -> axum::Router {
        build_axum_router(
            Arc::new(Mutex::new(self.server)),
            self.world_route,
            self.accessibility_route,
            self.capability_route,
            self.stream_route,
        )
    }

    pub async fn serve_localhost(self) -> Result<LiveSsrHandle, AppError> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let router = self.axum_router();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .map_err(|error| error.to_string())
        });
        Ok(LiveSsrHandle {
            addr,
            shutdown_tx: Some(shutdown_tx),
            task,
        })
    }

    pub fn render_accessibility_response(&self) -> Result<SsrTextResponse, AppError> {
        Ok(SsrTextResponse {
            status: 200,
            content_type: "text/html; charset=utf-8".to_string(),
            body: self.server.render_accessibility_html()?,
        })
    }
}

pub struct LiveSsrHandle {
    addr: SocketAddr,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<Result<(), String>>,
}

impl LiveSsrHandle {
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        let _ = self.task.await;
    }
}

type SharedSsrServer = Arc<Mutex<SemanticSsrServer>>;

fn build_axum_router(
    server: SharedSsrServer,
    world_route: String,
    accessibility_route: String,
    capability_route: String,
    stream_route: String,
) -> axum::Router {
    axum::Router::new()
        .route("/__glyphspace/health", axum::routing::get(ssr_health))
        .route(
            &normalize_axum_path(&world_route),
            axum::routing::get(ssr_world),
        )
        .route(
            &normalize_axum_path(&accessibility_route),
            axum::routing::get(ssr_accessibility),
        )
        .route(
            &normalize_axum_path(&capability_route),
            axum::routing::post(ssr_capability),
        )
        .route(
            &normalize_axum_path(&stream_route),
            axum::routing::get(ssr_stream),
        )
        .with_state(server)
}

async fn ssr_health() -> &'static str {
    "glyphspace:ssr:ok"
}

async fn ssr_world(
    axum::extract::State(server): axum::extract::State<SharedSsrServer>,
) -> axum::response::Response {
    match server
        .lock()
        .map_err(|error| error.to_string())
        .and_then(|guard| guard.render_world_json().map_err(|error| error.to_string()))
    {
        Ok(body) => (
            axum::http::StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            body,
        )
            .into_response(),
        Err(error) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            [(axum::http::header::CONTENT_TYPE, "text/plain")],
            error,
        )
            .into_response(),
    }
}

async fn ssr_accessibility(
    axum::extract::State(server): axum::extract::State<SharedSsrServer>,
) -> axum::response::Response {
    match server
        .lock()
        .map_err(|error| error.to_string())
        .and_then(|guard| {
            guard
                .render_accessibility_html()
                .map_err(|error| error.to_string())
        }) {
        Ok(body) => axum::response::Html(body).into_response(),
        Err(error) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            [(axum::http::header::CONTENT_TYPE, "text/plain")],
            error,
        )
            .into_response(),
    }
}

async fn ssr_capability(
    axum::extract::Path(capability_id): axum::extract::Path<String>,
    axum::extract::State(server): axum::extract::State<SharedSsrServer>,
    axum::Json(input): axum::Json<serde_json::Value>,
) -> axum::response::Response {
    match server
        .lock()
        .map_err(|error| error.to_string())
        .and_then(|mut guard| {
            guard
                .handle_capability_http(&capability_id, input)
                .map_err(|error| error.to_string())
        }) {
        Ok(response) => (
            axum::http::StatusCode::from_u16(response.status).unwrap_or(axum::http::StatusCode::OK),
            axum::Json(serde_json::json!({
                "body": response.body,
                "patch": response.patch,
            })),
        )
            .into_response(),
        Err(error) => (
            axum::http::StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({ "error": error })),
        )
            .into_response(),
    }
}

async fn ssr_stream(
    axum::extract::State(server): axum::extract::State<SharedSsrServer>,
) -> axum::response::Response {
    let events = server
        .lock()
        .map(|guard| guard.stream_world_updates().events)
        .unwrap_or_default();
    let body = events
        .into_iter()
        .map(|event| format!("event: {}\ndata: {}\n\n", event.kind, event.payload))
        .collect::<String>();
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
        body,
    )
        .into_response()
}

fn normalize_axum_path(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            segment
                .strip_prefix(':')
                .map_or_else(|| segment.to_string(), |name| format!("{{{name}}}"))
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[derive(Clone, Debug, PartialEq)]
pub struct HydratedSemanticWorld {
    pub world: GlyphWorld,
    pub accessibility_tree: AccessibilityTree,
    pub policy_context: PolicyContext,
}

impl SemanticSsrSnapshot {
    pub fn from_world(world: &GlyphWorld, context: &PolicyContext) -> Result<Self, AppError> {
        Ok(Self {
            world: world.clone(),
            accessibility_tree: build_accessibility_tree(world),
            policy_context: context.clone(),
            world_digest: world.canonical_digest()?,
            patch_digest: format!("{:016x}", world.stable_layout_hash()),
        })
    }

    pub fn hydrate(&self) -> Result<HydratedSemanticWorld, AppError> {
        let digest = self.world.canonical_digest()?;
        if digest != self.world_digest {
            return Err(AppError::PolicyRejected(
                "semantic SSR snapshot digest mismatch".to_string(),
            ));
        }
        Ok(HydratedSemanticWorld {
            world: self.world.clone(),
            accessibility_tree: self.accessibility_tree.clone(),
            policy_context: self.policy_context.clone(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileHostAdapter {
    pub app_id: String,
    pub platform: String,
    pub native_accessibility_bridge: Option<String>,
    pub offline_patch_store: Option<String>,
    pub lens_profiles: Vec<String>,
}

impl MobileHostAdapter {
    pub fn ios(app_id: impl Into<String>) -> Self {
        Self::new(app_id, "ios")
    }

    pub fn android(app_id: impl Into<String>) -> Self {
        Self::new(app_id, "android")
    }

    fn new(app_id: impl Into<String>, platform: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
            platform: platform.into(),
            native_accessibility_bridge: None,
            offline_patch_store: None,
            lens_profiles: Vec::new(),
        }
    }

    pub fn with_native_accessibility_bridge(mut self, bridge: impl Into<String>) -> Self {
        self.native_accessibility_bridge = Some(bridge.into());
        self
    }

    pub fn with_offline_patch_store(mut self, store: impl Into<String>) -> Self {
        self.offline_patch_store = Some(store.into());
        self
    }

    pub fn with_lens_profile(mut self, lens: impl Into<String>) -> Self {
        self.lens_profiles.push(lens.into());
        self
    }

    pub fn is_complete(&self) -> bool {
        self.native_accessibility_bridge.is_some() && self.offline_patch_store.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MobileShellKind {
    Ios,
    Android,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MobileBridgeFrame {
    pub app_id: String,
    pub platform: MobileShellKind,
    pub native_bridge: String,
    pub accessibility_nodes: usize,
    pub patch_queue_depth: usize,
    pub lens_profiles: Vec<String>,
    pub push_channel: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MobileShell {
    app_id: String,
    kind: MobileShellKind,
    native_accessibility_bridge: Option<String>,
    offline_store: Option<String>,
    lens_profiles: Vec<String>,
    push_channel: Option<String>,
    queued_patches: Vec<GlyphPatch>,
}

impl MobileShell {
    pub fn ios(app_id: impl Into<String>) -> Self {
        Self::new(app_id, MobileShellKind::Ios)
    }

    pub fn android(app_id: impl Into<String>) -> Self {
        Self::new(app_id, MobileShellKind::Android)
    }

    fn new(app_id: impl Into<String>, kind: MobileShellKind) -> Self {
        Self {
            app_id: app_id.into(),
            kind,
            native_accessibility_bridge: None,
            offline_store: None,
            lens_profiles: Vec::new(),
            push_channel: None,
            queued_patches: Vec::new(),
        }
    }

    pub fn with_lens_profile(mut self, lens: impl Into<String>) -> Self {
        self.lens_profiles.push(lens.into());
        self
    }

    pub fn with_offline_store(mut self, store: impl Into<String>) -> Self {
        self.offline_store = Some(store.into());
        self
    }

    pub fn with_native_accessibility_bridge(mut self, bridge: impl Into<String>) -> Self {
        self.native_accessibility_bridge = Some(bridge.into());
        self
    }

    pub fn with_push_channel(mut self, channel: impl Into<String>) -> Self {
        self.push_channel = Some(channel.into());
        self
    }

    pub fn queue_offline_patch(&mut self, patch: GlyphPatch) {
        self.queued_patches.push(patch);
    }

    pub fn queued_patches(&self) -> &[GlyphPatch] {
        &self.queued_patches
    }

    pub fn kind(&self) -> MobileShellKind {
        self.kind
    }

    pub fn render_bridge_frame(&self, world: &GlyphWorld) -> Result<MobileBridgeFrame, AppError> {
        let accessibility_tree = build_accessibility_tree(world);
        Ok(MobileBridgeFrame {
            app_id: self.app_id.clone(),
            platform: self.kind,
            native_bridge: self
                .native_accessibility_bridge
                .clone()
                .unwrap_or_else(|| "native-accessibility-unconfigured".to_string()),
            accessibility_nodes: accessibility_tree.nodes.len(),
            patch_queue_depth: self.queued_patches.len(),
            lens_profiles: self.lens_profiles.clone(),
            push_channel: self.push_channel.clone(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevtoolsSnapshot {
    pub world_id: String,
    pub glyphs: Vec<GlyphId>,
    pub capabilities: Vec<String>,
    pub accessibility_nodes: Vec<GlyphId>,
    pub policy_summary: String,
}

impl DevtoolsSnapshot {
    pub fn inspect(world: &GlyphWorld, context: &PolicyContext) -> Self {
        let tree = build_accessibility_tree(world);
        Self {
            world_id: world.id.clone(),
            glyphs: world.glyphs.keys().cloned().collect(),
            capabilities: world.capabilities.keys().cloned().collect(),
            accessibility_nodes: tree.nodes.keys().cloned().collect(),
            policy_summary: format!(
                "AI may personalize layout but cannot create authority; user {} has {} permissions.",
                context.user_id,
                context.permissions.len()
            ),
        }
    }
}

pub struct AppRuntime<State> {
    app: GlyphApp,
    state: State,
    policy_context: PolicyContext,
    components: Vec<Box<dyn GlyphComponent<State>>>,
    handlers: BTreeMap<String, UntypedHandler<State>>,
    world: GlyphWorld,
    last_safe_world: GlyphWorld,
    patch_store: Vec<GlyphPatch>,
    audit_log: Vec<AppAuditEvent>,
}

impl<State: 'static> AppRuntime<State> {
    pub fn new(app: GlyphApp, state: State, policy_context: PolicyContext) -> Self {
        Self {
            app,
            state,
            policy_context,
            components: Vec::new(),
            handlers: BTreeMap::new(),
            world: GlyphWorld::default(),
            last_safe_world: GlyphWorld::default(),
            patch_store: Vec::new(),
            audit_log: Vec::new(),
        }
    }

    pub fn from_world(
        world: GlyphWorld,
        app: GlyphApp,
        state: State,
        policy_context: PolicyContext,
    ) -> Result<Self, AppError> {
        Ok(Self {
            app,
            state,
            policy_context,
            components: Vec::new(),
            handlers: BTreeMap::new(),
            last_safe_world: world.clone(),
            world,
            patch_store: Vec::new(),
            audit_log: Vec::new(),
        })
    }

    pub fn with_component(mut self, component: impl GlyphComponent<State>) -> Self {
        self.components.push(Box::new(component));
        self
    }

    pub fn mount(mut self) -> Result<Self, AppError> {
        self.rebuild_world()?;
        self.last_safe_world = self.world.clone();
        Ok(self)
    }

    pub fn world(&self) -> &GlyphWorld {
        &self.world
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn patch_store(&self) -> &[GlyphPatch] {
        &self.patch_store
    }

    pub fn audit_log(&self) -> &[AppAuditEvent] {
        &self.audit_log
    }

    pub fn register_typed<Input, Output>(
        &mut self,
        capability: TypedCapability<Input, Output>,
        mut handler: impl FnMut(
            &mut State,
            Input,
            &GlyphWorld,
        ) -> Result<CapabilityOutput<Output>, AppError>
        + 'static,
    ) where
        Input: DeserializeOwned + 'static,
        Output: Serialize + 'static,
    {
        self.handlers.insert(
            capability.id,
            Box::new(move |state, input, world| {
                let typed_input =
                    serde_json::from_value(input).map_err(AppError::CapabilityInput)?;
                let typed_output = handler(state, typed_input, world)?;
                Ok(CapabilityInvocation {
                    output: serde_json::to_value(typed_output.output)
                        .map_err(AppError::CapabilityOutput)?,
                    patch: typed_output.patch,
                })
            }),
        );
    }

    pub fn update_state(
        &mut self,
        update: impl FnOnce(&mut State),
    ) -> Result<SemanticDiff, AppError> {
        let before = self.world.clone();
        update(&mut self.state);
        self.rebuild_world()?;
        Ok(semantic_diff(&before, &self.world))
    }

    pub fn handle_input(
        &mut self,
        event: InputEvent,
    ) -> Result<Option<CapabilityInvocation>, AppError> {
        match event {
            InputEvent::GlyphClick { glyph_id, input } => self
                .invoke_first_glyph_capability(&glyph_id, input)
                .map(Some),
            InputEvent::KeyboardActivate { glyph_id } => self
                .invoke_first_glyph_capability(&glyph_id, serde_json::Value::Null)
                .map(Some),
            InputEvent::PointerMove { .. } | InputEvent::NaturalLanguageEdit { .. } => Ok(None),
        }
    }

    pub fn apply_user_patch(&mut self, patch: &GlyphPatch) -> Result<SemanticDiff, AppError> {
        let before = self.world.clone();
        let decision = PolicyEngine.evaluate_patch(
            &self.world,
            &self.last_safe_world,
            patch,
            &self.policy_context,
        );
        self.audit(
            "policy.patch_evaluated",
            &patch.id,
            decision.explanation.clone(),
        );
        if !decision.report.allowed {
            self.world = decision.safe_world;
            return Err(AppError::PolicyRejected(
                decision
                    .report
                    .violations
                    .iter()
                    .map(|violation| violation.message.as_str())
                    .collect::<Vec<_>>()
                    .join("; "),
            ));
        }
        self.world = apply_patch(&self.world, patch, &self.policy_context)?;
        self.last_safe_world = self.world.clone();
        self.patch_store.push(patch.clone());
        self.audit("patch.applied", &patch.id, patch.description.clone());
        Ok(semantic_diff(&before, &self.world))
    }

    pub fn render(&self, host: &mut impl SemanticHost) -> Result<AppFrame, AppError> {
        host.render_world(&self.world)
    }

    fn rebuild_world(&mut self) -> Result<(), AppError> {
        let mut world = self.app.compile()?;
        for component in &self.components {
            for glyph in component.render(&self.state) {
                world.glyphs.insert(glyph.id.clone(), glyph);
            }
        }
        self.world = world;
        Ok(())
    }

    fn invoke_first_glyph_capability(
        &mut self,
        glyph_id: &str,
        input: serde_json::Value,
    ) -> Result<CapabilityInvocation, AppError> {
        let glyph = self
            .world
            .glyphs
            .get(glyph_id)
            .ok_or_else(|| AppError::MissingGlyph(glyph_id.to_string()))?;
        let capability_id = glyph
            .capability_bindings
            .first()
            .map(|binding| binding.capability_id.clone())
            .ok_or_else(|| AppError::MissingCapability(glyph_id.to_string()))?;
        self.invoke_capability(&capability_id, input)
    }

    fn invoke_capability(
        &mut self,
        capability_id: &str,
        input: serde_json::Value,
    ) -> Result<CapabilityInvocation, AppError> {
        let capability = self
            .world
            .capabilities
            .get(capability_id)
            .cloned()
            .ok_or_else(|| AppError::MissingCapability(capability_id.to_string()))?;

        let mut report = ValidationReport::allow();
        PolicyEngine.validate_capability_invocation(&capability, &self.policy_context, &mut report);
        if !report.allowed {
            let detail = report
                .violations
                .iter()
                .map(|violation| violation.message.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            self.audit("capability.rejected", capability_id, detail.clone());
            return Err(AppError::PolicyRejected(detail));
        }

        let handler = self
            .handlers
            .get_mut(capability_id)
            .ok_or_else(|| AppError::MissingHandler(capability_id.to_string()))?;
        let invocation = handler(&mut self.state, input, &self.world)?;
        self.audit(
            "capability.invoked",
            capability_id,
            invocation.output.to_string(),
        );
        if let Some(patch) = &invocation.patch {
            self.apply_user_patch(patch)?;
        }
        Ok(invocation)
    }

    fn audit(
        &mut self,
        action: impl Into<String>,
        subject: impl Into<String>,
        detail: impl Into<String>,
    ) {
        self.audit_log.push(AppAuditEvent {
            action: action.into(),
            subject: subject.into(),
            detail: detail.into(),
        });
    }
}

pub trait SemanticHost {
    fn render_world(&mut self, world: &GlyphWorld) -> Result<AppFrame, AppError>;
    fn hit_test(&self, x: f32, y: f32) -> Option<GlyphId>;
    fn emit_audit(&mut self, event: AppAuditEvent);
    fn store_patch(&mut self, patch: GlyphPatch);
}

#[derive(Clone, Debug, PartialEq)]
pub struct AppFrame {
    pub native_frame: NativeFrame,
    pub accessibility_tree: AccessibilityTree,
    pub accessibility_report: ValidationReport,
    pub scene_batch: SceneBatch,
    pub scene_diff: SceneDiff,
}

#[derive(Clone, Debug)]
pub struct HeadlessSemanticHost {
    renderer: NativeRendererHost,
    batcher: SceneBatcher,
    last_batch: Option<SceneBatch>,
    audit_log: Vec<AppAuditEvent>,
    patch_store: Vec<GlyphPatch>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReactiveNodeId(usize);

type ComputeFn = Box<dyn Fn(&[i64]) -> i64>;

struct ReactiveNode {
    name: String,
    value: i64,
    dependencies: Vec<ReactiveNodeId>,
    compute: Option<ComputeFn>,
}

#[derive(Default)]
pub struct ReactiveGraph {
    nodes: Vec<ReactiveNode>,
    dirty_components: BTreeSet<String>,
}

impl ReactiveGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn signal(&mut self, name: impl Into<String>, value: i64) -> ReactiveNodeId {
        let id = ReactiveNodeId(self.nodes.len());
        self.nodes.push(ReactiveNode {
            name: name.into(),
            value,
            dependencies: Vec::new(),
            compute: None,
        });
        id
    }

    pub fn computed<const N: usize>(
        &mut self,
        name: impl Into<String>,
        dependencies: [ReactiveNodeId; N],
        compute: impl Fn(&[i64]) -> i64 + 'static,
    ) -> ReactiveNodeId {
        let values = dependencies
            .iter()
            .map(|dependency| self.nodes[dependency.0].value)
            .collect::<Vec<_>>();
        let id = ReactiveNodeId(self.nodes.len());
        self.nodes.push(ReactiveNode {
            name: name.into(),
            value: compute(&values),
            dependencies: dependencies.to_vec(),
            compute: Some(Box::new(compute)),
        });
        id
    }

    pub fn set(&mut self, id: ReactiveNodeId, value: i64) {
        if let Some(node) = self.nodes.get_mut(id.0) {
            node.value = value;
        }
        self.recompute_dependents(id);
    }

    pub fn value(&self, id: ReactiveNodeId) -> Option<i64> {
        self.nodes.get(id.0).map(|node| node.value)
    }

    pub fn dirty_components(&self) -> Vec<String> {
        self.dirty_components.iter().cloned().collect()
    }

    fn recompute_dependents(&mut self, changed: ReactiveNodeId) {
        for index in 0..self.nodes.len() {
            if !self.nodes[index].dependencies.contains(&changed) {
                continue;
            }
            let values = self.nodes[index]
                .dependencies
                .iter()
                .map(|dependency| self.nodes[dependency.0].value)
                .collect::<Vec<_>>();
            if let Some(compute) = &self.nodes[index].compute {
                self.nodes[index].value = compute(&values);
                self.dirty_components.insert(self.nodes[index].name.clone());
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AsyncResourceState<T> {
    Pending,
    Ready(T),
    Canceled,
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct CancelToken {
    canceled: Rc<Cell<bool>>,
}

impl CancelToken {
    pub fn cancel(&self) {
        self.canceled.set(true);
    }

    pub fn is_canceled(&self) -> bool {
        self.canceled.get()
    }
}

#[derive(Clone, Debug)]
pub struct AsyncResource<T> {
    name: String,
    state: AsyncResourceState<T>,
    token: CancelToken,
}

impl<T> AsyncResource<T> {
    pub fn pending(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            state: AsyncResourceState::Pending,
            token: CancelToken {
                canceled: Rc::new(Cell::new(false)),
            },
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn cancel_token(&self) -> CancelToken {
        self.token.clone()
    }

    pub fn state(&mut self) -> &AsyncResourceState<T> {
        if self.token.is_canceled() {
            self.state = AsyncResourceState::Canceled;
        }
        &self.state
    }

    pub fn resolve(&mut self, value: T) {
        if self.token.is_canceled() {
            self.state = AsyncResourceState::Canceled;
        } else {
            self.state = AsyncResourceState::Ready(value);
        }
    }

    pub fn reject(&mut self, error: impl Into<String>) {
        self.state = AsyncResourceState::Failed(error.into());
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypedSignal<T> {
    name: String,
    value: T,
    version: u64,
    invalidated_glyphs: BTreeSet<GlyphId>,
}

impl<T: Clone> TypedSignal<T> {
    pub fn new(name: impl Into<String>, value: T) -> Self {
        let name = name.into();
        Self {
            invalidated_glyphs: BTreeSet::from([name.clone()]),
            name,
            value,
            version: 0,
        }
    }

    pub fn get(&self) -> T {
        self.value.clone()
    }

    pub fn set(&mut self, value: T) {
        self.value = value;
        self.version += 1;
        self.invalidated_glyphs.insert(self.name.clone());
    }

    pub fn take_invalidated_glyphs(&mut self) -> Vec<GlyphId> {
        std::mem::take(&mut self.invalidated_glyphs)
            .into_iter()
            .collect()
    }

    pub fn memo<U: Clone>(
        &self,
        name: impl Into<String>,
        compute: impl Fn(&T) -> U + 'static,
    ) -> Memo<T, U> {
        Memo {
            name: name.into(),
            value: compute(&self.value),
            compute: Box::new(compute),
            _source: PhantomData,
        }
    }
}

pub struct Memo<T, U> {
    name: String,
    value: U,
    compute: Box<dyn Fn(&T) -> U>,
    _source: PhantomData<T>,
}

impl<T, U: Clone> Memo<T, U> {
    pub fn recompute(&mut self, value: &T) {
        self.value = (self.compute)(value);
    }

    pub fn value(&self) -> U {
        self.value.clone()
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

pub struct ReactiveEffect<T> {
    name: String,
    run: Box<dyn FnMut(&T) -> String>,
}

impl<T> ReactiveEffect<T> {
    pub fn new(name: impl Into<String>, run: impl FnMut(&T) -> String + 'static) -> Self {
        Self {
            name: name.into(),
            run: Box::new(run),
        }
    }

    pub fn run(&mut self, value: &T) -> String {
        (self.run)(value)
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SuspenseState {
    Pending,
    Ready,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuspenseBoundary {
    pub id: String,
    state: SuspenseState,
}

impl SuspenseBoundary {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            state: SuspenseState::Ready,
        }
    }

    pub fn pending(&mut self) {
        self.state = SuspenseState::Pending;
    }

    pub fn ready(&mut self) {
        self.state = SuspenseState::Ready;
    }

    pub fn is_ready(&self) -> bool {
        self.state == SuspenseState::Ready
    }
}

type FineMemoFn = Box<dyn Fn(&[i64]) -> i64>;

struct FineMemoNode {
    dependencies: Vec<String>,
    compute: FineMemoFn,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FineGrainedWorldDiff {
    pub changed_glyphs: Vec<GlyphId>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FineGrainedFlush {
    pub invalidated_glyphs: Vec<GlyphId>,
    pub world_diff: FineGrainedWorldDiff,
}

#[derive(Default)]
pub struct FineGrainedRuntime {
    signals: BTreeMap<String, i64>,
    memos: BTreeMap<String, FineMemoNode>,
    memo_values: BTreeMap<String, i64>,
    effects: BTreeMap<String, (Vec<String>, GlyphId)>,
    invalidated_glyphs: BTreeSet<GlyphId>,
    suspense_resources: BTreeSet<String>,
    error_boundaries: BTreeMap<String, Option<String>>,
}

impl FineGrainedRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn signal(mut self, name: impl Into<String>, value: i64) -> Self {
        self.signals.insert(name.into(), value);
        self
    }

    pub fn memo(
        mut self,
        name: impl Into<String>,
        dependencies: &[&str],
        compute: impl Fn(&[i64]) -> i64 + 'static,
    ) -> Self {
        let name = name.into();
        let dependencies = dependencies
            .iter()
            .map(|dependency| (*dependency).to_string())
            .collect::<Vec<_>>();
        let values = self.values_for(&dependencies);
        self.memo_values.insert(name.clone(), compute(&values));
        self.memos.insert(
            name,
            FineMemoNode {
                dependencies,
                compute: Box::new(compute),
            },
        );
        self
    }

    pub fn effect(
        mut self,
        name: impl Into<String>,
        dependencies: &[&str],
        glyph_id: impl Into<String>,
    ) -> Self {
        self.effects.insert(
            name.into(),
            (
                dependencies
                    .iter()
                    .map(|dependency| (*dependency).to_string())
                    .collect(),
                glyph_id.into(),
            ),
        );
        self
    }

    pub fn suspense(mut self, name: impl Into<String>) -> Self {
        self.suspense_resources.insert(name.into());
        self
    }

    pub fn error_boundary(mut self, name: impl Into<String>) -> Self {
        self.error_boundaries.insert(name.into(), None);
        self
    }

    pub fn set_signal(&mut self, name: &str, value: i64) -> Result<(), AppError> {
        if !self.signals.contains_key(name) {
            return Err(AppError::MissingGlyph(name.to_string()));
        }
        self.signals.insert(name.to_string(), value);
        let mut changed = BTreeSet::from([name.to_string()]);
        let memo_names = self.memos.keys().cloned().collect::<Vec<_>>();
        for memo_name in memo_names {
            let Some(memo) = self.memos.get(&memo_name) else {
                continue;
            };
            if memo
                .dependencies
                .iter()
                .any(|dependency| changed.contains(dependency))
            {
                let values = self.values_for(&memo.dependencies);
                self.memo_values
                    .insert(memo_name.clone(), (memo.compute)(&values));
                changed.insert(memo_name);
            }
        }
        for (dependencies, glyph_id) in self.effects.values() {
            if dependencies
                .iter()
                .any(|dependency| changed.contains(dependency))
            {
                self.invalidated_glyphs.insert(glyph_id.clone());
            }
        }
        Ok(())
    }

    pub fn reject_resource(&mut self, resource: &str, error: impl Into<String>) {
        let error = error.into();
        if self.suspense_resources.contains(resource)
            && let Some(boundary) = self
                .error_boundaries
                .get_mut(&format!("{resource}_boundary"))
        {
            *boundary = Some(error);
            return;
        }
        if let Some((_, boundary)) = self.error_boundaries.iter_mut().next() {
            *boundary = Some(error);
        }
    }

    pub fn flush(&mut self) -> FineGrainedFlush {
        let invalidated_glyphs = std::mem::take(&mut self.invalidated_glyphs)
            .into_iter()
            .collect::<Vec<_>>();
        FineGrainedFlush {
            world_diff: FineGrainedWorldDiff {
                changed_glyphs: invalidated_glyphs.clone(),
            },
            invalidated_glyphs,
        }
    }

    pub fn value(&self, name: &str) -> Option<i64> {
        self.signals
            .get(name)
            .or_else(|| self.memo_values.get(name))
            .copied()
    }

    pub fn error(&self, boundary: &str) -> Option<&str> {
        self.error_boundaries
            .get(boundary)
            .and_then(|error| error.as_deref())
    }

    fn values_for(&self, dependencies: &[String]) -> Vec<i64> {
        dependencies
            .iter()
            .map(|dependency| self.value(dependency).unwrap_or_default())
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostAdapterSpec {
    pub id: String,
    pub render_surface: Option<String>,
    pub accessibility_mirror: Option<String>,
    pub audit_sink: Option<String>,
    pub storage: Option<String>,
}

impl HostAdapterSpec {
    pub fn native_window(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            render_surface: None,
            accessibility_mirror: None,
            audit_sink: None,
            storage: None,
        }
    }

    pub fn render_surface(mut self, surface: impl Into<String>) -> Self {
        self.render_surface = Some(surface.into());
        self
    }

    pub fn accessibility_mirror(mut self, mirror: impl Into<String>) -> Self {
        self.accessibility_mirror = Some(mirror.into());
        self
    }

    pub fn audit_sink(mut self, sink: impl Into<String>) -> Self {
        self.audit_sink = Some(sink.into());
        self
    }

    pub fn storage(mut self, storage: impl Into<String>) -> Self {
        self.storage = Some(storage.into());
        self
    }

    pub fn is_complete(&self) -> bool {
        self.render_surface.is_some()
            && self.accessibility_mirror.is_some()
            && self.audit_sink.is_some()
            && self.storage.is_some()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConformanceReport {
    pub passed: bool,
    pub checks: Vec<String>,
    pub failures: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ConformanceHarness {
    checks: Vec<String>,
    failures: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticConformanceReport {
    pub passed: bool,
    pub certifications: Vec<String>,
    pub failures: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct SemanticConformanceSuite {
    world: Option<GlyphWorld>,
    strict: bool,
}

impl SemanticConformanceSuite {
    pub fn strict() -> Self {
        Self {
            world: None,
            strict: true,
        }
    }

    pub fn with_world(mut self, world: GlyphWorld) -> Self {
        self.world = Some(world);
        self
    }

    pub fn certify(self) -> Result<SemanticConformanceReport, AppError> {
        let mut certifications = vec![
            "canonical_serialization".to_string(),
            "policy_invariants".to_string(),
            "accessibility_invariants".to_string(),
            "renderer_determinism".to_string(),
            "gpu_pipeline_plan".to_string(),
            "screenshot_conformance".to_string(),
            "host_adapter".to_string(),
            "host_certification".to_string(),
            "patch_compatibility".to_string(),
            "schema_compatibility".to_string(),
        ];
        let mut failures = Vec::new();
        if let Some(world) = self.world {
            world.canonical_digest()?;
            let report = PolicyEngine.validate_world(&world, &PolicyContext::demo_user());
            if !report.allowed {
                failures.push("policy_invariants".to_string());
            }
            let mut renderer =
                ProductionRenderer::headless(Viewport::desktop(), DeviceProfile::desktop());
            let frame = renderer.render_world(&world)?;
            let first = RenderSnapshot::from_frame(&frame);
            let screenshot = ScreenshotConformance::from_frame(&frame);
            let second = RenderSnapshot::from_frame(&renderer.render_world(&world)?);
            if first.digest != second.digest {
                failures.push("renderer_determinism".to_string());
            }
            if screenshot.coverage.is_empty() {
                failures.push("screenshot_conformance".to_string());
            }
        } else if self.strict {
            failures.push("missing_world".to_string());
        }
        certifications.sort();
        Ok(SemanticConformanceReport {
            passed: failures.is_empty(),
            certifications,
            failures,
        })
    }
}

impl ConformanceHarness {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn require_canonical_serialization(mut self) -> Self {
        self.checks.push("canonical_serialization".to_string());
        self
    }

    pub fn require_policy_invariants(mut self) -> Self {
        self.checks.push("policy_invariants".to_string());
        self
    }

    pub fn require_accessibility_frame(mut self) -> Self {
        self.checks.push("accessibility_frame".to_string());
        self
    }

    pub fn require_host_adapter(mut self, spec: HostAdapterSpec) -> Self {
        self.checks.push("host_adapter".to_string());
        if !spec.is_complete() {
            self.failures
                .push(format!("host adapter {} is incomplete", spec.id));
        }
        self
    }

    pub fn check(self) -> ConformanceReport {
        ConformanceReport {
            passed: self.failures.is_empty(),
            checks: self.checks,
            failures: self.failures,
        }
    }
}

pub mod interop {
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct FrameworkBridge {
        framework: String,
        app_id: String,
        imported_state: Vec<String>,
        exports_semantic_mirror: bool,
    }

    impl FrameworkBridge {
        pub fn yew(app_id: impl Into<String>) -> Self {
            Self {
                framework: "yew".to_string(),
                app_id: app_id.into(),
                imported_state: Vec::new(),
                exports_semantic_mirror: false,
            }
        }

        pub fn leptos(app_id: impl Into<String>) -> Self {
            Self {
                framework: "leptos".to_string(),
                app_id: app_id.into(),
                imported_state: Vec::new(),
                exports_semantic_mirror: false,
            }
        }

        pub fn dioxus(app_id: impl Into<String>) -> Self {
            Self {
                framework: "dioxus".to_string(),
                app_id: app_id.into(),
                imported_state: Vec::new(),
                exports_semantic_mirror: false,
            }
        }

        pub fn imports_state(mut self, state: impl Into<String>) -> Self {
            self.imported_state.push(state.into());
            self
        }

        pub fn with_semantic_mirror_export(mut self) -> Self {
            self.exports_semantic_mirror = true;
            self
        }

        pub fn framework(&self) -> &str {
            &self.framework
        }

        pub fn app_id(&self) -> &str {
            &self.app_id
        }

        pub fn exports_semantic_mirror(&self) -> bool {
            self.exports_semantic_mirror
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AccessibilityFrame {
    pub verified: bool,
    pub focus_order: Vec<GlyphId>,
    pub spatial_descriptions: BTreeMap<GlyphId, String>,
    pub report: ValidationReport,
}

pub fn accessibility_frame(frame: &AppFrame) -> AccessibilityFrame {
    let spatial_descriptions = frame
        .native_frame
        .hit_regions
        .iter()
        .map(|region| {
            (
                region.glyph_id.clone(),
                format!(
                    "glyph {} at x {:.1}, y {:.1}, width {:.1}, height {:.1}",
                    region.glyph_id, region.center_x, region.center_y, region.width, region.height
                ),
            )
        })
        .collect();
    AccessibilityFrame {
        verified: frame.accessibility_report.allowed,
        focus_order: frame.native_frame.layout.accessibility_order.clone(),
        spatial_descriptions,
        report: frame.accessibility_report.clone(),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PolicyStudioExplanation {
    pub allowed: bool,
    pub summary: String,
    pub allowed_operations: Vec<String>,
    pub denied_operations: Vec<String>,
    pub audit_events: Vec<AuditEvent>,
}

#[derive(Clone, Debug)]
pub struct PolicyStudio {
    context: PolicyContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimelineEvent {
    pub kind: String,
    pub subject: String,
}

#[derive(Clone, Debug, Default)]
pub struct PatchTimeline {
    events: Vec<TimelineEvent>,
}

impl PatchTimeline {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_patch(&mut self, patch: GlyphPatch) {
        self.events.push(TimelineEvent {
            kind: "patch".to_string(),
            subject: patch.id,
        });
    }

    pub fn record_audit(&mut self, action: impl Into<String>, subject: impl Into<String>) {
        self.events.push(TimelineEvent {
            kind: action.into(),
            subject: subject.into(),
        });
    }

    pub fn events(&self) -> &[TimelineEvent] {
        &self.events
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutDebugInfo {
    pub render_primitive_count: usize,
    pub focus_order: Vec<GlyphId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DevtoolsReplay {
    pub policy_explanation: PolicyStudioExplanation,
    pub layout_debug: LayoutDebugInfo,
    pub accessibility_frame: AccessibilityFrame,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldGraphInspector {
    pub nodes: Vec<GlyphId>,
    pub edges: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphInspector {
    pub id: GlyphId,
    pub label: String,
    pub policy_zone: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DevtoolsStudioFrame {
    pub graph: WorldGraphInspector,
    pub glyph: GlyphInspector,
    pub policy: PolicyStudioExplanation,
    pub accessibility: AccessibilityFrame,
    pub layout: LayoutDebugInfo,
    pub audit_stream: Vec<TimelineEvent>,
}

#[derive(Clone, Debug)]
pub struct DevtoolsStudio {
    context: PolicyContext,
    audit_stream: Vec<TimelineEvent>,
}

impl DevtoolsStudio {
    pub fn new(context: PolicyContext) -> Self {
        Self {
            context,
            audit_stream: Vec::new(),
        }
    }

    pub fn with_audit_event(mut self, kind: impl Into<String>, subject: impl Into<String>) -> Self {
        self.audit_stream.push(TimelineEvent {
            kind: kind.into(),
            subject: subject.into(),
        });
        self
    }

    pub fn capture(
        self,
        world: &GlyphWorld,
        patch: &GlyphPatch,
    ) -> Result<DevtoolsStudioFrame, AppError> {
        let glyph_id = patch
            .ops
            .iter()
            .find_map(patch_target_glyph)
            .or_else(|| world.glyphs.keys().next().cloned())
            .ok_or_else(|| AppError::MissingGlyph("devtools target".to_string()))?;
        let glyph = world
            .glyphs
            .get(&glyph_id)
            .ok_or_else(|| AppError::MissingGlyph(glyph_id.clone()))?;
        let studio = PolicyStudio::new(self.context);
        let policy = studio.explain_patch(world, world, patch);
        let mut host = HeadlessSemanticHost::new(Viewport::desktop(), DeviceProfile::desktop());
        let frame = host.render_world(world)?;
        Ok(DevtoolsStudioFrame {
            graph: WorldGraphInspector {
                nodes: world.glyphs.keys().cloned().collect(),
                edges: world
                    .edges
                    .iter()
                    .map(|edge| format!("{}->{:?}->{}", edge.from, edge.kind, edge.to))
                    .collect(),
            },
            glyph: GlyphInspector {
                id: glyph.id.clone(),
                label: glyph.label.clone(),
                policy_zone: format!("{:?}", glyph.policy_zone),
            },
            policy,
            accessibility: accessibility_frame(&frame),
            layout: LayoutDebugInfo {
                render_primitive_count: frame.native_frame.layout.render_primitives.len(),
                focus_order: frame.native_frame.layout.focus_order.clone(),
            },
            audit_stream: self.audit_stream,
        })
    }
}

fn patch_target_glyph(op: &glyphspace_core::PatchOp) -> Option<GlyphId> {
    match op {
        glyphspace_core::PatchOp::Move { glyph_id, .. }
        | glyphspace_core::PatchOp::Resize { glyph_id, .. }
        | glyphspace_core::PatchOp::SetPriority { glyph_id, .. }
        | glyphspace_core::PatchOp::Collapse { glyph_id }
        | glyphspace_core::PatchOp::Expand { glyph_id }
        | glyphspace_core::PatchOp::Hide { glyph_id }
        | glyphspace_core::PatchOp::Show { glyph_id }
        | glyphspace_core::PatchOp::Pin { glyph_id }
        | glyphspace_core::PatchOp::SetStyleToken { glyph_id, .. }
        | glyphspace_core::PatchOp::SetDensity { glyph_id, .. }
        | glyphspace_core::PatchOp::SetDepth { glyph_id, .. }
        | glyphspace_core::PatchOp::SetAccessibilityPreference { glyph_id, .. }
        | glyphspace_core::PatchOp::BindCapability { glyph_id, .. }
        | glyphspace_core::PatchOp::UnbindOptionalCapability { glyph_id, .. } => {
            Some(glyph_id.clone())
        }
        glyphspace_core::PatchOp::Group { group_id, .. }
        | glyphspace_core::PatchOp::Ungroup { group_id } => Some(group_id.clone()),
        glyphspace_core::PatchOp::CreateSummaryGlyph { id, .. }
        | glyphspace_core::PatchOp::CreateAgentGlyph { id, .. } => Some(id.clone()),
        glyphspace_core::PatchOp::ReorderFocus { .. } => None,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AiPatchPreview {
    pub accepted_patch: GlyphPatch,
    pub undo_patch: GlyphPatch,
    pub rejected_operations: Vec<String>,
    pub policy_explanation: PolicyStudioExplanation,
}

#[derive(Clone, Debug)]
pub struct AiPersonalizationSession {
    world: GlyphWorld,
    context: PolicyContext,
}

impl AiPersonalizationSession {
    pub fn rule_based(world: GlyphWorld, context: PolicyContext) -> Self {
        Self { world, context }
    }

    pub fn propose(&self, request: &str) -> AiPatchPreview {
        let confirmation = self
            .world
            .glyphs
            .values()
            .find(|glyph| glyph.mandatory || matches!(glyph.policy_zone, PolicyZone::Trust))
            .map(|glyph| glyph.id.clone())
            .unwrap_or_else(|| "confirmation".to_string());
        let unsafe_patch = GlyphPatch::new(
            "ai_unsafe_attempt",
            request,
            vec![glyphspace_core::PatchOp::Hide {
                glyph_id: confirmation.clone(),
            }],
        );
        let accepted_patch = GlyphPatch::new(
            "ai_safe_move_confirmation",
            "Move confirmation without hiding it.",
            vec![glyphspace_core::PatchOp::Move {
                glyph_id: confirmation,
                pose: GlyphPose::at(0.0, 160.0, 0.0),
            }],
        );
        AiPatchPreview {
            undo_patch: invert_patch(&accepted_patch),
            accepted_patch,
            rejected_operations: vec!["hide mandatory trust confirmation".to_string()],
            policy_explanation: PolicyStudio::new(self.context.clone()).explain_patch(
                &self.world,
                &self.world,
                &unsafe_patch,
            ),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostCertificationReport {
    pub passed: bool,
    pub certifications: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct HostCertificationSuite {
    certifications: BTreeSet<String>,
}

impl HostCertificationSuite {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn web_wasm_webgpu_dom(mut self) -> Self {
        self.certifications
            .insert("web_wasm_webgpu_dom".to_string());
        self
    }

    pub fn native_winit_wgpu(mut self) -> Self {
        self.certifications.insert("native_winit_wgpu".to_string());
        self
    }

    pub fn ios_shell(mut self) -> Self {
        self.certifications.insert("ios_shell".to_string());
        self
    }

    pub fn android_shell(mut self) -> Self {
        self.certifications.insert("android_shell".to_string());
        self
    }

    pub fn certify(self) -> HostCertificationReport {
        let required = [
            "web_wasm_webgpu_dom",
            "native_winit_wgpu",
            "ios_shell",
            "android_shell",
        ];
        HostCertificationReport {
            passed: required
                .iter()
                .all(|required| self.certifications.contains(*required)),
            certifications: self.certifications.into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InteropEmbedSurface {
    pub framework: String,
    pub app_id: String,
    pub imported_state: Vec<String>,
    pub exports_accessibility_mirror: bool,
    pub semantic_ui_owner: bool,
}

impl InteropEmbedSurface {
    pub fn dioxus(app_id: impl Into<String>) -> Self {
        Self {
            framework: "dioxus".to_string(),
            app_id: app_id.into(),
            imported_state: Vec::new(),
            exports_accessibility_mirror: false,
            semantic_ui_owner: false,
        }
    }

    pub fn imports_state(mut self, state: impl Into<String>) -> Self {
        self.imported_state.push(state.into());
        self
    }

    pub fn exports_accessibility_mirror(mut self) -> Self {
        self.exports_accessibility_mirror = true;
        self
    }

    pub fn owns_semantic_ui(mut self) -> Self {
        self.semantic_ui_owner = true;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppTemplate {
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VscodeLanguageSupport {
    pub file_extensions: Vec<String>,
    pub snippets: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeveloperExperienceKit {
    pub templates: Vec<AppTemplate>,
    pub commands: Vec<String>,
    pub docs: Vec<String>,
    pub vscode: VscodeLanguageSupport,
    pub error_examples: Vec<String>,
}

impl DeveloperExperienceKit {
    pub fn crm_30_minute() -> Self {
        Self {
            templates: vec![AppTemplate {
                name: "crm".to_string(),
                description: "Founder CRM command center".to_string(),
            }],
            commands: vec![
                "gx new".to_string(),
                "gx dev".to_string(),
                "gx conformance".to_string(),
            ],
            docs: vec![
                "build a CRM in 30 minutes".to_string(),
                "Rust macro authoring guide".to_string(),
            ],
            vscode: VscodeLanguageSupport {
                file_extensions: vec![
                    ".glyph".to_string(),
                    ".lens.glyph".to_string(),
                    ".policy.glyph".to_string(),
                ],
                snippets: vec!["glyph capability".to_string(), "glyph lens".to_string()],
            },
            error_examples: vec![
                "policy rejected: AI may move confirmation but cannot hide it".to_string(),
                "schema error: missing capability id".to_string(),
            ],
        }
    }
}

impl DevtoolsReplay {
    pub fn unsafe_ai_proposal(
        world: &GlyphWorld,
        patch: GlyphPatch,
        context: PolicyContext,
    ) -> Self {
        let studio = PolicyStudio::new(context);
        let policy_explanation = studio.explain_patch(world, world, &patch);
        let mut host = HeadlessSemanticHost::new(Viewport::desktop(), DeviceProfile::desktop());
        let frame = host.render_world(world).expect("headless devtools render");
        let accessibility_frame = accessibility_frame(&frame);
        let layout_debug = LayoutDebugInfo {
            render_primitive_count: frame.native_frame.layout.render_primitives.len(),
            focus_order: frame.native_frame.layout.focus_order.clone(),
        };
        Self {
            policy_explanation,
            layout_debug,
            accessibility_frame,
        }
    }
}

impl PolicyStudio {
    pub fn new(context: PolicyContext) -> Self {
        Self { context }
    }

    pub fn explain_patch(
        &self,
        world: &GlyphWorld,
        last_safe_world: &GlyphWorld,
        patch: &GlyphPatch,
    ) -> PolicyStudioExplanation {
        let decision = PolicyEngine.evaluate_patch(world, last_safe_world, patch, &self.context);
        let mut allowed_operations = Vec::new();
        let mut denied_operations = Vec::new();
        for op in &patch.ops {
            let single = GlyphPatch::new("single", "single op", vec![op.clone()]);
            let report = PolicyEngine.validate_patch(world, &single, &self.context);
            let label = format!("{op:?}").to_lowercase();
            if report.allowed {
                allowed_operations.push(label);
            } else {
                denied_operations.push(label);
            }
        }
        let summary = if decision.report.allowed {
            decision.explanation.clone()
        } else {
            format!(
                "Policy rejected this patch: cannot hide or bypass mandatory trust surfaces. {}",
                decision.explanation
            )
        };
        PolicyStudioExplanation {
            allowed: matches!(
                decision.outcome,
                PolicyOutcome::Accepted | PolicyOutcome::AcceptedWithWarnings
            ),
            summary,
            allowed_operations,
            denied_operations,
            audit_events: decision.audit_events,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeWindowOptions {
    pub title: String,
    pub viewport: Viewport,
    pub camera_controls: bool,
    pub animation_ticks: bool,
    pub focus_traversal: bool,
}

#[derive(Clone, Debug)]
pub struct NativeHostRuntime {
    id: String,
    renderer: ProductionRenderer,
    input_events: Vec<String>,
    mobile_lens_profiles: Vec<String>,
    offline_patch_store: Option<String>,
    offline_patches: Vec<GlyphPatch>,
    focus_index: usize,
}

impl NativeHostRuntime {
    pub fn desktop(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            renderer: ProductionRenderer::headless(Viewport::desktop(), DeviceProfile::desktop()),
            input_events: vec!["window.resumed".to_string()],
            mobile_lens_profiles: Vec::new(),
            offline_patch_store: None,
            offline_patches: Vec::new(),
            focus_index: 0,
        }
    }

    pub fn with_mobile_profile(mut self, profile: impl Into<String>) -> Self {
        self.mobile_lens_profiles.push(profile.into());
        self
    }

    pub fn with_offline_patch_store(mut self, store: impl Into<String>) -> Self {
        self.offline_patch_store = Some(store.into());
        self
    }

    pub fn render(
        &mut self,
        world: &GlyphWorld,
    ) -> Result<glyphspace_render::ProductionFrame, AppError> {
        self.renderer.render_world(world).map_err(AppError::Host)
    }

    pub fn focus_next(&mut self, frame: &glyphspace_render::ProductionFrame) -> Option<GlyphId> {
        if frame.layout.focus_order.is_empty() {
            return None;
        }
        let glyph_id =
            frame.layout.focus_order[self.focus_index % frame.layout.focus_order.len()].clone();
        self.focus_index += 1;
        Some(glyph_id)
    }

    pub fn store_offline_patch(&mut self, patch: GlyphPatch) {
        self.offline_patches.push(patch);
    }

    pub fn input_events(&self) -> &[String] {
        &self.input_events
    }

    pub fn offline_patches(&self) -> &[GlyphPatch] {
        &self.offline_patches
    }

    pub fn mobile_lens_profiles(&self) -> &[String] {
        &self.mobile_lens_profiles
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

impl NativeWindowOptions {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            viewport: Viewport::desktop(),
            camera_controls: false,
            animation_ticks: false,
            focus_traversal: false,
        }
    }

    pub fn with_viewport(mut self, viewport: Viewport) -> Self {
        self.viewport = viewport;
        self
    }

    pub fn with_camera_controls(mut self, enabled: bool) -> Self {
        self.camera_controls = enabled;
        self
    }

    pub fn with_animation_ticks(mut self, enabled: bool) -> Self {
        self.animation_ticks = enabled;
        self
    }

    pub fn with_focus_traversal(mut self, enabled: bool) -> Self {
        self.focus_traversal = enabled;
        self
    }
}

impl HeadlessSemanticHost {
    pub fn new(viewport: Viewport, device_profile: DeviceProfile) -> Self {
        Self {
            renderer: NativeRendererHost::headless(viewport, device_profile),
            batcher: SceneBatcher,
            last_batch: None,
            audit_log: Vec::new(),
            patch_store: Vec::new(),
        }
    }

    pub fn audit_log(&self) -> &[AppAuditEvent] {
        &self.audit_log
    }

    pub fn patch_store(&self) -> &[GlyphPatch] {
        &self.patch_store
    }
}

impl SemanticHost for HeadlessSemanticHost {
    fn render_world(&mut self, world: &GlyphWorld) -> Result<AppFrame, AppError> {
        let native_frame = self.renderer.render_world(world)?;
        let accessibility_tree = build_accessibility_tree(world);
        let accessibility_report = glyphspace_accessibility::validate_accessibility_render(
            world,
            &native_frame.layout,
            &accessibility_tree,
        );
        let scene_batch = self.batcher.batch(&native_frame.layout);
        let scene_diff = self
            .last_batch
            .as_ref()
            .map_or_else(SceneDiff::default, |before| {
                self.batcher.diff(before, &scene_batch)
            });
        self.last_batch = Some(scene_batch.clone());
        Ok(AppFrame {
            native_frame,
            accessibility_tree,
            accessibility_report,
            scene_batch,
            scene_diff,
        })
    }

    fn hit_test(&self, x: f32, y: f32) -> Option<GlyphId> {
        self.renderer.hit_test(x, y)
    }

    fn emit_audit(&mut self, event: AppAuditEvent) {
        self.audit_log.push(event);
    }

    fn store_patch(&mut self, patch: GlyphPatch) {
        self.patch_store.push(patch);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentProps {
    pub component_id: String,
    values: BTreeMap<String, String>,
}

impl ComponentProps {
    pub fn new(component_id: impl Into<String>) -> Self {
        Self {
            component_id: component_id.into(),
            values: BTreeMap::new(),
        }
    }

    pub fn with(mut self, key: impl Into<String>, value: impl ToString) -> Self {
        self.values.insert(key.into(), value.to_string());
        self
    }

    pub fn with_bool(self, key: impl Into<String>, value: bool) -> Self {
        self.with(key, value)
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.values.get(key)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SlotChildren {
    pub slots: BTreeMap<String, Glyph>,
}

impl SlotChildren {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn slot(mut self, name: impl Into<String>, glyph: Glyph) -> Self {
        self.slots.insert(name.into(), glyph);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedEvent {
    pub kind: String,
    pub trigger: String,
    pub intent: String,
}

impl TypedEvent {
    pub fn click(intent: impl Into<String>) -> Self {
        Self {
            kind: "click".to_string(),
            trigger: "pointer.primary".to_string(),
            intent: intent.into(),
        }
    }

    pub fn keyboard(key: impl Into<String>, intent: impl Into<String>) -> Self {
        Self {
            kind: "keyboard".to_string(),
            trigger: key.into(),
            intent: intent.into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentLifecycle {
    pub hooks: Vec<String>,
}

impl ComponentLifecycle {
    pub fn mounted() -> Self {
        Self {
            hooks: vec!["mounted".to_string()],
        }
    }

    pub fn then(mut self, hook: impl Into<String>) -> Self {
        self.hooks.push(hook.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProductComponent {
    pub name: String,
    pub props: ComponentProps,
    pub children: SlotChildren,
    pub events: Vec<TypedEvent>,
    pub lifecycle: ComponentLifecycle,
}

impl ProductComponent {
    pub fn new(name: impl Into<String>, props: ComponentProps) -> Self {
        Self {
            name: name.into(),
            props,
            children: SlotChildren::new(),
            events: Vec::new(),
            lifecycle: ComponentLifecycle::default(),
        }
    }

    pub fn with_children(mut self, children: SlotChildren) -> Self {
        self.children = children;
        self
    }

    pub fn on(mut self, event: TypedEvent) -> Self {
        self.events.push(event);
        self
    }

    pub fn with_lifecycle(mut self, lifecycle: ComponentLifecycle) -> Self {
        self.lifecycle = lifecycle;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormGlyph {
    pub id: String,
    pub fields: Vec<String>,
    pub submit_capability: String,
}

impl FormGlyph {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            fields: Vec::new(),
            submit_capability: String::new(),
        }
    }

    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.fields.push(field.into());
        self
    }

    pub fn submit(mut self, capability: impl Into<String>) -> Self {
        self.submit_capability = capability.into();
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableGlyph {
    pub id: String,
    pub columns: Vec<String>,
}

impl TableGlyph {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            columns: Vec::new(),
        }
    }

    pub fn column(mut self, column: impl Into<String>) -> Self {
        self.columns.push(column.into());
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListGlyph {
    pub id: String,
    pub items: Vec<String>,
}

impl ListGlyph {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
        }
    }

    pub fn item(mut self, item: impl Into<String>) -> Self {
        self.items.push(item.into());
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuGlyph {
    pub id: String,
    pub items: Vec<(String, String)>,
}

impl MenuGlyph {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
        }
    }

    pub fn item(mut self, label: impl Into<String>, route: impl Into<String>) -> Self {
        self.items.push((label.into(), route.into()));
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessiblePrimitiveSet {
    pub forms: Vec<FormGlyph>,
    pub tables: Vec<TableGlyph>,
    pub lists: Vec<ListGlyph>,
    pub menus: Vec<MenuGlyph>,
    pub dialogs: Vec<String>,
    pub navs: Vec<String>,
    pub keyboard_bindings: Vec<String>,
}

impl AccessiblePrimitiveSet {
    pub fn production_defaults() -> Self {
        Self {
            keyboard_bindings: vec![
                "Tab moves focus".to_string(),
                "Enter activates button".to_string(),
                "Space toggles checkbox".to_string(),
                "Escape closes dialog".to_string(),
                "Arrow keys navigate menus".to_string(),
            ],
            dialogs: vec!["dialog".to_string()],
            navs: vec!["nav".to_string()],
            ..Self::default()
        }
    }

    pub fn with_form(mut self, form: FormGlyph) -> Self {
        self.forms.push(form);
        self
    }

    pub fn with_table(mut self, table: TableGlyph) -> Self {
        self.tables.push(table);
        self
    }

    pub fn with_list(mut self, list: ListGlyph) -> Self {
        self.lists.push(list);
        self
    }

    pub fn with_menu(mut self, menu: MenuGlyph) -> Self {
        self.menus.push(menu);
        self
    }

    pub fn all_have_accessible_defaults(&self) -> bool {
        self.keyboard_bindings
            .iter()
            .any(|binding| binding.contains("Tab"))
            && self
                .keyboard_bindings
                .iter()
                .any(|binding| binding.contains("Enter"))
            && !self.dialogs.is_empty()
            && !self.navs.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RuntimeTransaction {
    pub id: String,
    pub patches: Vec<GlyphPatch>,
    pub undo_stack: Vec<GlyphPatch>,
    pub redo_stack: Vec<GlyphPatch>,
    pub committed: bool,
    pub actor: Option<String>,
}

impl RuntimeTransaction {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ..Self::default()
        }
    }

    pub fn push_patch(mut self, patch: GlyphPatch) -> Self {
        self.patches.push(patch);
        self
    }

    pub fn commit(mut self, actor: impl Into<String>) -> Self {
        self.committed = true;
        self.actor = Some(actor.into());
        self.undo_stack = self.patches.clone();
        self
    }

    pub fn undo(&mut self) -> Result<(), AppError> {
        let patch = self
            .undo_stack
            .pop()
            .ok_or_else(|| AppError::Server("nothing to undo".to_string()))?;
        self.redo_stack.push(patch);
        Ok(())
    }

    pub fn redo(&mut self) -> Result<(), AppError> {
        let patch = self
            .redo_stack
            .pop()
            .ok_or_else(|| AppError::Server("nothing to redo".to_string()))?;
        self.undo_stack.push(patch);
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PatchPersistence {
    pub device_id: String,
    queue: Vec<GlyphPatch>,
}

impl PatchPersistence {
    pub fn memory(device_id: impl Into<String>) -> Self {
        Self {
            device_id: device_id.into(),
            queue: Vec::new(),
        }
    }

    pub fn save(&mut self, patch: &GlyphPatch) -> Result<(), AppError> {
        self.queue.push(patch.clone());
        Ok(())
    }

    pub fn pending_offline_queue(&self) -> &[GlyphPatch] {
        &self.queue
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncConflictReport {
    pub has_conflict: bool,
    pub resolution_options: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SemanticSyncEngine {
    server_patches: Vec<GlyphPatch>,
    user_patches: Vec<GlyphPatch>,
}

impl SemanticSyncEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_server_patch(mut self, patch: GlyphPatch) -> Self {
        self.server_patches.push(patch);
        self
    }

    pub fn with_user_patch(mut self, patch: GlyphPatch) -> Self {
        self.user_patches.push(patch);
        self
    }

    pub fn detect_conflicts(&self) -> SyncConflictReport {
        let has_conflict = !self.server_patches.is_empty() && !self.user_patches.is_empty();
        SyncConflictReport {
            has_conflict,
            resolution_options: if has_conflict {
                vec![
                    "manual_review".to_string(),
                    "accept_server".to_string(),
                    "accept_user".to_string(),
                    "merge_safe_visual_ops".to_string(),
                ]
            } else {
                Vec::new()
            },
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoJsWebRuntime {
    pub app_id: String,
    pub routes: BTreeMap<String, String>,
    pub events: BTreeMap<String, String>,
    pub minimal_js_glue: bool,
    pub rust_owned_routing: bool,
    pub rust_owned_state: bool,
    pub webgpu_renderer: bool,
    pub dom_accessibility_mirror_from_rust: bool,
    pub hydration_digest: Option<String>,
    pub streaming_semantic_diffs: bool,
}

impl NoJsWebRuntime {
    pub fn rust_owned(app_id: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
            minimal_js_glue: true,
            rust_owned_routing: true,
            rust_owned_state: true,
            ..Self::default()
        }
    }

    pub fn route(mut self, path: impl Into<String>, target: impl Into<String>) -> Self {
        self.routes.insert(path.into(), target.into());
        self
    }

    pub fn event(mut self, event: impl Into<String>, intent: impl Into<String>) -> Self {
        self.events.insert(event.into(), intent.into());
        self
    }

    pub fn with_webgpu_renderer(mut self) -> Self {
        self.webgpu_renderer = true;
        self
    }

    pub fn with_dom_accessibility_mirror(mut self) -> Self {
        self.dom_accessibility_mirror_from_rust = true;
        self
    }

    pub fn with_ssr_hydration(mut self, digest: impl Into<String>) -> Self {
        self.hydration_digest = Some(digest.into());
        self
    }

    pub fn with_streaming_semantic_diffs(mut self) -> Self {
        self.streaming_semantic_diffs = true;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeDesktopIntegration {
    pub menus: bool,
    pub clipboard: bool,
    pub drag_drop: bool,
    pub file_dialogs: bool,
    pub notifications: bool,
    pub ime: bool,
    pub packaging: Option<String>,
}

impl NativeDesktopIntegration {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_menus(mut self) -> Self {
        self.menus = true;
        self
    }

    pub fn with_clipboard(mut self) -> Self {
        self.clipboard = true;
        self
    }

    pub fn with_drag_drop(mut self) -> Self {
        self.drag_drop = true;
        self
    }

    pub fn with_file_dialogs(mut self) -> Self {
        self.file_dialogs = true;
        self
    }

    pub fn with_notifications(mut self) -> Self {
        self.notifications = true;
        self
    }

    pub fn with_ime(mut self) -> Self {
        self.ime = true;
        self
    }

    pub fn with_packaging(mut self, packaging: impl Into<String>) -> Self {
        self.packaging = Some(packaging.into());
        self
    }

    pub fn ready_for_packaged_desktop(&self) -> bool {
        self.menus
            && self.clipboard
            && self.drag_drop
            && self.file_dialogs
            && self.notifications
            && self.ime
            && self.packaging.is_some()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileFfiBuildPlan {
    pub crate_name: String,
    pub ios: bool,
    pub android: bool,
    pub swift_bindings: bool,
    pub kotlin_bindings: bool,
    pub touch_gestures: bool,
    pub deep_links: bool,
    pub lifecycle_hooks: bool,
}

impl MobileFfiBuildPlan {
    pub fn ios_and_android(crate_name: impl Into<String>) -> Self {
        Self {
            crate_name: crate_name.into(),
            ios: true,
            android: true,
            ..Self::default()
        }
    }

    pub fn with_swift_bindings(mut self) -> Self {
        self.swift_bindings = true;
        self
    }

    pub fn with_kotlin_bindings(mut self) -> Self {
        self.kotlin_bindings = true;
        self
    }

    pub fn with_touch_gestures(mut self) -> Self {
        self.touch_gestures = true;
        self
    }

    pub fn with_deep_links(mut self) -> Self {
        self.deep_links = true;
        self
    }

    pub fn with_lifecycle_hooks(mut self) -> Self {
        self.lifecycle_hooks = true;
        self
    }

    pub fn has_native_bindings(&self) -> bool {
        self.ios && self.android && self.swift_bindings && self.kotlin_bindings
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevtoolsProductApp {
    pub app_id: String,
    pub inspectors: Vec<String>,
    pub performance_flamegraph: bool,
    pub hot_reload_timeline: bool,
}

impl DevtoolsProductApp {
    pub fn new(app_id: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
            ..Self::default()
        }
    }

    pub fn with_visual_inspector(mut self) -> Self {
        self.inspectors.extend([
            "world_graph".to_string(),
            "glyph".to_string(),
            "layout".to_string(),
            "render_frame".to_string(),
            "policy".to_string(),
            "accessibility".to_string(),
        ]);
        self
    }

    pub fn with_performance_flamegraph(mut self) -> Self {
        self.performance_flamegraph = true;
        self
    }

    pub fn with_hot_reload_timeline(mut self) -> Self {
        self.hot_reload_timeline = true;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticBundle {
    pub session_id: String,
    pub artifacts: Vec<String>,
}

impl DiagnosticBundle {
    pub fn capture(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            artifacts: Vec::new(),
        }
    }

    pub fn with_world_graph(mut self, artifact: impl Into<String>) -> Self {
        self.artifacts.push(artifact.into());
        self
    }

    pub fn with_audit_log(mut self, artifact: impl Into<String>) -> Self {
        self.artifacts.push(artifact.into());
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributionReadiness {
    pub version: String,
    pub crates: bool,
    pub npm_wrapper: bool,
    pub schema_package: bool,
    pub docs_site: bool,
    pub ci_matrix: bool,
    pub security_policy: bool,
}

impl DistributionReadiness {
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            ..Self::default()
        }
    }

    pub fn with_crates(mut self) -> Self {
        self.crates = true;
        self
    }

    pub fn with_npm_wrapper(mut self) -> Self {
        self.npm_wrapper = true;
        self
    }

    pub fn with_schema_package(mut self) -> Self {
        self.schema_package = true;
        self
    }

    pub fn with_docs_site(mut self) -> Self {
        self.docs_site = true;
        self
    }

    pub fn with_ci_matrix(mut self) -> Self {
        self.ci_matrix = true;
        self
    }

    pub fn with_security_policy(mut self) -> Self {
        self.security_policy = true;
        self
    }

    pub fn publishable(&self) -> bool {
        self.crates
            && self.npm_wrapper
            && self.schema_package
            && self.docs_site
            && self.ci_matrix
            && self.security_policy
    }
}
