mod default_page;
mod proxy;
mod services;
mod static_server;
mod stats;
mod tor;
mod tunnels;

use services::ServiceManager;
use std::sync::Arc;
use tauri::{Listener, Manager};
use tor::TorManager;
use tunnels::TunnelManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let manager = Arc::new(TorManager::new());
    let shutdown_manager = manager.clone();
    let listener_manager = manager.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            if let Some(window) = app.get_webview_window("main")
                && let Some(icon) = app.default_window_icon()
            {
                let _ = window.set_icon(icon.clone());
            }

            let services =
                Arc::new(ServiceManager::new(app.handle()).map_err(std::io::Error::other)?);
            app.manage(services.clone());

            let tunnels =
                Arc::new(TunnelManager::new(app.handle()).map_err(std::io::Error::other)?);
            app.manage(tunnels.clone());

            let handle = app.handle().clone();
            let service_manager = services.clone();
            let tor_manager = listener_manager.clone();
            app.listen("tor-status", move |_event| {
                let handle = handle.clone();
                let services = service_manager.clone();
                let tor = tor_manager.clone();
                tauri::async_runtime::spawn(async move {
                    services.sync(&handle, &tor).await;
                });
            });

            let tunnel_handle = app.handle().clone();
            let tunnel_manager = tunnels.clone();
            app.listen("service-stopped", move |event| {
                let Ok(stopped) =
                    serde_json::from_str::<services::ServiceStopped>(event.payload())
                else {
                    return;
                };
                tunnel_manager.stop_for_service(&tunnel_handle, &stopped.id);
            });

            let deleted_handle = app.handle().clone();
            let deleted_manager = tunnels.clone();
            app.listen("service-deleted", move |event| {
                let Ok(deleted) =
                    serde_json::from_str::<services::ServiceDeleted>(event.payload())
                else {
                    return;
                };
                deleted_manager.delete_for_service(&deleted_handle, &deleted.id);
            });

            Ok(())
        })
        .manage(manager)
        .invoke_handler(tauri::generate_handler![
            tor::get_tor_status,
            tor::get_tor_logs,
            tor::start_tor,
            tor::stop_tor,
            services::get_services,
            services::create_service,
            services::update_service,
            services::delete_service,
            services::start_service,
            services::stop_service,
            services::restart_service,
            tunnels::get_tunnels,
            tunnels::get_tunnel_logs,
            tunnels::create_tunnel,
            tunnels::update_tunnel,
            tunnels::delete_tunnel,
            tunnels::start_tunnel,
            tunnels::stop_tunnel,
        ])
        .build(tauri::generate_context!())
        .expect("error while building OnionHoster")
        .run(move |app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app.state::<Arc<TunnelManager>>().shutdown_all();
                app.state::<Arc<ServiceManager>>().shutdown_all();
                let _ = shutdown_manager.stop(None);
            }
        });
}
