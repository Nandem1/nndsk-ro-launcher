use std::collections::HashSet;
use std::time::Duration;

use ro_tools_linux::{
    capture_process_identity, find_game_processes, verify_process_identity, ProcessIdentity,
};
use tauri::{AppHandle, Emitter};
use tokio::process::Child;
use tokio::time::{sleep, Instant};

use crate::models::game_client::GameClientSnapshot;
use crate::models::launch::{LaunchStrategy, LaunchValues};
use crate::models::server::ServerConfig;
use crate::state::{GameProcessHandle, GameState, LaunchReservation};
use crate::tools::autobuff::AutobuffHandle;
use crate::tools::autopot::AutopotHandle;
use crate::tools::input::InputGateway;
use crate::tools::runners::ensure_managed_runtime;
use crate::tools::server_tools;
use crate::tools::spammer::SpammerHandle;
use crate::utils::audio;
use crate::utils::gecko::install_gecko_for_runner;
use crate::utils::process::drain_game_streams_redacted;
use crate::utils::{
    apply_game_env, emit_tool_log_opt, pipe_output, required_game_dir,
    resolve_server_wine_context_with_runner, validate_runtime_prefix, work_dir_from_exe, ExitEvent,
    OperationGuard, EVENT_GAME_EXIT,
};

const DIRECT_LAUNCH_TIMEOUT: Duration = Duration::from_secs(30);
const PATCHER_LAUNCH_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const CONTROLLER_EXIT_GRACE: Duration = Duration::from_secs(30);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(250);
const PROCESS_HANDOFF_GRACE: Duration = Duration::from_secs(5);

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
    runner: Option<String>,
    launch_values: LaunchValues,
) -> Result<GameClientSnapshot, String> {
    let LaunchTools {
        autopot,
        autobuff,
        spammer,
        input,
    } = tools;
    ensure_managed_runtime(&app).await?;
    let ctx = resolve_server_wine_context_with_runner(Some(&server), runner).await?;
    let prefix_operation =
        OperationGuard::acquire_shared("prefix", std::path::Path::new(&ctx.prefix))?;
    validate_runtime_prefix(&ctx)?;
    let missing_components =
        server_tools::missing_runtime_components(&server, std::path::Path::new(&ctx.prefix));
    if !missing_components.is_empty() {
        return Err(format!(
            "El entorno requiere reparación: {}",
            missing_components.join(" · ")
        ));
    }

    let rendered_args = server.launch.render_args(&launch_values)?;
    let redaction_values = launch_values.redaction_values();
    let (launch_exe, is_patcher) = match server.launch.strategy {
        LaunchStrategy::Direct => (server.executable_path.clone(), false),
        LaunchStrategy::Patcher => (
            server
                .patcher_path
                .clone()
                .ok_or_else(|| "No se configuró el patcher".to_string())?,
            true,
        ),
    };
    let work_dir = work_dir_from_exe(&launch_exe);
    let game_exe = server.executable_path.clone();
    let baseline: HashSet<ProcessIdentity> = find_game_processes(0, &game_exe, &ctx.prefix)
        .into_iter()
        .filter_map(|candidate| capture_process_identity(candidate.pid))
        .collect();

    let devices = input
        .prepare()
        .await
        .map_err(|error| format!("No se pudo preparar input uinput: {error}"))?;
    emit_tool_log_opt(
        Some(&app),
        format!("[Launch] uinput preparado antes del runner: {devices}"),
    );

    install_gecko_for_runner(&app, &ctx.prefix, &ctx.resolved).await?;
    audio::ensure_audio_driver(Some(&app), &ctx.prefix, &ctx.resolved).await?;

    let game_dir = required_game_dir(&server.executable_path)?;
    let dgvoodoo_operation =
        OperationGuard::acquire_shared("dgvoodoo", std::path::Path::new(&game_dir))?;
    let use_dgvoodoo = server_tools::scan_status(&app, &server)
        .map(|status| status.dgvoodoo.configured)
        .unwrap_or(false);
    if game.stop_requested(reservation) {
        return Err("El lanzamiento fue cancelado por el usuario".to_string());
    }
    let mut cmd =
        ctx.resolved
            .game_command(&ctx.prefix, &launch_exe, rendered_args.iter(), &work_dir);
    apply_game_env(&mut cmd, use_dgvoodoo);
    pipe_output(&mut cmd);

    let mut child = cmd
        .spawn()
        .map_err(|error| format!("Error al iniciar el runner: {error}"))?;
    let controller_pid = child
        .id()
        .ok_or_else(|| "El runner no informó su PID".to_string())?;
    let Some(controller_identity) = capture_process_identity(controller_pid) else {
        let _ = child.kill().await;
        return Err("El proceso controlador terminó antes de poder identificarlo".to_string());
    };
    if let Err(error) = game.mark_controller(reservation, controller_identity) {
        let _ = child.kill().await;
        return Err(error);
    }

    emit_tool_log_opt(
        Some(&app),
        format!(
            "[Launch] controller={controller_pid} runner={} prefix={}",
            ctx.resolved.kind_label(),
            ctx.prefix
        ),
    );
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let output_task = tokio::spawn(drain_game_streams_redacted(
        app.clone(),
        stdout,
        stderr,
        redaction_values,
    ));

    let timeout = if is_patcher {
        PATCHER_LAUNCH_TIMEOUT
    } else {
        DIRECT_LAUNCH_TIMEOUT
    };
    let search = ProcessSearch {
        controller_pid,
        exe_path: &game_exe,
        prefix: &ctx.prefix,
        baseline: &baseline,
        exclude_controller: ctx.resolved.is_proton(),
        game: &game,
        reservation,
    };
    let identity = match wait_for_game_process(&mut child, &search, timeout).await {
        Ok(identity) => identity,
        Err(error) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            let _ = output_task.await;
            return Err(error);
        }
    };

    let snapshot = match game.mark_running(reservation, identity) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            let _ = output_task.await;
            return Err(error);
        }
    };
    emit_tool_log_opt(
        Some(&app),
        format!(
            "[Launch] cliente detectado PID={} exe={} prefix={}",
            identity.pid, game_exe, ctx.prefix
        ),
    );

    let autopot = autopot.clone();
    let autobuff = autobuff.clone();
    let spammer = spammer.clone();
    let app_for_exit = app.clone();
    tokio::spawn(async move {
        let _prefix_operation = prefix_operation;
        let _dgvoodoo_operation = dgvoodoo_operation;
        let mut active_identity = identity;
        let mut seen = baseline;
        seen.insert(active_identity);
        loop {
            while verify_process_identity(&active_identity) {
                sleep(Duration::from_millis(500)).await;
            }
            if game.stop_requested(reservation) {
                break;
            }

            let Some(replacement) = wait_for_process_handoff(
                controller_pid,
                &game_exe,
                &ctx.prefix,
                &seen,
                PROCESS_HANDOFF_GRACE,
                &game,
                reservation,
            )
            .await
            else {
                break;
            };
            if !game.replace_running(reservation, active_identity, replacement) {
                break;
            }
            emit_tool_log_opt(
                Some(&app_for_exit),
                format!(
                    "[Launch] handoff del cliente PID={} -> PID={}",
                    active_identity.pid, replacement.pid
                ),
            );
            seen.insert(replacement);
            active_identity = replacement;
        }

        if child.try_wait().ok().flatten().is_none() {
            let _ = child.kill().await;
        }
        let code = child
            .wait()
            .await
            .map(|status| status.code().unwrap_or(-1))
            .unwrap_or(-1);
        let _ = output_task.await;

        if let Some(finished) = game.finish(reservation) {
            if finished.remaining_clients == 0 {
                let stops = tokio::join!(autopot.stop(), autobuff.stop(), spammer.stop());
                for error in [stops.0.err(), stops.1.err(), stops.2.err()]
                    .into_iter()
                    .flatten()
                {
                    emit_tool_log_opt(Some(&app_for_exit), format!("[Launch] Cleanup: {error}"));
                }
                emit_tool_log_opt(
                    Some(&app_for_exit),
                    "[Launch] Último cliente terminado; herramientas de combate detenidas",
                );
            }
            let _ = app_for_exit.emit(
                EVENT_GAME_EXIT,
                ExitEvent {
                    client_id: finished.client_id,
                    server_id: finished.server_id,
                    server_name: finished.server_name,
                    code,
                    requested: finished.stop_requested,
                },
            );
        }
    });

    Ok(snapshot)
}

struct ProcessSearch<'a> {
    controller_pid: u32,
    exe_path: &'a str,
    prefix: &'a str,
    baseline: &'a HashSet<ProcessIdentity>,
    exclude_controller: bool,
    game: &'a GameProcessHandle,
    reservation: LaunchReservation,
}

async fn wait_for_game_process(
    child: &mut Child,
    search: &ProcessSearch<'_>,
    timeout: Duration,
) -> Result<ProcessIdentity, String> {
    let mut deadline = Instant::now() + timeout;
    let mut controller_exit: Option<i32> = None;
    loop {
        if search.game.stop_requested(search.reservation) {
            return Err("El lanzamiento fue cancelado por el usuario".to_string());
        }
        for candidate in find_game_processes(search.controller_pid, search.exe_path, search.prefix)
        {
            if search.exclude_controller && candidate.pid == search.controller_pid {
                continue;
            }
            if let Some(identity) = capture_process_identity(candidate.pid) {
                if search.baseline.contains(&identity) {
                    continue;
                }
                return Ok(identity);
            }
        }

        if controller_exit.is_none() {
            if let Some(status) = child
                .try_wait()
                .map_err(|error| format!("No se pudo consultar el runner: {error}"))?
            {
                controller_exit = Some(status.code().unwrap_or(-1));
                deadline = deadline.min(Instant::now() + CONTROLLER_EXIT_GRACE);
            }
        }
        if Instant::now() >= deadline {
            let exit_detail = controller_exit
                .map(|code| format!("; el controlador ya había terminado con código {code}"))
                .unwrap_or_default();
            return Err(format!(
                "No apareció el proceso {} dentro de {} segundos{}",
                search.exe_path,
                timeout.as_secs(),
                exit_detail
            ));
        }
        sleep(PROCESS_POLL_INTERVAL).await;
    }
}

async fn wait_for_process_handoff(
    controller_pid: u32,
    exe_path: &str,
    prefix: &str,
    seen: &HashSet<ProcessIdentity>,
    grace: Duration,
    game: &GameProcessHandle,
    reservation: LaunchReservation,
) -> Option<ProcessIdentity> {
    let deadline = Instant::now() + grace;
    loop {
        if game.stop_requested(reservation) {
            return None;
        }
        for candidate in find_game_processes(controller_pid, exe_path, prefix) {
            let Some(identity) = capture_process_identity(candidate.pid) else {
                continue;
            };
            if !seen.contains(&identity)
                && game.candidate_available_for_handoff(reservation, identity)
            {
                return Some(identity);
            }
        }
        if Instant::now() >= deadline {
            return None;
        }
        sleep(PROCESS_POLL_INTERVAL).await;
    }
}

pub async fn stop_game(state: &GameState, client_id: &str) -> Result<(), String> {
    let request = state.game.request_stop(client_id)?;
    let tool_errors = if request.was_only_client {
        stop_combat_tools(state).await
    } else {
        Vec::new()
    };
    let process_errors = terminate_processes(request.identities).await;
    combine_stop_errors(process_errors, tool_errors)
}

pub async fn stop_all_games(state: &GameState) -> Result<(), String> {
    let tool_errors = stop_combat_tools(state).await;
    let identities = state.game.request_stop_all()?;
    let process_errors = terminate_processes(identities).await;
    combine_stop_errors(process_errors, tool_errors)
}

pub async fn stop_tools_for_additional_client(state: &GameState) -> Result<(), String> {
    combine_stop_errors(Vec::new(), stop_combat_tools(state).await)
}

async fn stop_combat_tools(state: &GameState) -> Vec<String> {
    let stops = tokio::join!(
        state.autopot.stop(),
        state.autobuff.stop(),
        state.spammer.stop()
    );
    [stops.0.err(), stops.1.err(), stops.2.err()]
        .into_iter()
        .flatten()
        .collect()
}

async fn terminate_processes(identities: Vec<ProcessIdentity>) -> Vec<String> {
    let mut process_errors = Vec::new();
    for identity in identities {
        if verify_process_identity(&identity) {
            let status = match tokio::process::Command::new("kill")
                .args(["-TERM", &identity.pid.to_string()])
                .status()
                .await
            {
                Ok(status) => status,
                Err(error) => {
                    process_errors.push(format!("No se pudo enviar TERM al proceso: {error}"));
                    continue;
                }
            };
            if !status.success() {
                process_errors.push(format!(
                    "No se pudo detener el proceso (kill terminó con {status})"
                ));
            }
        }
    }
    process_errors
}

fn combine_stop_errors(
    process_errors: Vec<String>,
    tool_errors: Vec<String>,
) -> Result<(), String> {
    if !process_errors.is_empty() {
        return Err(process_errors.join("; "));
    }
    if !tool_errors.is_empty() {
        return Err(tool_errors.join("; "));
    }
    Ok(())
}
