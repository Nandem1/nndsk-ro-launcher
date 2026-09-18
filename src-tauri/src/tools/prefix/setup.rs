use std::path::Path;

use ro_tools_linux::find_prefix_processes;
use tauri::AppHandle;

use crate::state::GameProcessHandle;
use crate::tools::runner_sessions::RunnerOperation;
use crate::tools::runner_sessions::RunnerSessionRegistry;
use crate::tools::runners::{ensure_managed_dxvk, managed_dxvk_ready, managed_dxvk_root};
use crate::utils::audio;
use crate::utils::gecko::install_gecko_for_runner;
use crate::utils::{
    dxvk_cache_path, dxvk_config_path, dxvk_log_path, emit_log, emit_progress, inspect_prefix,
    resolve_runner, write_prefix_manifest, OperationGuard, PrefixManifest, ResolvedRunner,
    WineContext, WineSyncMode, PREFIX_SCHEMA_VERSION,
};

pub const MANAGED_DXVK_COMPONENT: &str = "dxvk-2.6.2";
const MANAGED_DXVK_DLLS: [&str; 5] = [
    "d3d8.dll",
    "d3d9.dll",
    "d3d10core.dll",
    "d3d11.dll",
    "dxgi.dll",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DxvkProvision {
    Runner,
    Winetricks,
    Managed,
}

impl DxvkProvision {
    pub fn for_runner(runner: &ResolvedRunner) -> Self {
        if runner.is_proton() {
            Self::Runner
        } else if runner.is_wine_7_16() {
            Self::Managed
        } else {
            Self::Winetricks
        }
    }

    pub fn manifest_component(self) -> &'static str {
        match self {
            Self::Managed => MANAGED_DXVK_COMPONENT,
            Self::Runner | Self::Winetricks => "dxvk",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeRequirements {
    pub webview2: bool,
    pub dxvk: DxvkProvision,
}

pub async fn setup_runtime_prefix(
    app: &AppHandle,
    game: &GameProcessHandle,
    sessions: &RunnerSessionRegistry,
    ctx: &WineContext,
    requirements: RuntimeRequirements,
) -> Result<(), String> {
    let root = Path::new(&ctx.prefix);
    let clean_managed_start = ctx.location.managed
        && (!root.exists()
            || (root.is_dir()
                && root
                    .read_dir()
                    .is_ok_and(|mut entries| entries.next().is_none())));

    let mut op = RunnerOperation::begin(Some(app), sessions, game, ctx).await?;
    let _operation = OperationGuard::acquire("prefix", root)?;

    let result = async {
        setup_resolved_prefix(app, &op, requirements).await?;
        write_runtime_manifest(ctx, requirements)
    }
    .await;

    if result.is_err() && clean_managed_start && root.exists() && !root.is_symlink() {
        match op.quiesce_for_restore().await {
            Ok(()) => {
                if let Err(error) = std::fs::remove_dir_all(root) {
                    let _ = emit_log(
                        app,
                        format!(
                            "No se pudo limpiar el entorno inicial incompleto {}: {error}",
                            root.display()
                        ),
                    );
                }
            }
            Err(error) => {
                let _ = emit_log(
                    app,
                    format!(
                        "El entorno inicial incompleto se conservó porque no pudo apagarse con seguridad: {error}"
                    ),
                );
            }
        }
    }
    result
}

pub async fn reset_runtime_prefix(
    app: &AppHandle,
    game: &GameProcessHandle,
    sessions: &RunnerSessionRegistry,
    ctx: &WineContext,
    requirements: RuntimeRequirements,
) -> Result<(), String> {
    emit_progress(app, "Preparando reconstrucción del entorno...", 40)?;

    let mut op = RunnerOperation::begin(Some(app), sessions, game, ctx).await?;
    let _operation = OperationGuard::acquire("prefix", Path::new(&ctx.prefix))?;

    shutdown_existing_prefix_for_reset(app, &op, ctx).await?;

    let root = Path::new(&ctx.prefix);
    let backup = if root.exists() {
        let backup = reset_backup_path(root)?;
        emit_log(
            app,
            format!("Preservando entorno anterior en {}...", backup.display()),
        )?;
        std::fs::rename(root, &backup)
            .map_err(|error| format!("No se pudo preservar el entorno anterior: {error}"))?;
        Some(backup)
    } else {
        None
    };

    let result = async {
        setup_resolved_prefix(app, &op, requirements).await?;
        write_runtime_manifest(ctx, requirements)
    }
    .await;

    match result {
        Ok(()) => {
            if let Some(backup) = backup {
                if let Err(error) = std::fs::remove_dir_all(&backup) {
                    emit_log(
                        app,
                        format!(
                            "El entorno nuevo está listo, pero el respaldo quedó en {}: {error}",
                            backup.display()
                        ),
                    )?;
                }
            }
            Ok(())
        }
        Err(error) => {
            if let Err(cleanup_error) = op.quiesce_for_restore().await {
                let backup_note = backup
                    .as_ref()
                    .map(|path| {
                        format!(
                            " El entorno anterior sigue preservado en {}.",
                            path.display()
                        )
                    })
                    .unwrap_or_default();
                return Err(format!(
                    "{error}. No se restauró el entorno anterior porque el entorno nuevo no pudo apagarse por completo: {cleanup_error}.{backup_note}"
                ));
            }
            if root.exists() {
                if root.is_symlink() {
                    return Err(format!(
                        "{error}. No se restauró el entorno anterior porque el entorno nuevo fue reemplazado por un enlace simbólico"
                    ));
                }
                std::fs::remove_dir_all(root).map_err(|remove_error| {
                    format!(
                        "{error}. No se pudo retirar el entorno nuevo; el respaldo anterior se conservó: {remove_error}"
                    )
                })?;
            }
            if let Some(backup) = backup {
                std::fs::rename(&backup, root).map_err(|restore_error| {
                    format!(
                        "{error}. Además no se pudo restaurar {}: {restore_error}",
                        backup.display()
                    )
                })?;
                Err(format!(
                    "{error}. El entorno anterior fue restaurado correctamente"
                ))
            } else {
                Err(error)
            }
        }
    }
}

async fn setup_resolved_prefix(
    app: &AppHandle,
    op: &RunnerOperation,
    requirements: RuntimeRequirements,
) -> Result<(), String> {
    let ctx = op.ctx();
    let prefix = &ctx.prefix;
    let resolved = &ctx.resolved;

    emit_progress(app, "Creando entorno aislado...", 40)?;

    if prefix_has_state(prefix) && !find_prefix_processes(prefix).is_empty() {
        let invocation = resolved.shutdown_invocation(prefix)?;
        op.run_shutdown_ok(invocation, "apagado del entorno")
            .await?;
    }
    std::fs::create_dir_all(prefix).map_err(|e| e.to_string())?;

    emit_progress(app, "Inicializando entorno...", 45)?;
    op.run_ok(
        resolved.create_prefix_invocation(prefix)?,
        "inicialización del prefix",
    )
    .await?;

    emit_progress(app, "Preparando Wine Gecko...", 50)?;
    install_gecko_for_runner(app, op).await?;

    emit_progress(app, "Preparando gráficos...", 55)?;
    match requirements.dxvk {
        DxvkProvision::Runner => emit_log(app, "DXVK administrado por Proton/UMU.")?,
        DxvkProvision::Winetricks => run_winetricks(app, op, &["dxvk"]).await?,
        DxvkProvision::Managed => {
            ensure_managed_dxvk(app).await?;
            install_managed_dxvk(app, prefix)?;
        }
    }

    emit_progress(app, "Instalando vcredist_2019...", 65)?;
    run_winetricks(app, op, &["vcrun2019"]).await?;

    emit_progress(app, "Instalando d3dx9...", 75)?;
    run_winetricks(app, op, &["d3dx9"]).await?;

    if requirements.webview2 {
        if !resolved.supports_winetricks_verb("webview2") {
            return Err(
                "Este cliente requiere WebView2, pero el winetricks del runner no ofrece ese componente"
                    .to_string(),
            );
        }
        emit_progress(app, "Instalando Microsoft Edge WebView2...", 82)?;
        install_webview2(app, op).await?;
    }

    emit_progress(app, "Instalando corefonts...", 88)?;
    run_winetricks(app, op, &["corefonts"]).await?;

    configure_ui_font_fallback(app, op).await?;

    emit_progress(app, "Configurando audio...", 96)?;
    audio::ensure_audio_driver(Some(app), op).await?;

    emit_progress(app, "¡Listo!", 100)?;
    Ok(())
}

fn reset_backup_path(prefix: &Path) -> Result<std::path::PathBuf, String> {
    let parent = prefix
        .parent()
        .ok_or_else(|| "La ruta del entorno no tiene directorio padre".to_string())?;
    let name = prefix
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "La ruta del entorno no tiene nombre válido".to_string())?;
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let backup = parent.join(format!(
        ".{name}.reset-backup-{}-{unique}",
        std::process::id()
    ));
    if backup.exists() {
        return Err(format!(
            "Ya existe el respaldo temporal {}",
            backup.display()
        ));
    }
    Ok(backup)
}

async fn configure_ui_font_fallback(app: &AppHandle, op: &RunnerOperation) -> Result<(), String> {
    let ctx = op.ctx();
    emit_progress(app, "Configurando fuentes de interfaz...", 93)?;
    for family in ["Segoe UI", "Segoe UI Semibold"] {
        op.run_ok(
            ctx.resolved.builtin_invocation(
                &ctx.prefix,
                "reg",
                [
                    "add",
                    r"HKCU\Software\Wine\Fonts\Replacements",
                    "/v",
                    family,
                    "/t",
                    "REG_SZ",
                    "/d",
                    "Arial",
                    "/f",
                ],
            )?,
            "configuración de fuente Segoe UI",
        )
        .await?;
    }
    Ok(())
}

fn write_runtime_manifest(
    ctx: &WineContext,
    requirements: RuntimeRequirements,
) -> Result<(), String> {
    let mut components = vec![
        "vcrun2019".to_string(),
        "d3dx9".to_string(),
        "corefonts".to_string(),
        "font-fallbacks".to_string(),
    ];
    components.push(requirements.dxvk.manifest_component().to_string());
    if requirements.webview2 {
        components.push("webview2".to_string());
    }
    let runner_path = std::fs::canonicalize(ctx.resolved.runner_path())
        .unwrap_or_else(|_| ctx.resolved.runner_path().to_path_buf());
    write_prefix_manifest(
        &ctx.prefix,
        &PrefixManifest {
            schema_version: PREFIX_SCHEMA_VERSION,
            scope: ctx.location.scope,
            server_id: ctx.location.server_id.clone(),
            runner_kind: ctx.resolved.kind_label().to_string(),
            runner_path: runner_path.to_string_lossy().to_string(),
            components,
        },
    )
}

fn install_managed_dxvk(app: &AppHandle, prefix: &str) -> Result<(), String> {
    if !managed_dxvk_ready() {
        return Err(
            "El runtime administrado no contiene DXVK 2.6.2 completo para x86 y x86_64".to_string(),
        );
    }

    let prefix_root = Path::new(prefix);
    let windows = prefix_root.join("drive_c/windows");
    let system_reg = std::fs::read_to_string(prefix_root.join("system.reg"))
        .map_err(|error| format!("No se pudo leer la arquitectura del prefix: {error}"))?;
    let prefix_arch = system_reg
        .lines()
        .find_map(|line| line.strip_prefix("#arch="))
        .ok_or_else(|| "El prefix no declara #arch en system.reg".to_string())?;

    let source = managed_dxvk_root();
    let targets = match prefix_arch {
        "win64" => vec![
            (source.join("x64"), windows.join("system32")),
            (source.join("x32"), windows.join("syswow64")),
        ],
        "win32" => vec![(source.join("x32"), windows.join("system32"))],
        other => {
            return Err(format!(
                "Arquitectura de prefix no soportada por DXVK: {other}"
            ))
        }
    };

    for (source_dir, target_dir) in targets {
        std::fs::create_dir_all(&target_dir).map_err(|error| {
            format!(
                "No se pudo preparar el destino DXVK {}: {error}",
                target_dir.display()
            )
        })?;
        for dll in MANAGED_DXVK_DLLS {
            install_runtime_file(&source_dir.join(dll), &target_dir.join(dll))?;
        }
    }

    let config = dxvk_config_path(prefix);
    let logs = dxvk_log_path(prefix);
    let cache = dxvk_cache_path(prefix);
    let state_root = config
        .parent()
        .ok_or_else(|| "La ruta de configuración DXVK no tiene padre".to_string())?;
    std::fs::create_dir_all(state_root)
        .map_err(|error| format!("No se pudo crear el estado de DXVK: {error}"))?;
    std::fs::create_dir_all(&logs)
        .map_err(|error| format!("No se pudo crear el directorio de logs DXVK: {error}"))?;
    std::fs::create_dir_all(&cache)
        .map_err(|error| format!("No se pudo crear el cache DXVK: {error}"))?;
    std::fs::write(
        &config,
        "# RO-Launcher · DXVK 2.6.2 + Wine 7.16 old-WoW64\nd3d9.forceSamplerTypeSpecConstants = True\n",
    )
    .map_err(|error| format!("No se pudo escribir dxvk.conf: {error}"))?;

    emit_log(
        app,
        format!("DXVK 2.6.2 instalado para {prefix_arch}; Vulkan moderno y old WoW64 preservado."),
    )
}

fn install_runtime_file(source: &Path, destination: &Path) -> Result<(), String> {
    if !source.is_file() || source.is_symlink() {
        return Err(format!(
            "El componente DXVK no es un archivo regular: {}",
            source.display()
        ));
    }
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("Destino DXVK inválido: {}", destination.display()))?;
    let temporary =
        destination.with_file_name(format!(".{file_name}.ro-launcher-{}", std::process::id()));
    if temporary.exists() {
        std::fs::remove_file(&temporary).map_err(|error| {
            format!(
                "No se pudo limpiar el staging DXVK {}: {error}",
                temporary.display()
            )
        })?;
    }
    std::fs::copy(source, &temporary).map_err(|error| {
        format!(
            "No se pudo copiar {} a {}: {error}",
            source.display(),
            temporary.display()
        )
    })?;
    std::fs::rename(&temporary, destination).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        format!(
            "No se pudo activar {} en el prefix: {error}",
            destination.display()
        )
    })
}

async fn run_winetricks(
    app: &AppHandle,
    op: &RunnerOperation,
    packages: &[&str],
) -> Result<(), String> {
    let ctx = op.ctx();
    let _ = app;
    op.run_ok(
        ctx.resolved
            .winetricks_invocation(&ctx.prefix, packages.iter().copied())?,
        "winetricks",
    )
    .await
}

fn needs_legacy_webview2_install_mode(is_wine_7_16: bool, sync_mode: WineSyncMode) -> bool {
    is_wine_7_16 && sync_mode != WineSyncMode::WineServer
}

async fn set_windows_version(
    app: &AppHandle,
    op: &RunnerOperation,
    version: &str,
) -> Result<(), String> {
    let ctx = op.ctx();
    let _ = app;
    op.run_ok(
        ctx.resolved
            .builtin_invocation(&ctx.prefix, "winecfg", ["-v", version])?,
        &format!("configuración temporal de Windows {version}"),
    )
    .await
}

async fn install_webview2(app: &AppHandle, op: &RunnerOperation) -> Result<(), String> {
    let resolved = &op.ctx().resolved;
    if !needs_legacy_webview2_install_mode(resolved.is_wine_7_16(), resolved.wine_sync_mode()) {
        return run_winetricks(app, op, &["webview2"]).await;
    }

    emit_log(
        app,
        "WebView2: usando Windows 7 temporal para instalar la rama 109 compatible con Wine 7.16.",
    )?;
    set_windows_version(app, op, "win7").await?;

    let install_result = run_winetricks(app, op, &["webview2"]).await;
    let restore_result = set_windows_version(app, op, "win10").await;

    match (install_result, restore_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(install_error), Ok(())) => Err(install_error),
        (Ok(()), Err(restore_error)) => Err(format!(
            "WebView2 se instaló, pero no se pudo restaurar Windows 10: {restore_error}"
        )),
        (Err(install_error), Err(restore_error)) => Err(format!(
            "{install_error}; además no se pudo restaurar Windows 10: {restore_error}"
        )),
    }
}

fn prefix_has_state(prefix_path: &str) -> bool {
    let path = Path::new(prefix_path);
    if !path.is_dir() {
        return false;
    }
    path.read_dir()
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false)
}

async fn shutdown_existing_prefix_for_reset(
    app: &AppHandle,
    op: &RunnerOperation,
    ctx: &WineContext,
) -> Result<(), String> {
    if find_prefix_processes(&ctx.prefix).is_empty() {
        emit_log(
            app,
            "El entorno ya estaba detenido; se puede rearmar sin invocar su runner anterior.",
        )?;
        return Ok(());
    }

    op.run_shutdown_ok(
        ctx.resolved.shutdown_invocation(&ctx.prefix)?,
        "apagado del entorno",
    )
    .await?;
    if find_prefix_processes(&ctx.prefix).is_empty() {
        return Ok(());
    }

    let health = inspect_prefix(&ctx.prefix);
    let recorded = health.manifest.as_ref().filter(|manifest| {
        manifest.schema_version == PREFIX_SCHEMA_VERSION && manifest.runner_kind != "unknown"
    });

    match plan_reset_shutdown(
        find_prefix_processes(&ctx.prefix).len(),
        recorded.map(|manifest| ResetManifestView {
            runner_kind: &manifest.runner_kind,
            runner_path: &manifest.runner_path,
            resolved: recorded.and_then(|manifest| {
                resolve_runner(&manifest.runner_path)
                    .ok()
                    .map(|runner| (runner.kind_label().to_string(), runner.runner_path().to_path_buf()))
            }),
            resolve_error: recorded.and_then(|manifest| {
                resolve_runner(&manifest.runner_path)
                    .err()
            }),
            current_runner_path: ctx.resolved.runner_path(),
        }),
    ) {
        ResetShutdownDecision::NothingToDo => Ok(()),
        ResetShutdownDecision::ShutdownForeign => {
            let manifest = recorded.expect("foreign requires manifest");
            emit_log(
                app,
                format!(
                    "Deteniendo el entorno con su runner original: {}",
                    manifest.runner_path
                ),
            )?;
            let runner = resolve_runner(&manifest.runner_path)?;
            op.run_shutdown_ok(runner.shutdown_invocation(&ctx.prefix)?, "apagado del entorno")
                .await?;
            if find_prefix_processes(&ctx.prefix).is_empty() {
                Ok(())
            } else {
                Err(
                    "No se pudo detener el entorno antes de rearmarlo; cierra los procesos activos"
                        .to_string(),
                )
            }
        }
        ResetShutdownDecision::KindMismatch => Err(format!(
            "El runner registrado {} ya no coincide con su tipo; cierra todos los procesos del entorno antes de rearmarlo",
            recorded.expect("mismatch requires manifest").runner_path
        )),
        ResetShutdownDecision::Unresolvable { error } => Err(format!(
            "Hay procesos activos en el entorno y no se pudo resolver su runner original: {error}"
        )),
        ResetShutdownDecision::OriginalGone => {
            emit_log(
                app,
                "El runner original ya no existe y no hay procesos activos; se omitió su apagado.",
            )?;
            Ok(())
        }
        ResetShutdownDecision::LegacyActive { count } => Err(format!(
            "El entorno no registra qué runner lo creó y aún tiene {count} proceso(s) activo(s). Ciérralos antes de rearmar"
        )),
        ResetShutdownDecision::StillActive => Err(
            "No se pudo detener el entorno antes de rearmarlo; cierra los procesos activos"
                .to_string(),
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ResetShutdownDecision {
    NothingToDo,
    ShutdownForeign,
    KindMismatch,
    Unresolvable { error: String },
    OriginalGone,
    LegacyActive { count: usize },
    StillActive,
}

struct ResetManifestView<'a> {
    runner_kind: &'a str,
    runner_path: &'a str,
    resolved: Option<(String, std::path::PathBuf)>,
    resolve_error: Option<String>,
    current_runner_path: &'a Path,
}

fn plan_reset_shutdown(
    remaining: usize,
    manifest: Option<ResetManifestView<'_>>,
) -> ResetShutdownDecision {
    if remaining == 0 {
        return ResetShutdownDecision::NothingToDo;
    }
    let Some(view) = manifest else {
        return ResetShutdownDecision::LegacyActive { count: remaining };
    };
    match (&view.resolved, &view.resolve_error) {
        (Some((kind, _)), _) if kind != view.runner_kind => ResetShutdownDecision::KindMismatch,
        (Some((_, path)), _) if path != view.current_runner_path => {
            ResetShutdownDecision::ShutdownForeign
        }
        (Some(_), _) => ResetShutdownDecision::StillActive,
        (None, Some(error)) if remaining > 0 => ResetShutdownDecision::Unresolvable {
            error: error.clone(),
        },
        (None, _) if remaining == 0 => ResetShutdownDecision::OriginalGone,
        (None, _) => ResetShutdownDecision::Unresolvable {
            error: format!("No se pudo resolver {}", view.runner_path),
        },
    }
}

#[allow(dead_code)] // usado en tests; la lógica de apagado vive en RunnerOperation
fn shutdown_is_complete(status_success: bool, active_processes: usize) -> bool {
    status_success || active_processes == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_dir(label: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "ro-launcher-prefix-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    fn wine_runner(root: &Path, version: &str) -> ResolvedRunner {
        let wine = root.join("bin/wine");
        let wineserver = root.join("bin/wineserver");
        fs::create_dir_all(wine.parent().unwrap()).unwrap();
        fs::write(&wine, format!("#!/bin/sh\nprintf '%s\\n' '{version}'\n")).unwrap();
        fs::set_permissions(&wine, fs::Permissions::from_mode(0o755)).unwrap();
        ResolvedRunner::test_wine(wine, wineserver)
    }

    #[test]
    fn prefix_state_requires_at_least_one_entry() {
        let path = test_dir("state");
        fs::create_dir_all(&path).unwrap();
        assert!(!prefix_has_state(path.to_str().unwrap()));

        fs::write(path.join("system.reg"), "test").unwrap();
        assert!(prefix_has_state(path.to_str().unwrap()));

        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn legacy_dxvk_provision_is_characterized_for_each_runner_family() {
        let root = test_dir("dxvk-policy");
        let wine_716 = wine_runner(&root.join("wine-716"), "wine-7.16");
        let current_wine = wine_runner(&root.join("wine-current"), "wine-10.0");
        let proton = ResolvedRunner::test_proton(
            root.join("proton/proton"),
            root.join("proton"),
            root.join("umu-run"),
        );

        assert_eq!(DxvkProvision::for_runner(&proton), DxvkProvision::Runner);
        assert_eq!(DxvkProvision::for_runner(&wine_716), DxvkProvision::Managed);
        assert_eq!(
            DxvkProvision::for_runner(&current_wine),
            DxvkProvision::Winetricks
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reset_shutdown_empty_is_nothing_to_do() {
        assert_eq!(
            plan_reset_shutdown(0, None),
            ResetShutdownDecision::NothingToDo
        );
    }

    #[test]
    fn reset_shutdown_foreign_manifest_uses_recorded_runner() {
        let current = std::path::PathBuf::from("/runners/new/wine");
        let recorded = std::path::PathBuf::from("/runners/old/wine");
        let view = ResetManifestView {
            runner_kind: "wine",
            runner_path: "/runners/old/wine",
            resolved: Some(("wine".into(), recorded)),
            resolve_error: None,
            current_runner_path: &current,
        };
        assert_eq!(
            plan_reset_shutdown(2, Some(view)),
            ResetShutdownDecision::ShutdownForeign
        );
    }

    #[test]
    fn reset_shutdown_kind_mismatch_is_error() {
        let current = std::path::PathBuf::from("/runners/new/wine");
        let recorded = std::path::PathBuf::from("/runners/proton/proton");
        let view = ResetManifestView {
            runner_kind: "wine",
            runner_path: "/runners/proton/proton",
            resolved: Some(("proton".into(), recorded)),
            resolve_error: None,
            current_runner_path: &current,
        };
        assert_eq!(
            plan_reset_shutdown(1, Some(view)),
            ResetShutdownDecision::KindMismatch
        );
    }

    #[test]
    fn reset_shutdown_legacy_without_manifest_errors() {
        assert_eq!(
            plan_reset_shutdown(3, None),
            ResetShutdownDecision::LegacyActive { count: 3 }
        );
    }

    #[test]
    fn shutdown_is_idempotent_when_the_prefix_is_already_stopped() {
        assert!(shutdown_is_complete(false, 0));
        assert!(shutdown_is_complete(true, 1));
        assert!(!shutdown_is_complete(false, 1));
    }

    #[test]
    fn webview2_legacy_mode_is_limited_to_accelerated_wine_7_16() {
        assert!(needs_legacy_webview2_install_mode(
            true,
            WineSyncMode::Fsync
        ));
        assert!(needs_legacy_webview2_install_mode(
            true,
            WineSyncMode::Esync
        ));
        assert!(!needs_legacy_webview2_install_mode(
            true,
            WineSyncMode::WineServer
        ));
        assert!(!needs_legacy_webview2_install_mode(
            false,
            WineSyncMode::Fsync
        ));
    }
}
