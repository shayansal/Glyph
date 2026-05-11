use glyphspace_accessibility::{AccessibilityTree, build_accessibility_tree};
use glyphspace_core::{
    Glyph, GlyphId, GlyphPatch, GlyphWorld, PolicyContext, SemanticDiff, ValidationReport,
    semantic_diff,
};
use glyphspace_dsl::{DslError, GlyphApp};
use glyphspace_input::InputEvent;
use glyphspace_layout::{DeviceProfile, Viewport};
use glyphspace_personalization::{PatchError, apply_patch};
use glyphspace_policy::PolicyEngine;
use glyphspace_render::render_core::{SceneBatch, SceneBatcher, SceneDiff};
use glyphspace_render::{NativeFrame, NativeHostError, NativeRendererHost};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::marker::PhantomData;
use thiserror::Error;

pub use glyphspace_input::InputEvent as GlyphInputEvent;
pub use glyphspace_layout::{DeviceProfile as GlyphDeviceProfile, Viewport as GlyphViewport};

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

#[macro_export]
macro_rules! glyph_component {
    ($render:expr $(,)?) => {
        $crate::component($render)
    };
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
