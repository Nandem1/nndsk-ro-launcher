use tauri::{AppHandle, Emitter};

use crate::models::server::ServerConfig;
use crate::state::{GameProcessHandle, GameState, LaunchReservation};
use crate::tools::autobuff::AutobuffHandle;
use crate::tools::autopot::AutopotHandle;
use crate::tools::input::InputGateway;
use crate::tools::spammer::SpammerHandle;
use crate::utils::audio;
use crate::utils::gecko::install_gecko;
use crate::utils::process::drain_game_output;
use crate::utils::{
    apply_game_env, emit_tool_log_opt, is_prefix_configured, pipe_output, resolve_wine_context,
    wine_command, work_dir_from_exe, ExitEvent, EVENT_GAME_EXIT,
};

pub struct LaunchTools<'a> {
    pub autopot: &'a AutopotHandle,
    pub autobuff: &'a AutobuffHandle,
    pub spammer: &'a SpammerHandle,
    pub input: &'a InputGateway,
}

pub async fn launch_game(
    app: AppHandle,
    game: GameProcessHandle,
    reservation: LaunchReservation,
    tools: LaunchTools<'_>,
    server: ServerConfig,
) -> Result<(), String> {
    let LaunchTools {
        autopot,
        autobuff,
        spammer,
        input,
    } = tools;
    let ctx = resolve_wine_context(server.wine_prefix.clone(), server.runner.clone()).await?;

    if !is_prefix_configured(&ctx.prefix) {
        return Err("El WINEPREFIX no está configurado. Ejecuta el setup primero.".to_string());
    }

    let devices = input
        .prepare()
        .await
        .map_err(|error| format!("No se pudo preparar input uinput: {error}"))?;
    emit_tool_log_opt(
        Some(&app),
        format!("[Launch] uinput preparado antes de Wine: {devices}"),
    );

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

    let launcher_pid = child
        .id()
        .ok_or_else(|| "Wine no informó el PID del proceso".to_string())?;
    if let Err(error) = game.mark_running(reservation, launcher_pid) {
        let _ = child.kill().await;
        return Err(error);
    }
    emit_tool_log_opt(
        Some(&app),
        format!("[Launch] Wine PID={launcher_pid} prefix={}", ctx.prefix),
    );

    let autopot = autopot.clone();
    let autobuff = autobuff.clone();
    let spammer = spammer.clone();
    let app_for_exit = app.clone();
    tokio::spawn(async move {
        drain_game_output(&app_for_exit, &mut child).await;
        let stops = tokio::join!(autopot.stop(), autobuff.stop(), spammer.stop());
        for error in [stops.0.err(), stops.1.err(), stops.2.err()]
            .into_iter()
            .flatten()
        {
            emit_tool_log_opt(Some(&app_for_exit), format!("[Launch] Cleanup: {error}"));
        }
        emit_tool_log_opt(
            Some(&app_for_exit),
            "[Launch] Juego terminado, AutoPot, AutoBuff y Spammer detenidos",
        );
        let code = child
            .wait()
            .await
            .map(|s| s.code().unwrap_or(-1))
            .unwrap_or(-1);
        if game.finish(reservation) {
            let _ = app_for_exit.emit(EVENT_GAME_EXIT, ExitEvent { code });
        }
    });

    Ok(())
}

pub async fn stop_game(state: &GameState) -> Result<(), String> {
    let stops = tokio::join!(
        state.autopot.stop(),
        state.autobuff.stop(),
        state.spammer.stop()
    );
    let tool_errors: Vec<_> = [stops.0.err(), stops.1.err(), stops.2.err()]
        .into_iter()
        .flatten()
        .collect();
    if let Some(pid) = state.game.running_pid()? {
        let status = tokio::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .await
            .map_err(|error| format!("No se pudo enviar TERM al juego: {error}"))?;
        if !status.success() {
            return Err(format!(
                "No se pudo detener el juego (kill terminó con {status})"
            ));
        }
    }
    if !tool_errors.is_empty() {
        return Err(tool_errors.join("; "));
    }
    Ok(())
}
