use crate::models::dependency::DependencyStatus;
use crate::models::server::ServerConfig;
use crate::tools::deps;
use tauri::AppHandle;

#[tauri::command]
pub async fn check_dependencies(
    app: AppHandle,
    server: Option<ServerConfig>,
    runner: Option<String>,
) -> Result<DependencyStatus, String> {
    if let Some(server) = &server {
        server.validate()?;
    }
    deps::check_dependencies(&app, server, runner).await
}
