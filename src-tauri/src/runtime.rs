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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallType {
    Appimage,
    Deb,
    Tarball,
    Android,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub appimage: bool,
    pub install_type: InstallType,
    pub ready: bool,
    pub components: Vec<RuntimeComponent>,
}

pub fn appimage_active() -> bool {
    // Primary: the AppImage runtime always sets this env var.
    if std::env::var_os("APPIMAGE").is_some() {
        return true;
    }
    // Fallback: check if the real binary path looks like an AppImage.
    // AppImage mounts the squashfs to /tmp/.mount_<name>XXXXXX and the
    // APPIMAGE env var points to the .AppImage file on disk.  We can also
    // check whether /proc/self/exe resolves to a path ending in .AppImage
    // or is inside a typical AppImage mount directory.
    if let Ok(exe) = std::env::current_exe() {
        let path_str = exe.to_string_lossy();
        if path_str.contains(".AppImage") || path_str.contains("/tmp/.mount_") {
            return true;
        }
    }
    false
}

/// Detect how OnionHoster was installed.
pub fn detect_install_type() -> InstallType {
    if cfg!(target_os = "android") {
        return InstallType::Android;
    }
    if appimage_active() {
        return InstallType::Appimage;
    }
    // Deb packages install the binary at /usr/bin/onionhoster.
    if std::env::current_exe().is_ok_and(|exe| exe.starts_with("/usr")) {
        return InstallType::Deb;
    }
    // Everything else (portable tarball, running from build dir, etc.)
    InstallType::Tarball
}

fn runtime_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|path| path.join("runtime"))
        .map_err(|error| format!("Could not resolve the runtime directory: {error}"))
}

#[cfg(not(target_os = "android"))]
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

#[cfg(not(target_os = "android"))]
fn valid_tor_dir(dir: &Path) -> bool {
    dir.join("tor").is_file() && dir.join("lyrebird").is_file()
}

#[cfg(not(target_os = "android"))]
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

#[cfg(not(target_os = "android"))]
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

#[cfg(not(target_os = "android"))]
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

#[cfg(not(target_os = "android"))]
fn find_copied_tor(app: &AppHandle) -> Option<PathBuf> {
    let root = runtime_root(app).ok()?.join("tor");
    let entries = fs::read_dir(root).ok()?;
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| valid_tor_dir(path))
        .max()
}

#[cfg(not(target_os = "android"))]
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

#[cfg(not(target_os = "android"))]
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

#[cfg(target_os = "android")]
pub fn ensure_tor_runtime(app: &AppHandle) -> Result<PathBuf, String> {
    let paths = crate::mobile::runtime_paths(app)?;
    if paths.native_dir.join("libTor.so").is_file()
        && paths.native_dir.join("liblyrebird.so").is_file()
    {
        Ok(paths.native_dir)
    } else {
        Err("The bundled Android Tor runtime was not found. Reinstall OnionHoster.".to_string())
    }
}

#[cfg(not(target_os = "android"))]
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

#[cfg(target_os = "android")]
pub fn ensure_cloudflared(app: &AppHandle) -> Result<PathBuf, String> {
    let path = crate::mobile::runtime_paths(app)?
        .native_dir
        .join("libcloudflared.so");
    if path.is_file() {
        Ok(path)
    } else {
        Err(
            "The bundled Android cloudflared runtime was not found. Reinstall OnionHoster."
                .to_string(),
        )
    }
}

#[cfg(not(target_os = "android"))]
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
    #[cfg(target_os = "android")]
    let (tor_version, cloudflared_version) = crate::mobile::runtime_paths(app)
        .map(|paths| {
            (
                read_version(&paths.data_dir.join("tor"), Some("tor")),
                read_version(&paths.data_dir.join("cloudflared"), Some("cloudflared")),
            )
        })
        .unwrap_or_default();
    #[cfg(not(target_os = "android"))]
    let tor_version = tor
        .as_ref()
        .ok()
        .and_then(|path| read_version(path, Some("tor")));
    #[cfg(not(target_os = "android"))]
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
        install_type: detect_install_type(),
        ready,
        components,
    }
}

#[tauri::command]
pub async fn pick_android_source(
    app: AppHandle,
    directory: bool,
) -> Result<Option<String>, String> {
    #[cfg(target_os = "android")]
    return crate::mobile::pick_source(&app, directory);
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, directory);
        Err("The Android file picker is only available on Android.".to_string())
    }
}

#[tauri::command(async)]
pub fn discard_android_source(app: AppHandle, path: String) -> Result<(), String> {
    #[cfg(target_os = "android")]
    return crate::mobile::discard_source(&app, &path);
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, path);
        Ok(())
    }
}

#[tauri::command]
pub async fn get_android_update(app: AppHandle) -> Result<serde_json::Value, String> {
    #[cfg(target_os = "android")]
    return crate::mobile::check_update(&app);
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(serde_json::Value::Null)
    }
}

#[tauri::command]
pub async fn open_android_release(app: AppHandle, url: String) -> Result<(), String> {
    if !url.starts_with("https://github.com/uzairdeveloper223/OnionHoster/releases/download/v")
        || !url.ends_with("_android_arm64.apk")
        || url.contains(['?', '#'])
    {
        return Err("The Android release URL is invalid.".to_string());
    }
    #[cfg(target_os = "android")]
    return crate::mobile::open_release(&app, url);
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Err("APK updates are only available on Android.".to_string())
    }
}

#[tauri::command(async)]
pub fn get_runtime_status(app: AppHandle) -> RuntimeStatus {
    status_for(&app)
}

#[tauri::command(async)]
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
