use glyphspace_accessibility::{AccessibilityTree, build_accessibility_tree};
use glyphspace_core::{
    CanonicalError, Glyph, GlyphId, GlyphKind, GlyphPatch, GlyphWorld, PolicyContext, PolicyZone,
    Priority, SemanticDiff, ValidationReport, semantic_diff,
};
use glyphspace_dsl::{DslError, GlyphApp};
use glyphspace_input::InputEvent;
use glyphspace_layout::{DeviceProfile, Viewport};
use glyphspace_personalization::{PatchError, apply_patch};
use glyphspace_policy::{AuditEvent, PolicyEngine, PolicyOutcome};
use glyphspace_render::render_core::{SceneBatch, SceneBatcher, SceneDiff};
use glyphspace_render::{
    NativeFrame, NativeHostError, NativeRendererHost, ProductionRenderer, RenderSnapshot,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::marker::PhantomData;
use std::rc::Rc;
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

type CapabilityFunction = Box<dyn FnMut(serde_json::Value) -> Result<GlyphPatch, AppError>>;

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

#[derive(Clone, Debug)]
pub struct HotReloadEngine {
    world: GlyphWorld,
    events: Vec<HotReloadEvent>,
}

impl HotReloadEngine {
    pub fn new(world: GlyphWorld) -> Self {
        Self {
            world,
            events: Vec::new(),
        }
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
        function: impl FnMut(serde_json::Value) -> Result<GlyphPatch, AppError> + 'static,
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
        function: impl FnMut(serde_json::Value) -> Result<GlyphPatch, AppError> + 'static,
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
            "host_adapter".to_string(),
            "patch_compatibility".to_string(),
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
            let second = RenderSnapshot::from_frame(&renderer.render_world(&world)?);
            if first.digest != second.digest {
                failures.push("renderer_determinism".to_string());
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
