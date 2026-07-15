use tauri::State;

use crate::models::server::ServerConfig;
use crate::state::ServerRepository;

#[tauri::command]
pub fn list_servers(repository: State<'_, ServerRepository>) -> Result<Vec<ServerConfig>, String> {
    repository.list()
}

#[tauri::command]
pub fn save_servers(
    repository: State<'_, ServerRepository>,
    servers: Vec<ServerConfig>,
) -> Result<(), String> {
    repository.save(&servers)
}
