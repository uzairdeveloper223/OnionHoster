use crate::{
    default_page, proxy, static_server,
    stats::ServiceStats,
    tor::{OnionLease, TorManager},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::oneshot;
use uuid::Uuid;

const STATE_VERSION: u32 = 1;
const EVENT_SERVICE_STATUS: &str = "service-status";
const EVENT_SERVICE_LOG: &str = "service-log";
const EVENT_SERVICE_STOPPED: &str = "service-stopped";
const EVENT_SERVICE_DELETED: &str = "service-deleted";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyScheme {
    Http,
    Https,
}

impl ProxyScheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ServiceSource {
    Static {
        root: String,
        #[serde(default)]
        spa_fallback: bool,
        #[serde(default)]
        directory_listing: bool,
    },
    Proxy {
        scheme: ProxyScheme,
        host: String,
        port: u16,
        #[serde(default)]
        rewrite_host: bool,
        #[serde(default)]
        accept_invalid_certs: bool,
    },
    Tcp {
        host: String,
        port: u16,
    },
    DefaultPage,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnionRecord {
    pub virtual_port: u16,
    #[serde(default)]
    pub service_id: Option<String>,
    #[serde(default)]
    pub private_key: Option<String>,
    #[serde(default)]
    pub pow: bool,
    #[serde(default)]
    pub max_streams: Option<u16>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceRecord {
    pub id: String,
    pub name: String,
    pub source: ServiceSource,
    pub onion: OnionRecord,
    #[serde(default)]
    pub autostart: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedState {
    version: u32,
    #[serde(default)]
    services: Vec<ServiceRecord>,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            services: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceState {
    Stopped,
    WaitingForTor,
    Starting,
    Running,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnionView {
    pub virtual_port: u16,
    pub service_id: Option<String>,
    pub pow: bool,
    pub max_streams: Option<u16>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatsView {
    pub requests: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub active: u64,
    pub last_request_at: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceView {
    pub id: String,
    pub name: String,
    pub source: ServiceSource,
    pub onion: OnionView,
    pub autostart: bool,
    pub created_at: u64,
    pub updated_at: u64,
    pub state: ServiceState,
    pub onion_address: Option<String>,
    pub local_port: Option<u16>,
    pub error: Option<String>,
    pub stats: ServiceStatsView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStopped {
    pub id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDeleted {
    pub id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceLog {
    pub id: String,
    pub level: String,
    pub message: String,
    pub timestamp_ms: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInput {
    pub name: String,
    pub source: ServiceSource,
    #[serde(default)]
    pub virtual_port: Option<u16>,
    #[serde(default)]
    pub autostart: bool,
}

struct RuntimeEntry {
    state: ServiceState,
    desired_running: bool,
    onion_address: Option<String>,
    local_port: Option<u16>,
    error: Option<String>,
    stats: Arc<ServiceStats>,
    shutdown: Option<oneshot::Sender<()>>,
}

struct LocalServer {
    port: u16,
    shutdown: oneshot::Sender<()>,
}

pub struct ServiceManager {
    state_path: PathBuf,
    state: Mutex<PersistedState>,
    runtime: Mutex<HashMap<String, RuntimeEntry>>,
    syncing: AtomicBool,
}

impl ServiceManager {
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

        let state_path = data_dir.join("services.json");
        let state = load_state(&state_path)?;
        let manager = Self {
            state_path,
            state: Mutex::new(state),
            runtime: Mutex::new(HashMap::new()),
            syncing: AtomicBool::new(false),
        };
        manager.ensure_runtime();
        Ok(manager)
    }

    fn ensure_runtime(&self) {
        let state = self.state.lock().expect("service state lock poisoned");
        let mut runtime = self.runtime.lock().expect("service runtime lock poisoned");

        for service in &state.services {
            runtime
                .entry(service.id.clone())
                .or_insert_with(|| RuntimeEntry {
                    state: ServiceState::Stopped,
                    desired_running: service.autostart,
                    onion_address: service
                        .onion
                        .service_id
                        .as_ref()
                        .map(|id| format!("{id}.onion")),
                    local_port: None,
                    error: None,
                    stats: Arc::new(ServiceStats::default()),
                    shutdown: None,
                });
        }
    }

    pub fn list(&self) -> Vec<ServiceView> {
        let state = self.state.lock().expect("service state lock poisoned");
        let runtime = self.runtime.lock().expect("service runtime lock poisoned");
        state
            .services
            .iter()
            .map(|service| build_view(service, runtime.get(&service.id)))
            .collect()
    }

    fn record(&self, id: &str) -> Result<ServiceRecord, String> {
        self.state
            .lock()
            .expect("service state lock poisoned")
            .services
            .iter()
            .find(|service| service.id == id)
            .cloned()
            .ok_or_else(|| "The service was not found.".to_string())
    }

    fn save(&self) -> Result<(), String> {
        let state = self.state.lock().expect("service state lock poisoned");
        let contents = serde_json::to_string_pretty(&*state)
            .map_err(|error| format!("Could not serialize the service configuration: {error}"))?;
        atomic_write(&self.state_path, &contents)
    }

    pub fn create(&self, app: &AppHandle, input: ServiceInput) -> Result<ServiceView, String> {
        let autostart = input.autostart;
        let (name, source, virtual_port) = validate_input(input)?;
        let id = Uuid::new_v4().to_string();
        let timestamp = now_ms();
        let record = ServiceRecord {
            id: id.clone(),
            name,
            source,
            onion: OnionRecord {
                virtual_port,
                service_id: None,
                private_key: None,
                pow: false,
                max_streams: None,
            },
            autostart,
            created_at: timestamp,
            updated_at: timestamp,
        };
        let view = {
            let mut state = self.state.lock().expect("service state lock poisoned");
            state.services.push(record.clone());

            let mut runtime = self.runtime.lock().expect("service runtime lock poisoned");
            runtime.insert(
                id.clone(),
                RuntimeEntry {
                    state: ServiceState::Stopped,
                    desired_running: autostart,
                    onion_address: None,
                    local_port: None,
                    error: None,
                    stats: Arc::new(ServiceStats::default()),
                    shutdown: None,
                },
            );

            build_view(&record, runtime.get(&id))
        };

        if let Err(error) = self.save() {
            let mut state = self.state.lock().expect("service state lock poisoned");
            state.services.retain(|service| service.id != id);
            self.runtime
                .lock()
                .expect("service runtime lock poisoned")
                .remove(&id);
            return Err(error);
        }

        self.log(app, &id, "info", "Service created.");
        let _ = app.emit(EVENT_SERVICE_STATUS, view.clone());
        Ok(view)
    }

    pub fn update(
        &self,
        app: &AppHandle,
        id: &str,
        input: ServiceInput,
    ) -> Result<ServiceView, String> {
        self.ensure_editable(id)?;
        let autostart = input.autostart;
        let (name, source, virtual_port) = validate_input(input)?;

        let (view, record) = {
            let mut state = self.state.lock().expect("service state lock poisoned");
            let record = state
                .services
                .iter_mut()
                .find(|service| service.id == id)
                .ok_or_else(|| "The service was not found.".to_string())?;

            record.name = name;
            record.source = source;
            record.autostart = autostart;
            record.onion.virtual_port = virtual_port;
            record.updated_at = now_ms();
            let record = record.clone();

            let mut runtime = self.runtime.lock().expect("service runtime lock poisoned");
            if let Some(entry) = runtime.get_mut(id) {
                entry.desired_running = autostart;
            }
            (build_view(&record, runtime.get(id)), record)
        };

        self.save()?;
        let _ = app.emit(EVENT_SERVICE_STATUS, view.clone());
        self.log(app, &record.id, "info", "Service settings updated.");
        Ok(view)
    }

    pub async fn delete(
        self: &Arc<Self>,
        app: &AppHandle,
        tor: &Arc<TorManager>,
        id: &str,
    ) -> Result<(), String> {
        let _ = self.stop(app, tor, id).await;

        {
            let mut state = self.state.lock().expect("service state lock poisoned");
            state.services.retain(|service| service.id != id);
            self.runtime
                .lock()
                .expect("service runtime lock poisoned")
                .remove(id);
        }

        self.save()?;
        let _ = app.emit(
            EVENT_SERVICE_DELETED,
            ServiceDeleted { id: id.to_string() },
        );
        self.log(app, id, "info", "Service deleted.");
        Ok(())
    }

    pub async fn start(
        self: &Arc<Self>,
        app: &AppHandle,
        tor: &Arc<TorManager>,
        id: &str,
    ) -> Result<ServiceView, String> {
        let record = self.record(id)?;

        let action = {
            let mut runtime = self.runtime.lock().expect("service runtime lock poisoned");
            let entry = runtime
                .get_mut(id)
                .ok_or_else(|| "The service runtime was not found.".to_string())?;

            entry.desired_running = true;
            entry.error = None;

            if matches!(entry.state, ServiceState::Starting | ServiceState::Running) {
                false
            } else if !tor.is_ready() {
                entry.state = ServiceState::WaitingForTor;
                entry.local_port = None;
                true
            } else {
                entry.state = ServiceState::Starting;
                true
            }
        };

        if !action {
            return self.view(id);
        }

        self.emit_status(app, id);
        if !tor.is_ready() {
            self.log(
                app,
                id,
                "info",
                "Service queued until the Tor runtime is ready.",
            );
            return self.view(id);
        }

        self.log(app, id, "info", "Starting the service.");
        match self.start_internal(app, tor, &record).await {
            Ok(view) => Ok(view),
            Err(error) => {
                self.fail(app, id, &error);
                Err(error)
            }
        }
    }

    async fn start_internal(
        self: &Arc<Self>,
        app: &AppHandle,
        tor: &Arc<TorManager>,
        record: &ServiceRecord,
    ) -> Result<ServiceView, String> {
        let stats = self
            .runtime
            .lock()
            .expect("service runtime lock poisoned")
            .get(&record.id)
            .map(|entry| entry.stats.clone())
            .unwrap_or_else(|| Arc::new(ServiceStats::default()));

        let (target, local_server) = match &record.source {
            ServiceSource::Static {
                root,
                spa_fallback,
                directory_listing,
            } => {
                let server = static_server::start(
                    PathBuf::from(root),
                    *spa_fallback,
                    *directory_listing,
                    stats.clone(),
                )
                .await?;
                (
                    format!("127.0.0.1:{}", server.port),
                    Some(LocalServer {
                        port: server.port,
                        shutdown: server.shutdown,
                    }),
                )
            }
            ServiceSource::Proxy {
                scheme,
                host,
                port,
                rewrite_host,
                accept_invalid_certs,
            } => {
                let server = proxy::start(
                    *scheme,
                    host.clone(),
                    *port,
                    *rewrite_host,
                    *accept_invalid_certs,
                    stats.clone(),
                )
                .await?;
                (
                    format!("127.0.0.1:{}", server.port),
                    Some(LocalServer {
                        port: server.port,
                        shutdown: server.shutdown,
                    }),
                )
            }
            ServiceSource::Tcp { host, port } => (format!("{host}:{port}"), None),
            ServiceSource::DefaultPage => {
                let server = default_page::start(stats.clone()).await?;
                (
                    format!("127.0.0.1:{}", server.port),
                    Some(LocalServer {
                        port: server.port,
                        shutdown: server.shutdown,
                    }),
                )
            }
        };

        let lease = self.add_onion(app, tor, record, &target).await;
        let lease = match lease {
            Ok(lease) => lease,
            Err(error) => {
                if let Some(server) = local_server {
                    let _ = server.shutdown.send(());
                }
                return Err(error);
            }
        };

        let (view, stored) = {
            let mut state = self.state.lock().expect("service state lock poisoned");
            let stored = state
                .services
                .iter_mut()
                .find(|service| service.id == record.id)
                .ok_or_else(|| "The service was not found.".to_string())?;
            stored.onion.service_id = Some(lease.service_id.clone());
            if let Some(private_key) = lease.private_key {
                stored.onion.private_key = Some(private_key);
            }
            stored.updated_at = now_ms();
            let stored = stored.clone();

            let mut runtime = self.runtime.lock().expect("service runtime lock poisoned");
            let entry = runtime
                .get_mut(&record.id)
                .ok_or_else(|| "The service runtime was not found.".to_string())?;
            entry.state = ServiceState::Running;
            entry.onion_address = Some(format!("{}.onion", lease.service_id));
            entry.local_port = local_server.as_ref().map(|server| server.port);
            entry.error = None;
            entry.shutdown = local_server.map(|server| server.shutdown);

            (build_view(&stored, Some(entry)), stored)
        };

        if let Err(error) = self.save() {
            if let Some(shutdown) = self
                .runtime
                .lock()
                .expect("service runtime lock poisoned")
                .get_mut(&record.id)
                .and_then(|entry| entry.shutdown.take())
            {
                let _ = shutdown.send(());
            }
            let _ = self.delete_onion(app, tor, &lease.service_id).await;
            return Err(error);
        }

        let _ = app.emit(EVENT_SERVICE_STATUS, view.clone());
        self.log(
            app,
            &stored.id,
            "info",
            &format!(
                "Onion service published at {}.",
                view.onion_address.as_deref().unwrap_or("unknown")
            ),
        );
        Ok(view)
    }

    async fn add_onion(
        self: &Arc<Self>,
        app: &AppHandle,
        tor: &Arc<TorManager>,
        record: &ServiceRecord,
        target: &str,
    ) -> Result<OnionLease, String> {
        let tor = tor.clone();
        let app = app.clone();
        let private_key = record.onion.private_key.clone();
        let service_id = record.onion.service_id.clone();
        let virtual_port = record.onion.virtual_port;
        let target = target.to_string();

        tauri::async_runtime::spawn_blocking(move || {
            if let Some(service_id) = service_id {
                let _ = tor.delete_onion(&app, &service_id);
            }
            tor.add_onion(&app, private_key.as_deref(), virtual_port, &target)
        })
        .await
        .map_err(|error| format!("The Tor command task failed: {error}"))?
    }

    async fn delete_onion(
        self: &Arc<Self>,
        app: &AppHandle,
        tor: &Arc<TorManager>,
        service_id: &str,
    ) -> Result<(), String> {
        let tor = tor.clone();
        let app = app.clone();
        let service_id = service_id.to_string();
        tauri::async_runtime::spawn_blocking(move || tor.delete_onion(&app, &service_id))
            .await
            .map_err(|error| format!("The Tor command task failed: {error}"))?
    }

    pub async fn stop(
        self: &Arc<Self>,
        app: &AppHandle,
        tor: &Arc<TorManager>,
        id: &str,
    ) -> Result<ServiceView, String> {
        let record = self.record(id)?;
        let (shutdown, service_id) = {
            let mut runtime = self.runtime.lock().expect("service runtime lock poisoned");
            let entry = runtime
                .get_mut(id)
                .ok_or_else(|| "The service runtime was not found.".to_string())?;
            entry.desired_running = false;
            entry.state = ServiceState::Stopped;
            entry.local_port = None;
            entry.error = None;
            (entry.shutdown.take(), record.onion.service_id.clone())
        };

        if let Some(shutdown) = shutdown {
            let _ = shutdown.send(());
        }

        if let Some(service_id) = service_id
            && tor.is_ready()
            && let Err(error) = self.delete_onion(app, tor, &service_id).await
        {
            self.log(app, id, "warn", &format!("Could not delete onion: {error}"));
        }

        let view = self.view(id)?;
        let _ = app.emit(EVENT_SERVICE_STATUS, view.clone());
        let _ = app.emit(
            EVENT_SERVICE_STOPPED,
            ServiceStopped { id: id.to_string() },
        );
        self.log(app, id, "info", "Service stopped.");
        Ok(view)
    }

    pub async fn restart(
        self: &Arc<Self>,
        app: &AppHandle,
        tor: &Arc<TorManager>,
        id: &str,
    ) -> Result<ServiceView, String> {
        let _ = self.stop(app, tor, id).await?;
        self.start(app, tor, id).await
    }

    pub async fn sync(self: &Arc<Self>, app: &AppHandle, tor: &Arc<TorManager>) {
        if self.syncing.swap(true, Ordering::SeqCst) {
            return;
        }

        self.sync_inner(app, tor).await;
        self.syncing.store(false, Ordering::SeqCst);
    }

    async fn sync_inner(self: &Arc<Self>, app: &AppHandle, tor: &Arc<TorManager>) {
        let ids: Vec<String> = self
            .runtime
            .lock()
            .expect("service runtime lock poisoned")
            .iter()
            .filter(|(_, entry)| entry.desired_running)
            .map(|(id, _)| id.clone())
            .collect();

        if !tor.is_ready() {
            self.mark_waiting(app, &ids);
            return;
        }

        for id in ids {
            let should_start = {
                let runtime = self.runtime.lock().expect("service runtime lock poisoned");
                runtime
                    .get(&id)
                    .map(|entry| {
                        !matches!(entry.state, ServiceState::Starting | ServiceState::Running)
                    })
                    .unwrap_or(false)
            };

            if should_start && let Err(error) = self.start(app, tor, &id).await {
                self.log(app, &id, "error", &error);
            }
        }
    }

    fn mark_waiting(&self, app: &AppHandle, ids: &[String]) {
        {
            let mut runtime = self.runtime.lock().expect("service runtime lock poisoned");
            for id in ids {
                if let Some(entry) = runtime.get_mut(id) {
                    if let Some(shutdown) = entry.shutdown.take() {
                        let _ = shutdown.send(());
                    }
                    entry.state = ServiceState::WaitingForTor;
                    entry.local_port = None;
                }
            }
        }

        for id in ids {
            if let Ok(view) = self.view(id) {
                let _ = app.emit(EVENT_SERVICE_STATUS, view);
            }
            let _ = app.emit(
                EVENT_SERVICE_STOPPED,
                ServiceStopped { id: id.clone() },
            );
        }
    }

    pub fn shutdown_all(&self) {
        let mut runtime = self.runtime.lock().expect("service runtime lock poisoned");
        for entry in runtime.values_mut() {
            entry.desired_running = false;
            if let Some(shutdown) = entry.shutdown.take() {
                let _ = shutdown.send(());
            }
        }
    }

    fn ensure_editable(&self, id: &str) -> Result<(), String> {
        let runtime = self.runtime.lock().expect("service runtime lock poisoned");
        match runtime.get(id) {
            Some(entry)
                if matches!(
                    entry.state,
                    ServiceState::Starting | ServiceState::Running | ServiceState::WaitingForTor
                ) =>
            {
                Err("Stop the service before changing its settings.".to_string())
            }
            _ => Ok(()),
        }
    }

    fn view(&self, id: &str) -> Result<ServiceView, String> {
        let state = self.state.lock().expect("service state lock poisoned");
        let runtime = self.runtime.lock().expect("service runtime lock poisoned");
        let service = state
            .services
            .iter()
            .find(|service| service.id == id)
            .ok_or_else(|| "The service was not found.".to_string())?;
        Ok(build_view(service, runtime.get(id)))
    }

    fn emit_status(&self, app: &AppHandle, id: &str) {
        if let Ok(view) = self.view(id) {
            let _ = app.emit(EVENT_SERVICE_STATUS, view);
        }
    }

    fn fail(&self, app: &AppHandle, id: &str, message: &str) {
        {
            let mut runtime = self.runtime.lock().expect("service runtime lock poisoned");
            if let Some(entry) = runtime.get_mut(id) {
                entry.state = ServiceState::Error;
                entry.error = Some(message.to_string());
                entry.local_port = None;
                if let Some(shutdown) = entry.shutdown.take() {
                    let _ = shutdown.send(());
                }
            }
        }

        if let Ok(view) = self.view(id) {
            let _ = app.emit(EVENT_SERVICE_STATUS, view);
        }
        let _ = app.emit(
            EVENT_SERVICE_STOPPED,
            ServiceStopped { id: id.to_string() },
        );

        self.log(app, id, "error", message);
    }

    fn log(&self, app: &AppHandle, id: &str, level: &str, message: &str) {
        let _ = app.emit(
            EVENT_SERVICE_LOG,
            ServiceLog {
                id: id.to_string(),
                level: level.to_string(),
                message: message.to_string(),
                timestamp_ms: now_ms(),
            },
        );
    }
}

fn build_view(service: &ServiceRecord, runtime: Option<&RuntimeEntry>) -> ServiceView {
    let (state, onion_address, local_port, error, stats) = match runtime {
        Some(entry) => (
            entry.state,
            entry.onion_address.clone().or_else(|| {
                service
                    .onion
                    .service_id
                    .as_ref()
                    .map(|id| format!("{id}.onion"))
            }),
            entry.local_port,
            entry.error.clone(),
            stats_view(&entry.stats),
        ),
        None => (
            ServiceState::Stopped,
            service
                .onion
                .service_id
                .as_ref()
                .map(|id| format!("{id}.onion")),
            None,
            None,
            ServiceStatsView {
                requests: 0,
                bytes_in: 0,
                bytes_out: 0,
                active: 0,
                last_request_at: None,
            },
        ),
    };

    ServiceView {
        id: service.id.clone(),
        name: service.name.clone(),
        source: service.source.clone(),
        onion: OnionView {
            virtual_port: service.onion.virtual_port,
            service_id: service.onion.service_id.clone(),
            pow: service.onion.pow,
            max_streams: service.onion.max_streams,
        },
        autostart: service.autostart,
        created_at: service.created_at,
        updated_at: service.updated_at,
        state,
        onion_address,
        local_port,
        error,
        stats,
    }
}

fn stats_view(stats: &ServiceStats) -> ServiceStatsView {
    let last_request = stats.last_request_ms.load(Ordering::Relaxed);
    ServiceStatsView {
        requests: stats.requests.load(Ordering::Relaxed),
        bytes_in: stats.bytes_in.load(Ordering::Relaxed),
        bytes_out: stats.bytes_out.load(Ordering::Relaxed),
        active: stats.active.load(Ordering::Relaxed),
        last_request_at: (last_request > 0).then_some(last_request),
    }
}

fn validate_input(input: ServiceInput) -> Result<(String, ServiceSource, u16), String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("Give the service a name.".to_string());
    }
    if name.chars().count() > 80 {
        return Err("The service name must be 80 characters or fewer.".to_string());
    }

    let source = match input.source {
        ServiceSource::Static {
            root,
            spa_fallback,
            directory_listing,
        } => {
            let root = root.trim().to_string();
            if root.is_empty() {
                return Err("Choose a folder or HTML file to host.".to_string());
            }
            let metadata = fs::metadata(&root)
                .map_err(|error| format!("Could not read the static path: {error}"))?;
            if !metadata.is_file() && !metadata.is_dir() {
                return Err("The static path must be a file or a folder.".to_string());
            }
            ServiceSource::Static {
                root,
                spa_fallback,
                directory_listing,
            }
        }
        ServiceSource::Proxy {
            scheme,
            host,
            port,
            rewrite_host,
            accept_invalid_certs,
        } => {
            let host = valid_host(&host)?;
            if port == 0 {
                return Err("The local server port must be between 1 and 65535.".to_string());
            }
            ServiceSource::Proxy {
                scheme,
                host,
                port,
                rewrite_host,
                accept_invalid_certs,
            }
        }
        ServiceSource::Tcp { host, port } => {
            let host = valid_host(&host)?;
            if port == 0 {
                return Err("The TCP port must be between 1 and 65535.".to_string());
            }
            ServiceSource::Tcp { host, port }
        }
        ServiceSource::DefaultPage => ServiceSource::DefaultPage,
    };

    let virtual_port = input
        .virtual_port
        .unwrap_or_else(|| default_virtual_port(&source));
    if virtual_port == 0 {
        return Err("The onion virtual port must be between 1 and 65535.".to_string());
    }

    Ok((name, source, virtual_port))
}

fn valid_host(host: &str) -> Result<String, String> {
    let host = host.trim().to_string();
    if host.is_empty() {
        return Err("Enter the local host or IP address.".to_string());
    }
    if host.chars().any(char::is_whitespace)
        || host.contains('/')
        || host.contains('\\')
        || host.contains('@')
    {
        return Err("The local host is not valid.".to_string());
    }
    Ok(host)
}

fn default_virtual_port(source: &ServiceSource) -> u16 {
    match source {
        ServiceSource::Static { .. } | ServiceSource::Proxy { .. } | ServiceSource::DefaultPage => {
            80
        }
        ServiceSource::Tcp { port, .. } => *port,
    }
}

fn load_state(path: &Path) -> Result<PersistedState, String> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let state: PersistedState = serde_json::from_str(&contents)
                .map_err(|error| format!("Could not parse the service configuration: {error}"))?;
            if state.version != STATE_VERSION {
                return Err(format!(
                    "Unsupported service configuration version {}.",
                    state.version
                ));
            }
            Ok(state)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(PersistedState::default()),
        Err(error) => Err(format!("Could not read the service configuration: {error}")),
    }
}

fn atomic_write(path: &Path, contents: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "The service configuration path has no parent.".to_string())?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Could not create the service configuration directory at {}: {error}",
            parent.display()
        )
    })?;

    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temporary, contents).map_err(|error| {
        format!(
            "Could not write the service configuration at {}: {error}",
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
            "Could not replace the service configuration at {}: {error}",
            path.display()
        ));
    }

    Ok(())
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[tauri::command]
pub fn get_services(services: State<'_, Arc<ServiceManager>>) -> Vec<ServiceView> {
    services.list()
}

#[tauri::command]
pub fn create_service(
    app: AppHandle,
    services: State<'_, Arc<ServiceManager>>,
    input: ServiceInput,
) -> Result<ServiceView, String> {
    services.create(&app, input)
}

#[tauri::command]
pub fn update_service(
    app: AppHandle,
    services: State<'_, Arc<ServiceManager>>,
    id: String,
    input: ServiceInput,
) -> Result<ServiceView, String> {
    services.update(&app, &id, input)
}

#[tauri::command]
pub async fn delete_service(
    app: AppHandle,
    tor: State<'_, Arc<TorManager>>,
    services: State<'_, Arc<ServiceManager>>,
    id: String,
) -> Result<(), String> {
    let tor = tor.inner().clone();
    let services = services.inner().clone();
    services.delete(&app, &tor, &id).await
}

#[tauri::command]
pub async fn start_service(
    app: AppHandle,
    tor: State<'_, Arc<TorManager>>,
    services: State<'_, Arc<ServiceManager>>,
    id: String,
) -> Result<ServiceView, String> {
    let tor = tor.inner().clone();
    let services = services.inner().clone();
    services.start(&app, &tor, &id).await
}

#[tauri::command]
pub async fn stop_service(
    app: AppHandle,
    tor: State<'_, Arc<TorManager>>,
    services: State<'_, Arc<ServiceManager>>,
    id: String,
) -> Result<ServiceView, String> {
    let tor = tor.inner().clone();
    let services = services.inner().clone();
    services.stop(&app, &tor, &id).await
}

#[tauri::command]
pub async fn restart_service(
    app: AppHandle,
    tor: State<'_, Arc<TorManager>>,
    services: State<'_, Arc<ServiceManager>>,
    id: String,
) -> Result<ServiceView, String> {
    let tor = tor.inner().clone();
    let services = services.inner().clone();
    services.restart(&app, &tor, &id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn static_input(root: &str) -> ServiceInput {
        ServiceInput {
            name: "Test".to_string(),
            source: ServiceSource::Static {
                root: root.to_string(),
                spa_fallback: false,
                directory_listing: false,
            },
            virtual_port: None,
            autostart: false,
        }
    }

    #[test]
    fn validates_static_folders() {
        let root = std::env::temp_dir().join(format!(
            "onionhoster-services-test-{}-{}",
            std::process::id(),
            now_ms()
        ));
        fs::create_dir_all(&root).expect("create test root");
        let input = static_input(root.to_str().expect("utf-8 path"));
        let (name, source, virtual_port) = validate_input(input).expect("validate input");
        assert_eq!(name, "Test");
        assert_eq!(virtual_port, 80);
        assert!(matches!(source, ServiceSource::Static { .. }));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_missing_static_folders() {
        let input = static_input("/definitely/not/a/real/onionhoster/path");
        assert!(validate_input(input).is_err());
    }

    #[test]
    fn rejects_invalid_tcp_hosts_and_ports() {
        let input = ServiceInput {
            name: "SSH".to_string(),
            source: ServiceSource::Tcp {
                host: "127.0.0.1".to_string(),
                port: 0,
            },
            virtual_port: None,
            autostart: false,
        };
        assert!(validate_input(input).is_err());

        let input = ServiceInput {
            name: "SSH".to_string(),
            source: ServiceSource::Tcp {
                host: "not a host".to_string(),
                port: 22,
            },
            virtual_port: None,
            autostart: false,
        };
        assert!(validate_input(input).is_err());
    }

    #[test]
    fn validates_default_page_services() {
        let input = ServiceInput {
            name: "About".to_string(),
            source: ServiceSource::DefaultPage,
            virtual_port: None,
            autostart: false,
        };
        let (name, source, virtual_port) = validate_input(input).expect("validate input");
        assert_eq!(name, "About");
        assert_eq!(virtual_port, 80);
        assert!(matches!(source, ServiceSource::DefaultPage));
    }

    #[test]
    fn serializes_default_page_source() {
        let json = serde_json::to_string(&ServiceSource::DefaultPage).expect("serialize source");
        assert_eq!(json, r#"{"kind":"default_page"}"#);
        let source: ServiceSource = serde_json::from_str(&json).expect("deserialize source");
        assert_eq!(source, ServiceSource::DefaultPage);
    }
}
