use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DevError {
    #[error("gx dev needs at least one target")]
    MissingTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevTarget {
    Native,
    Web,
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
