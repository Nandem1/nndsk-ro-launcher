use tauri::State;

use crate::models::server::ServerConfig;
use crate::state::{ServerRepository, StorageNotices};

#[tauri::command]
pub fn list_servers(
    repository: State<'_, ServerRepository>,
    notices: State<'_, StorageNotices>,
) -> Result<Vec<ServerConfig>, String> {
    repository.list(&notices)
}

#[tauri::command]
pub fn save_servers(
    repository: State<'_, ServerRepository>,
    servers: Vec<ServerConfig>,
) -> Result<(), String> {
    repository.save(&servers)
}
