use tauri::State;

use crate::models::server::ServerConfig;
use crate::state::{GameState, ServerRepository, StorageNotices};
use crate::tools::presence::overrides_from_autopot;

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
    state: State<'_, GameState>,
    servers: Vec<ServerConfig>,
) -> Result<(), String> {
    repository.save(&servers)?;
    for server in &servers {
        state
            .presence
            .apply_overrides(&server.id, overrides_from_autopot(&server.autopot));
    }
    Ok(())
}
