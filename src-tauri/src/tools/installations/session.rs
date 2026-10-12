use std::{future::Future, path::Path};
use tauri::AppHandle;
use tokio::sync::watch;

use super::InstallationManager;
use crate::models::{
    installation::{validate_exe, InstallationBinding, InstallationDraft, InstallationStatus},
    server::{PrefixMode, ServerConfig},
};
use crate::state::{GameProcessHandle, ServerRepository, StorageNotices};
use crate::tools::{
    prefix::{setup_runtime_prefix, DxvkProvision, RuntimeRequirements},
    runner_sessions::{RunnerOperation, RunnerSessionRegistry, SpawnedRunner},
    runtime::{
        apply_graphics_environment_to_invocation, operational_session_anchor,
        resolve_operational_plan, DgVoodooState, InvocationPlan, InvocationTarget,
        OperationalRuntimeInput,
    },
};
use crate::utils::{
    ensure_managed_path_safe, replace_json, resolve_installation_wine_context,
    validate_installation_context, validate_runtime_prefix, OperationGuard, INSTALLATION_MARKER,
};

pub fn start(
    manager: InstallationManager,
    app: AppHandle,
    game: GameProcessHandle,
    sessions: RunnerSessionRegistry,
    id: &str,
) -> Result<InstallationDraft, String> {
    if !crate::tools::runner_sessions::session_supervisor_enabled() {
        return Err("La instalación guiada requiere el supervisor de sesiones. Retira RO_LAUNCHER_SESSION_SUPERVISOR=0 y vuelve a abrir el launcher".into());
    }
    dispatch(manager, id, move |manager, draft, cancel| async move {
        crate::utils::PROGRESS_OPERATION_ID
            .scope(
                Some(draft.id.clone()),
                run(&manager, &app, &game, &sessions, &draft, cancel),
            )
            .await
    })
}

/// Shared by the synchronous IPC entry and its no-runtime regression tests.
/// Admission, background ownership and error recovery must use the same boundary.
pub(super) fn dispatch<F, Task>(
    manager: InstallationManager,
    id: &str,
    execute: F,
) -> Result<InstallationDraft, String>
where
    F: FnOnce(InstallationManager, InstallationDraft, watch::Receiver<bool>) -> Task
        + Send
        + 'static,
    Task: Future<Output = Result<(), String>> + Send + 'static,
{
    let (draft, cancel, active) = manager.reserve(id)?;
    let result = draft.clone();
    // Ownership outlives the IPC call and the modal. The guard recovers an interrupted
    // state even if this task unwinds; subprocess cleanup belongs to the session registry.
    // A synchronous Tauri command can arrive on the native UI thread. Dispatch
    // through Tauri's owned runtime instead of relying on a thread-local Tokio handle.
    tauri::async_runtime::spawn(async move {
        let _active = active;
        let result = execute(manager.clone(), draft.clone(), cancel).await;
        if let Err(error) = result {
            let _ = manager.update(&draft.id, |draft| {
                draft.status = InstallationStatus::Failed;
                draft.message = Some(error);
                Ok(())
            });
        }
    });
    Ok(result)
}

async fn run(
    manager: &InstallationManager,
    app: &AppHandle,
    game: &GameProcessHandle,
    sessions: &RunnerSessionRegistry,
    draft: &InstallationDraft,
    mut cancel: watch::Receiver<bool>,
) -> Result<(), String> {
    // Preparation is not dropped midway on cancellation: its transactional cleanup must finish.
    crate::tools::runners::ensure_selected_runtime_for_preparation(app, Some(&draft.runner), None)
        .await?;
    if *cancel.borrow() {
        return interrupted(manager, &draft.id);
    }
    let ctx = resolve_installation_wine_context(&draft.server_id, &draft.runner).await?;
    ensure_managed_path_safe(&ctx.location)?;
    if let Some(binding) = &draft.binding {
        validate_installation_context(binding, &ctx.identity)?;
    }
    if !Path::new(&draft.installer_path).is_file() {
        return Err("El instalador ya no existe".into());
    }
    let requirements = RuntimeRequirements {
        webview2: draft.require_webview2,
        dxvk: DxvkProvision::for_runner(&ctx.resolved),
    };
    if validate_runtime_prefix(&ctx).is_err()
        || !crate::tools::server_tools::missing_runtime_components_for_executable(
            Path::new(&draft.installer_path),
            Path::new(&ctx.prefix),
            draft.require_webview2,
        )
        .is_empty()
    {
        setup_runtime_prefix(app, game, sessions, &ctx, requirements).await?;
    }
    if *cancel.borrow() {
        return interrupted(manager, &draft.id);
    }
    // Prefix guard comes before any directory mapping or installer writes. Other prefixes
    // and clients remain independent; programs using this prefix cannot enter.
    let plan = resolve_operational_plan(OperationalRuntimeInput {
        server_runner: Some(&draft.runner),
        default_runner: None,
        context: &ctx,
        dgvoodoo: DgVoodooState::verified(false),
        webview2_required: draft.require_webview2,
    })
    .map_err(|error| error.code().to_string())?;
    let anchor = operational_session_anchor(&ctx, Some(&plan), draft.require_webview2);
    let mut op = RunnerOperation::begin(Some(app), sessions, game, &ctx, &anchor).await?;
    let _prefix = OperationGuard::acquire("prefix", Path::new(&ctx.prefix))?;
    let _destination = draft
        .destination_path
        .as_deref()
        .map(|path| OperationGuard::acquire("dgvoodoo", Path::new(path)))
        .transpose()?;
    validate_runtime_prefix(&ctx)?;
    let missing = crate::tools::server_tools::missing_runtime_components_for_executable(
        Path::new(&draft.installer_path),
        Path::new(&ctx.prefix),
        draft.require_webview2,
    );
    if !missing.is_empty() {
        return Err(missing.join(" · "));
    }
    if crate::tools::runner_sessions::prefix_has_active_programs(
        &ctx.prefix,
        op.lease().map(|lease| lease.supervisor_identity()),
    ) {
        return Err("Todavía hay programas usando este entorno. Ciérralos antes de volver a ejecutar el instalador".into());
    }
    if let Some(destination) = &draft.destination_path {
        map_destination(Path::new(&ctx.prefix), Path::new(destination))?;
    }
    let binding = InstallationBinding {
        installation_id: draft.id.clone(),
        prefix_path: ctx.prefix.clone(),
        prefix_fingerprint: ctx.identity.desired_fingerprint.digest.hex_digest(),
    };
    // Persist protection before running arbitrary installer code, including failure/cancel paths.
    write_protection(Path::new(&ctx.prefix), &binding)?;
    manager.update(&draft.id, |draft| {
        draft.binding = Some(binding);
        Ok(())
    })?;
    if *cancel.borrow() {
        op.quiesce().await?;
        return interrupted(manager, &draft.id);
    }
    let work_dir = Path::new(&draft.installer_path)
        .parent()
        .ok_or("El instalador no tiene carpeta")?;
    let mut invocation = ctx.resolved.tool_invocation(
        &ctx.prefix,
        &draft.installer_path,
        std::iter::empty::<&str>(),
        work_dir.to_string_lossy().as_ref(),
    )?;
    let graphics = InvocationPlan {
        target: InvocationTarget::Installer,
        plan: &plan,
    }
    .environment()
    .map_err(|error| format!("Entorno gráfico del instalador: {error:?}"))?;
    apply_graphics_environment_to_invocation(&mut invocation, &graphics)
        .map_err(|error| format!("Entorno gráfico del instalador: {error:?}"))?;
    let redactions: Vec<_> = std::iter::once(draft.installer_path.clone())
        .chain(draft.destination_path.iter().cloned())
        .collect();
    let spawn_result = op.spawn(invocation, &redactions).await;
    let mut spawned = match spawn_result {
        Ok(spawned) => spawned,
        Err(primary) => {
            return Err(match op.quiesce().await {
                Ok(()) => primary,
                Err(cleanup) => format!("{primary}. El cierre del entorno falló: {cleanup}"),
            });
        }
    };
    let mut drains = tokio::task::JoinSet::new();
    let (stdout, stderr) = spawned.take_direct_stdout_stderr();
    if let Some(mut pipe) = stdout {
        drains.spawn(async move { tokio::io::copy(&mut pipe, &mut tokio::io::sink()).await });
    }
    if let Some(mut pipe) = stderr {
        drains.spawn(async move { tokio::io::copy(&mut pipe, &mut tokio::io::sink()).await });
    }
    // A failed persistence write must still stop and reap the running process.
    let execution: Result<Option<i32>, String> = async {
        manager.update(&draft.id, |draft| {
            if draft.status != InstallationStatus::Stopping {
                draft.status = InstallationStatus::Running;
            }
            Ok(())
        })?;
        wait_installer(
            &mut spawned,
            &ctx.prefix,
            op.lease().map(|lease| lease.supervisor_identity()),
            &mut cancel,
        )
        .await
    }
    .await;
    let termination = if !matches!(execution, Ok(Some(_))) {
        spawned.terminate().await
    } else {
        Ok(())
    };
    let cleanup = op.quiesce().await;
    while let Some(result) = drains.join_next().await {
        let _ = result;
    }
    termination?;
    cleanup?;
    match execution? {
        None => interrupted(manager, &draft.id),
        Some(code) => record_installer_exit(manager, &draft.id, code),
    }
}

pub(super) fn record_installer_exit(
    manager: &InstallationManager,
    id: &str,
    code: i32,
) -> Result<(), String> {
    manager.update(id, |draft| {
        draft.exit_code = Some(code);
        draft.status = if code == 0 {
            InstallationStatus::AwaitingClient
        } else {
            InstallationStatus::Failed
        };
        draft.message = Some(if code == 0 {
            "El instalador se cerró. Esto no confirma que el juego esté instalado. Selecciona el cliente o vuelve a abrir el instalador".into()
        } else {
            format!("El instalador terminó con código {code}. Los archivos y el entorno se conservaron")
        });
        Ok(())
    })?;
    Ok(())
}

pub(super) async fn wait_installer(
    spawned: &mut SpawnedRunner,
    prefix: &str,
    supervisor: Option<ro_tools_linux::ProcessIdentity>,
    cancel: &mut watch::Receiver<bool>,
) -> Result<Option<i32>, String> {
    let code = tokio::select! {
        biased;
        _ = cancelled(cancel) => return Ok(None),
        result = spawned.wait() => result?,
    };
    // Controller exit is not wizard exit: retain ownership through handoff children.
    tokio::select! {
        biased;
        _ = cancelled(cancel) => Ok(None),
        _ = crate::tools::runner_sessions::wait_for_prefix_programs(prefix, supervisor) => Ok(Some(code)),
    }
}

async fn cancelled(cancel: &mut watch::Receiver<bool>) {
    loop {
        if *cancel.borrow_and_update() {
            return;
        }
        if cancel.changed().await.is_err() {
            return;
        }
    }
}

fn interrupted(manager: &InstallationManager, id: &str) -> Result<(), String> {
    manager.update(id, |draft| {
        draft.status = InstallationStatus::Interrupted;
        draft.message = Some("La instalación fue detenida. Puedes volver a ejecutarla; sus archivos y configuración se conservaron".into());
        Ok(())
    })?;
    Ok(())
}

pub(super) fn map_destination(prefix: &Path, destination: &Path) -> Result<(), String> {
    let mapping = prefix.join("dosdevices/r:");
    match mapping.symlink_metadata() {
        Ok(_) => {
            if let Ok(destination) = std::fs::canonicalize(destination) {
                if std::fs::canonicalize(&mapping).ok().as_ref() == Some(&destination) {
                    return Ok(());
                }
            }
            return Err(
                "La unidad R: ya apunta a otra carpeta; se conservó su configuración".into(),
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "No se pudo verificar la unidad R:; se conservó su configuración: {error}"
            ))
        }
    }
    std::fs::create_dir_all(destination)
        .map_err(|error| format!("No se pudo preparar la carpeta del juego: {error}"))?;
    let destination = std::fs::canonicalize(destination).map_err(|error| error.to_string())?;
    std::os::unix::fs::symlink(destination, mapping).map_err(|error| error.to_string())
}

pub(super) fn write_protection(prefix: &Path, binding: &InstallationBinding) -> Result<(), String> {
    let marker = prefix.join(INSTALLATION_MARKER);
    let mut marker_present = false;
    for existing in [&marker, &crate::utils::backup_path(&marker)] {
        match existing.symlink_metadata() {
            Ok(metadata) => {
                marker_present |= existing == &marker;
                if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 8192
                {
                    return Err(
                    "El recibo de instalación no es un archivo regular; se conservaron los datos"
                        .into(),
                );
                }
                let saved: InstallationBinding = serde_json::from_slice(
                    &std::fs::read(existing).map_err(|error| error.to_string())?,
                )
                .map_err(|_| "El recibo de instalación está dañado; se conservaron los datos")?;
                if saved != *binding {
                    return Err(
                        "El entorno está protegido por otra instalación; no se modificará".into(),
                    );
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                let message =
                    "No se pudo verificar el recibo de instalación; se conservaron los datos";
                return Err(format!("{message}: {error}"));
            }
        }
    }
    if !marker_present {
        replace_json(&marker, binding)?;
    }
    Ok(())
}

pub async fn confirm(
    manager: &InstallationManager,
    servers: &ServerRepository,
    notices: &StorageNotices,
    id: &str,
    executable_path: String,
    patcher_path: Option<String>,
) -> Result<ServerConfig, String> {
    // Reserve a separate admission lock so duplicate finalizations/retries cannot race
    // a new installer execution. Completed entries remain idempotently recoverable.
    let _admission = OperationGuard::acquire("installation-confirm", Path::new(id))?;
    let draft = manager.get(id)?;
    if manager.is_active(id)?
        || !matches!(
            draft.status,
            InstallationStatus::AwaitingClient | InstallationStatus::Completed
        )
    {
        return Err(
            "El instalador debe finalizar correctamente antes de agregar el servidor".into(),
        );
    }
    validate_exe(&executable_path)?;
    if std::fs::canonicalize(&executable_path).ok()
        == std::fs::canonicalize(&draft.installer_path).ok()
    {
        return Err("Selecciona el cliente instalado, no el instalador".into());
    }
    let ctx = resolve_installation_wine_context(&draft.server_id, &draft.runner).await?;
    let binding = draft
        .binding
        .clone()
        .ok_or("La instalación no conserva su entorno")?;
    validate_installation_context(&binding, &ctx.identity)?;
    let _prefix = OperationGuard::acquire("prefix", Path::new(&ctx.prefix))?;
    validate_runtime_prefix(&ctx)?;
    if crate::tools::runner_sessions::prefix_has_active_programs(&ctx.prefix, None) {
        return Err("Todavía hay programas usando el entorno de instalación".into());
    }
    let server = ServerConfig {
        id: draft.server_id.clone(),
        name: draft.name.clone(),
        executable_path,
        patcher_path,
        runner: Some(draft.runner.clone()),
        installation: Some(binding.clone()),
        prefix_mode: Some(PrefixMode::Isolated),
        wine_prefix: None,
        launch: Default::default(),
        autopot: Default::default(),
        autobuff: Default::default(),
        spammer: Default::default(),
    };
    server.validate_executable_available()?;
    if let Some(patcher) = &server.patcher_path {
        if !Path::new(patcher).is_file() {
            return Err("El patcher seleccionado no existe".into());
        }
    }
    write_protection(Path::new(&ctx.prefix), &binding)?;
    let server = servers.insert_installed(server, notices)?;
    // Server persistence comes first. If this write fails, a retry recognizes the same
    // installation without duplicating or replacing a user's existing server.
    manager.update(id, |draft| {
        draft.status = InstallationStatus::Completed;
        Ok(())
    })?;
    Ok(server)
}
