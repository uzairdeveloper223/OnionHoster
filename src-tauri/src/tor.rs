use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, VecDeque},
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager, State};

const EVENT_STATUS: &str = "tor-status";
const EVENT_LOG: &str = "tor-log";
const MAX_LOGS: usize = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TorPhase {
    Stopped,
    Starting,
    Bootstrapping,
    Ready,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorStatus {
    pub state: TorPhase,
    pub bootstrap: u8,
    pub summary: String,
    pub version: Option<String>,
    pub error: Option<String>,
    pub started_at: Option<u64>,
}

impl TorStatus {
    fn stopped() -> Self {
        Self {
            state: TorPhase::Stopped,
            bootstrap: 0,
            summary: "Tor is stopped.".to_string(),
            version: None,
            error: None,
            started_at: None,
        }
    }

    fn starting() -> Self {
        Self {
            state: TorPhase::Starting,
            bootstrap: 0,
            summary: "Starting the Tor runtime.".to_string(),
            version: None,
            error: None,
            started_at: Some(now_ms()),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorConfig {
    #[serde(default)]
    pub bridge_lines: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorLogEntry {
    pub id: u64,
    pub level: String,
    pub message: String,
    pub timestamp_ms: u64,
}

struct TorInner {
    child: Option<Child>,
    control: Option<TcpStream>,
    stop: Option<Arc<AtomicBool>>,
    lock: Option<fs::File>,
    active: bool,
    generation: u64,
}

pub struct TorManager {
    inner: Mutex<TorInner>,
    status: Mutex<TorStatus>,
    logs: Mutex<VecDeque<TorLogEntry>>,
    next_log_id: AtomicU64,
    generation: AtomicU64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OnionLease {
    pub service_id: String,
    pub private_key: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct AddOnionOptions {
    pub pow: bool,
    pub max_streams: Option<u16>,
    pub client_auth: Vec<String>,
}

impl TorManager {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(TorInner {
                child: None,
                control: None,
                stop: None,
                lock: None,
                active: false,
                generation: 0,
            }),
            status: Mutex::new(TorStatus::stopped()),
            logs: Mutex::new(VecDeque::with_capacity(MAX_LOGS)),
            next_log_id: AtomicU64::new(1),
            generation: AtomicU64::new(0),
        }
    }

    pub fn status(&self) -> TorStatus {
        self.status
            .lock()
            .expect("tor status lock poisoned")
            .clone()
    }

    pub fn logs(&self) -> Vec<TorLogEntry> {
        self.logs
            .lock()
            .expect("tor log lock poisoned")
            .iter()
            .cloned()
            .collect()
    }

    pub fn is_ready(&self) -> bool {
        self.status().state == TorPhase::Ready
    }

    pub fn add_onion(
        &self,
        app: &AppHandle,
        private_key: Option<&str>,
        virtual_port: u16,
        target: &str,
        options: &AddOnionOptions,
    ) -> Result<OnionLease, String> {
        let key = private_key.unwrap_or("NEW:ED25519-V3");
        let command = build_add_onion_command(key, virtual_port, target, options)?;

        let (port, cookie) = self.command_endpoint(app)?;
        let (mut stream, mut reader) = authenticate_control(port, &cookie)?;
        write_command(&mut stream, &command)?;
        let response = read_response(&mut reader)?;
        parse_add_onion_response(&response)
    }

    pub fn delete_onion(&self, app: &AppHandle, service_id: &str) -> Result<(), String> {
        if service_id.is_empty()
            || !service_id
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
        {
            return Err("The onion service identifier is invalid.".to_string());
        }

        let (port, cookie) = self.command_endpoint(app)?;
        let (mut stream, mut reader) = authenticate_control(port, &cookie)?;
        write_command(&mut stream, &format!("DEL_ONION {service_id}"))?;

        match read_response(&mut reader) {
            Ok(_) => Ok(()),
            Err(error) if error.contains("552") => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn command_endpoint(&self, app: &AppHandle) -> Result<(u16, PathBuf), String> {
        if !self.is_ready() {
            return Err("Tor is not ready yet.".to_string());
        }

        let paths = RuntimePaths::resolve(app)?;
        let port = read_control_port(&paths.control_port_file)
            .ok_or_else(|| "Tor's control port is not available.".to_string())?;

        if !paths.cookie_file.is_file() {
            return Err("Tor's authentication cookie is not available.".to_string());
        }

        Ok((port, paths.cookie_file))
    }

    fn recent_logs(&self, count: usize) -> String {
        self.logs
            .lock()
            .expect("tor log lock poisoned")
            .iter()
            .rev()
            .take(count)
            .rev()
            .map(|entry| entry.message.clone())
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn start(self: &Arc<Self>, app: AppHandle, config: TorConfig) -> Result<(), String> {
        let generation;
        let stop;
        let lock_file = RuntimePaths::resolve(&app)?.lock_file;

        {
            let mut inner = self.inner.lock().expect("tor state lock poisoned");
            if inner.active {
                return Err("Tor is already running or starting.".to_string());
            }

            if let Some(mut child) = inner.child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }

            if inner.lock.is_none() {
                inner.lock = Some(acquire_tor_lock(&lock_file)?);
            }

            generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
            stop = Arc::new(AtomicBool::new(false));

            inner.active = true;
            inner.generation = generation;
            inner.stop = Some(stop.clone());
            inner.control = None;
        }

        self.set_status(&app, TorStatus::starting());
        self.push_log(&app, "info", "Starting the Tor runtime.");

        let manager = self.clone();
        thread::spawn(move || manager.launch(app, config, generation, stop));
        Ok(())
    }

    pub fn stop(&self, app: Option<&AppHandle>) -> Result<(), String> {
        let (child, control, lock) = {
            let mut inner = self.inner.lock().expect("tor state lock poisoned");

            if !inner.active && inner.child.is_none() {
                drop(inner);
                if let Some(app) = app {
                    self.set_status(app, TorStatus::stopped());
                }
                return Ok(());
            }

            inner.active = false;
            self.generation.fetch_add(1, Ordering::SeqCst);
            if let Some(stop) = inner.stop.take() {
                stop.store(true, Ordering::SeqCst);
            }

            (inner.child.take(), inner.control.take(), inner.lock.take())
        };

        drop(lock);

        if let Some(mut control) = control {
            let _ = control.write_all(b"SIGNAL SHUTDOWN\r\n");
            let _ = control.flush();
        }

        if let Some(mut child) = child {
            terminate_child(&mut child);
        }

        if let Some(app) = app {
            self.set_status(app, TorStatus::stopped());
            self.push_log(app, "info", "Tor stopped.");
        }

        Ok(())
    }

    fn launch(
        self: Arc<Self>,
        app: AppHandle,
        config: TorConfig,
        generation: u64,
        stop: Arc<AtomicBool>,
    ) {
        match self.launch_inner(&app, &config, generation, &stop) {
            Err(error) if self.is_current(generation) => {
                self.fail(&app, generation, error);
            }
            _ => {}
        }
    }

    fn launch_inner(
        self: &Arc<Self>,
        app: &AppHandle,
        config: &TorConfig,
        generation: u64,
        stop: &AtomicBool,
    ) -> Result<(), String> {
        let paths = RuntimePaths::resolve(app)?;
        paths.prepare()?;
        paths.write_torrc(config)?;

        let mut command = Command::new(&paths.tor);
        command
            .arg("--defaults-torrc")
            .arg(&paths.defaults_torrc)
            .arg("-f")
            .arg(&paths.torrc)
            .env("LD_LIBRARY_PATH", &paths.tor_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command.spawn().map_err(|error| {
            format!(
                "Could not start the bundled Tor binary at {}: {error}",
                paths.tor.display()
            )
        })?;

        if let Some(stdout) = child.stdout.take() {
            let manager = self.clone();
            let app = app.clone();
            thread::spawn(move || manager.read_output(app, generation, stdout, "info"));
        }

        if let Some(stderr) = child.stderr.take() {
            let manager = self.clone();
            let app = app.clone();
            thread::spawn(move || manager.read_output(app, generation, stderr, "warn"));
        }

        if stop.load(Ordering::SeqCst) || !self.is_current(generation) {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(());
        }

        self.push_log(app, "info", "Tor process started.");

        let control = match wait_for_control(self, &paths, &mut child, generation, stop) {
            Ok(control) => control,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };

        {
            let mut inner = self.inner.lock().expect("tor state lock poisoned");
            if stop.load(Ordering::SeqCst)
                || !inner.active
                || inner.generation != generation
                || self.generation.load(Ordering::SeqCst) != generation
            {
                drop(inner);
                let _ = child.kill();
                let _ = child.wait();
                return Ok(());
            }

            inner.child = Some(child);
            inner.control = Some(control.stream);
        }

        let control_reader = control.reader;
        {
            let manager = self.clone();
            let app = app.clone();
            thread::spawn(move || manager.read_control(app, generation, control_reader));
        }

        {
            let manager = self.clone();
            let app = app.clone();
            thread::spawn(move || manager.monitor(app, generation));
        }

        let (progress, summary) = control.bootstrap.unwrap_or((
            0,
            "Connected to Tor. Bootstrapping the network.".to_string(),
        ));
        let mut status = self.status.lock().expect("tor status lock poisoned");
        status.state = if progress >= 100 {
            TorPhase::Ready
        } else {
            TorPhase::Bootstrapping
        };
        status.bootstrap = progress;
        status.summary = summary;
        status.version = control.version;
        status.error = None;
        let snapshot = status.clone();
        drop(status);

        let _ = app.emit(EVENT_STATUS, snapshot);
        self.push_log(app, "info", "Tor control connection established.");

        Ok(())
    }

    fn read_output<R: Read + Send + 'static>(
        self: Arc<Self>,
        app: AppHandle,
        generation: u64,
        reader: R,
        default_level: &'static str,
    ) {
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }

            let (level, message) = parse_tor_output_line(&line, default_level);
            if message.is_empty() {
                continue;
            }

            self.push_log(&app, level, &message);

            if let Some((progress, summary)) = parse_bootstrap_line(&message) {
                self.update_bootstrap(&app, generation, progress, summary);
            }
        }
    }

    fn read_control(
        self: Arc<Self>,
        app: AppHandle,
        generation: u64,
        reader: BufReader<TcpStream>,
    ) {
        let mut reader = reader;
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }

            let message = line.trim_end();
            if let Some((progress, summary)) = parse_bootstrap_line(message) {
                self.update_bootstrap(&app, generation, progress, summary);
            } else if message.starts_with("650") && message.contains("CIRCUIT_ESTABLISHED") {
                self.push_log(&app, "info", "Tor circuit established.");
            }
        }

        if self.is_current(generation) {
            self.fail(
                &app,
                generation,
                "Lost the Tor control connection.".to_string(),
            );
        }
    }

    fn monitor(self: Arc<Self>, app: AppHandle, generation: u64) {
        loop {
            thread::sleep(Duration::from_millis(500));

            if self.generation.load(Ordering::SeqCst) != generation {
                return;
            }

            let outcome = {
                let mut inner = self.inner.lock().expect("tor state lock poisoned");
                if !inner.active || inner.generation != generation {
                    return;
                }

                let Some(child) = inner.child.as_mut() else {
                    return;
                };

                match child.try_wait() {
                    Ok(Some(status)) => {
                        inner.child = None;
                        inner.control = None;
                        inner.active = false;
                        Ok(Some(status))
                    }
                    Ok(None) => Ok(None),
                    Err(error) => {
                        inner.child = None;
                        inner.control = None;
                        inner.active = false;
                        Err(error.to_string())
                    }
                }
            };

            match outcome {
                Ok(Some(status)) => {
                    let message = if status.success() {
                        "Tor exited unexpectedly.".to_string()
                    } else {
                        format!("Tor exited unexpectedly with {status}.")
                    };
                    self.fail(&app, generation, message);
                    return;
                }
                Ok(None) => {}
                Err(error) => {
                    self.fail(
                        &app,
                        generation,
                        format!("Could not monitor the Tor process: {error}"),
                    );
                    return;
                }
            }
        }
    }

    fn update_bootstrap(&self, app: &AppHandle, generation: u64, progress: u8, summary: String) {
        if self.generation.load(Ordering::SeqCst) != generation {
            return;
        }

        let mut status = self.status.lock().expect("tor status lock poisoned");
        if matches!(status.state, TorPhase::Stopped | TorPhase::Error) {
            return;
        }

        if progress < status.bootstrap
            && matches!(status.state, TorPhase::Bootstrapping | TorPhase::Ready)
        {
            return;
        }

        let progress = progress.min(100);
        let next_state = if progress >= 100 {
            TorPhase::Ready
        } else {
            TorPhase::Bootstrapping
        };

        let changed = status.state != next_state
            || status.bootstrap != progress
            || (!summary.is_empty() && status.summary != summary);

        if !changed {
            return;
        }

        status.state = next_state;
        status.bootstrap = progress;
        if !summary.is_empty() {
            status.summary = summary;
        }
        status.error = None;

        let snapshot = status.clone();
        drop(status);

        let _ = app.emit(EVENT_STATUS, snapshot);
    }

    fn fail(&self, app: &AppHandle, generation: u64, message: String) {
        if self.generation.load(Ordering::SeqCst) != generation {
            return;
        }

        let child = {
            let mut inner = self.inner.lock().expect("tor state lock poisoned");
            inner.active = false;
            inner.control = None;
            inner.stop = None;
            inner.child.take()
        };

        if let Some(mut child) = child {
            terminate_child(&mut child);
        }

        let mut status = self.status.lock().expect("tor status lock poisoned");
        if matches!(status.state, TorPhase::Stopped | TorPhase::Error) {
            return;
        }

        status.state = TorPhase::Error;
        status.summary = "Tor stopped unexpectedly.".to_string();
        status.error = Some(message.clone());
        let snapshot = status.clone();
        drop(status);

        let _ = app.emit(EVENT_STATUS, snapshot);
        self.push_log(app, "error", &message);
    }

    fn is_current(&self, generation: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == generation
            && self.inner.lock().expect("tor state lock poisoned").active
    }

    fn set_status(&self, app: &AppHandle, status: TorStatus) {
        {
            let mut current = self.status.lock().expect("tor status lock poisoned");
            *current = status.clone();
        }
        let _ = app.emit(EVENT_STATUS, status);
    }

    fn push_log(&self, app: &AppHandle, level: &str, message: &str) {
        let entry = TorLogEntry {
            id: self.next_log_id.fetch_add(1, Ordering::SeqCst),
            level: level.to_string(),
            message: message.to_string(),
            timestamp_ms: now_ms(),
        };

        {
            let mut logs = self.logs.lock().expect("tor log lock poisoned");
            if logs.len() >= MAX_LOGS {
                logs.pop_front();
            }
            logs.push_back(entry.clone());
        }

        let _ = app.emit(EVENT_LOG, entry);
    }
}

impl Default for TorManager {
    fn default() -> Self {
        Self::new()
    }
}

struct RuntimePaths {
    tor_dir: PathBuf,
    tor: PathBuf,
    lyrebird: PathBuf,
    geoip: PathBuf,
    geoip6: PathBuf,
    data_dir: PathBuf,
    torrc: PathBuf,
    defaults_torrc: PathBuf,
    control_port_file: PathBuf,
    cookie_file: PathBuf,
    lock_file: PathBuf,
}

impl RuntimePaths {
    fn resolve(app: &AppHandle) -> Result<Self, String> {
        let tor_dir = crate::runtime::ensure_tor_runtime(app)?;

        let base_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| format!("Could not resolve the application data directory: {error}"))?
            .join("tor");

        Ok(Self {
            tor: tor_dir.join("tor"),
            lyrebird: tor_dir.join("lyrebird"),
            geoip: tor_dir.join("geoip"),
            geoip6: tor_dir.join("geoip6"),
            data_dir: base_dir.join("data"),
            torrc: base_dir.join("torrc"),
            defaults_torrc: base_dir.join("defaults.torrc"),
            control_port_file: base_dir.join("control-port"),
            cookie_file: base_dir.join("data").join("control_auth_cookie"),
            lock_file: base_dir.join("tor.lock"),
            tor_dir,
        })
    }

    fn prepare(&self) -> Result<(), String> {
        fs::create_dir_all(&self.data_dir).map_err(|error| {
            format!(
                "Could not create the Tor data directory at {}: {error}",
                self.data_dir.display()
            )
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.data_dir, fs::Permissions::from_mode(0o700));
        }

        let _ = fs::remove_file(&self.control_port_file);

        fs::write(&self.defaults_torrc, "AvoidDiskWrites 1\nSafeLogging 1\n").map_err(|error| {
            format!(
                "Could not write the Tor defaults file at {}: {error}",
                self.defaults_torrc.display()
            )
        })?;

        set_private_file(&self.defaults_torrc)?;
        Ok(())
    }

    fn write_torrc(&self, config: &TorConfig) -> Result<(), String> {
        let mut lines = vec![
            format!("DataDirectory \"{}\"", self.data_dir.display()),
            "ControlPort auto".to_string(),
            format!(
                "ControlPortWriteToFile \"{}\"",
                self.control_port_file.display()
            ),
            "CookieAuthentication 1".to_string(),
            "SocksPort 0".to_string(),
            "Log notice stdout".to_string(),
            "SafeLogging 1".to_string(),
        ];

        if self.geoip.is_file() {
            lines.push(format!("GeoIPFile \"{}\"", self.geoip.display()));
        }
        if self.geoip6.is_file() {
            lines.push(format!("GeoIPv6File \"{}\"", self.geoip6.display()));
        }

        let bridges: Vec<String> = config
            .bridge_lines
            .iter()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| {
                line.strip_prefix("Bridge ")
                    .unwrap_or(line)
                    .trim()
                    .to_string()
            })
            .collect();

        if bridges.is_empty() {
            lines.push("UseBridges 0".to_string());
        } else {
            if bridges.len() > 20 {
                return Err("At most 20 Tor bridges can be used at once.".to_string());
            }

            let mut transports = BTreeSet::new();
            for bridge in &bridges {
                if bridge.contains('\r') || bridge.contains('\n') {
                    return Err("Bridge lines must be single lines.".to_string());
                }

                let transport = bridge
                    .split_whitespace()
                    .next()
                    .ok_or_else(|| "A bridge line is missing its transport.".to_string())?;

                match transport {
                    "obfs4" | "webtunnel" | "meek_lite" => {
                        transports.insert("lyrebird");
                    }
                    "snowflake" => {
                        transports.insert("snowflake");
                    }
                    other => {
                        return Err(format!(
                            "Unsupported bridge transport: {other}. Use obfs4, snowflake, webtunnel, or meek_lite."
                        ));
                    }
                }
            }

            lines.push("UseBridges 1".to_string());

            if transports.contains("lyrebird") {
                lines.push(format!(
                    "ClientTransportPlugin meek_lite,obfs4,webtunnel exec \"{}\"",
                    self.lyrebird.display()
                ));
            }
            if transports.contains("snowflake") {
                lines.push(format!(
                    "ClientTransportPlugin snowflake exec \"{}\"",
                    self.lyrebird.display()
                ));
            }

            for bridge in bridges {
                lines.push(format!("Bridge {bridge}"));
            }
        }

        let content = format!("{}\n", lines.join("\n"));
        fs::write(&self.torrc, content).map_err(|error| {
            format!(
                "Could not write the Tor configuration at {}: {error}",
                self.torrc.display()
            )
        })?;
        set_private_file(&self.torrc)?;
        Ok(())
    }
}

struct ControlConnection {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
    version: Option<String>,
    bootstrap: Option<(u8, String)>,
}

fn build_add_onion_command(
    key: &str,
    virtual_port: u16,
    target: &str,
    options: &AddOnionOptions,
) -> Result<String, String> {
    if !(1..=65535).contains(&virtual_port) {
        return Err("The onion virtual port must be between 1 and 65535.".to_string());
    }
    if target.is_empty() || target.chars().any(char::is_whitespace) {
        return Err("The onion service target is invalid.".to_string());
    }
    if key.is_empty() || key.chars().any(char::is_whitespace) {
        return Err("The onion private key is invalid.".to_string());
    }
    for public_key in &options.client_auth {
        if public_key.len() != 52
            || !public_key
                .chars()
                .all(|character| character.is_ascii_uppercase() || ('2'..='7').contains(&character))
        {
            return Err("A client authorization public key is invalid.".to_string());
        }
    }

    let max_streams = options.max_streams.filter(|value| *value > 0);
    let mut flags = vec!["Detach"];
    if !options.client_auth.is_empty() {
        flags.push("V3Auth");
    }
    if max_streams.is_some() {
        flags.push("MaxStreamsCloseCircuit");
    }

    let mut command = format!("ADD_ONION {key} Flags={}", flags.join(","));
    if let Some(max_streams) = max_streams {
        command.push_str(&format!(" MaxStreams={max_streams}"));
    }
    if options.pow {
        command.push_str(" PoWDefensesEnabled=1");
    }
    command.push_str(&format!(" Port={virtual_port},{target}"));
    for public_key in &options.client_auth {
        command.push_str(&format!(" ClientAuthV3={public_key}"));
    }

    Ok(command)
}

fn acquire_tor_lock(path: &Path) -> Result<fs::File, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Could not create the Tor runtime directory at {}: {error}",
                parent.display()
            )
        })?;
    }

    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
        .map_err(|error| format!("Could not open the Tor lock file: {error}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }

    file.try_lock().map_err(|error| match error {
        std::fs::TryLockError::WouldBlock => {
            "Another OnionHoster instance is already using Tor.".to_string()
        }
        std::fs::TryLockError::Error(error) => {
            format!("Could not lock the Tor runtime: {error}")
        }
    })?;
    Ok(file)
}

fn wait_for_control(
    manager: &TorManager,
    paths: &RuntimePaths,
    child: &mut Child,
    generation: u64,
    stop: &AtomicBool,
) -> Result<ControlConnection, String> {
    let started = Instant::now();
    let mut last_error = None;

    loop {
        if stop.load(Ordering::SeqCst) || !manager.is_current(generation) {
            return Err("Tor start was cancelled.".to_string());
        }

        if let Ok(Some(status)) = child.try_wait() {
            let tail = manager.recent_logs(4);
            let suffix = if tail.is_empty() {
                String::new()
            } else {
                format!(" Last log lines: {tail}")
            };
            return Err(format!(
                "Tor exited before opening its control port ({status}).{suffix}"
            ));
        }

        if paths.control_port_file.is_file()
            && paths.cookie_file.is_file()
            && let Some(port) = read_control_port(&paths.control_port_file)
        {
            match connect_control(port, &paths.cookie_file) {
                Ok(connection) => return Ok(connection),
                Err(error) => last_error = Some(error),
            }
        }

        if started.elapsed() > Duration::from_secs(20) {
            return Err(last_error.unwrap_or_else(|| {
                "Timed out while waiting for Tor to open its control port.".to_string()
            }));
        }

        if !manager.is_current(generation) {
            return Err("Tor start was cancelled.".to_string());
        }

        thread::sleep(Duration::from_millis(150));
    }
}

fn authenticate_control(
    port: u16,
    cookie_path: &Path,
) -> Result<(TcpStream, BufReader<TcpStream>), String> {
    let cookie = fs::read(cookie_path).map_err(|error| {
        format!(
            "Could not read the Tor authentication cookie at {}: {error}",
            cookie_path.display()
        )
    })?;

    if cookie.is_empty() {
        return Err("Tor wrote an empty authentication cookie.".to_string());
    }

    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .map_err(|error| format!("Could not connect to Tor's control port {port}: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|error| format!("Could not configure the Tor control connection: {error}"))?;

    let mut reader = BufReader::new(
        stream
            .try_clone()
            .map_err(|error| format!("Could not read from the Tor control connection: {error}"))?,
    );

    let cookie_hex: String = cookie.iter().map(|byte| format!("{byte:02x}")).collect();
    write_command(&mut stream, &format!("AUTHENTICATE {cookie_hex}"))?;
    let response = read_response(&mut reader)?;
    if !response.iter().any(|line| line.starts_with("250")) {
        return Err(format!(
            "Tor rejected the control connection: {}",
            response.join(" ")
        ));
    }

    Ok((stream, reader))
}

fn connect_control(port: u16, cookie_path: &Path) -> Result<ControlConnection, String> {
    let (mut stream, mut reader) = authenticate_control(port, cookie_path)?;

    write_command(&mut stream, "SETEVENTS STATUS_CLIENT")?;
    let _ = read_response(&mut reader)?;

    write_command(&mut stream, "GETINFO version")?;
    let response = read_response(&mut reader)?;
    let version = response.iter().find_map(|line| parse_version_line(line));

    write_command(&mut stream, "GETINFO status/bootstrap-phase")?;
    let response = read_response(&mut reader)?;
    let bootstrap = response.iter().find_map(|line| parse_bootstrap_line(line));

    stream
        .set_read_timeout(None)
        .map_err(|error| format!("Could not configure the Tor control connection: {error}"))?;

    Ok(ControlConnection {
        stream,
        reader,
        version,
        bootstrap,
    })
}

fn write_command(stream: &mut TcpStream, command: &str) -> Result<(), String> {
    stream
        .write_all(format!("{command}\r\n").as_bytes())
        .map_err(|error| format!("Could not write to the Tor control connection: {error}"))?;
    stream
        .flush()
        .map_err(|error| format!("Could not flush the Tor control connection: {error}"))
}

fn read_response(reader: &mut BufReader<TcpStream>) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();

    loop {
        let mut line = String::new();
        let read = reader
            .read_line(&mut line)
            .map_err(|error| format!("Could not read the Tor control response: {error}"))?;

        if read == 0 {
            return Err("Tor closed the control connection.".to_string());
        }

        let line = line.trim_end_matches(['\r', '\n']).to_string();

        if line.starts_with("650") {
            continue;
        }

        let final_line = line.starts_with("250 ")
            || line.starts_with("250 OK")
            || line.starts_with("4")
            || line.starts_with("5");

        lines.push(line.clone());

        if final_line {
            if line.starts_with("4") || line.starts_with("5") {
                return Err(format!("Tor control error: {}", lines.join(" ")));
            }
            return Ok(lines);
        }
    }
}

fn read_control_port(path: &Path) -> Option<u16> {
    let contents = fs::read_to_string(path).ok()?;
    contents.lines().find_map(|line| {
        line.trim()
            .strip_prefix("PORT=")
            .and_then(|address| address.rsplit(':').next())
            .and_then(|port| port.parse().ok())
    })
}

fn parse_tor_output_line(line: &str, default_level: &'static str) -> (&'static str, String) {
    let line = line.trim();
    let Some(open) = line.find('[') else {
        return (default_level, line.to_string());
    };
    let Some(close_offset) = line[open..].find(']') else {
        return (default_level, line.to_string());
    };
    let close = open + close_offset;
    let prefix = &line[..open];
    let has_timestamp = prefix.is_empty() || prefix.contains(':');

    if !has_timestamp {
        return (default_level, line.to_string());
    }

    let tag = &line[open + 1..close];
    let level = match tag {
        "warn" => "warn",
        "err" | "error" => "error",
        _ => "info",
    };
    let message = line[close + 1..].trim_start().to_string();

    (level, message)
}

fn parse_version_line(line: &str) -> Option<String> {
    line.split_once("version=").map(|(_, value)| {
        value
            .split_whitespace()
            .next()
            .unwrap_or(value)
            .trim()
            .to_string()
    })
}

fn parse_add_onion_response(response: &[String]) -> Result<OnionLease, String> {
    let service_id = response
        .iter()
        .find_map(|line| line.strip_prefix("250-ServiceID="))
        .map(str::to_string)
        .ok_or_else(|| {
            format!(
                "Tor did not return an onion service identifier: {}",
                response.join(" ")
            )
        })?;

    if service_id.is_empty()
        || !service_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        return Err("Tor returned an invalid onion service identifier.".to_string());
    }

    let private_key = response
        .iter()
        .find_map(|line| line.strip_prefix("250-PrivateKey="))
        .map(str::to_string);

    Ok(OnionLease {
        service_id,
        private_key,
    })
}

fn parse_bootstrap_line(line: &str) -> Option<(u8, String)> {
    let marker = "BOOTSTRAP PROGRESS=";
    let index = line.find(marker)?;
    let rest = &line[index + marker.len()..];
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let progress = digits.parse::<u8>().ok()?.min(100);
    let summary = parse_summary(line).or_else(|| {
        line.find("TAG=").map(|tag_index| {
            let tag = line[tag_index + 4..]
                .split_whitespace()
                .next()
                .unwrap_or("unknown");
            format!("Bootstrapping ({tag})")
        })
    });

    Some((progress, summary.unwrap_or_default()))
}

fn parse_summary(line: &str) -> Option<String> {
    let marker = "SUMMARY=\"";
    let start = line.find(marker)? + marker.len();
    let mut summary = String::new();
    let mut escaped = false;

    for character in line[start..].chars() {
        if escaped {
            summary.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return Some(summary);
        } else {
            summary.push(character);
        }
    }

    None
}

fn terminate_child(child: &mut Child) {
    let started = Instant::now();

    while started.elapsed() < Duration::from_secs(4) {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(_) => break,
        }
    }

    let _ = child.kill();
    let _ = child.wait();
}

fn set_private_file(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("Could not set permissions on {}: {error}", path.display()))?;
    }

    Ok(())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[tauri::command]
pub fn get_tor_status(manager: State<'_, Arc<TorManager>>) -> TorStatus {
    manager.status()
}

#[tauri::command]
pub fn get_tor_logs(manager: State<'_, Arc<TorManager>>) -> Vec<TorLogEntry> {
    manager.logs()
}

#[tauri::command]
pub fn start_tor(
    app: AppHandle,
    manager: State<'_, Arc<TorManager>>,
    config: TorConfig,
) -> Result<(), String> {
    manager.inner().clone().start(app, config)
}

#[tauri::command]
pub async fn stop_tor(app: AppHandle) -> Result<(), String> {
    let tor = app.state::<Arc<TorManager>>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || tor.stop(Some(&app)))
        .await
        .map_err(|error| format!("Stopping Tor failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    fn temp_paths() -> RuntimePaths {
        let root = std::env::temp_dir().join(format!(
            "onionhoster-tor-test-{}-{}",
            std::process::id(),
            TEST_COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(root.join("data")).expect("create test data dir");

        RuntimePaths {
            tor_dir: PathBuf::from("/tmp/tor"),
            tor: PathBuf::from("/tmp/tor/tor"),
            lyrebird: PathBuf::from("/tmp/tor/lyrebird"),
            geoip: PathBuf::from("/tmp/tor/geoip"),
            geoip6: PathBuf::from("/tmp/tor/geoip6"),
            data_dir: root.join("data"),
            torrc: root.join("torrc"),
            defaults_torrc: root.join("defaults.torrc"),
            control_port_file: root.join("control-port"),
            cookie_file: root.join("data/control_auth_cookie"),
            lock_file: root.join("tor.lock"),
        }
    }

    #[test]
    fn strips_tor_output_prefix() {
        assert_eq!(
            parse_tor_output_line(
                "Oct 25 07:31:12.000 [notice] Bootstrapped 100% (done): Done\n",
                "info"
            ),
            ("info", "Bootstrapped 100% (done): Done".to_string())
        );
        assert_eq!(
            parse_tor_output_line("[warn] Failed to open a circuit", "info"),
            ("warn", "Failed to open a circuit".to_string())
        );
        assert_eq!(
            parse_tor_output_line("plain stderr output", "warn"),
            ("warn", "plain stderr output".to_string())
        );
        assert_eq!(
            parse_tor_output_line("a message with [brackets] inside", "info"),
            ("info", "a message with [brackets] inside".to_string())
        );
    }

    #[test]
    fn trims_git_suffix_from_version() {
        assert_eq!(
            parse_version_line("250-version=0.4.9.14 (git-1a2b3c4d5e6f7890)"),
            Some("0.4.9.14".to_string())
        );
        assert_eq!(
            parse_version_line("250-version=0.4.9.14"),
            Some("0.4.9.14".to_string())
        );
        assert_eq!(parse_version_line("250-uptime=42"), None);
    }

    #[test]
    fn parses_add_onion_response() {
        let response = vec![
            "250-ServiceID=abcdefghijklmnopqrstuvwxyz234567abcdefghijklmnopqrstuvwxyz234567"
                .to_string(),
            "250-PrivateKey=ED25519-V3:example-key".to_string(),
            "250 OK".to_string(),
        ];
        let lease = parse_add_onion_response(&response).expect("parse onion lease");
        assert_eq!(
            lease.service_id,
            "abcdefghijklmnopqrstuvwxyz234567abcdefghijklmnopqrstuvwxyz234567"
        );
        assert_eq!(lease.private_key.as_deref(), Some("ED25519-V3:example-key"));
    }

    #[test]
    fn rejects_add_onion_response_without_service_id() {
        let response = vec!["250 OK".to_string()];
        assert!(parse_add_onion_response(&response).is_err());
    }

    #[test]
    fn builds_add_onion_security_options() {
        let public_key = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567ABCDEFGHIJKLMNOPQRST";
        let options = AddOnionOptions {
            pow: true,
            max_streams: Some(8),
            client_auth: vec![public_key.to_string()],
        };
        let command = build_add_onion_command("NEW:ED25519-V3", 80, "127.0.0.1:43210", &options)
            .expect("build command");
        assert_eq!(
            command,
            format!(
                "ADD_ONION NEW:ED25519-V3 Flags=Detach,V3Auth,MaxStreamsCloseCircuit MaxStreams=8 PoWDefensesEnabled=1 Port=80,127.0.0.1:43210 ClientAuthV3={public_key}"
            )
        );
    }

    #[test]
    fn rejects_invalid_client_authorization_keys() {
        let options = AddOnionOptions {
            client_auth: vec!["not-a-public-key".to_string()],
            ..Default::default()
        };
        assert!(build_add_onion_command("NEW:ED25519-V3", 80, "127.0.0.1:8080", &options).is_err());
    }

    #[test]
    fn parses_bootstrap_events() {
        let line = r#"650 STATUS_CLIENT NOTICE BOOTSTRAP PROGRESS=50 TAG=loading_descriptors SUMMARY="Loading relay descriptors""#;
        assert_eq!(
            parse_bootstrap_line(line),
            Some((50, "Loading relay descriptors".to_string()))
        );
    }

    #[test]
    fn parses_escaped_bootstrap_summary() {
        let line = r#"250-status/bootstrap-phase=NOTICE BOOTSTRAP PROGRESS=100 TAG=done SUMMARY="Connected to a \"bridge\"""#;
        assert_eq!(
            parse_bootstrap_line(line),
            Some((100, "Connected to a \"bridge\"".to_string()))
        );
    }

    #[test]
    fn parses_control_port_file() {
        let path = std::env::temp_dir().join(format!(
            "onionhoster-control-port-{}-{}",
            std::process::id(),
            now_ms()
        ));
        fs::write(&path, "PORT=127.0.0.1:9051\n").expect("write control port");
        assert_eq!(read_control_port(&path), Some(9051));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn writes_bridge_configuration() {
        let paths = temp_paths();
        let config = TorConfig {
            bridge_lines: vec![
                "obfs4 192.0.2.1:443 FINGERPRINT cert=abc iat-mode=0".to_string(),
                "Bridge snowflake 192.0.2.2:80 FINGERPRINT".to_string(),
                "# ignored comment".to_string(),
            ],
        };

        paths.write_torrc(&config).expect("write torrc");
        let torrc = fs::read_to_string(&paths.torrc).expect("read torrc");

        assert!(torrc.contains("UseBridges 1"));
        assert!(torrc.contains("ClientTransportPlugin meek_lite,obfs4,webtunnel"));
        assert!(torrc.contains("ClientTransportPlugin snowflake"));
        assert!(torrc.contains("Bridge obfs4 192.0.2.1:443"));
        assert!(torrc.contains("Bridge snowflake 192.0.2.2:80"));
        assert!(!torrc.contains("# ignored comment"));

        let _ = fs::remove_dir_all(paths.data_dir.parent().expect("test root"));
    }

    #[test]
    fn writes_direct_configuration() {
        let paths = temp_paths();
        paths
            .write_torrc(&TorConfig::default())
            .expect("write torrc");
        let torrc = fs::read_to_string(&paths.torrc).expect("read torrc");
        assert!(torrc.contains("UseBridges 0"));
        assert!(!torrc.contains("ClientTransportPlugin"));
        let _ = fs::remove_dir_all(paths.data_dir.parent().expect("test root"));
    }

    #[test]
    fn rejects_unknown_bridge_transport() {
        let paths = temp_paths();
        let error = paths
            .write_torrc(&TorConfig {
                bridge_lines: vec!["magic 192.0.2.1:443".to_string()],
            })
            .expect_err("unknown transport must fail");
        assert!(error.contains("Unsupported bridge transport"));
        let _ = fs::remove_dir_all(paths.data_dir.parent().expect("test root"));
    }
}
