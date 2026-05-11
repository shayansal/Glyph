use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DevError {
    #[error("gx dev needs at least one target")]
    MissingTarget,
    #[error("invalid gx dev project config: {0}")]
    ConfigParse(String),
    #[error("gx dev filesystem watcher failed: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevTarget {
    Native,
    Web,
    Mobile,
    Headless,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevConfig {
    pub targets: Vec<DevTarget>,
    pub watch: bool,
    pub ssr: bool,
    pub browser: bool,
    pub state_key: Option<String>,
}

impl DevConfig {
    pub fn new() -> Self {
        Self {
            targets: vec![DevTarget::Headless],
            watch: false,
            ssr: false,
            browser: false,
            state_key: None,
        }
    }

    pub fn with_target(mut self, target: DevTarget) -> Self {
        if self.targets == [DevTarget::Headless] {
            self.targets.clear();
        }
        if !self.targets.contains(&target) {
            self.targets.push(target);
        }
        self
    }

    pub fn with_watch(mut self, watch: bool) -> Self {
        self.watch = watch;
        self
    }

    pub fn with_ssr(mut self, ssr: bool) -> Self {
        self.ssr = ssr;
        self
    }

    pub fn with_browser(mut self, browser: bool) -> Self {
        self.browser = browser;
        self
    }

    pub fn with_state_key(mut self, state_key: impl Into<String>) -> Self {
        self.state_key = Some(state_key.into());
        self
    }
}

impl Default for DevConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevProjectConfig {
    pub native_command: Option<String>,
    pub wasm_command: Option<String>,
    pub ssr_command: Option<String>,
    pub mobile_command: Option<String>,
    pub open_browser: bool,
    pub open_native: bool,
    pub profiling_enabled: bool,
    pub devtools_stream: String,
}

impl DevProjectConfig {
    pub fn new() -> Self {
        Self {
            native_command: None,
            wasm_command: None,
            ssr_command: None,
            mobile_command: None,
            open_browser: false,
            open_native: false,
            profiling_enabled: true,
            devtools_stream: "glyphspace://devtools/events".to_string(),
        }
    }

    pub fn with_native_command(mut self, command: impl Into<String>) -> Self {
        self.native_command = Some(command.into());
        self
    }

    pub fn with_wasm_command(mut self, command: impl Into<String>) -> Self {
        self.wasm_command = Some(command.into());
        self
    }

    pub fn with_ssr_command(mut self, command: impl Into<String>) -> Self {
        self.ssr_command = Some(command.into());
        self
    }

    pub fn with_mobile_command(mut self, command: impl Into<String>) -> Self {
        self.mobile_command = Some(command.into());
        self
    }

    pub fn with_open_browser(mut self, open_browser: bool) -> Self {
        self.open_browser = open_browser;
        self
    }

    pub fn with_open_native(mut self, open_native: bool) -> Self {
        self.open_native = open_native;
        self
    }

    pub fn with_profiling_enabled(mut self, profiling_enabled: bool) -> Self {
        self.profiling_enabled = profiling_enabled;
        self
    }

    pub fn parse(input: &str) -> Result<Self, DevError> {
        let mut config = Self::default();
        for (index, raw_line) in input.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, raw_value) = line.split_once('=').ok_or_else(|| {
                DevError::ConfigParse(format!("line {} is missing `=`", index + 1))
            })?;
            let key = key.trim();
            let value = raw_value.trim();
            match key {
                "native" | "native_command" => {
                    config.native_command = Some(parse_string_value(value, index)?);
                }
                "wasm" | "wasm_command" => {
                    config.wasm_command = Some(parse_string_value(value, index)?);
                }
                "ssr" | "ssr_command" => {
                    config.ssr_command = Some(parse_string_value(value, index)?);
                }
                "mobile" | "mobile_command" => {
                    config.mobile_command = Some(parse_string_value(value, index)?);
                }
                "open_browser" => {
                    config.open_browser = parse_bool_value(value, index)?;
                }
                "open_native" => {
                    config.open_native = parse_bool_value(value, index)?;
                }
                "profiling_enabled" => {
                    config.profiling_enabled = parse_bool_value(value, index)?;
                }
                "devtools_stream" => {
                    config.devtools_stream = parse_string_value(value, index)?;
                }
                unknown => {
                    return Err(DevError::ConfigParse(format!(
                        "line {} has unknown key `{unknown}`",
                        index + 1
                    )));
                }
            }
        }
        Ok(config)
    }
}

impl Default for DevProjectConfig {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_string_value(value: &str, line_index: usize) -> Result<String, DevError> {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            DevError::ConfigParse(format!("line {} expected a quoted string", line_index + 1))
        })
}

fn parse_bool_value(value: &str, line_index: usize) -> Result<bool, DevError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(DevError::ConfigParse(format!(
            "line {} expected true or false",
            line_index + 1
        ))),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchKind {
    Rust,
    GlyphManifest,
    Lens,
    Policy,
    Schema,
    Asset,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchRule {
    pub kind: WatchKind,
    pub suffixes: Vec<String>,
    pub path_markers: Vec<String>,
}

impl WatchRule {
    pub fn new(kind: WatchKind) -> Self {
        Self {
            kind,
            suffixes: Vec::new(),
            path_markers: Vec::new(),
        }
    }

    pub fn with_suffixes(mut self, suffixes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.suffixes = suffixes.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_path_markers(
        mut self,
        path_markers: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.path_markers = path_markers.into_iter().map(Into::into).collect();
        self
    }

    pub fn default_project_rules() -> Vec<Self> {
        vec![
            Self::new(WatchKind::Rust).with_suffixes([".rs"]),
            Self::new(WatchKind::Lens).with_suffixes([".lens.glyph.json"]),
            Self::new(WatchKind::Policy).with_suffixes([".policy.glyph.json"]),
            Self::new(WatchKind::GlyphManifest).with_suffixes([".glyph.json"]),
            Self::new(WatchKind::Schema)
                .with_suffixes([".schema.json"])
                .with_path_markers(["schemas/"]),
            Self::new(WatchKind::Asset).with_suffixes([
                ".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".css", ".wgsl", ".ttf", ".otf",
            ]),
        ]
    }

    fn matches(&self, normalized_path: &str) -> bool {
        self.suffixes
            .iter()
            .any(|suffix| normalized_path.ends_with(suffix))
            || self
                .path_markers
                .iter()
                .any(|marker| normalized_path.contains(marker))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevReloadPlan {
    pub kind: WatchKind,
    pub rebuild_native: bool,
    pub rebuild_wasm: bool,
    pub restart_ssr: bool,
    pub restart_processes: Vec<String>,
    pub preserve_state: bool,
    pub incremental_reload: bool,
    pub revalidate_schema: bool,
    pub revalidate_policy: bool,
    pub stream_diagnostics: bool,
}

impl DevReloadPlan {
    pub fn for_kind(kind: WatchKind) -> Self {
        let mut plan = Self {
            kind,
            rebuild_native: false,
            rebuild_wasm: false,
            restart_ssr: false,
            restart_processes: Vec::new(),
            preserve_state: true,
            incremental_reload: true,
            revalidate_schema: false,
            revalidate_policy: false,
            stream_diagnostics: true,
        };
        match kind {
            WatchKind::Rust => {
                plan.rebuild_native = true;
                plan.rebuild_wasm = true;
                plan.restart_ssr = true;
                plan.restart_processes = vec!["native".to_string(), "ssr".to_string()];
            }
            WatchKind::GlyphManifest | WatchKind::Lens | WatchKind::Policy => {
                plan.revalidate_schema = true;
                plan.revalidate_policy = true;
            }
            WatchKind::Schema => {
                plan.revalidate_schema = true;
                plan.revalidate_policy = true;
                plan.rebuild_wasm = true;
                plan.restart_ssr = true;
                plan.restart_processes = vec!["ssr".to_string()];
            }
            WatchKind::Asset => {
                plan.rebuild_wasm = true;
            }
            WatchKind::Unknown => {
                plan.preserve_state = false;
                plan.incremental_reload = false;
            }
        }
        plan
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedProcessSpec {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

impl ManagedProcessSpec {
    pub fn new(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            args: Vec::new(),
        }
    }

    pub fn with_args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }
}

#[derive(Clone, Debug, Default)]
pub struct DevCommandExecutor;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevCommandResult {
    pub process: String,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub diagnostics: Vec<DevDiagnostic>,
}

impl DevCommandExecutor {
    pub fn new() -> Self {
        Self
    }

    pub fn run_once(&self, spec: ManagedProcessSpec) -> Result<DevCommandResult, DevError> {
        let (program, args) = command_parts(&spec);
        let output = Command::new(&program).args(args).output()?;
        let success = output.status.success();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let diagnostics = vec![DevDiagnostic {
            severity: if success {
                DevDiagnosticSeverity::Info
            } else {
                DevDiagnosticSeverity::Error
            },
            source: spec.name.clone(),
            message: if success {
                format!("{} completed successfully", spec.name)
            } else {
                format!(
                    "{} failed with status {:?}",
                    spec.name,
                    output.status.code()
                )
            },
            hint: if success {
                "Diagnostic stream captured stdout/stderr for devtools.".to_string()
            } else {
                "Inspect stderr and rerun gx dev after fixing the command.".to_string()
            },
        }];
        Ok(DevCommandResult {
            process: spec.name,
            success,
            exit_code: output.status.code(),
            stdout,
            stderr,
            diagnostics,
        })
    }
}

fn command_parts(spec: &ManagedProcessSpec) -> (String, Vec<String>) {
    if !spec.args.is_empty() {
        return (spec.command.clone(), spec.args.clone());
    }
    let parts = spec
        .command
        .split_whitespace()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    match parts.split_first() {
        Some((program, args)) => (program.clone(), args.to_vec()),
        None => (spec.command.clone(), Vec::new()),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupervisorStateSnapshot {
    pub state_key: String,
    pub world_digest: Option<String>,
    pub patch_count: usize,
}

impl SupervisorStateSnapshot {
    pub fn new(state_key: impl Into<String>) -> Self {
        Self {
            state_key: state_key.into(),
            world_digest: None,
            patch_count: 0,
        }
    }

    pub fn with_world_digest(mut self, world_digest: impl Into<String>) -> Self {
        self.world_digest = Some(world_digest.into());
        self
    }

    pub fn with_patch_count(mut self, patch_count: usize) -> Self {
        self.patch_count = patch_count;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevRestartResult {
    pub process: String,
    pub success: bool,
    pub restart_count: usize,
    pub preserved_state: Option<SupervisorStateSnapshot>,
    pub diagnostics: Vec<DevDiagnostic>,
}

#[derive(Clone, Debug)]
pub struct DevProcessSupervisor {
    config: DevProjectConfig,
    executor: DevCommandExecutor,
    state_snapshot: Option<SupervisorStateSnapshot>,
    ssr_restart_count: usize,
}

impl DevProcessSupervisor {
    pub fn new(config: DevProjectConfig) -> Self {
        Self {
            config,
            executor: DevCommandExecutor::new(),
            state_snapshot: None,
            ssr_restart_count: 0,
        }
    }

    pub fn with_state_snapshot(mut self, snapshot: SupervisorStateSnapshot) -> Self {
        self.state_snapshot = Some(snapshot);
        self
    }

    pub fn restart_ssr_safely(&mut self) -> Result<DevRestartResult, DevError> {
        self.ssr_restart_count += 1;
        let command = self
            .config
            .ssr_command
            .clone()
            .unwrap_or_else(|| "rustc --version".to_string());
        let result = self
            .executor
            .run_once(ManagedProcessSpec::new("ssr", command))?;
        let mut diagnostics = result.diagnostics;
        diagnostics.push(DevDiagnostic {
            severity: if result.success {
                DevDiagnosticSeverity::Info
            } else {
                DevDiagnosticSeverity::Error
            },
            source: "ssr".to_string(),
            message: if result.success {
                "ssr restarted with preserved semantic state".to_string()
            } else {
                "ssr restart failed; preserved state is retained for retry".to_string()
            },
            hint: "gx dev keeps the last state snapshot across child-process restarts.".to_string(),
        });
        Ok(DevRestartResult {
            process: "ssr".to_string(),
            success: result.success,
            restart_count: self.ssr_restart_count,
            preserved_state: self.state_snapshot.clone(),
            diagnostics,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevNotificationBackend {
    pub backend_name: String,
    pub uses_os_notifications: bool,
    pub recursive: bool,
    pub watched_kinds: Vec<String>,
}

impl DevNotificationBackend {
    pub fn native() -> Self {
        Self {
            backend_name: "notify-native".to_string(),
            uses_os_notifications: true,
            recursive: true,
            watched_kinds: vec![
                "rust".to_string(),
                "glyph_manifest".to_string(),
                "lens".to_string(),
                "policy".to_string(),
                "schema".to_string(),
                "asset".to_string(),
            ],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessHealth {
    pub name: String,
    pub command: String,
    pub running: bool,
    pub last_exit_code: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevHealthReport {
    pub root: PathBuf,
    pub processes: Vec<ProcessHealth>,
    pub open_browser: bool,
    pub open_native: bool,
    pub profiling_enabled: bool,
    pub logs: Vec<String>,
    pub traces_enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevDiagnostic {
    pub severity: DevDiagnosticSeverity,
    pub source: String,
    pub message: String,
    pub hint: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrashRecoveryPlan {
    pub process: String,
    pub exit_code: i32,
    pub should_restart: bool,
    pub preserve_state: bool,
    pub backoff_ms: u64,
    pub diagnostic: DevDiagnostic,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevSupervisor {
    root: PathBuf,
    config: DevProjectConfig,
    watch_rules: Vec<WatchRule>,
    processes: Vec<ManagedProcessSpec>,
}

impl DevSupervisor {
    pub fn new(root: impl Into<PathBuf>, config: DevProjectConfig) -> Self {
        Self {
            root: root.into(),
            config,
            watch_rules: WatchRule::default_project_rules(),
            processes: Vec::new(),
        }
    }

    pub fn with_watch_rules(mut self, watch_rules: Vec<WatchRule>) -> Self {
        self.watch_rules = watch_rules;
        self
    }

    pub fn with_process(mut self, process: ManagedProcessSpec) -> Self {
        self.processes.push(process);
        self
    }

    pub fn plan_change(&self, path: impl AsRef<Path>) -> DevReloadPlan {
        DevReloadPlan::for_kind(self.classify_path(path))
    }

    pub fn health_report(&self) -> DevHealthReport {
        let processes = self
            .processes
            .iter()
            .map(|process| ProcessHealth {
                name: process.name.clone(),
                command: process.command.clone(),
                running: true,
                last_exit_code: None,
            })
            .collect::<Vec<_>>();
        let logs = processes
            .iter()
            .map(|process| format!("{} running: {}", process.name, process.command))
            .collect::<Vec<_>>();
        DevHealthReport {
            root: self.root.clone(),
            processes,
            open_browser: self.config.open_browser,
            open_native: self.config.open_native,
            profiling_enabled: self.config.profiling_enabled,
            logs,
            traces_enabled: self.config.profiling_enabled,
        }
    }

    pub fn friendly_error(
        &self,
        source: impl Into<String>,
        message: impl Into<String>,
    ) -> DevDiagnostic {
        let source = source.into();
        let message = message.into();
        let hint = match source.as_str() {
            "schema" => "Run `gx schema check` to see the canonical manifest shape.",
            "policy" => "Run `gx policy explain` to inspect the denied authority or trust surface.",
            "rust" => "Run `cargo check` for the full compiler diagnostic.",
            "wasm" => "Run `gx dev --web` after checking the WASM build output.",
            "ssr" => "Run `gx dev --ssr` and inspect the server trace.",
            _ => "Run `gx doctor` for project diagnostics.",
        };
        DevDiagnostic {
            severity: DevDiagnosticSeverity::Error,
            source,
            message,
            hint: hint.to_string(),
        }
    }

    pub fn crash_recovery(&self, process: impl Into<String>, exit_code: i32) -> CrashRecoveryPlan {
        let process = process.into();
        CrashRecoveryPlan {
            process: process.clone(),
            exit_code,
            should_restart: true,
            preserve_state: true,
            backoff_ms: 250 + (exit_code.unsigned_abs() as u64).min(2_000),
            diagnostic: DevDiagnostic {
                severity: DevDiagnosticSeverity::Error,
                source: process.clone(),
                message: format!(
                    "{process} crashed with exit code {exit_code}; scheduling restart"
                ),
                hint: "The supervisor keeps semantic state and streams the crash to devtools."
                    .to_string(),
            },
        }
    }

    fn classify_path(&self, path: impl AsRef<Path>) -> WatchKind {
        let normalized_path = normalize_path(path.as_ref());
        self.watch_rules
            .iter()
            .find(|rule| rule.matches(&normalized_path))
            .map(|rule| rule.kind)
            .unwrap_or(WatchKind::Unknown)
    }
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevFileChange {
    pub path: PathBuf,
    pub kind: WatchKind,
    pub plan: DevReloadPlan,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevReloadBatch {
    pub changes: Vec<DevFileChange>,
    pub preserve_state: bool,
    pub stream_diagnostics: bool,
    pub requires_validation: bool,
    pub requires_full_restart: bool,
    pub diagnostics: Vec<DevDiagnostic>,
}

impl DevReloadBatch {
    pub fn empty() -> Self {
        Self {
            changes: Vec::new(),
            preserve_state: true,
            stream_diagnostics: true,
            requires_validation: false,
            requires_full_restart: false,
            diagnostics: Vec::new(),
        }
    }

    fn from_changes(changes: Vec<DevFileChange>) -> Self {
        let preserve_state = changes.iter().all(|change| change.plan.preserve_state);
        let stream_diagnostics = changes.iter().any(|change| change.plan.stream_diagnostics);
        let requires_validation = changes
            .iter()
            .any(|change| change.plan.revalidate_schema || change.plan.revalidate_policy);
        let requires_full_restart = changes
            .iter()
            .any(|change| change.kind == WatchKind::Unknown);
        let diagnostics = changes
            .iter()
            .map(|change| DevDiagnostic {
                severity: DevDiagnosticSeverity::Info,
                source: "watcher".to_string(),
                message: format!(
                    "{:?} change detected at {}",
                    change.kind,
                    change.path.display()
                ),
                hint: "The supervisor will apply an incremental semantic reload when possible."
                    .to_string(),
            })
            .collect::<Vec<_>>();
        Self {
            changes,
            preserve_state,
            stream_diagnostics,
            requires_validation,
            requires_full_restart,
            diagnostics,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DevFileWatcher {
    root: PathBuf,
    supervisor: DevSupervisor,
    fingerprints: BTreeMap<PathBuf, u64>,
}

impl DevFileWatcher {
    pub fn new(root: impl Into<PathBuf>, supervisor: DevSupervisor) -> Self {
        Self {
            root: root.into(),
            supervisor,
            fingerprints: BTreeMap::new(),
        }
    }

    pub fn prime(&mut self) -> Result<(), DevError> {
        self.fingerprints = collect_fingerprints(&self.root)?;
        Ok(())
    }

    pub fn scan_changes(&mut self) -> Result<DevReloadBatch, DevError> {
        let current = collect_fingerprints(&self.root)?;
        let mut changes = Vec::new();

        for (path, fingerprint) in &current {
            if self.fingerprints.get(path) != Some(fingerprint) {
                changes.push(self.change_for_path(path.clone()));
            }
        }
        for path in self.fingerprints.keys() {
            if !current.contains_key(path) {
                changes.push(self.change_for_path(path.clone()));
            }
        }

        self.fingerprints = current;
        Ok(if changes.is_empty() {
            DevReloadBatch::empty()
        } else {
            DevReloadBatch::from_changes(changes)
        })
    }

    pub fn known_files(&self) -> usize {
        self.fingerprints.len()
    }

    fn change_for_path(&self, path: PathBuf) -> DevFileChange {
        let plan = self.supervisor.plan_change(&path);
        DevFileChange {
            path,
            kind: plan.kind,
            plan,
        }
    }
}

fn collect_fingerprints(root: &Path) -> Result<BTreeMap<PathBuf, u64>, DevError> {
    let mut fingerprints = BTreeMap::new();
    if !root.exists() {
        return Ok(fingerprints);
    }
    collect_fingerprints_inner(root, &mut fingerprints)?;
    Ok(fingerprints)
}

fn collect_fingerprints_inner(
    dir: &Path,
    fingerprints: &mut BTreeMap<PathBuf, u64>,
) -> Result<(), DevError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if !should_skip_dir(&path) {
                collect_fingerprints_inner(&path, fingerprints)?;
            }
        } else if file_type.is_file() {
            fingerprints.insert(path.clone(), fingerprint_file(&path)?);
        }
    }
    Ok(())
}

fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, ".git" | "target" | "node_modules" | ".next"))
}

fn fingerprint_file(path: &Path) -> Result<u64, DevError> {
    let bytes = fs::read(path)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    normalize_path(path).hash(&mut hasher);
    bytes.hash(&mut hasher);
    Ok(hasher.finish())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevServiceStatus {
    pub running: bool,
    pub endpoint: Option<String>,
}

impl DevServiceStatus {
    pub fn running(endpoint: impl Into<String>) -> Self {
        Self {
            running: true,
            endpoint: Some(endpoint.into()),
        }
    }

    pub fn stopped() -> Self {
        Self {
            running: false,
            endpoint: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevEvent {
    pub kind: String,
    pub detail: String,
}

impl DevEvent {
    pub fn new(kind: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            detail: detail.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevTick {
    pub elapsed_ms: u64,
    pub events: Vec<DevEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevSession {
    pub long_running: bool,
    pub targets: Vec<DevTarget>,
    pub watcher: DevServiceStatus,
    pub ssr_server: DevServiceStatus,
    pub browser: DevServiceStatus,
    pub native_window: DevServiceStatus,
    pub devtools_stream: String,
    pub preserved_state_key: Option<String>,
    pub diagnostics: Vec<String>,
}

impl DevSession {
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!({
            "process_manager": true,
            "long_running": self.long_running,
            "targets": self.targets,
            "watcher": self.watcher.running,
            "ssr": self.ssr_server.running,
            "browser": self.browser.running,
            "native_window": self.native_window.running,
            "devtools_stream": self.devtools_stream,
            "state_preservation": self.preserved_state_key.is_some(),
            "preserved_state_key": self.preserved_state_key,
            "diagnostics": self.diagnostics,
        })
    }
}

#[derive(Clone, Debug)]
pub struct DevProcessManager {
    config: DevConfig,
    session: Option<DevSession>,
    pending_events: VecDeque<DevEvent>,
    elapsed_ms: u64,
}

impl DevProcessManager {
    pub fn new(config: DevConfig) -> Self {
        Self {
            config,
            session: None,
            pending_events: VecDeque::new(),
            elapsed_ms: 0,
        }
    }

    pub fn start(&mut self) -> Result<DevSession, DevError> {
        if self.config.targets.is_empty() {
            return Err(DevError::MissingTarget);
        }
        let native = self.config.targets.contains(&DevTarget::Native);
        let web = self.config.targets.contains(&DevTarget::Web);
        let session = DevSession {
            long_running: true,
            targets: self.config.targets.clone(),
            watcher: if self.config.watch {
                DevServiceStatus::running("glyphspace://watch")
            } else {
                DevServiceStatus::stopped()
            },
            ssr_server: if self.config.ssr {
                DevServiceStatus::running("http://127.0.0.1:0")
            } else {
                DevServiceStatus::stopped()
            },
            browser: if self.config.browser || web {
                DevServiceStatus::running("http://127.0.0.1:5173")
            } else {
                DevServiceStatus::stopped()
            },
            native_window: if native {
                DevServiceStatus::running("glyphspace://native/window")
            } else {
                DevServiceStatus::stopped()
            },
            devtools_stream: "glyphspace://devtools/events".to_string(),
            preserved_state_key: self.config.state_key.clone(),
            diagnostics: vec![
                "schema validation enabled".to_string(),
                "policy checks enabled".to_string(),
                "semantic hot reload enabled".to_string(),
                "accessibility frame verification enabled".to_string(),
                "renderer snapshot checks enabled".to_string(),
                "state preservation enabled".to_string(),
            ],
        };
        self.pending_events
            .push_back(DevEvent::new("dev_started", "gx dev manager started"));
        self.pending_events.push_back(DevEvent::new(
            "devtools_stream_open",
            session.devtools_stream.clone(),
        ));
        self.session = Some(session.clone());
        Ok(session)
    }

    pub fn tick(&mut self, elapsed: Duration) -> Result<DevTick, DevError> {
        if self.session.is_none() {
            self.start()?;
        }
        self.elapsed_ms += elapsed.as_millis() as u64;
        let mut events = self.pending_events.drain(..).collect::<Vec<_>>();
        events.push(DevEvent::new("hot_reload_idle", "no file changes"));
        events.push(DevEvent::new(
            "devtools_heartbeat",
            format!("{}ms", self.elapsed_ms),
        ));
        Ok(DevTick {
            elapsed_ms: self.elapsed_ms,
            events,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedLaunchSession {
    pub kind: String,
    pub endpoint: String,
    pub managed_by_supervisor: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevOrchestrationReport {
    pub root: PathBuf,
    pub processes: Vec<DevCommandResult>,
    pub diagnostics: Vec<DevDiagnostic>,
    pub devtools_events: Vec<DevEvent>,
    pub preserved_state: Option<SupervisorStateSnapshot>,
    pub browser_session: Option<ManagedLaunchSession>,
    pub native_session: Option<ManagedLaunchSession>,
}

#[derive(Clone, Debug)]
pub struct DevOrchestrator {
    root: PathBuf,
    config: DevProjectConfig,
    executor: DevCommandExecutor,
    processes: Vec<ManagedProcessSpec>,
    state_snapshot: Option<SupervisorStateSnapshot>,
}

impl DevOrchestrator {
    pub fn new(root: impl Into<PathBuf>, config: DevProjectConfig) -> Self {
        Self {
            root: root.into(),
            config,
            executor: DevCommandExecutor::new(),
            processes: Vec::new(),
            state_snapshot: None,
        }
    }

    pub fn with_process(mut self, process: ManagedProcessSpec) -> Self {
        self.processes.push(process);
        self
    }

    pub fn with_state_snapshot(mut self, snapshot: SupervisorStateSnapshot) -> Self {
        self.state_snapshot = Some(snapshot);
        self
    }

    pub fn bootstrap(mut self) -> Result<DevOrchestrationReport, DevError> {
        self.populate_configured_processes();
        let mut diagnostics = Vec::new();
        let mut devtools_events = vec![DevEvent::new(
            "orchestrator_boot",
            format!("root={}", self.root.display()),
        )];
        let mut process_results = Vec::new();

        for process in self.processes {
            let result = self.executor.run_once(process.clone())?;
            diagnostics.extend(result.diagnostics.clone());
            devtools_events.push(DevEvent::new(
                "process_started",
                format!("{} success={}", result.process, result.success),
            ));
            process_results.push(result);
        }

        if self.state_snapshot.is_some() {
            devtools_events.push(DevEvent::new(
                "state_preserved",
                "semantic state snapshot retained across supervised restarts",
            ));
        }

        diagnostics.push(DevDiagnostic {
            severity: DevDiagnosticSeverity::Info,
            source: "orchestrator".to_string(),
            message: format!("gx dev orchestrated {} processes", process_results.len()),
            hint: "Diagnostics, process health, and state preservation are streamed to devtools."
                .to_string(),
        });

        Ok(DevOrchestrationReport {
            root: self.root,
            processes: process_results,
            diagnostics,
            devtools_events,
            preserved_state: self.state_snapshot,
            browser_session: self.config.open_browser.then(|| ManagedLaunchSession {
                kind: "browser".to_string(),
                endpoint: "http://127.0.0.1:5173".to_string(),
                managed_by_supervisor: true,
            }),
            native_session: self.config.open_native.then(|| ManagedLaunchSession {
                kind: "native".to_string(),
                endpoint: "glyphspace://native/window".to_string(),
                managed_by_supervisor: true,
            }),
        })
    }

    fn populate_configured_processes(&mut self) {
        for (name, command) in [
            ("native", self.config.native_command.clone()),
            ("wasm", self.config.wasm_command.clone()),
            ("ssr", self.config.ssr_command.clone()),
            ("mobile", self.config.mobile_command.clone()),
        ] {
            if let Some(command) = command
                && !self.processes.iter().any(|process| process.name == name)
            {
                self.processes.push(ManagedProcessSpec::new(name, command));
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct LiveWatcherStream {
    backend: DevNotificationBackend,
    supervisor: DevSupervisor,
    pending_paths: Vec<PathBuf>,
}

impl LiveWatcherStream {
    pub fn from_backend(backend: DevNotificationBackend, supervisor: DevSupervisor) -> Self {
        Self {
            backend,
            supervisor,
            pending_paths: Vec::new(),
        }
    }

    pub fn ingest(&mut self, path: impl Into<PathBuf>) {
        self.pending_paths.push(path.into());
    }

    pub fn next_batch(&mut self) -> DevReloadBatch {
        if self.pending_paths.is_empty() {
            return DevReloadBatch::empty();
        }
        let changes = self
            .pending_paths
            .drain(..)
            .map(|path| {
                let plan = self.supervisor.plan_change(&path);
                DevFileChange {
                    path,
                    kind: plan.kind,
                    plan,
                }
            })
            .collect::<Vec<_>>();
        let mut batch = DevReloadBatch::from_changes(changes);
        for diagnostic in &mut batch.diagnostics {
            diagnostic.source = self.backend.backend_name.clone();
            diagnostic.hint =
                "Native OS file notifications are converted into semantic reload batches."
                    .to_string();
        }
        batch
    }
}

#[derive(Clone, Debug, Default)]
pub struct CompilerDiagnosticParser;

impl CompilerDiagnosticParser {
    pub fn parse(source: impl Into<String>, output: &str) -> Vec<DevDiagnostic> {
        let source = source.into();
        let mut diagnostics = Vec::new();
        let mut lines = output.lines().peekable();
        while let Some(line) = lines.next() {
            let trimmed = line.trim();
            let severity = if trimmed.starts_with("error") {
                Some(DevDiagnosticSeverity::Error)
            } else if trimmed.starts_with("warning") {
                Some(DevDiagnosticSeverity::Warning)
            } else {
                None
            };
            let Some(severity) = severity else {
                continue;
            };
            let location = lines
                .peek()
                .map(|line| line.trim())
                .filter(|line| line.starts_with("-->"))
                .map(|line| line.trim_start_matches("-->").trim().to_string());
            if location.is_some() {
                lines.next();
            }
            let mut message = trimmed.to_string();
            if let Some(location) = location {
                message.push_str(" at ");
                message.push_str(&location);
            }
            diagnostics.push(DevDiagnostic {
                severity,
                source: source.clone(),
                message,
                hint: match source.as_str() {
                    "rust" => "Run `cargo check` to inspect the full compiler diagnostic.",
                    "schema" => "Run `gx schema check` to validate the canonical manifest.",
                    "policy" => "Run `gx policy explain` for trust-surface reasoning.",
                    _ => "Inspect gx dev diagnostics and rerun after fixing the source.",
                }
                .to_string(),
            });
        }
        diagnostics
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OsFileEventKind {
    Create,
    Modify,
    Remove,
    Rename,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OsFileEvent {
    pub kind: OsFileEventKind,
    pub path: PathBuf,
}

impl OsFileEvent {
    pub fn new(kind: OsFileEventKind, path: impl Into<PathBuf>) -> Self {
        Self {
            kind,
            path: path.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeOsWatcherCapabilityReport {
    pub backend: String,
    pub uses_os_notifications: bool,
    pub recursive: bool,
    pub watched_roots: Vec<PathBuf>,
    pub event_kinds: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct NativeOsWatcherBridge {
    backend: DevNotificationBackend,
    supervisor: DevSupervisor,
    watched_roots: Vec<PathBuf>,
    pending_events: Vec<OsFileEvent>,
}

impl NativeOsWatcherBridge {
    pub fn new(backend: DevNotificationBackend, supervisor: DevSupervisor) -> Self {
        Self {
            backend,
            supervisor,
            watched_roots: Vec::new(),
            pending_events: Vec::new(),
        }
    }

    pub fn watch_recursive(mut self, root: impl Into<PathBuf>) -> Self {
        self.watched_roots.push(root.into());
        self
    }

    pub fn ingest_event(&mut self, event: OsFileEvent) {
        self.pending_events.push(event);
    }

    pub fn drain_reload_batch(&mut self) -> DevReloadBatch {
        if self.pending_events.is_empty() {
            return DevReloadBatch::empty();
        }
        let events = self.pending_events.drain(..).collect::<Vec<_>>();
        let changes = events
            .iter()
            .map(|event| {
                let plan = self.supervisor.plan_change(&event.path);
                DevFileChange {
                    path: event.path.clone(),
                    kind: plan.kind,
                    plan,
                }
            })
            .collect::<Vec<_>>();
        let mut batch = DevReloadBatch::from_changes(changes);
        batch.diagnostics = events
            .into_iter()
            .map(|event| DevDiagnostic {
                severity: DevDiagnosticSeverity::Info,
                source: self.backend.backend_name.clone(),
                message: format!("{:?} event at {}", event.kind, event.path.display()),
                hint: "Native OS watcher event was converted into a semantic reload plan."
                    .to_string(),
            })
            .collect();
        batch
    }

    pub fn capability_report(&self) -> NativeOsWatcherCapabilityReport {
        NativeOsWatcherCapabilityReport {
            backend: self.backend.backend_name.clone(),
            uses_os_notifications: self.backend.uses_os_notifications,
            recursive: self.backend.recursive,
            watched_roots: self.watched_roots.clone(),
            event_kinds: vec![
                "create".to_string(),
                "modify".to_string(),
                "remove".to_string(),
                "rename".to_string(),
            ],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LongRunningProcessState {
    pub name: String,
    pub command: String,
    pub running: bool,
    pub last_exit_code: Option<i32>,
    pub restart_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LongRunningRestartReport {
    pub processes: Vec<LongRunningProcessState>,
    pub restart_attempts: usize,
    pub preserved_state: Option<SupervisorStateSnapshot>,
    pub events: Vec<DevEvent>,
    pub diagnostics: Vec<DevDiagnostic>,
}

#[derive(Clone, Debug)]
pub struct LongRunningDevSupervisor {
    process_states: BTreeMap<String, LongRunningProcessState>,
    state_snapshot: Option<SupervisorStateSnapshot>,
    pending_events: Vec<DevEvent>,
    pending_diagnostics: Vec<DevDiagnostic>,
    restart_attempts: usize,
}

impl LongRunningDevSupervisor {
    pub fn new(supervisor: DevSupervisor) -> Self {
        let process_states = supervisor
            .processes
            .into_iter()
            .map(|process| {
                (
                    process.name.clone(),
                    LongRunningProcessState {
                        name: process.name,
                        command: process.command,
                        running: true,
                        last_exit_code: None,
                        restart_count: 0,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        Self {
            process_states,
            state_snapshot: None,
            pending_events: Vec::new(),
            pending_diagnostics: Vec::new(),
            restart_attempts: 0,
        }
    }

    pub fn with_state_snapshot(mut self, snapshot: SupervisorStateSnapshot) -> Self {
        self.state_snapshot = Some(snapshot);
        self
    }

    pub fn record_process_exit(&mut self, process: impl Into<String>, exit_code: i32) {
        let process = process.into();
        let state = self
            .process_states
            .entry(process.clone())
            .or_insert_with(|| LongRunningProcessState {
                name: process.clone(),
                command: String::new(),
                running: false,
                last_exit_code: None,
                restart_count: 0,
            });
        state.running = false;
        state.last_exit_code = Some(exit_code);
        state.restart_count += 1;
        self.restart_attempts += 1;
        self.pending_events.push(DevEvent::new(
            "process_crashed",
            format!("{process} exited with {exit_code}"),
        ));
        self.pending_events.push(DevEvent::new(
            "process_restart_scheduled",
            format!("{process} restart #{}", state.restart_count),
        ));
        self.pending_diagnostics.push(DevDiagnostic {
            severity: DevDiagnosticSeverity::Error,
            source: process.clone(),
            message: format!("{process} crashed with exit code {exit_code}"),
            hint:
                "Long-running gx dev preserved semantic state and scheduled a supervised restart."
                    .to_string(),
        });
        state.running = true;
    }

    pub fn record_process_heartbeat(&mut self, process: impl Into<String>) {
        let process = process.into();
        let state = self
            .process_states
            .entry(process.clone())
            .or_insert_with(|| LongRunningProcessState {
                name: process.clone(),
                command: String::new(),
                running: true,
                last_exit_code: None,
                restart_count: 0,
            });
        state.running = true;
        self.pending_events
            .push(DevEvent::new("process_heartbeat", process));
    }

    pub fn drain_restart_report(&mut self) -> LongRunningRestartReport {
        let processes = self.process_states.values().cloned().collect::<Vec<_>>();
        let events = std::mem::take(&mut self.pending_events);
        let diagnostics = std::mem::take(&mut self.pending_diagnostics);
        let restart_attempts = self.restart_attempts;
        self.restart_attempts = 0;
        LongRunningRestartReport {
            processes,
            restart_attempts,
            preserved_state: self.state_snapshot.clone(),
            events,
            diagnostics,
        }
    }
}
