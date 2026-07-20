use std::path::Path;

use tauri::AppHandle;

use crate::models::server::ServerConfig;
use crate::models::server_tools::{
    InstallDgVoodooResult, ServerToolsStatus, UninstallDgVoodooResult,
};
use crate::models::tool_kind::ToolKind;
use crate::tools::server_tools;
use crate::utils::{required_game_dir, OperationGuard};

#[tauri::command]
pub async fn scan_server_tools(
    app: AppHandle,
    server: ServerConfig,
) -> Result<ServerToolsStatus, String> {
    server.validate_executable_available()?;
    server_tools::scan_status(&app, &server)
}

#[tauri::command]
pub async fn install_dgvoodoo(
    app: AppHandle,
    server: ServerConfig,
) -> Result<InstallDgVoodooResult, String> {
    server.validate_executable_available()?;
    let game_dir = required_game_dir(&server.executable_path)?;
    let _operation = OperationGuard::acquire("dgvoodoo", Path::new(&game_dir))?;
    server_tools::install_dgvoodoo(&app, &server).await
}

#[tauri::command]
pub async fn uninstall_dgvoodoo(
    app: AppHandle,
    server: ServerConfig,
) -> Result<UninstallDgVoodooResult, String> {
    server.validate_executable_available()?;
    let game_dir = required_game_dir(&server.executable_path)?;
    let _operation = OperationGuard::acquire("dgvoodoo", Path::new(&game_dir))?;
    server_tools::uninstall_dgvoodoo(&app, &server).await
}

#[tauri::command]
pub async fn launch_server_tool(
    app: AppHandle,
    server: ServerConfig,
    tool: ToolKind,
    runner: Option<String>,
) -> Result<(), String> {
    server.validate_executable_available()?;
    server_tools::launch_tool(&app, &server, tool, runner).await
}
