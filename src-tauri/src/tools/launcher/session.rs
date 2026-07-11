use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use crate::models::server::ServerConfig;
use crate::state::GameState;
use crate::tools::autobuff::AutobuffHandle;
use crate::tools::autopot::AutopotHandle;
use crate::tools::spammer::SpammerHandle;
use crate::utils::audio;
use crate::utils::gecko::install_gecko;
use crate::utils::process::drain_game_output;
use crate::utils::{
    apply_game_env, emit_tool_log_opt, is_prefix_configured, pipe_output, resolve_wine_context,
    wine_command, work_dir_from_exe, ExitEvent, EVENT_GAME_EXIT,
};

pub async fn launch_game(
    app: AppHandle,
    pid_slot: &Arc<std::sync::Mutex<Option<u32>>>,
    autopot: &AutopotHandle,
    autobuff: &AutobuffHandle,
    spammer: &SpammerHandle,
    server: ServerConfig,
) -> Result<(), String> {
    let ctx = resolve_wine_context(server.wine_prefix.clone(), server.runner.clone()).await?;

    if !is_prefix_configured(&ctx.prefix) {
        return Err("El WINEPREFIX no está configurado. Ejecuta el setup primero.".to_string());
    }

    install_gecko(&app, &ctx.prefix, &ctx.resolved.wine_bin).await?;
    audio::ensure_audio_driver(Some(&app), &ctx.prefix, &ctx.resolved).await?;

    let exe_path = server.executable_path.clone();
    let work_dir = work_dir_from_exe(&exe_path);

    let mut cmd = wine_command(
        &ctx.resolved.wine_bin,
        ctx.resolved.ld_library_path.as_deref(),
        &exe_path,
        &ctx.prefix,
        &work_dir,
        apply_game_env,
    );
    pipe_output(&mut cmd);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Error al lanzar el juego: {e}"))?;

    let launcher_pid = child.id();
    if let Some(pid) = launcher_pid {
        *pid_slot.lock().unwrap() = Some(pid);
        emit_tool_log_opt(
            Some(&app),
            format!("[Launch] Wine PID={pid} prefix={}", ctx.prefix),
        );
    }

    let pid_state = Arc::clone(pid_slot);
    let autopot = autopot.clone();
    let autobuff = autobuff.clone();
    let spammer = spammer.clone();
    let app_for_exit = app.clone();
    tokio::spawn(async move {
        drain_game_output(&app_for_exit, &mut child).await;
        autopot.stop().await;
        autobuff.stop().await;
        spammer.stop().await;
        emit_tool_log_opt(
            Some(&app_for_exit),
            "[Launch] Juego terminado, AutoPot, AutoBuff y Spammer detenidos",
        );
        let code = child
            .wait()
            .await
            .map(|s| s.code().unwrap_or(-1))
            .unwrap_or(-1);
        *pid_state.lock().unwrap() = None;
        let _ = app_for_exit.emit(EVENT_GAME_EXIT, ExitEvent { code });
    });

    Ok(())
}

pub async fn stop_game(state: &GameState) -> Result<(), String> {
    state.autopot.stop().await;
    state.autobuff.stop().await;
    state.spammer.stop().await;
    let pid = state.pid.lock().unwrap().take();
    if let Some(pid) = pid {
        let _ = tokio::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .await;
    }
    Ok(())
}
