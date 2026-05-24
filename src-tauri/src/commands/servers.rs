use crate::models::server::ServerConfig;

fn servers_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::PathBuf::from(format!(
        "{}/.local/share/ro-launcher/servers.json",
        home
    ))
}

fn default_servers() -> Vec<ServerConfig> {
    vec![ServerConfig {
        id: "osro-midrate".to_string(),
        name: "OsRO Midrate".to_string(),
        executable_path: "/home/nndsk/Downloads/OsRO MR Full v4.3/OsRO MR Full v4.3/OldschoolRO [MR]/OsRO Midrate.exe".to_string(),
        patcher_path: None,
        wine_prefix: None,
        runner: None,
    }]
}

#[tauri::command]
pub async fn list_servers() -> Result<Vec<ServerConfig>, String> {
    let path = servers_path();
    if !path.exists() {
        let defaults = default_servers();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let json = serde_json::to_string_pretty(&defaults).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).ok();
        return Ok(defaults);
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_servers(servers: Vec<ServerConfig>) -> Result<(), String> {
    let path = servers_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&servers).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}
