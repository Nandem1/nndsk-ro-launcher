use tauri::AppHandle;

use crate::models::server::ServerConfig;
use crate::models::server_tools::{
    InstallDgVoodooResult, ServerToolsStatus, UninstallDgVoodooResult,
};
use crate::models::tool_kind::ToolKind;
use crate::tools::server_tools;

#[tauri::command]
pub async fn scan_server_tools(
    app: AppHandle,
    server: ServerConfig,
) -> Result<ServerToolsStatus, String> {
    server_tools::scan_status(&app, &server)
}

#[tauri::command]
pub async fn install_dgvoodoo(
    app: AppHandle,
    server: ServerConfig,
) -> Result<InstallDgVoodooResult, String> {
    server_tools::install_dgvoodoo(&app, &server).await
}

#[tauri::command]
pub async fn uninstall_dgvoodoo(
    app: AppHandle,
    server: ServerConfig,
) -> Result<UninstallDgVoodooResult, String> {
    server_tools::uninstall_dgvoodoo(&app, &server).await
}

#[tauri::command]
pub async fn launch_server_tool(
    app: AppHandle,
    server: ServerConfig,
    tool: ToolKind,
) -> Result<(), String> {
    server_tools::launch_tool(&app, &server, tool).await
}
