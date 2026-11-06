use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    os::fd::FromRawFd,
    path::PathBuf,
    sync::{Arc, OnceLock},
};
use tauri::{
    AppHandle, Manager,
    plugin::{Builder, PluginHandle, TauriPlugin},
};

static APP: OnceLock<AppHandle> = OnceLock::new();

pub struct Mobile {
    handle: PluginHandle<tauri::Wry>,
    paths: OnceLock<RuntimePaths>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePaths {
    pub native_dir: PathBuf,
    pub data_dir: PathBuf,
    pub sites_dir: PathBuf,
}

pub fn init() -> TauriPlugin<tauri::Wry> {
    Builder::new("hosting")
        .setup(|app, api| {
            let handle = api.register_android_plugin("dev.uzair.onionhoster", "HostingPlugin")?;
            app.manage(Mobile {
                handle,
                paths: OnceLock::new(),
            });
            Ok(())
        })
        .build()
}

pub fn set_app_handle(app: AppHandle) {
    let _ = APP.set(app);
}

pub fn runtime_paths(app: &AppHandle) -> Result<RuntimePaths, String> {
    let mobile = app.state::<Mobile>();
    if let Some(paths) = mobile.paths.get() {
        return Ok(paths.clone());
    }
    let paths: RuntimePaths = mobile
        .handle
        .run_mobile_plugin("runtimePaths", ())
        .map_err(|error| format!("Could not prepare the Android runtime: {error}"))?;
    let _ = mobile.paths.set(paths.clone());
    Ok(paths)
}

pub fn set_hosting(app: &AppHandle, active: bool) -> Result<(), String> {
    app.state::<Mobile>()
        .handle
        .run_mobile_plugin::<serde_json::Value>(
            "setHosting",
            serde_json::json!({ "active": active }),
        )
        .map(|_| ())
        .map_err(|error| format!("Could not update background hosting: {error}"))
}

pub fn pick_source(app: &AppHandle, directory: bool) -> Result<Option<String>, String> {
    #[derive(Deserialize)]
    struct Selected {
        path: Option<String>,
    }
    app.state::<Mobile>()
        .handle
        .run_mobile_plugin::<Selected>(
            "chooseSource",
            serde_json::json!({ "directory": directory }),
        )
        .map(|result| result.path)
        .map_err(|error| format!("Could not choose a site: {error}"))
}

pub fn discard_source(app: &AppHandle, path: &str) -> Result<(), String> {
    let paths = runtime_paths(app)?;
    let relative = std::path::Path::new(path)
        .strip_prefix(&paths.sites_dir)
        .map_err(|_| "The path is not an imported Android site.".to_string())?;
    let Some(std::path::Component::Normal(folder)) = relative.components().next() else {
        return Err("The imported site path is invalid.".to_string());
    };
    uuid::Uuid::parse_str(&folder.to_string_lossy())
        .map_err(|_| "The imported site identifier is invalid.".to_string())?;
    let target = paths.sites_dir.join(folder);
    // An import can be shared by services. Only remove copies that are no longer referenced.
    if app
        .state::<Arc<crate::services::ServiceManager>>()
        .backup_records()
        .iter()
        .any(|record| {
            matches!(&record.source, crate::services::ServiceSource::Static { root, .. }
            if std::path::Path::new(root).starts_with(&target))
        })
    {
        return Ok(());
    }
    if target.is_dir() {
        std::fs::remove_dir_all(target)
            .map_err(|error| format!("Could not remove the imported site: {error}"))?;
    }
    Ok(())
}

pub fn check_update(app: &AppHandle) -> Result<serde_json::Value, String> {
    app.state::<Mobile>()
        .handle
        .run_mobile_plugin("checkUpdate", ())
        .map_err(|error| format!("Could not check Android updates: {error}"))
}

pub fn open_document(app: &AppHandle, uri: &str, write: bool) -> Result<File, String> {
    #[derive(Deserialize)]
    struct Descriptor {
        fd: i32,
    }
    #[derive(Serialize)]
    struct Request<'a> {
        uri: &'a str,
        write: bool,
    }
    let result = app
        .state::<Mobile>()
        .handle
        .run_mobile_plugin::<Descriptor>("openDocument", Request { uri, write })
        .map_err(|error| format!("Could not open the backup document: {error}"))?;
    if result.fd < 0 {
        return Err("The document provider returned an invalid file descriptor.".to_string());
    }
    // The Kotlin side detaches this descriptor. This File now owns it and closes it on drop.
    Ok(unsafe { File::from_raw_fd(result.fd) })
}

pub fn open_release(app: &AppHandle, url: String) -> Result<(), String> {
    app.state::<Mobile>()
        .handle
        .run_mobile_plugin::<serde_json::Value>("openRelease", serde_json::json!({ "url": url }))
        .map(|_| ())
        .map_err(|error| format!("Could not open the Android release: {error}"))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_uzair_onionhoster_HostingService_stopHosting(
    _env: *mut std::ffi::c_void,
    _object: *mut std::ffi::c_void,
) {
    if let Some(app) = APP.get() {
        app.state::<Arc<crate::tunnels::TunnelManager>>()
            .shutdown_all();
        app.state::<Arc<crate::services::ServiceManager>>()
            .shutdown_all();
        let _ = app.state::<Arc<crate::tor::TorManager>>().stop(Some(app));
    }
}
