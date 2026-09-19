use std::path::PathBuf;

use ro_tools_linux::verify_process_identity;
use tauri::{AppHandle, State};

use crate::models::benchmark::{
    BenchmarkComparisonIpc, BenchmarkExportResultIpc, BenchmarkRunSummaryIpc,
    CompareRuntimeBenchmarksIpc, StartRuntimeBenchmarkRunIpc,
};
use crate::models::server::ServerConfig;
use crate::state::{GameState, ServerRepository, StorageNotices};
use crate::tools::runner_sessions::{prefix_log_token, session_supervisor_enabled};
use crate::tools::runtime::{
    attach_benchmark_run, begin_benchmark_capture,
    benchmark::{
        map_subject_observation, parse_arm, parse_thermal, parse_visual_check,
        runtime_fingerprint_envelope, validate_spec, BenchmarkFlagsIpc, BenchmarkRecordState,
        BenchmarkResolutionV1, BenchmarkSpecV1, BenchmarkVisualCheck, CaptureAdapterKind,
        RuntimeBenchmarkRunV1, BENCHMARK_PROTOCOL_REVISION, RUNTIME_BENCHMARK_SCHEMA_VERSION,
    },
    benchmark_store::{
        build_invalid_at_attach, compare_benchmark_runs_by_id, delete_benchmark_runs,
        export_benchmark_comparison, export_benchmark_runs, finish_benchmark_capture,
        import_benchmark_samples, list_benchmark_runs, load_benchmark_run, new_benchmark_run_id,
        set_benchmark_visual_check, to_process_identity_ipc,
    },
    build_runtime_plan_summary, host_clk_tck, host_uptime_seconds, inspect_subject,
    lookup_observation_for_identity, operational_session_anchor, probe_host_gpu,
    process_age_seconds, resolve_operational_plan_with_profile, runtime_benchmark_enabled,
    runtime_graphics_plan_enabled, runtime_observe_enabled, server_path_token16,
    session_anchor_unavailable, DgVoodooState, FingerprintEnvelopeIpc, ObservationProcessIpc,
    OperationalRuntimeInput,
};
use crate::tools::server_tools;
use crate::utils::{required_game_dir, resolve_server_wine_context_with_runner, RunnerKind};

async fn resolve_plan_id(
    app: &AppHandle,
    server: &ServerConfig,
    runner: Option<String>,
) -> Result<String, String> {
    let ctx = resolve_server_wine_context_with_runner(Some(server), runner).await?;
    let tools_status = server_tools::scan_status(app, server).ok();
    let dgvoodoo_configured = tools_status
        .as_ref()
        .is_some_and(|status| status.dgvoodoo.configured);
    let webview2_required = tools_status
        .as_ref()
        .is_some_and(|status| status.diagnostics.webview2_required);
    let (_, plan) = resolve_operational_plan_with_profile(OperationalRuntimeInput {
        server_runner: server.runner.as_deref(),
        default_runner: server.runner.as_deref(),
        context: &ctx,
        dgvoodoo: DgVoodooState::verified(dgvoodoo_configured),
        webview2_required,
    })
    .map_err(|_| "plan-resolution-failed".to_string())?;
    let anchor = operational_session_anchor(&ctx, Some(&plan), webview2_required);
    if anchor.plan_id == session_anchor_unavailable().plan_id {
        return Err("plan-id-unavailable".into());
    }
    Ok(anchor.plan_id)
}

fn server_by_id(
    repository: &ServerRepository,
    notices: &StorageNotices,
    server_id: &str,
) -> Result<ServerConfig, String> {
    repository
        .list(notices)?
        .into_iter()
        .find(|server| server.id == server_id)
        .ok_or_else(|| "client-not-running".to_string())
}

#[tauri::command]
pub async fn start_runtime_benchmark_run(
    app: AppHandle,
    state: State<'_, GameState>,
    input: StartRuntimeBenchmarkRunIpc,
) -> Result<BenchmarkRunSummaryIpc, String> {
    if !runtime_benchmark_enabled() {
        return Err("runtime-benchmark-disabled".into());
    }
    input.server.validate_executable_available()?;
    let arm = parse_arm(&input.arm).ok_or_else(|| "invalid-arm".to_string())?;
    let thermal = parse_thermal(&input.thermal_declared)
        .ok_or_else(|| "invalid-thermal-declared".to_string())?;
    validate_spec(
        &input.scene_id,
        &input.load_descriptor,
        input.resolution.width,
        input.resolution.height,
        input.warmup_seconds,
        input.capture_seconds,
    )?;

    let facts = state.game.running_facts_for(&input.client_id)?;
    if facts.server_id != input.server.id {
        return Err("client-not-running".into());
    }
    if !verify_process_identity(&facts.identity) {
        return Err("process-identity-stale".into());
    }

    let ctx =
        resolve_server_wine_context_with_runner(Some(&input.server), input.runner.clone()).await?;
    let tools_status = server_tools::scan_status(&app, &input.server).ok();
    let dgvoodoo_configured = tools_status
        .as_ref()
        .is_some_and(|status| status.dgvoodoo.configured);
    let webview2_required = tools_status
        .as_ref()
        .is_some_and(|status| status.diagnostics.webview2_required);

    let (profile, plan) = resolve_operational_plan_with_profile(OperationalRuntimeInput {
        server_runner: input.server.runner.as_deref(),
        default_runner: input.runner.as_deref(),
        context: &ctx,
        dgvoodoo: DgVoodooState::verified(dgvoodoo_configured),
        webview2_required,
    })
    .map_err(|_| "plan-resolution-failed".to_string())?;

    let anchor = operational_session_anchor(&ctx, Some(&plan), webview2_required);
    if anchor.plan_id == session_anchor_unavailable().plan_id {
        return Err("plan-id-unavailable".into());
    }

    let supervisor = session_supervisor_enabled();
    if supervisor {
        let session_plan = state
            .sessions
            .plan_id_for_prefix(std::path::Path::new(&ctx.prefix))
            .ok_or_else(|| "session-not-active".to_string())?;
        if session_plan != anchor.plan_id {
            return Err("plan-id-mismatch".into());
        }
    }

    let (observation_id, linked_outcome) =
        lookup_observation_for_identity(&input.client_id, &facts.identity)
            .map(|(id, outcome)| (Some(id), outcome))
            .unwrap_or((None, None));

    let invalid_reason = build_invalid_at_attach(linked_outcome.as_deref());
    let summary =
        build_runtime_plan_summary(anchor.plan_id.clone(), &profile, &plan, dgvoodoo_configured);
    let game_dir = required_game_dir(&input.server.executable_path).ok();
    let subject = map_subject_observation(inspect_subject(
        game_dir.as_deref().map(std::path::Path::new),
    ));
    let host_gpu = probe_host_gpu();
    let run_id = new_benchmark_run_id();
    let record_state = if invalid_reason.is_some() {
        BenchmarkRecordState::Invalid
    } else {
        BenchmarkRecordState::Attached
    };

    let record = RuntimeBenchmarkRunV1 {
        schema_version: RUNTIME_BENCHMARK_SCHEMA_VERSION,
        run_id: run_id.clone(),
        record_state,
        invalid_reason,
        attached_at: chrono::Utc::now().to_rfc3339(),
        capture_started_at: None,
        capture_finished_at: None,
        client_id: input.client_id.clone(),
        server_local_id: input.server.id.clone(),
        server_token: server_path_token16(&input.server.id),
        observation_id,
        arm,
        spec: BenchmarkSpecV1 {
            protocol_revision: BENCHMARK_PROTOCOL_REVISION,
            scene_id: input.scene_id.clone(),
            load_descriptor: input.load_descriptor.clone(),
            resolution: BenchmarkResolutionV1 {
                width: input.resolution.width,
                height: input.resolution.height,
                fullscreen: input.resolution.fullscreen,
            },
            warmup_seconds: input.warmup_seconds,
            capture_seconds: input.capture_seconds,
            thermal_declared: thermal,
        },
        plan_id: anchor.plan_id.clone(),
        runtime_fingerprint: runtime_fingerprint_envelope(&anchor.runtime_fingerprint),
        prefix_fingerprint: FingerprintEnvelopeIpc {
            schema_version: 1,
            algorithm: "sha256".to_string(),
            digest: ctx.identity.desired_fingerprint.digest.hex_digest(),
        },
        prefix_token: prefix_log_token(std::path::Path::new(&ctx.prefix)),
        selection_source: summary.selection_source.to_string(),
        runner_kind: match plan.runner().resolved().kind() {
            RunnerKind::Wine => "wine".to_string(),
            RunnerKind::Proton => "proton".to_string(),
        },
        graphics_profile: summary.graphics_profile.to_string(),
        dxvk_provider: summary.dxvk_provider.to_string(),
        dxvk_component_id: summary.dxvk_component_id.clone(),
        overlay_verified: dgvoodoo_configured,
        subject,
        host_gpu,
        process: ObservationProcessIpc {
            game: Some(to_process_identity_ipc(facts.identity)),
            controller: facts.controller.map(to_process_identity_ipc),
            final_game: None,
            identity_stale: false,
            handoff_count: 0,
        },
        flags: BenchmarkFlagsIpc {
            graphics_plan: runtime_graphics_plan_enabled(),
            supervisor,
            observe: runtime_observe_enabled(),
            benchmark: true,
        },
        capture_adapter: CaptureAdapterKind::ImportedCsvV1,
        samples: None,
        visual_check: BenchmarkVisualCheck::Pending,
        linked_outcome_kind: linked_outcome,
    };

    attach_benchmark_run(record)
}

#[tauri::command]
pub async fn begin_runtime_benchmark_capture(
    app: AppHandle,
    state: State<'_, GameState>,
    servers: State<'_, ServerRepository>,
    notices: State<'_, StorageNotices>,
    run_id: String,
) -> Result<BenchmarkRunSummaryIpc, String> {
    if !runtime_benchmark_enabled() {
        return Err("runtime-benchmark-disabled".into());
    }
    let record = load_benchmark_run(&run_id)?;
    let server = server_by_id(&servers, &notices, &record.server_local_id)?;
    let facts = state.game.running_facts_for(&record.client_id)?;
    if !verify_process_identity(&facts.identity) {
        return Err("process-identity-stale".into());
    }
    let resolved_plan_id = resolve_plan_id(&app, &server, server.runner.clone()).await?;
    let process_age = host_uptime_seconds()
        .and_then(|uptime| process_age_seconds(facts.identity.start_time, uptime, host_clk_tck()));
    begin_benchmark_capture(
        &run_id,
        &facts.identity,
        &resolved_plan_id,
        record.spec.warmup_seconds,
        process_age,
    )
}

#[tauri::command]
pub async fn finish_runtime_benchmark_capture(
    state: State<'_, GameState>,
    run_id: String,
) -> Result<BenchmarkRunSummaryIpc, String> {
    if !runtime_benchmark_enabled() {
        return Err("runtime-benchmark-disabled".into());
    }
    let record = load_benchmark_run(&run_id)?;
    let facts = state.game.running_facts_for(&record.client_id)?;
    if !verify_process_identity(&facts.identity) {
        return Err("process-identity-stale".into());
    }
    finish_benchmark_capture(&run_id, &facts.identity)
}

#[tauri::command]
pub fn import_runtime_benchmark_samples(
    run_id: String,
    csv_path: String,
) -> Result<BenchmarkRunSummaryIpc, String> {
    import_benchmark_samples(&run_id, PathBuf::from(csv_path).as_path())
}

#[tauri::command]
pub fn set_runtime_benchmark_visual_check(
    run_id: String,
    status: String,
) -> Result<BenchmarkRunSummaryIpc, String> {
    let status = parse_visual_check(&status).ok_or_else(|| "invalid-visual-check".to_string())?;
    set_benchmark_visual_check(&run_id, status)
}

#[tauri::command]
pub fn list_runtime_benchmarks() -> Result<Vec<BenchmarkRunSummaryIpc>, String> {
    list_benchmark_runs()
}

#[tauri::command]
pub fn compare_runtime_benchmarks(
    input: CompareRuntimeBenchmarksIpc,
) -> Result<BenchmarkComparisonIpc, String> {
    compare_benchmark_runs_by_id(&input)
}

#[tauri::command]
pub fn export_runtime_benchmarks(
    dest_path: String,
    run_ids: Vec<String>,
) -> Result<BenchmarkExportResultIpc, String> {
    export_benchmark_runs(PathBuf::from(dest_path).as_path(), &run_ids)
}

#[tauri::command]
pub fn export_runtime_benchmark_comparison(
    dest_path: String,
    input: CompareRuntimeBenchmarksIpc,
) -> Result<BenchmarkExportResultIpc, String> {
    export_benchmark_comparison(PathBuf::from(dest_path).as_path(), &input)
}

#[tauri::command]
pub fn delete_runtime_benchmarks() -> Result<(), String> {
    delete_benchmark_runs()
}
