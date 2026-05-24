mod commands;
mod models;

use std::sync::{Arc, Mutex};

use commands::{
    check::check_dependencies,
    launch::{launch_game, stop_game},
    runners::list_runners,
    servers::{list_servers, save_servers},
    server_tools::{install_dgvoodoo, launch_server_tool, scan_server_tools, uninstall_dgvoodoo},
    settings::{load_settings, save_settings},
    setup::setup_prefix,
    setup::reset_prefix,
};

pub struct GameState {
    pub pid: Arc<Mutex<Option<u32>>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(GameState { pid: Arc::new(Mutex::new(None)) })
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
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar la aplicación");
}
