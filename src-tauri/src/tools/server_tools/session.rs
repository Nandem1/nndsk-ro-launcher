use std::path::Path;

use tauri::AppHandle;

use crate::models::server::ServerConfig;
use crate::models::server_tools::{
    InstallDgVoodooResult, ServerToolsStatus, UninstallDgVoodooResult,
};
use crate::models::tool_kind::ToolKind;
use crate::tools::runners::ensure_managed_runtime;
use crate::utils::{
    apply_tool_env, drain_and_log, emit_log_opt, pipe_output, required_game_dir,
    resolve_server_wine_context_with_runner, validate_runtime_prefix, OperationGuard,
};

use super::{dgvoodoo, scan};

pub fn scan_status(app: &AppHandle, server: &ServerConfig) -> Result<ServerToolsStatus, String> {
    let game_dir = required_game_dir(&server.executable_path)?;
    let can_auto_install = dgvoodoo::template_dir(app).is_ok();
    let mut status = scan::scan_game_dir(&game_dir, server, can_auto_install)?;
    let mut validation_issues = dgvoodoo::entry_collision_issues(Path::new(&game_dir));
    if status.dgvoodoo.d3dimm_dll.found && status.dgvoodoo.ddraw_dll.found {
        if let Err(issues) = dgvoodoo::verify_wrapper_files(app, Path::new(&game_dir)) {
            validation_issues.extend(issues);
        }
    }
    validation_issues.sort();
    validation_issues.dedup();
    if !validation_issues.is_empty() {
        status.dgvoodoo.configured = false;
        status.dgvoodoo.needs_install = true;
        status.dgvoodoo.issues.extend(validation_issues);
    }
    Ok(status)
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

pub async fn launch_tool(
    app: &AppHandle,
    server: &ServerConfig,
    tool: ToolKind,
    runner: Option<String>,
) -> Result<(), String> {
    ensure_managed_runtime(app).await?;
    let status = scan_status(app, server)?;
    let use_dgvoodoo = tool.should_apply_dgvoodoo_overrides(status.dgvoodoo.configured);
    let exe_path = match tool {
        ToolKind::OpenSetup => status
            .open_setup
            .path
            .ok_or_else(|| "OpenSetup no encontrado".to_string())?,
        ToolKind::Patcher => status
            .patcher
            .path
            .ok_or_else(|| "Patcher no encontrado".to_string())?,
        ToolKind::DgVoodoo => status
            .dgvoodoo
            .cpl
            .path
            .ok_or_else(|| "dgVoodoo Control Panel no encontrado".to_string())?,
    };

    let ctx = resolve_server_wine_context_with_runner(Some(server), runner).await?;
    let prefix_operation = OperationGuard::acquire("prefix", Path::new(&ctx.prefix))?;
    validate_runtime_prefix(&ctx)?;
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
    let dgvoodoo_operation = if matches!(tool, ToolKind::DgVoodoo) {
        Some(OperationGuard::acquire("dgvoodoo", Path::new(&work_dir))?)
    } else {
        None
    };

    // El botón Herramientas abre el patcher en modo mantenimiento. Los argumentos de inicio
    // (incluidos placeholders de credenciales) pertenecen exclusivamente al botón Jugar.
    let args: Vec<String> = Vec::new();
    let mut cmd = ctx
        .resolved
        .tool_command(&ctx.prefix, &exe_path, args.iter(), &work_dir);
    apply_tool_env(&mut cmd, use_dgvoodoo);
    pipe_output(&mut cmd);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Error al abrir la herramienta: {e}"))?;

    let app = app.clone();
    let tool_label = match tool {
        ToolKind::OpenSetup => "OpenSetup",
        ToolKind::Patcher => "Patcher",
        ToolKind::DgVoodoo => "dgVoodoo CPL",
    };
    tokio::spawn(async move {
        let _prefix_operation = prefix_operation;
        let _dgvoodoo_operation = dgvoodoo_operation;
        drain_and_log(&app, &mut child).await;
        match child.wait().await {
            Ok(status) => emit_log_opt(
                Some(&app),
                format!(
                    "[Tool:{tool_label}] finalizó con código {}",
                    status.code().unwrap_or(-1)
                ),
            ),
            Err(error) => emit_log_opt(
                Some(&app),
                format!("[Tool:{tool_label}] no se pudo obtener el código de salida: {error}"),
            ),
        }
    });

    Ok(())
}
