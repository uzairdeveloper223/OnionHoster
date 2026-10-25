use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Manager};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeComponent {
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub appimage: bool,
    pub ready: bool,
    pub components: Vec<RuntimeComponent>,
}

pub fn appimage_active() -> bool {
    std::env::var_os("APPIMAGE").is_some()
}

fn runtime_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|path| path.join("runtime"))
        .map_err(|error| format!("Could not resolve the runtime directory: {error}"))
}

fn resource_dirs(app: &AppHandle, name: &str) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(resource_dir) = app.path().resource_dir() {
        dirs.push(resource_dir.join(name));
        dirs.push(resource_dir.join("resources").join(name));
    }
    dirs.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join(name),
    );
    dirs
}

fn valid_tor_dir(dir: &Path) -> bool {
    dir.join("tor").is_file() && dir.join("lyrebird").is_file()
}

fn tor_source(app: &AppHandle) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ONIONHOSTER_TOR_DIR") {
        let dir = PathBuf::from(path);
        if valid_tor_dir(&dir) {
            return Some(dir);
        }
    }
    resource_dirs(app, "tor")
        .into_iter()
        .find(|dir| valid_tor_dir(dir))
}

fn cloudflared_source(app: &AppHandle) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ONIONHOSTER_CLOUDFLARED") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    for dir in resource_dirs(app, "cloudflared") {
        let candidate = dir.join("cloudflared");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn read_version(dir: &Path, prefix: Option<&str>) -> Option<String> {
    let contents = fs::read_to_string(dir.join("VERSION")).ok()?;
    let first = contents.lines().next()?.trim();
    Some(match prefix {
        Some(prefix) => first
            .strip_prefix(prefix)
            .unwrap_or(first)
            .trim()
            .to_string(),
        None => first.to_string(),
    })
}

fn safe_version(value: &str) -> String {
    let value: String = value
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
        .collect();
    if value.is_empty() {
        "bundled".to_string()
    } else {
        value
    }
}

fn find_copied_tor(app: &AppHandle) -> Option<PathBuf> {
    let root = runtime_root(app).ok()?.join("tor");
    let entries = fs::read_dir(root).ok()?;
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| valid_tor_dir(path))
        .max()
}

fn copy_dir(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|error| {
        format!(
            "Could not create the runtime directory at {}: {error}",
            target.display()
        )
    })?;
    for entry in fs::read_dir(source)
        .map_err(|error| format!("Could not read the bundled runtime: {error}"))?
    {
        let entry =
            entry.map_err(|error| format!("Could not read the bundled runtime: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Could not inspect the bundled runtime: {error}"))?;
        let destination = target.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir(&entry.path(), &destination)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &destination).map_err(|error| {
                format!(
                    "Could not copy the bundled runtime to {}: {error}",
                    destination.display()
                )
            })?;
        }
    }
    Ok(())
}

fn copy_dir_atomic(source: &Path, target: &Path) -> Result<(), String> {
    if valid_tor_dir(target) {
        return Ok(());
    }

    let temporary = target.with_extension(format!("tmp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temporary);
    copy_dir(source, &temporary)?;
    if target.exists() {
        fs::remove_dir_all(target).map_err(|error| {
            format!(
                "Could not replace the runtime at {}: {error}",
                target.display()
            )
        })?;
    }
    fs::rename(&temporary, target).map_err(|error| {
        format!(
            "Could not install the runtime at {}: {error}",
            target.display()
        )
    })
}

pub fn ensure_tor_runtime(app: &AppHandle) -> Result<PathBuf, String> {
    if let Some(source) = tor_source(app) {
        if appimage_active() {
            let version = safe_version(
                &read_version(&source, Some("tor")).unwrap_or_else(|| "bundled".to_string()),
            );
            let target = runtime_root(app)?.join("tor").join(version);
            copy_dir_atomic(&source, &target)?;
            return Ok(target);
        }
        return Ok(source);
    }

    if let Some(copied) = find_copied_tor(app) {
        return Ok(copied);
    }

    Err("The bundled Tor runtime was not found. Reinstall OnionHoster or run `pnpm fetch:tor` in development."
        .to_string())
}

pub fn ensure_cloudflared(app: &AppHandle) -> Result<PathBuf, String> {
    if let Some(source) = cloudflared_source(app) {
        if appimage_active() {
            let target = runtime_root(app)?.join("cloudflared").join("cloudflared");
            if !target.is_file() {
                fs::create_dir_all(
                    target
                        .parent()
                        .ok_or_else(|| "The cloudflared runtime path is invalid.".to_string())?,
                )
                .map_err(|error| format!("Could not create the cloudflared runtime: {error}"))?;
                fs::copy(&source, &target).map_err(|error| {
                    format!("Could not copy cloudflared into the app runtime: {error}")
                })?;
                if let Some(source_dir) = source.parent() {
                    let version = source_dir.join("VERSION");
                    if version.is_file()
                        && let Some(target_dir) = target.parent()
                    {
                        let _ = fs::copy(version, target_dir.join("VERSION"));
                    }
                }
            }
            return Ok(target);
        }
        return Ok(source);
    }

    let copied = runtime_root(app)?.join("cloudflared").join("cloudflared");
    if copied.is_file() {
        return Ok(copied);
    }

    Err("The bundled cloudflared runtime was not found. Reinstall OnionHoster or run `pnpm fetch:cloudflared` in development."
        .to_string())
}

fn component(
    name: &str,
    path: Result<PathBuf, String>,
    version: Option<String>,
) -> RuntimeComponent {
    match path {
        Ok(path) => RuntimeComponent {
            name: name.to_string(),
            installed: true,
            version,
            path: Some(path.display().to_string()),
        },
        Err(_) => RuntimeComponent {
            name: name.to_string(),
            installed: false,
            version: None,
            path: None,
        },
    }
}

fn status_for(app: &AppHandle) -> RuntimeStatus {
    let tor = ensure_tor_runtime(app);
    let cloudflared = ensure_cloudflared(app);
    let tor_version = tor
        .as_ref()
        .ok()
        .and_then(|path| read_version(path, Some("tor")));
    let cloudflared_version = cloudflared
        .as_ref()
        .ok()
        .and_then(|path| read_version(path.parent().unwrap_or(path), Some("cloudflared")));

    let components = vec![
        component("Tor", tor, tor_version),
        component("cloudflared", cloudflared, cloudflared_version),
    ];
    let ready = components.iter().all(|component| component.installed);

    RuntimeStatus {
        appimage: appimage_active(),
        ready,
        components,
    }
}

#[tauri::command]
pub fn get_runtime_status(app: AppHandle) -> RuntimeStatus {
    status_for(&app)
}

#[tauri::command]
pub fn repair_runtime(app: AppHandle) -> Result<RuntimeStatus, String> {
    if appimage_active() {
        let root = runtime_root(&app)?;
        let _ = fs::remove_dir_all(root.join("tor"));
        let _ = fs::remove_file(root.join("cloudflared").join("cloudflared"));
    }

    let status = status_for(&app);
    if status.ready {
        Ok(status)
    } else {
        Err(
            "The runtime could not be repaired automatically. Reinstall OnionHoster to restore its bundled components."
                .to_string(),
        )
    }
}
