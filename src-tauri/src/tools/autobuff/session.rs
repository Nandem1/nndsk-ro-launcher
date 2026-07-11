use super::AutobuffHandle;
use crate::models::server::ServerConfig;
use crate::tools::autopot::{load_profiles, resolve_profile};
use crate::tools::input::{ensure_ydotoold, InputGateway, YdotoolDaemon};
use crate::utils::{effective_prefix, emit_tool_log_opt};
use ro_tools_linux::resolve_best_game_pid;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::time::{sleep, Duration};

pub async fn start_session(
    app: AppHandle,
    handle: &AutobuffHandle,
    input: InputGateway,
    ydotoold: Arc<YdotoolDaemon>,
    launcher_pid: u32,
    server: ServerConfig,
) -> Result<(), String> {
    let profile = resolve_profile(&load_profiles(), &server.executable_path, &server.autopot);
    let prefix = effective_prefix(server.wine_prefix.clone());
    for attempt in 1..=20 {
        if let Some((pid, detail)) =
            resolve_best_game_pid(launcher_pid, &server.executable_path, &prefix, &profile)
        {
            emit_tool_log_opt(
                Some(&app),
                format!("[AutoBuff] PID seleccionado: {pid} ({detail})"),
            );
            ensure_ydotoold(Some(&app), ydotoold.as_ref()).await?;
            return handle
                .start(app, pid, server.autobuff, profile, input, ydotoold)
                .await;
        }
        emit_tool_log_opt(
            Some(&app),
            format!("[AutoBuff] PID no encontrado (intento {attempt}/20)..."),
        );
        sleep(Duration::from_millis(500)).await;
    }
    Err("No se pudo resolver el PID del cliente RO. ¿Está abierto y logueado?".into())
}
