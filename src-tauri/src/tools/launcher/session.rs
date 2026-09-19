use std::collections::HashSet;
use std::time::Duration;

use ro_tools_linux::{
    capture_process_identity, find_game_processes, read_ppid, signal_process_identity,
    verify_process_identity, ProcessIdentity,
};
use tauri::{AppHandle, Emitter};
use tokio::time::{sleep, Instant};

use crate::models::game_client::GameClientSnapshot;
use crate::models::launch::{LaunchStrategy, LaunchValues};
use crate::models::server::ServerConfig;
use crate::state::{GameProcessHandle, GameState, LaunchReservation};
use crate::tools::autobuff::AutobuffHandle;
use crate::tools::autopot::{load_profiles, resolve_profile, AutopotHandle};
use crate::tools::input::InputGateway;
use crate::tools::memory_sessions::{
    launcher_memory_ancestor, MemoryAncestor, MemorySessionRegistry,
};
use crate::tools::prefix::{DxvkProvision, MANAGED_DXVK_COMPONENT};
use crate::tools::presence::{overrides_from_autopot, PresenceHandle};
use crate::tools::runner_sessions::{
    path_log_token, prefix_log_token, ClientRuntimeGuard, RunnerOperation, RunnerSessionRegistry,
    SessionOwnership, SpawnedRunner,
};
use crate::tools::runners::ensure_managed_runtime;
use crate::tools::runtime::{
    apply_graphics_environment_to_invocation, classify_run_outcome, enqueue_persist_finished,
    enqueue_persist_started, enqueue_persist_unreached, new_observation_id, observe_legacy_runtime,
    operational_session_anchor, resolve_operational_plan_with_profile,
    runtime_graphics_plan_enabled, runtime_observe_enabled, runtime_shadow_enabled,
    DgVoodooObservation, DgVoodooState, InvocationPlan, InvocationTarget, LegacyRuntimeInput,
    ObservationFinishedPayload, ObservationStartedPayload, OperationalRuntimeInput, OutcomeInput,
    PlanAvailability, RunOutcome, RuntimePlan, RuntimeProfile, ShadowOperation,
    StartupFailureClass,
};
use crate::tools::server_tools;
use crate::tools::spammer::SpammerHandle;
use crate::utils::audio;
use crate::utils::gecko::install_gecko_for_runner;
use crate::utils::process::drain_game_streams_redacted;
use crate::utils::{
    emit_tool_log_opt, required_game_dir, resolve_server_wine_context_with_runner,
    validate_runtime_prefix, work_dir_from_exe, ExitEvent, OperationGuard, EVENT_GAME_CLIENT,
    EVENT_GAME_EXIT,
};

fn graphics_environment_error_message(
    error: crate::tools::runtime::GraphicsEnvironmentError,
) -> String {
    match error {
        crate::tools::runtime::GraphicsEnvironmentError::InvalidDllName => {
            "invalid-dll-name".to_string()
        }
        crate::tools::runtime::GraphicsEnvironmentError::OverrideConflict(conflict) => {
            format!("override-conflict:{}", conflict.dll)
        }
        crate::tools::runtime::GraphicsEnvironmentError::EnvironmentConflict(conflict) => {
            format!("environment-conflict:{}", conflict.key)
        }
    }
}

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
    pub presence: &'a PresenceHandle,
    pub sessions: &'a RunnerSessionRegistry,
    pub memory: &'a MemorySessionRegistry,
}

#[allow(clippy::too_many_arguments)]
pub async fn launch_game(
    app: AppHandle,
    game: GameProcessHandle,
    reservation: LaunchReservation,
    tools: LaunchTools<'_>,
    client_id: String,
    server: ServerConfig,
    runner: Option<String>,
    launch_values: LaunchValues,
) -> Result<GameClientSnapshot, String> {
    let LaunchTools {
        autopot,
        autobuff,
        spammer,
        input,
        presence,
        sessions,
        memory,
    } = tools;
    let default_runner = runner.clone();
    ensure_managed_runtime(&app).await?;
    let ctx = resolve_server_wine_context_with_runner(Some(&server), runner).await?;
    let prefix_health = validate_runtime_prefix(&ctx)?;
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

    let tools_status = server_tools::scan_status(&app, &server).ok();
    let dgvoodoo_configured = tools_status
        .as_ref()
        .is_some_and(|status| status.dgvoodoo.configured);
    let webview2_required = tools_status
        .as_ref()
        .is_some_and(|status| status.diagnostics.webview2_required);
    let game_dir = required_game_dir(&server.executable_path)?;
    let operational_input = OperationalRuntimeInput {
        server_runner: server.runner.as_deref(),
        default_runner: default_runner.as_deref(),
        context: &ctx,
        dgvoodoo: DgVoodooState::verified(dgvoodoo_configured),
        webview2_required,
    };
    let operational_profile_plan = if runtime_graphics_plan_enabled() || runtime_observe_enabled() {
        resolve_operational_plan_with_profile(operational_input).ok()
    } else {
        None
    };
    let anchor = operational_session_anchor(
        &ctx,
        operational_profile_plan.as_ref().map(|(_, plan)| plan),
        webview2_required,
    );
    let observation_id = if runtime_observe_enabled() {
        Some(new_observation_id())
    } else {
        None
    };
    let op = RunnerOperation::begin(Some(&app), sessions, &game, &ctx, &anchor).await?;
    let prefix_operation =
        OperationGuard::acquire_shared("prefix", std::path::Path::new(&ctx.prefix))?;
    let dgvoodoo_operation =
        OperationGuard::acquire_shared("dgvoodoo", std::path::Path::new(&game_dir))?;
    install_gecko_for_runner(&app, &op).await?;
    audio::ensure_audio_driver(Some(&app), &op).await?;
    let wine_7_16 = ctx.resolved.is_wine_7_16();
    let manifest_has_managed_dxvk = prefix_health.manifest.as_ref().is_some_and(|manifest| {
        manifest
            .components()
            .iter()
            .any(|component| component == MANAGED_DXVK_COMPONENT)
    });
    if runtime_shadow_enabled() {
        let dxvk = if ctx.resolved.is_proton() {
            DxvkProvision::Runner
        } else if wine_7_16 {
            DxvkProvision::Managed
        } else {
            DxvkProvision::Winetricks
        };
        observe_legacy_runtime(
            Some(&app),
            ShadowOperation::Launch,
            LegacyRuntimeInput {
                server_runner: server.runner.as_deref(),
                default_runner: default_runner.as_deref(),
                context: &ctx,
                dxvk,
                dgvoodoo: tools_status
                    .as_ref()
                    .map(|status| DgVoodooObservation::verified(status.dgvoodoo.configured))
                    .unwrap_or(DgVoodooObservation::Unavailable),
                webview2_required,
                recommendation: None,
            },
        );
    }
    let graphics_target = if is_patcher {
        InvocationTarget::LaunchPatcher
    } else {
        InvocationTarget::Game
    };
    let operational_plan = if runtime_graphics_plan_enabled() {
        let plan = operational_profile_plan
            .as_ref()
            .map(|(_, plan)| plan)
            .ok_or_else(|| "runtime-plan-resolution-failed".to_string())?;
        if plan.graphics().dxvk_provider().is_managed_prefix() && !manifest_has_managed_dxvk {
            return Err(
                "El entorno Wine 7.16 no registra DXVK 2.6.2; rearma este entorno antes de jugar"
                    .to_string(),
            );
        }
        if plan.graphics().dxvk_provider().is_managed_prefix() {
            let prefix_token = prefix_log_token(std::path::Path::new(&ctx.prefix));
            emit_tool_log_opt(
                Some(&app),
                format!("[Graphics] Wine 7.16 old WoW64 + DXVK 2.6.2 | prefix={prefix_token}"),
            );
        }
        Some(plan)
    } else {
        let use_managed_dxvk = wine_7_16 && manifest_has_managed_dxvk;
        if wine_7_16 && !use_managed_dxvk {
            return Err(
                "El entorno Wine 7.16 no registra DXVK 2.6.2; rearma este entorno antes de jugar"
                    .to_string(),
            );
        }
        if use_managed_dxvk {
            let prefix_token = prefix_log_token(std::path::Path::new(&ctx.prefix));
            emit_tool_log_opt(
                Some(&app),
                format!("[Graphics] Wine 7.16 old WoW64 + DXVK 2.6.2 | prefix={prefix_token}"),
            );
        }
        None
    };
    if wine_7_16 {
        emit_tool_log_opt(
            Some(&app),
            format!(
                "[Sync] Wine 7.16 · {}",
                ctx.resolved.wine_sync_mode().label()
            ),
        );
    }
    if game.stop_requested(reservation) {
        return Err("El lanzamiento fue cancelado por el usuario".to_string());
    }

    let supervised_session = op.lease().is_some();
    let timeout = if is_patcher {
        PATCHER_LAUNCH_TIMEOUT
    } else {
        DIRECT_LAUNCH_TIMEOUT
    };

    let mut invocation =
        ctx.resolved
            .game_invocation(&ctx.prefix, &launch_exe, rendered_args.iter(), &work_dir)?;
    if let Some(plan) = &operational_plan {
        let graphics = InvocationPlan {
            target: graphics_target,
            plan,
        }
        .environment()
        .map_err(graphics_environment_error_message)?;
        apply_graphics_environment_to_invocation(&mut invocation, &graphics)
            .map_err(graphics_environment_error_message)?;
    } else {
        let use_dgvoodoo = dgvoodoo_configured;
        let use_managed_dxvk = wine_7_16 && manifest_has_managed_dxvk;
        {
            #[allow(deprecated)]
            crate::utils::apply_game_env(
                &mut invocation,
                use_dgvoodoo,
                use_managed_dxvk,
                &ctx.prefix,
            );
        }
    }

    let mut spawned = op.spawn(invocation, &redaction_values).await?;
    let controller_pid = spawned
        .controller_pid()
        .ok_or_else(|| "El runner no informó su PID".to_string())?;
    let Some(controller_identity) = capture_process_identity(controller_pid) else {
        let _ = spawned.terminate().await;
        enqueue_unreached_observation(
            observation_id.clone(),
            operational_profile_plan.as_ref(),
            &server,
            &client_id,
            is_patcher,
            &anchor,
            &ctx,
            dgvoodoo_configured,
            &game_dir,
            None,
            op.lease().map(|lease| lease.supervisor_identity()),
            supervised_session,
            classify_run_outcome(OutcomeInput {
                reached_running: false,
                stop_requested: false,
                startup_timeout: false,
                startup_failure: Some(StartupFailureClass::ControllerIdentityMissing),
                controller_exit_before_terminate: None,
                controller_exit_after_terminate: -1,
            }),
        );
        return Err("El proceso controlador terminó antes de poder identificarlo".to_string());
    };
    if let Err(error) = game.mark_controller(reservation, controller_identity) {
        let _ = spawned.terminate().await;
        return Err(error);
    }

    emit_tool_log_opt(
        Some(&app),
        format!(
            "[Launch] controller={controller_pid} runner={} prefix={} supervised={supervised_session}",
            ctx.resolved.kind_label(),
            prefix_log_token(std::path::Path::new(&ctx.prefix))
        ),
    );

    let output_task = if matches!(spawned, SpawnedRunner::Direct(_)) {
        let (stdout, stderr) = spawned.take_direct_stdout_stderr();
        Some(tokio::spawn(drain_game_streams_redacted(
            app.clone(),
            stdout,
            stderr,
            redaction_values,
        )))
    } else {
        None
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
    let identity = match wait_for_game_process(
        &mut ControllerHandle::Spawned(&mut spawned),
        &search,
        timeout,
        CONTROLLER_EXIT_GRACE,
    )
    .await
    {
        Ok(identity) => identity,
        Err(error) => {
            let _ = spawned.terminate().await;
            if let Some(task) = output_task {
                let _ = task.await;
            }
            let stop_requested = error.contains("cancelado por el usuario");
            let startup_timeout = error.contains("dentro de");
            enqueue_unreached_observation(
                observation_id.clone(),
                operational_profile_plan.as_ref(),
                &server,
                &client_id,
                is_patcher,
                &anchor,
                &ctx,
                dgvoodoo_configured,
                &game_dir,
                Some(controller_identity),
                op.lease().map(|lease| lease.supervisor_identity()),
                supervised_session,
                classify_run_outcome(OutcomeInput {
                    reached_running: false,
                    stop_requested,
                    startup_timeout,
                    startup_failure: if stop_requested || startup_timeout {
                        None
                    } else {
                        Some(StartupFailureClass::GameProcessWaitFailed)
                    },
                    controller_exit_before_terminate: None,
                    controller_exit_after_terminate: -1,
                }),
            );
            return Err(error);
        }
    };

    let memory_ancestor = match op.lease() {
        Some(lease) => MemoryAncestor::Supervisor(lease.supervisor_identity()),
        None => launcher_memory_ancestor(),
    };
    let memory_lease = match memory.register(identity, memory_ancestor) {
        Ok(lease) => lease,
        Err(error) => {
            let _ = spawned.terminate().await;
            if let Some(task) = output_task {
                let _ = task.await;
            }
            return Err(error);
        }
    };
    let memory_access = memory
        .get(&identity)
        .map(|session| session.access())
        .ok_or_else(|| "Sesión de memoria no disponible tras el registro".to_string())?;
    let profile = resolve_profile(&load_profiles(), &game_exe, &server.autopot);
    let hp_base = profile.hp_base;
    let profile_memory = memory
        .get(&identity)
        .map(|session| session.profile_memory(Some(hp_base)));

    let runtime = match op.lease() {
        Some(lease) => {
            let client_lease = sessions
                .attach_client(lease, &client_id)
                .map_err(|error| error.message)?;
            ClientRuntimeGuard {
                session: SessionOwnership::Supervised(client_lease),
                memory: Some(memory_lease),
                memory_access: Some(memory_access),
                profile_memory,
            }
        }
        None => ClientRuntimeGuard {
            session: SessionOwnership::Direct,
            memory: Some(memory_lease),
            memory_access: Some(memory_access),
            profile_memory,
        },
    };

    let snapshot = match game.mark_running(reservation, identity, runtime) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            let _ = spawned.terminate().await;
            if let Some(task) = output_task {
                let _ = task.await;
            }
            return Err(error);
        }
    };
    let supervisor_identity = op.lease().map(|lease| lease.supervisor_identity());
    drop(op);

    if supervised_session {
        let client_ppid = read_ppid(identity.pid).unwrap_or(0);
        emit_tool_log_opt(
            Some(&app),
            format!(
                "[Launch] client={} ppid={} supervised=true handoff=none",
                identity.pid, client_ppid
            ),
        );
    }
    emit_tool_log_opt(
        Some(&app),
        format!(
            "[Launch] cliente detectado PID={} exe={} prefix={}",
            identity.pid,
            path_log_token(std::path::Path::new(&game_exe)),
            prefix_log_token(std::path::Path::new(&ctx.prefix))
        ),
    );

    presence.register(
        snapshot.client_id.clone(),
        snapshot.server_id.clone(),
        snapshot.server_name.clone(),
        identity,
        game_exe.clone(),
        overrides_from_autopot(&server.autopot),
    );

    let mut launch_snapshot = snapshot;
    launch_snapshot.profile_memory = profile_memory;
    if let (Some(observation_id), Some((profile, plan))) =
        (observation_id.as_ref(), operational_profile_plan.as_ref())
    {
        enqueue_persist_started(ObservationStartedPayload {
            observation_id: observation_id.clone(),
            client_id: client_id.clone(),
            server_local_id: server.id.clone(),
            game_executable_name: std::path::Path::new(&server.executable_path)
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "game.exe".to_string()),
            invocation_target: if is_patcher {
                "launch-patcher".to_string()
            } else {
                "game".to_string()
            },
            plan_id: anchor.plan_id.clone(),
            runtime_fingerprint: anchor.runtime_fingerprint.clone(),
            prefix_fingerprint_hex: ctx.identity.desired_fingerprint.digest.hex_digest(),
            prefix_token: prefix_log_token(std::path::Path::new(&ctx.prefix)),
            profile: profile.clone(),
            plan: plan.clone(),
            overlay_verified: dgvoodoo_configured,
            game_dir: Some(game_dir.clone()),
            game_identity: Some(identity),
            controller_identity: Some(controller_identity),
            supervisor_identity,
            supervised: supervised_session,
            plan_availability: if operational_profile_plan.is_some() {
                PlanAvailability::Resolved
            } else {
                PlanAvailability::Unresolved
            },
        });
    }
    spawn_exit_task(
        app,
        game,
        reservation,
        prefix_operation,
        dgvoodoo_operation,
        spawned,
        output_task,
        supervised_session,
        ctx,
        game_exe,
        baseline,
        identity,
        launch_snapshot.clone(),
        autopot.clone(),
        autobuff.clone(),
        spammer.clone(),
        presence.clone(),
        memory.clone(),
        memory_ancestor,
        hp_base,
        observation_id,
        controller_identity,
    );

    Ok(launch_snapshot)
}

#[allow(clippy::too_many_arguments)]
fn enqueue_unreached_observation(
    observation_id: Option<String>,
    plan_pair: Option<&(RuntimeProfile, RuntimePlan)>,
    server: &ServerConfig,
    client_id: &str,
    is_patcher: bool,
    anchor: &crate::tools::runtime::SessionAnchorV2,
    ctx: &crate::utils::WineContext,
    dgvoodoo_configured: bool,
    game_dir: &str,
    controller_identity: Option<ProcessIdentity>,
    supervisor_identity: Option<ProcessIdentity>,
    supervised: bool,
    outcome: RunOutcome,
) {
    let (Some(observation_id), Some((profile, plan))) = (observation_id, plan_pair) else {
        return;
    };
    enqueue_persist_unreached(
        ObservationStartedPayload {
            observation_id: observation_id.clone(),
            client_id: client_id.to_string(),
            server_local_id: server.id.clone(),
            game_executable_name: std::path::Path::new(&server.executable_path)
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "game.exe".to_string()),
            invocation_target: if is_patcher {
                "launch-patcher".to_string()
            } else {
                "game".to_string()
            },
            plan_id: anchor.plan_id.clone(),
            runtime_fingerprint: anchor.runtime_fingerprint.clone(),
            prefix_fingerprint_hex: ctx.identity.desired_fingerprint.digest.hex_digest(),
            prefix_token: prefix_log_token(std::path::Path::new(&ctx.prefix)),
            profile: profile.clone(),
            plan: plan.clone(),
            overlay_verified: dgvoodoo_configured,
            game_dir: Some(game_dir.to_string()),
            game_identity: None,
            controller_identity,
            supervisor_identity,
            supervised,
            plan_availability: PlanAvailability::Resolved,
        },
        ObservationFinishedPayload {
            observation_id,
            outcome,
            game_identity: None,
            controller_identity,
            identity_stale: false,
            handoff_count: 0,
        },
    );
}

enum ControllerHandle<'a> {
    Spawned(&'a mut SpawnedRunner),
}

impl ControllerHandle<'_> {
    async fn poll_exit_code(&mut self) -> Result<Option<i32>, String> {
        match self {
            Self::Spawned(runner) => Ok(runner.try_exit_code()),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_exit_task(
    app: AppHandle,
    game: GameProcessHandle,
    reservation: LaunchReservation,
    prefix_operation: OperationGuard,
    dgvoodoo_operation: OperationGuard,
    mut controller: SpawnedRunner,
    output_task: Option<tokio::task::JoinHandle<()>>,
    supervised_session: bool,
    ctx: crate::utils::WineContext,
    game_exe: String,
    baseline: HashSet<ProcessIdentity>,
    identity: ProcessIdentity,
    snapshot: GameClientSnapshot,
    autopot: AutopotHandle,
    autobuff: AutobuffHandle,
    spammer: SpammerHandle,
    presence: PresenceHandle,
    memory: MemorySessionRegistry,
    memory_ancestor: MemoryAncestor,
    hp_base: u32,
    observation_id: Option<String>,
    initial_controller_identity: ProcessIdentity,
) {
    let presence_client_id = snapshot.client_id.clone();
    let app_for_exit = app.clone();
    tokio::spawn(async move {
        let _prefix_operation = prefix_operation;
        let _dgvoodoo_operation = dgvoodoo_operation;
        let mut active_identity = identity;
        let mut seen = baseline;
        seen.insert(active_identity);
        let controller_pid = controller.controller_pid().unwrap_or(0);
        let mut handoff_count = 0u32;
        let started_game_identity = identity;
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
            let new_lease = match memory.register(replacement, memory_ancestor) {
                Ok(lease) => lease,
                Err(error) => {
                    emit_tool_log_opt(
                        Some(&app_for_exit),
                        format!("[Memory] handoff falló: {error}"),
                    );
                    break;
                }
            };
            let memory_access = memory
                .get(&replacement)
                .map(|session| session.access())
                .unwrap_or(crate::models::memory::MemoryAccess::BackendError);
            let profile_memory = memory
                .get(&replacement)
                .map(|session| session.profile_memory(Some(hp_base)))
                .unwrap_or(crate::models::memory::ProfileMemory::InvalidRead);
            if !game.replace_running(
                reservation,
                active_identity,
                replacement,
                new_lease,
                memory_access,
                profile_memory,
            ) {
                break;
            }
            presence.handoff(&presence_client_id, replacement);
            autopot.handoff(replacement);
            autobuff.handoff(replacement);
            if let Some(client_snapshot) = game.snapshot_for(reservation) {
                let _ = app_for_exit.emit(EVENT_GAME_CLIENT, client_snapshot);
            }
            let client_ppid = read_ppid(replacement.pid).unwrap_or(0);
            if supervised_session {
                emit_tool_log_opt(
                    Some(&app_for_exit),
                    format!(
                        "[Launch] client={} ppid={} supervised=true handoff={} -> {}",
                        replacement.pid, client_ppid, active_identity.pid, replacement.pid
                    ),
                );
            } else {
                emit_tool_log_opt(
                    Some(&app_for_exit),
                    format!(
                        "[Launch] handoff del cliente PID={} -> PID={}",
                        active_identity.pid, replacement.pid
                    ),
                );
            }
            seen.insert(replacement);
            active_identity = replacement;
            handoff_count += 1;
        }

        let spontaneous = controller.try_exit_status();
        let stop_requested = game.stop_requested(reservation);
        let code = {
            let _ = controller.terminate().await;
            controller.wait().await.unwrap_or(-1)
        };
        if let Some(observation_id) = observation_id {
            enqueue_persist_finished(ObservationFinishedPayload {
                observation_id,
                outcome: classify_run_outcome(OutcomeInput {
                    reached_running: true,
                    stop_requested,
                    startup_timeout: false,
                    startup_failure: None,
                    controller_exit_before_terminate: spontaneous,
                    controller_exit_after_terminate: code,
                }),
                game_identity: Some(active_identity),
                controller_identity: Some(initial_controller_identity),
                identity_stale: active_identity != started_game_identity && handoff_count == 0,
                handoff_count,
            });
        }
        if let Some(task) = output_task {
            let _ = task.await;
        }

        if let Some(finished) = game.finish(reservation) {
            presence.unregister(&finished.client_id);
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

#[allow(clippy::too_many_arguments)]
async fn wait_for_game_process(
    controller: &mut ControllerHandle<'_>,
    search: &ProcessSearch<'_>,
    timeout: Duration,
    grace: Duration,
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
            if let Some(code) = controller.poll_exit_code().await? {
                controller_exit = Some(code);
                deadline = deadline.min(Instant::now() + grace);
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
    let process_errors = terminate_processes(request.identities);
    combine_stop_errors(process_errors, tool_errors)
}

pub async fn stop_all_games(state: &GameState) -> Result<(), String> {
    let tool_errors = stop_combat_tools(state).await;
    let identities = state.game.request_stop_all()?;
    let process_errors = terminate_processes(identities);
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

fn terminate_processes(identities: Vec<ProcessIdentity>) -> Vec<String> {
    let mut process_errors = Vec::new();
    for identity in identities {
        if let Err(error) = signal_process_identity(&identity, libc::SIGTERM) {
            process_errors.push(format!("No se pudo enviar TERM al proceso: {error}"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::process::Stdio;
    use std::time::Duration;
    use tokio::time::Instant;

    #[tokio::test]
    async fn wait_for_game_process_shortens_deadline_after_controller_exits() {
        let game = GameProcessHandle::new();
        let reservation = game
            .begin_launch("c1".into(), "srv".into(), "Srv".into())
            .unwrap();
        let mut spawned = SpawnedRunner::Direct(
            tokio::process::Command::new("/bin/true")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn true"),
        );
        let controller_pid = spawned.controller_pid().expect("controller pid");
        let _ = spawned.wait().await;

        let baseline = HashSet::new();
        let prefix = format!(
            "/tmp/ro-launcher-wait-grace-{}-no-match",
            std::process::id()
        );
        let search = ProcessSearch {
            controller_pid,
            exe_path: "/nonexistent/ragexe.exe",
            prefix: &prefix,
            baseline: &baseline,
            exclude_controller: false,
            game: &game,
            reservation,
        };

        let started = Instant::now();
        let err = wait_for_game_process(
            &mut ControllerHandle::Spawned(&mut spawned),
            &search,
            Duration::from_secs(5),
            Duration::from_millis(80),
        )
        .await
        .expect_err("game process should not appear");
        let elapsed = started.elapsed();
        assert!(
            elapsed < Duration::from_secs(2),
            "expected grace-bound wait, took {:?}",
            elapsed
        );
        assert!(
            err.contains("el controlador ya había terminado"),
            "unexpected error: {err}"
        );
    }
}
