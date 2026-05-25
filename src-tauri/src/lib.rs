mod commands;
mod models;
mod state;
mod tools;
mod utils;

use std::sync::{Arc, Mutex};

use tauri::{Manager, RunEvent};

use commands::{
    autopot::{
        get_autopot_status, list_client_profiles, start_autopot, stop_autopot,
        update_autopot_config,
    },
    deps::check_dependencies,
    launcher::{launch_game, stop_game},
    prefix::{reset_prefix, setup_prefix},
    runners::list_runners,
    servers::{list_servers, save_servers},
    server_tools::{install_dgvoodoo, launch_server_tool, scan_server_tools, uninstall_dgvoodoo},
    settings::{load_settings, save_settings},
};
use state::GameState;
use tools::{autopot::AutopotHandle, input::YdotoolDaemon};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(GameState {
            pid: Arc::new(Mutex::new(None)),
            autopot: AutopotHandle::new(),
            ydotoold: Arc::new(YdotoolDaemon::new()),
        })
        .invoke_handler(tauri::generate_handler![
            check_dependencies,
            launch_game,
            stop_game,
            list_runners,
            list_servers,
            load_settings,
            save_servers,
            scan_server_tools,
            install_dgvoodoo,
            uninstall_dgvoodoo,
            launch_server_tool,
            save_settings,
            setup_prefix,
            reset_prefix,
            start_autopot,
            stop_autopot,
            update_autopot_config,
            get_autopot_status,
            list_client_profiles,
        ])
        .build(tauri::generate_context!())
        .expect("error al iniciar la aplicación")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                if let Some(state) = app.try_state::<GameState>() {
                    tauri::async_runtime::block_on(state.ydotoold.shutdown());
                }
            }
        });
}
