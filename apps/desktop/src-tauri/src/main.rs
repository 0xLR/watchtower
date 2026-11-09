#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod commands;
mod logging;
mod recovery;
mod state;
use commands::cameras::*;
use commands::discovery::*;
use tauri::Manager;
#[tauri::command]
fn app_info() -> watchtower_core::AppInfo {
    watchtower_core::AppInfo {
        version: env!("CARGO_PKG_VERSION").into(),
    }
}
fn main() {
    // Application logs contain fixed operational messages only. Never log IPC inputs,
    // network responses or source URLs. SQL statement logging and sidecar logs are off.
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_writer(|| logging::RedactingWriter::new(std::io::stderr()))
        .init();
    let application = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let state = tauri::async_runtime::block_on(state::AppState::new(app.handle()))
                .map_err(std::io::Error::other)?;
            recovery::start(state.clone(), app.handle().clone());
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            snapshot,
            test_camera,
            save_camera,
            playback,
            delete_camera,
            reorder_cameras,
            save_settings,
            network_interfaces,
            scan_cameras,
            player_state
        ])
        .build(tauri::generate_context!());
    match application {
        Ok(application) => application.run(|app, event| {
            if matches!(
                event,
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
            ) {
                if let Some(state) = app.try_state::<std::sync::Arc<state::AppState>>() {
                    if let Ok(media) = state.media() {
                        tauri::async_runtime::block_on(media.shutdown());
                    }
                }
            }
        }),
        Err(_) => {
            eprintln!("Watchtower could not start");
            std::process::exit(1);
        }
    }
}
