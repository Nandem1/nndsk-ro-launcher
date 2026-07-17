use super::AutobuffHandle;
use crate::models::server::ServerConfig;
use crate::tools::autopot::{load_profiles, resolve_profile};
use crate::tools::game_pid::resolve_game_pid_with_retry;
use crate::tools::input::InputGateway;
use crate::utils::{effective_prefix, emit_tool_log_opt};
use tauri::AppHandle;

pub async fn start_session(
    app: AppHandle,
    handle: &AutobuffHandle,
    input: InputGateway,
    launcher_pid: u32,
    server: ServerConfig,
) -> Result<(), String> {
    let profile = resolve_profile(&load_profiles(), &server.executable_path, &server.autopot);
    let prefix = effective_prefix(server.wine_prefix.clone());
    let (pid, detail) = resolve_game_pid_with_retry(
        &app,
        "AutoBuff",
        launcher_pid,
        &server.executable_path,
        &prefix,
        &profile,
    )
    .await?;
    emit_tool_log_opt(
        Some(&app),
        format!("[AutoBuff] PID seleccionado: {pid} ({detail})"),
    );
    if !input.is_prepared() {
        return Err("AutoBuff no puede iniciar: uinput no fue preparado antes de Wine".into());
    }
    handle
        .start(app, pid, server.autobuff, profile, input)
        .await
}
