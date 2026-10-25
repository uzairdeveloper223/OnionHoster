use crate::services::{ServiceManager, ServiceSource, ServiceState, ServiceView, now_ms};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

const STATE_VERSION: u32 = 1;
const EVENT_TUNNEL_STATUS: &str = "tunnel-status";
const EVENT_TUNNEL_LOG: &str = "tunnel-log";
const EVENT_TUNNEL_DELETED: &str = "tunnel-deleted";
const MAX_LOGS: usize = 400;
const QUICK_TUNNEL_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelProvider {
    CloudflareQuick,
    CloudflareNamed,
    Custom,
}

impl TunnelProvider {
    pub fn label(self) -> &'static str {
        match self {
            Self::CloudflareQuick => "Cloudflare Quick",
            Self::CloudflareNamed => "Cloudflare Named",
            Self::Custom => "Custom command",
        }
    }

    fn is_cloudflare(self) -> bool {
        matches!(self, Self::CloudflareQuick | Self::CloudflareNamed)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelState {
    Stopped,
    Starting,
    Running,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelRecord {
    pub id: String,
    pub name: String,
    pub service_id: String,
    pub provider: TunnelProvider,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub public_hostname: Option<String>,
    #[serde(default)]
    pub custom_command: Option<String>,
    #[serde(default)]
    pub expected_url: Option<String>,
    #[serde(default)]
    pub autostart: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelView {
    pub id: String,
    pub name: String,
    pub service_id: String,
    pub provider: TunnelProvider,
    pub state: TunnelState,
    pub public_url: Option<String>,
    pub local_target: Option<String>,
    pub error: Option<String>,
    pub autostart: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelLog {
    pub id: u64,
    pub tunnel_id: String,
    pub level: String,
    pub message: String,
    pub timestamp_ms: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelInput {
    pub name: String,
    pub service_id: String,
    pub provider: TunnelProvider,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub public_hostname: Option<String>,
    #[serde(default)]
    pub custom_command: Option<String>,
    #[serde(default)]
    pub expected_url: Option<String>,
    #[serde(default)]
    pub autostart: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedState {
    version: u32,
    #[serde(default)]
    tunnels: Vec<TunnelRecord>,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            tunnels: Vec::new(),
        }
    }
}

struct TunnelRuntime {
    state: TunnelState,
    public_url: Option<String>,
    local_target: Option<String>,
    error: Option<String>,
    process: Option<Arc<Mutex<Child>>>,
    stop: Option<Arc<AtomicBool>>,
    generation: u64,
    started_at: Option<u64>,
}

impl TunnelRuntime {
    fn stopped() -> Self {
        Self {
            state: TunnelState::Stopped,
            public_url: None,
            local_target: None,
            error: None,
            process: None,
            stop: None,
            generation: 0,
            started_at: None,
        }
    }
}

pub struct TunnelManager {
    state_path: PathBuf,
    state: Mutex<PersistedState>,
    runtime: Mutex<HashMap<String, TunnelRuntime>>,
    logs: Mutex<HashMap<String, VecDeque<TunnelLog>>>,
    next_log_id: AtomicU64,
}

struct ServiceTarget {
    host: String,
    port: u16,
    url: String,
    is_tcp: bool,
}

struct ValidatedTunnel {
    name: String,
    service_id: String,
    provider: TunnelProvider,
    token: Option<String>,
    public_hostname: Option<String>,
    custom_command: Option<String>,
    expected_url: Option<String>,
}

impl ServiceTarget {
    fn target(&self) -> String {
        if self.is_tcp {
            format!("tcp://{}:{}", self.host, self.port)
        } else {
            self.url.clone()
        }
    }
}

impl TunnelManager {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let data_dir = app.path().app_data_dir().map_err(|error| {
            format!("Could not resolve the application data directory: {error}")
        })?;
        fs::create_dir_all(&data_dir).map_err(|error| {
            format!(
                "Could not create the application data directory at {}: {error}",
                data_dir.display()
            )
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&data_dir, fs::Permissions::from_mode(0o700));
        }

        let state_path = data_dir.join("tunnels.json");
        let state = load_state(&state_path)?;
        let manager = Self {
            state_path,
            state: Mutex::new(state),
            runtime: Mutex::new(HashMap::new()),
            logs: Mutex::new(HashMap::new()),
            next_log_id: AtomicU64::new(1),
        };
        manager.ensure_runtime();
        Ok(manager)
    }

    fn ensure_runtime(&self) {
        let state = self.state.lock().expect("tunnel state lock poisoned");
        let mut runtime = self.runtime.lock().expect("tunnel runtime lock poisoned");
        for tunnel in &state.tunnels {
            runtime
                .entry(tunnel.id.clone())
                .or_insert_with(TunnelRuntime::stopped);
        }
    }

    pub fn list(&self) -> Vec<TunnelView> {
        let state = self.state.lock().expect("tunnel state lock poisoned");
        let runtime = self.runtime.lock().expect("tunnel runtime lock poisoned");
        state
            .tunnels
            .iter()
            .map(|tunnel| build_view(tunnel, runtime.get(&tunnel.id)))
            .collect()
    }

    fn record(&self, id: &str) -> Result<TunnelRecord, String> {
        self.state
            .lock()
            .expect("tunnel state lock poisoned")
            .tunnels
            .iter()
            .find(|tunnel| tunnel.id == id)
            .cloned()
            .ok_or_else(|| "The tunnel was not found.".to_string())
    }

    fn save(&self) -> Result<(), String> {
        let state = self.state.lock().expect("tunnel state lock poisoned");
        let contents = serde_json::to_string_pretty(&*state)
            .map_err(|error| format!("Could not serialize the tunnel configuration: {error}"))?;
        atomic_write(&self.state_path, &contents)
    }

    pub fn create(
        &self,
        app: &AppHandle,
        services: &ServiceManager,
        input: TunnelInput,
    ) -> Result<TunnelView, String> {
        let autostart = input.autostart;
        let validated = validate_input(services, input)?;
        let id = Uuid::new_v4().to_string();
        let timestamp = now_ms();
        let record = TunnelRecord {
            id: id.clone(),
            name: validated.name,
            service_id: validated.service_id,
            provider: validated.provider,
            token: validated.token,
            public_hostname: validated.public_hostname,
            custom_command: validated.custom_command,
            expected_url: validated.expected_url,
            autostart,
            created_at: timestamp,
            updated_at: timestamp,
        };

        let view = {
            let mut state = self.state.lock().expect("tunnel state lock poisoned");
            state.tunnels.push(record.clone());
            let mut runtime = self.runtime.lock().expect("tunnel runtime lock poisoned");
            runtime.insert(id.clone(), TunnelRuntime::stopped());
            build_view(&record, runtime.get(&id))
        };

        if let Err(error) = self.save() {
            let mut state = self.state.lock().expect("tunnel state lock poisoned");
            state.tunnels.retain(|tunnel| tunnel.id != id);
            self.runtime
                .lock()
                .expect("tunnel runtime lock poisoned")
                .remove(&id);
            return Err(error);
        }

        self.log(
            app,
            &id,
            "info",
            &format!("Tunnel created for {}.", record.provider.label()),
        );
        let _ = app.emit(EVENT_TUNNEL_STATUS, view.clone());
        Ok(view)
    }

    pub fn update(
        &self,
        app: &AppHandle,
        services: &ServiceManager,
        id: &str,
        input: TunnelInput,
    ) -> Result<TunnelView, String> {
        self.ensure_editable(id)?;
        let autostart = input.autostart;
        let mut input = input;
        if input.provider == TunnelProvider::CloudflareNamed
            && input
                .token
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                .is_empty()
        {
            input.token = self.record(id)?.token;
        }
        let validated = validate_input(services, input)?;

        let (view, record) = {
            let mut state = self.state.lock().expect("tunnel state lock poisoned");
            let record = state
                .tunnels
                .iter_mut()
                .find(|tunnel| tunnel.id == id)
                .ok_or_else(|| "The tunnel was not found.".to_string())?;
            record.name = validated.name;
            record.service_id = validated.service_id;
            record.provider = validated.provider;
            record.token = validated.token;
            record.public_hostname = validated.public_hostname;
            record.custom_command = validated.custom_command;
            record.expected_url = validated.expected_url;
            record.autostart = autostart;
            record.updated_at = now_ms();
            let record = record.clone();
            let mut runtime = self.runtime.lock().expect("tunnel runtime lock poisoned");
            let view = build_view(&record, runtime.get(&record.id));
            if let Some(entry) = runtime.get_mut(&record.id) {
                entry.error = None;
            }
            (view, record)
        };

        self.save()?;
        let _ = app.emit(EVENT_TUNNEL_STATUS, view.clone());
        self.log(app, &record.id, "info", "Tunnel settings updated.");
        Ok(view)
    }

    pub fn delete(&self, app: &AppHandle, id: &str) -> Result<(), String> {
        self.stop(app, id)?;
        {
            let mut state = self.state.lock().expect("tunnel state lock poisoned");
            state.tunnels.retain(|tunnel| tunnel.id != id);
            self.runtime
                .lock()
                .expect("tunnel runtime lock poisoned")
                .remove(id);
            self.logs
                .lock()
                .expect("tunnel log lock poisoned")
                .remove(id);
        }
        self.save()?;
        let _ = app.emit(EVENT_TUNNEL_DELETED, TunnelDeleted { id: id.to_string() });
        Ok(())
    }

    pub fn start(
        self: &Arc<Self>,
        app: &AppHandle,
        services: &ServiceManager,
        id: &str,
    ) -> Result<TunnelView, String> {
        let record = self.record(id)?;
        let target = service_target(services, &record.service_id)?;

        if record.provider.is_cloudflare() && target.is_tcp {
            return Err(
                "Cloudflare tunnels support HTTP and HTTPS services only. Use a custom command for raw TCP."
                    .to_string(),
            );
        }

        let cloudflared = if record.provider.is_cloudflare() {
            Some(resolve_cloudflared(app)?)
        } else {
            None
        };

        let generation = {
            let mut runtime = self.runtime.lock().expect("tunnel runtime lock poisoned");
            let entry = runtime
                .get_mut(id)
                .ok_or_else(|| "The tunnel runtime was not found.".to_string())?;

            if matches!(entry.state, TunnelState::Starting | TunnelState::Running) {
                return Ok(build_view(&record, Some(entry)));
            }

            entry.generation = entry.generation.saturating_add(1);
            entry.state = TunnelState::Starting;
            entry.public_url = expected_public_url(&record);
            entry.local_target = Some(target.target());
            entry.error = None;
            entry.started_at = Some(now_ms());
            entry.generation
        };

        self.emit_status(app, id);
        self.log(
            app,
            id,
            "info",
            &format!("Starting {} tunnel to {}.", record.provider.label(), target.target()),
        );

        let (program, args) = match record.provider {
            TunnelProvider::CloudflareQuick => {
                let cloudflared = cloudflared
                    .as_ref()
                    .ok_or_else(|| "The Cloudflare tunnel helper was not found.".to_string())?;
                (
                    cloudflared.clone(),
                    vec![
                        "tunnel".to_string(),
                        "--no-autoupdate".to_string(),
                        "--protocol".to_string(),
                        "http2".to_string(),
                        "--url".to_string(),
                        target.url.clone(),
                    ],
                )
            }
            TunnelProvider::CloudflareNamed => {
                let cloudflared = cloudflared
                    .as_ref()
                    .ok_or_else(|| "The Cloudflare tunnel helper was not found.".to_string())?;
                let token = record
                    .token
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| "Enter the Cloudflare tunnel token.".to_string())?;
                (
                    cloudflared.clone(),
                    vec![
                        "tunnel".to_string(),
                        "--no-autoupdate".to_string(),
                        "--protocol".to_string(),
                        "http2".to_string(),
                        "run".to_string(),
                        "--token".to_string(),
                        token.to_string(),
                    ],
                )
            }
            TunnelProvider::Custom => {
                let command = record
                    .custom_command
                    .as_deref()
                    .ok_or_else(|| "Enter a custom tunnel command.".to_string())?;
                (
                    PathBuf::from("sh"),
                    vec![
                        "-c".to_string(),
                        command
                            .replace("{host}", &target.host)
                            .replace("{port}", &target.port.to_string())
                            .replace("{target}", &target.target())
                            .replace("{url}", &target.url),
                    ],
                )
            }
        };

        let mut command = Command::new(&program);
        command
            .args(&args)
            .env("NO_COLOR", "1")
            .env("TERM", "dumb")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command.spawn().map_err(|error| {
            let message = format!(
                "Could not start the tunnel process ({}): {error}",
                program.display()
            );
            self.fail(app, id, generation, &message);
            message
        })?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let process = Arc::new(Mutex::new(child));
        let stopped = Arc::new(AtomicBool::new(false));

        {
            let mut runtime = self.runtime.lock().expect("tunnel runtime lock poisoned");
            if let Some(entry) = runtime.get_mut(id) {
                if entry.generation != generation {
                    let _ = process.lock().expect("tunnel process lock poisoned").kill();
                    return self.view(id);
                }
                entry.process = Some(process.clone());
                entry.stop = Some(stopped.clone());
            }
        }

        if let Some(stream) = stdout {
            spawn_reader(
                self.clone(),
                app.clone(),
                id.to_string(),
                generation,
                stream,
                "info",
            );
        }
        if let Some(stream) = stderr {
            spawn_reader(
                self.clone(),
                app.clone(),
                id.to_string(),
                generation,
                stream,
                "warn",
            );
        }

        spawn_monitor(
            self.clone(),
            app.clone(),
            id.to_string(),
            generation,
            process,
            stopped,
        );

        if record.provider == TunnelProvider::CloudflareQuick {
            let manager = self.clone();
            let app = app.clone();
            let id = id.to_string();
            thread::spawn(move || {
                thread::sleep(QUICK_TUNNEL_TIMEOUT);
                manager.timeout(app, id, generation);
            });
        } else {
            let mut runtime = self.runtime.lock().expect("tunnel runtime lock poisoned");
            if let Some(entry) = runtime.get_mut(id)
                && entry.generation == generation
                && entry.state == TunnelState::Starting
            {
                entry.state = TunnelState::Running;
                entry.error = None;
            }
            self.emit_status(app, id);
        }

        self.log(app, id, "info", "Tunnel process started.");
        self.view(id)
    }

    pub fn stop(&self, app: &AppHandle, id: &str) -> Result<TunnelView, String> {
        let (process, stopped) = {
            let mut runtime = self.runtime.lock().expect("tunnel runtime lock poisoned");
            let entry = runtime
                .get_mut(id)
                .ok_or_else(|| "The tunnel runtime was not found.".to_string())?;
            entry.generation = entry.generation.saturating_add(1);
            entry.state = TunnelState::Stopped;
            entry.public_url = None;
            entry.local_target = None;
            entry.error = None;
            entry.started_at = None;
            (entry.process.take(), entry.stop.take())
        };

        if let Some(stopped) = stopped {
            stopped.store(true, Ordering::SeqCst);
        }
        if let Some(process) = process {
            let mut child = process.lock().expect("tunnel process lock poisoned");
            let _ = child.kill();
            let _ = child.wait();
        }

        let view = self.view(id)?;
        let _ = app.emit(EVENT_TUNNEL_STATUS, view.clone());
        self.log(app, id, "info", "Tunnel stopped.");
        Ok(view)
    }

    pub fn stop_for_service(&self, app: &AppHandle, service_id: &str) {
        let ids: Vec<String> = self
            .state
            .lock()
            .expect("tunnel state lock poisoned")
            .tunnels
            .iter()
            .filter(|tunnel| tunnel.service_id == service_id)
            .map(|tunnel| tunnel.id.clone())
            .collect();

        for id in ids {
            if let Err(error) = self.stop(app, &id) {
                self.log(app, &id, "warn", &format!("Could not stop tunnel: {error}"));
            }
        }
    }

    pub fn delete_for_service(&self, app: &AppHandle, service_id: &str) {
        let ids: Vec<String> = self
            .state
            .lock()
            .expect("tunnel state lock poisoned")
            .tunnels
            .iter()
            .filter(|tunnel| tunnel.service_id == service_id)
            .map(|tunnel| tunnel.id.clone())
            .collect();

        for id in ids {
            if let Err(error) = self.delete(app, &id) {
                self.log(
                    app,
                    &id,
                    "warn",
                    &format!("Could not delete tunnel for removed service: {error}"),
                );
            }
        }
    }

    pub fn shutdown_all(&self) {
        let mut runtime = self.runtime.lock().expect("tunnel runtime lock poisoned");
        for entry in runtime.values_mut() {
            entry.state = TunnelState::Stopped;
            entry.public_url = None;
            entry.local_target = None;
            if let Some(stopped) = entry.stop.take() {
                stopped.store(true, Ordering::SeqCst);
            }
            if let Some(process) = entry.process.take() {
                let mut child = process.lock().expect("tunnel process lock poisoned");
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    pub fn logs(&self, id: &str) -> Vec<TunnelLog> {
        self.logs
            .lock()
            .expect("tunnel log lock poisoned")
            .get(id)
            .map(|logs| logs.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn ensure_editable(&self, id: &str) -> Result<(), String> {
        let runtime = self.runtime.lock().expect("tunnel runtime lock poisoned");
        match runtime.get(id) {
            Some(entry) if matches!(entry.state, TunnelState::Starting | TunnelState::Running) => {
                Err("Stop the tunnel before changing its settings.".to_string())
            }
            _ => Ok(()),
        }
    }

    fn view(&self, id: &str) -> Result<TunnelView, String> {
        let state = self.state.lock().expect("tunnel state lock poisoned");
        let runtime = self.runtime.lock().expect("tunnel runtime lock poisoned");
        let tunnel = state
            .tunnels
            .iter()
            .find(|tunnel| tunnel.id == id)
            .ok_or_else(|| "The tunnel was not found.".to_string())?;
        Ok(build_view(tunnel, runtime.get(id)))
    }

    fn emit_status(&self, app: &AppHandle, id: &str) {
        if let Ok(view) = self.view(id) {
            let _ = app.emit(EVENT_TUNNEL_STATUS, view);
        }
    }

    fn log(&self, app: &AppHandle, id: &str, level: &str, message: &str) {
        let message = self.redact(id, message);
        let entry = TunnelLog {
            id: self.next_log_id.fetch_add(1, Ordering::SeqCst),
            tunnel_id: id.to_string(),
            level: level.to_string(),
            message,
            timestamp_ms: now_ms(),
        };

        {
            let mut logs = self.logs.lock().expect("tunnel log lock poisoned");
            let entries = logs.entry(id.to_string()).or_default();
            if entries.len() >= MAX_LOGS {
                entries.pop_front();
            }
            entries.push_back(entry.clone());
        }

        let _ = app.emit(EVENT_TUNNEL_LOG, entry);
    }

    fn redact(&self, id: &str, message: &str) -> String {
        let token = self
            .state
            .lock()
            .expect("tunnel state lock poisoned")
            .tunnels
            .iter()
            .find(|tunnel| tunnel.id == id)
            .and_then(|tunnel| tunnel.token.clone());

        match token {
            Some(token) if !token.is_empty() => message.replace(&token, "[redacted]"),
            _ => message.to_string(),
        }
    }

    fn handle_output(
        &self,
        app: &AppHandle,
        id: &str,
        generation: u64,
        fallback_level: &str,
        line: &str,
    ) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }

        let level = if line.contains(" ERR ") || line.starts_with("ERR ") {
            "error"
        } else if line.contains(" WRN ") || line.starts_with("WRN ") {
            "warn"
        } else {
            fallback_level
        };
        self.log(app, id, level, line);

        if let Some(url) = extract_public_url(line) {
            let mut runtime = self.runtime.lock().expect("tunnel runtime lock poisoned");
            if let Some(entry) = runtime.get_mut(id)
                && entry.generation == generation
                && entry.public_url.is_none()
            {
                entry.public_url = Some(url);
                entry.state = TunnelState::Running;
                entry.error = None;
            }
            drop(runtime);
            self.emit_status(app, id);
        }
    }

    fn timeout(&self, app: AppHandle, id: String, generation: u64) {
        let (process, stopped) = {
            let mut runtime = self.runtime.lock().expect("tunnel runtime lock poisoned");
            let Some(entry) = runtime.get_mut(&id) else {
                return;
            };
            if entry.generation != generation || entry.state != TunnelState::Starting {
                return;
            }
            entry.state = TunnelState::Error;
            entry.error =
                Some("Cloudflare did not return a public URL within 45 seconds.".to_string());
            entry.public_url = None;
            (entry.process.take(), entry.stop.take())
        };

        if let Some(stopped) = stopped {
            stopped.store(true, Ordering::SeqCst);
        }
        if let Some(process) = process {
            let mut child = process.lock().expect("tunnel process lock poisoned");
            let _ = child.kill();
            let _ = child.wait();
        }

        self.emit_status(&app, &id);
        self.log(
            &app,
            &id,
            "error",
            "Cloudflare did not return a public URL within 45 seconds.",
        );
    }

    fn fail(&self, app: &AppHandle, id: &str, generation: u64, message: &str) {
        {
            let mut runtime = self.runtime.lock().expect("tunnel runtime lock poisoned");
            if let Some(entry) = runtime.get_mut(id) {
                if entry.generation != generation {
                    return;
                }
                entry.state = TunnelState::Error;
                entry.error = Some(message.to_string());
                entry.public_url = None;
            }
        }
        self.emit_status(app, id);
        self.log(app, id, "error", message);
    }

    fn process_exited(
        &self,
        app: &AppHandle,
        id: &str,
        generation: u64,
        status: std::process::ExitStatus,
    ) {
        let message = if status.success() {
            "The tunnel process exited.".to_string()
        } else {
            format!("The tunnel process exited ({status}).")
        };

        {
            let mut runtime = self.runtime.lock().expect("tunnel runtime lock poisoned");
            let Some(entry) = runtime.get_mut(id) else {
                return;
            };
            if entry.generation != generation || entry.state == TunnelState::Stopped {
                return;
            }
            entry.state = TunnelState::Error;
            entry.error = Some(message.clone());
            entry.public_url = None;
            entry.process = None;
        }

        self.emit_status(app, id);
        self.log(app, id, "error", &message);
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TunnelDeleted {
    id: String,
}

fn build_view(tunnel: &TunnelRecord, runtime: Option<&TunnelRuntime>) -> TunnelView {
    let (state, public_url, local_target, error) = match runtime {
        Some(entry) => (
            entry.state,
            entry.public_url.clone(),
            entry.local_target.clone(),
            entry.error.clone(),
        ),
        None => (TunnelState::Stopped, None, None, None),
    };

    TunnelView {
        id: tunnel.id.clone(),
        name: tunnel.name.clone(),
        service_id: tunnel.service_id.clone(),
        provider: tunnel.provider,
        state,
        public_url,
        local_target,
        error,
        autostart: tunnel.autostart,
        created_at: tunnel.created_at,
        updated_at: tunnel.updated_at,
    }
}

fn expected_public_url(record: &TunnelRecord) -> Option<String> {
    record
        .expected_url
        .clone()
        .or_else(|| {
            record
                .public_hostname
                .as_ref()
                .map(|host| format!("https://{host}"))
        })
        .filter(|url| !url.is_empty())
}

fn service_target(services: &ServiceManager, service_id: &str) -> Result<ServiceTarget, String> {
    let service = services
        .list()
        .into_iter()
        .find(|service| service.id == service_id)
        .ok_or_else(|| "The selected service was not found.".to_string())?;

    if service.state != ServiceState::Running {
        return Err("Start the service before starting its tunnel.".to_string());
    }

    match &service.source {
        ServiceSource::Static { .. }
        | ServiceSource::Proxy { .. }
        | ServiceSource::DefaultPage => {
            let port = service
                .local_port
                .ok_or_else(|| "The service has no local HTTP port yet.".to_string())?;
            Ok(ServiceTarget {
                host: "127.0.0.1".to_string(),
                port,
                url: format!("http://127.0.0.1:{port}"),
                is_tcp: false,
            })
        }
        ServiceSource::Tcp { host, port } => Ok(ServiceTarget {
            host: host.clone(),
            port: *port,
            url: format!("tcp://{host}:{port}"),
            is_tcp: true,
        }),
    }
}

fn validate_input(
    services: &ServiceManager,
    input: TunnelInput,
) -> Result<ValidatedTunnel, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("Give the tunnel a name.".to_string());
    }
    if name.chars().count() > 80 {
        return Err("The tunnel name must be 80 characters or fewer.".to_string());
    }

    let service_id = input.service_id.trim().to_string();
    let service: ServiceView = services
        .list()
        .into_iter()
        .find(|service| service.id == service_id)
        .ok_or_else(|| "Choose a service to expose.".to_string())?;

    if input.provider.is_cloudflare() && matches!(service.source, ServiceSource::Tcp { .. }) {
        return Err(
            "Cloudflare tunnels support HTTP and HTTPS services only. Use a custom command for raw TCP."
                .to_string(),
        );
    }

    let token = input
        .token
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if input.provider == TunnelProvider::CloudflareNamed && token.is_none() {
        return Err("Enter the Cloudflare tunnel token.".to_string());
    }
    if token.as_ref().is_some_and(|value| value.len() > 8192) {
        return Err("The Cloudflare tunnel token is too long.".to_string());
    }

    let public_hostname = input
        .public_hostname
        .map(|value| normalize_hostname(&value))
        .transpose()?;
    if input.provider == TunnelProvider::CloudflareNamed && public_hostname.is_none() {
        return Err("Enter the public hostname configured in Cloudflare.".to_string());
    }

    let custom_command = input
        .custom_command
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if input.provider == TunnelProvider::Custom {
        let command = custom_command
            .as_deref()
            .ok_or_else(|| "Enter the custom tunnel command.".to_string())?;
        if command.len() > 4096 {
            return Err("The custom tunnel command is too long.".to_string());
        }
        if !["{port}", "{target}", "{url}", "{host}"]
            .iter()
            .any(|token| command.contains(token))
        {
            return Err(
                "The custom command must include {port}, {target}, {url}, or {host}."
                    .to_string(),
            );
        }
    }

    let expected_url = input
        .expected_url
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(|value| normalize_url(&value))
        .transpose()?;

    Ok(ValidatedTunnel {
        name,
        service_id,
        provider: input.provider,
        token,
        public_hostname,
        custom_command,
        expected_url,
    })
}

fn normalize_hostname(value: &str) -> Result<String, String> {
    let value = value.trim();
    let value = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
        .unwrap_or(value)
        .trim_end_matches('/');
    if value.is_empty()
        || value.chars().any(char::is_whitespace)
        || value.contains('/')
        || value.contains('\\')
        || value.contains('@')
    {
        return Err("Enter a valid public hostname, such as host.example.com.".to_string());
    }
    Ok(value.to_string())
}

fn normalize_url(value: &str) -> Result<String, String> {
    let value = value.trim();
    if !(value.starts_with("https://") || value.starts_with("http://")) {
        return Err("The public URL must start with http:// or https://.".to_string());
    }
    if value.chars().any(char::is_whitespace) {
        return Err("The public URL cannot contain spaces.".to_string());
    }
    Ok(value.trim_end_matches('/').to_string())
}

fn extract_public_url(line: &str) -> Option<String> {
    for raw in line.split_whitespace() {
        let token = raw.trim_matches(|character: char| {
            matches!(
                character,
                '"' | '\'' | '`' | ',' | '.' | ';' | ':' | ')' | ']' | '}' | '|'
            )
        });
        if !(token.starts_with("https://") || token.starts_with("http://")) {
            continue;
        }
        if token.contains("trycloudflare.com") {
            return Some(token.to_string());
        }
        if token.contains("github.com")
            || token.contains("cloudflare.com")
            || token.contains("developers.")
            || token.contains("torproject.org")
            || token.contains("localhost")
            || token.contains("127.0.0.1")
        {
            continue;
        }
        if token.contains("ngrok") || token.contains('.') {
            return Some(token.to_string());
        }
    }
    None
}

fn spawn_reader<R: Read + Send + 'static>(
    manager: Arc<TunnelManager>,
    app: AppHandle,
    id: String,
    generation: u64,
    stream: R,
    level: &'static str,
) {
    thread::spawn(move || {
        let reader = BufReader::new(stream);
        for line in reader.lines() {
            match line {
                Ok(line) => manager.handle_output(&app, &id, generation, level, &line),
                Err(_) => break,
            }
        }
    });
}

fn spawn_monitor(
    manager: Arc<TunnelManager>,
    app: AppHandle,
    id: String,
    generation: u64,
    process: Arc<Mutex<Child>>,
    stopped: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        loop {
            if stopped.load(Ordering::SeqCst) {
                return;
            }

            let status = {
                let mut child = process.lock().expect("tunnel process lock poisoned");
                child.try_wait()
            };

            match status {
                Ok(Some(status)) => {
                    if !stopped.load(Ordering::SeqCst) {
                        manager.process_exited(&app, &id, generation, status);
                    }
                    return;
                }
                Ok(None) => thread::sleep(Duration::from_millis(250)),
                Err(error) => {
                    if !stopped.load(Ordering::SeqCst) {
                        manager.fail(
                            &app,
                            &id,
                            generation,
                            &format!("Could not monitor the tunnel process: {error}"),
                        );
                    }
                    return;
                }
            }
        }
    });
}

fn resolve_cloudflared(app: &AppHandle) -> Result<PathBuf, String> {
    let mut candidates = Vec::new();

    if let Some(path) = std::env::var_os("ONIONHOSTER_CLOUDFLARED") {
        candidates.push(PathBuf::from(path));
    }
    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("cloudflared").join("cloudflared"));
        candidates.push(
            resource_dir
                .join("resources")
                .join("cloudflared")
                .join("cloudflared"),
        );
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("cloudflared")
            .join("cloudflared"),
    );
    candidates.push(PathBuf::from("/usr/lib/OnionHoster/resources/cloudflared/cloudflared"));
    candidates.push(PathBuf::from("/usr/lib/onionhoster/resources/cloudflared/cloudflared"));
    candidates.push(PathBuf::from("/usr/bin/cloudflared"));
    candidates.push(PathBuf::from("/usr/local/bin/cloudflared"));

    if let Some(path) = find_in_path("cloudflared") {
        candidates.push(path);
    }

    candidates
        .into_iter()
        .find(|path| is_executable(path))
        .ok_or_else(|| {
            "The Cloudflare tunnel helper was not found. Install cloudflared or bundle it with OnionHoster."
                .to_string()
        })
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn load_state(path: &Path) -> Result<PersistedState, String> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let state: PersistedState = serde_json::from_str(&contents)
                .map_err(|error| format!("Could not parse the tunnel configuration: {error}"))?;
            if state.version != STATE_VERSION {
                return Err(format!(
                    "Unsupported tunnel configuration version {}.",
                    state.version
                ));
            }
            Ok(state)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(PersistedState::default()),
        Err(error) => Err(format!("Could not read the tunnel configuration: {error}")),
    }
}

fn atomic_write(path: &Path, contents: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "The tunnel configuration path has no parent.".to_string())?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Could not create the tunnel configuration directory at {}: {error}",
            parent.display()
        )
    })?;

    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temporary, contents).map_err(|error| {
        format!(
            "Could not write the tunnel configuration at {}: {error}",
            temporary.display()
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600));
    }

    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "Could not replace the tunnel configuration at {}: {error}",
            path.display()
        ));
    }

    Ok(())
}

#[tauri::command]
pub fn get_tunnels(tunnels: State<'_, Arc<TunnelManager>>) -> Vec<TunnelView> {
    tunnels.list()
}

#[tauri::command]
pub fn get_tunnel_logs(
    tunnels: State<'_, Arc<TunnelManager>>,
    id: String,
) -> Vec<TunnelLog> {
    tunnels.logs(&id)
}

#[tauri::command]
pub fn create_tunnel(
    app: AppHandle,
    tunnels: State<'_, Arc<TunnelManager>>,
    services: State<'_, Arc<ServiceManager>>,
    input: TunnelInput,
) -> Result<TunnelView, String> {
    tunnels.create(&app, services.inner().as_ref(), input)
}

#[tauri::command]
pub fn update_tunnel(
    app: AppHandle,
    tunnels: State<'_, Arc<TunnelManager>>,
    services: State<'_, Arc<ServiceManager>>,
    id: String,
    input: TunnelInput,
) -> Result<TunnelView, String> {
    tunnels.update(&app, services.inner().as_ref(), &id, input)
}

#[tauri::command]
pub fn delete_tunnel(
    app: AppHandle,
    tunnels: State<'_, Arc<TunnelManager>>,
    id: String,
) -> Result<(), String> {
    tunnels.delete(&app, &id)
}

#[tauri::command]
pub fn start_tunnel(
    app: AppHandle,
    tunnels: State<'_, Arc<TunnelManager>>,
    services: State<'_, Arc<ServiceManager>>,
    id: String,
) -> Result<TunnelView, String> {
    tunnels.start(&app, services.inner().as_ref(), &id)
}

#[tauri::command]
pub fn stop_tunnel(
    app: AppHandle,
    tunnels: State<'_, Arc<TunnelManager>>,
    id: String,
) -> Result<TunnelView, String> {
    tunnels.stop(&app, &id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_trycloudflare_urls() {
        let line = "INF |  https://example-tunnel.trycloudflare.com  |";
        assert_eq!(
            extract_public_url(line).as_deref(),
            Some("https://example-tunnel.trycloudflare.com")
        );
    }

    #[test]
    fn normalizes_public_urls() {
        assert_eq!(
            normalize_url("https://example.com/").unwrap(),
            "https://example.com"
        );
        assert!(normalize_url("example.com").is_err());
    }

    #[test]
    fn normalizes_hostnames() {
        assert_eq!(
            normalize_hostname("https://host.example.com/").unwrap(),
            "host.example.com"
        );
        assert!(normalize_hostname("host example.com").is_err());
    }
}
