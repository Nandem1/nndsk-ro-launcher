use std::path::Path;

use tauri::AppHandle;

use crate::models::server::ServerConfig;
use crate::models::server_tools::{
    DgVoodooStatus, InstallDgVoodooResult, ServerToolsStatus, UninstallDgVoodooResult,
};
use crate::models::tool_kind::ToolKind;
use crate::state::GameProcessHandle;
use crate::tools::prefix::{DxvkProvision, MANAGED_DXVK_COMPONENT};
use crate::tools::runner_sessions::{RunnerOperation, RunnerSessionRegistry, SpawnedRunner};
use crate::tools::runners::ensure_managed_runtime;
use crate::tools::runtime::{
    apply_graphics_environment_to_invocation, observe_legacy_runtime,
    runtime_graphics_plan_enabled, runtime_shadow_enabled, DgVoodooObservation, InvocationPlan,
    InvocationTarget, LegacyRuntimeInput, ShadowOperation,
};
use crate::utils::{
    drain_and_log, emit_log_opt, required_game_dir, resolve_server_wine_context_with_runner,
    validate_runtime_prefix, OperationGuard,
};

use super::{dgvoodoo, scan};

pub fn scan_status(app: &AppHandle, server: &ServerConfig) -> Result<ServerToolsStatus, String> {
    let game_dir = required_game_dir(&server.executable_path)?;
    let can_auto_install = dgvoodoo::template_dir(app).is_ok();
    let mut status = scan::scan_game_dir(&game_dir, server, can_auto_install)?;
    verify_dgvoodoo_status(app, Path::new(&game_dir), &mut status.dgvoodoo);
    Ok(status)
}

pub(crate) fn scan_dgvoodoo_status(
    app: &AppHandle,
    server: &ServerConfig,
) -> Result<DgVoodooStatus, String> {
    let game_dir = required_game_dir(&server.executable_path)?;
    let can_auto_install = dgvoodoo::template_dir(app).is_ok();
    let mut status = scan::detect_dgvoodoo(Path::new(&game_dir), can_auto_install);
    verify_dgvoodoo_status(app, Path::new(&game_dir), &mut status);
    Ok(status)
}

fn verify_dgvoodoo_status(app: &AppHandle, game_dir: &Path, status: &mut DgVoodooStatus) {
    let mut validation_issues = dgvoodoo::entry_collision_issues(game_dir);
    if status.d3dimm_dll.found && status.ddraw_dll.found {
        if let Err(issues) = dgvoodoo::verify_wrapper_files(app, game_dir) {
            validation_issues.extend(issues);
        }
    }
    validation_issues.sort();
    validation_issues.dedup();
    if !validation_issues.is_empty() {
        status.configured = false;
        status.needs_install = true;
        status.issues.extend(validation_issues);
    }
}

pub async fn install_dgvoodoo(
    app: &AppHandle,
    server: &ServerConfig,
) -> Result<InstallDgVoodooResult, String> {
    let game_dir = required_game_dir(&server.executable_path)?;
    let installed = dgvoodoo::install_files(app, Path::new(&game_dir))?;
    let status = scan_status(app, server)?;
    Ok(InstallDgVoodooResult { installed, status })
}

pub async fn uninstall_dgvoodoo(
    app: &AppHandle,
    server: &ServerConfig,
) -> Result<UninstallDgVoodooResult, String> {
    let game_dir = required_game_dir(&server.executable_path)?;
    let removed = dgvoodoo::uninstall_files(Path::new(&game_dir))?;
    let status = scan_status(app, server)?;
    Ok(UninstallDgVoodooResult { removed, status })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DgvoodooLockMode {
    None,
    Shared,
    Exclusive,
}

fn dgvoodoo_lock_mode(tool: ToolKind) -> DgvoodooLockMode {
    match tool {
        ToolKind::OpenSetup => DgvoodooLockMode::Shared,
        ToolKind::Patcher => DgvoodooLockMode::None,
        ToolKind::DgVoodoo => DgvoodooLockMode::Exclusive,
    }
}

fn tool_invocation_target(tool: ToolKind) -> InvocationTarget {
    match tool {
        ToolKind::OpenSetup => InvocationTarget::OpenSetup,
        ToolKind::Patcher => InvocationTarget::MaintenancePatcher,
        ToolKind::DgVoodoo => InvocationTarget::GraphicsControlPanel,
    }
}

fn tool_executable_path(status: &ServerToolsStatus, tool: ToolKind) -> Result<String, String> {
    match tool {
        ToolKind::OpenSetup => status
            .open_setup
            .path
            .clone()
            .ok_or_else(|| "OpenSetup no encontrado".to_string()),
        ToolKind::Patcher => status
            .patcher
            .path
            .clone()
            .ok_or_else(|| "Patcher no encontrado".to_string()),
        ToolKind::DgVoodoo => status
            .dgvoodoo
            .cpl
            .path
            .clone()
            .ok_or_else(|| "dgVoodoo Control Panel no encontrado".to_string()),
    }
}

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

pub async fn launch_tool(
    app: &AppHandle,
    game: &GameProcessHandle,
    sessions: &RunnerSessionRegistry,
    server: &ServerConfig,
    tool: ToolKind,
    runner: Option<String>,
) -> Result<(), String> {
    let default_runner = runner.clone();
    ensure_managed_runtime(app).await?;
    let initial_status = scan_status(app, server)?;
    let _initial_exe = tool_executable_path(&initial_status, tool)?;

    let ctx = resolve_server_wine_context_with_runner(Some(server), runner).await?;
    let dgvoodoo_configured = initial_status.dgvoodoo.configured;
    let webview2_required = initial_status.diagnostics.webview2_required;
    let resolved_operational_plan = crate::tools::runtime::resolve_operational_plan(
        crate::tools::runtime::OperationalRuntimeInput {
            server_runner: server.runner.as_deref(),
            default_runner: default_runner.as_deref(),
            context: &ctx,
            dgvoodoo: crate::tools::runtime::DgVoodooState::verified(dgvoodoo_configured),
            webview2_required,
        },
    );
    let anchor = crate::tools::runtime::operational_session_anchor(
        &ctx,
        resolved_operational_plan.as_ref().ok(),
        webview2_required,
    );
    let op = RunnerOperation::begin(Some(app), sessions, game, &ctx, &anchor).await?;
    let prefix_operation = OperationGuard::acquire("prefix", Path::new(&ctx.prefix))?;
    let prefix_health = validate_runtime_prefix(&ctx)?;

    let preliminary_exe = tool_executable_path(&initial_status, tool)?;
    let preliminary_work_dir = required_game_dir(&preliminary_exe)
        .or_else(|_| required_game_dir(&server.executable_path))?;
    let dgvoodoo_operation = match dgvoodoo_lock_mode(tool) {
        DgvoodooLockMode::None => None,
        DgvoodooLockMode::Shared => Some(OperationGuard::acquire_shared(
            "dgvoodoo",
            Path::new(&preliminary_work_dir),
        )?),
        DgvoodooLockMode::Exclusive => Some(OperationGuard::acquire(
            "dgvoodoo",
            Path::new(&preliminary_work_dir),
        )?),
    };

    let status = scan_status(app, server)?;
    let exe_path = tool_executable_path(&status, tool)?;
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
            Some(app),
            ShadowOperation::ServerTool,
            LegacyRuntimeInput {
                server_runner: server.runner.as_deref(),
                default_runner: default_runner.as_deref(),
                context: &ctx,
                dxvk,
                dgvoodoo: DgVoodooObservation::verified(dgvoodoo_configured),
                webview2_required,
                recommendation: None,
            },
        );
    }

    let graphics_target = tool_invocation_target(tool);
    let operational_plan = if runtime_graphics_plan_enabled() {
        let plan = resolved_operational_plan.map_err(|error| error.code().to_string())?;
        if plan.graphics().dxvk_provider().is_managed_prefix() && !manifest_has_managed_dxvk {
            return Err(
                "El entorno Wine 7.16 no registra DXVK 2.6.2; rearma este entorno antes de abrir herramientas"
                    .to_string(),
            );
        }
        Some(plan)
    } else {
        let use_managed_dxvk = wine_7_16 && manifest_has_managed_dxvk;
        if wine_7_16 && !use_managed_dxvk {
            return Err(
                "El entorno Wine 7.16 no registra DXVK 2.6.2; rearma este entorno antes de abrir herramientas"
                    .to_string(),
            );
        }
        None
    };

    let missing_components = super::pe::missing_runtime_components_for_executable(
        Path::new(&exe_path),
        Path::new(&ctx.prefix),
        matches!(tool, ToolKind::Patcher) && server.launch.require_webview2,
    );
    if !missing_components.is_empty() {
        return Err(format!(
            "El entorno requiere reparación: {}",
            missing_components.join(" · ")
        ));
    }

    let work_dir =
        required_game_dir(&exe_path).or_else(|_| required_game_dir(&server.executable_path))?;

    let args: Vec<String> = Vec::new();
    let mut invocation =
        ctx.resolved
            .tool_invocation(&ctx.prefix, &exe_path, args.iter(), &work_dir)?;
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
        let use_dgvoodoo = tool.should_apply_dgvoodoo_overrides(dgvoodoo_configured);
        let use_managed_dxvk = wine_7_16 && manifest_has_managed_dxvk;
        {
            #[allow(deprecated)]
            crate::utils::apply_tool_env(
                &mut invocation,
                use_dgvoodoo,
                use_managed_dxvk,
                &ctx.prefix,
            );
        }
    }

    let mut spawned = op.spawn(invocation, &[]).await?;

    let app = app.clone();
    let tool_label = match tool {
        ToolKind::OpenSetup => "OpenSetup",
        ToolKind::Patcher => "Patcher",
        ToolKind::DgVoodoo => "dgVoodoo CPL",
    };
    tokio::spawn(async move {
        let _prefix_operation = prefix_operation;
        let _dgvoodoo_operation = dgvoodoo_operation;
        let _operation = op;
        match &mut spawned {
            SpawnedRunner::Direct(child) => {
                drain_and_log(&app, child).await;
            }
            SpawnedRunner::Supervised(_) => {}
        }
        let code = spawned.wait().await.unwrap_or(-1);
        emit_log_opt(
            Some(&app),
            format!("[Tool:{tool_label}] finalizó con código {code}"),
        );
    });

    Ok(())
}
