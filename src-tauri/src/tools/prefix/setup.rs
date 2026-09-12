use std::path::Path;

use tauri::AppHandle;

use crate::tools::runners::{managed_dxvk_sarek_ready, managed_dxvk_sarek_root};
use crate::utils::audio;
use crate::utils::gecko::install_gecko_for_runner;
use crate::utils::process::run_logged_command_ok;
use crate::utils::{
    dxvk_sarek_cache_path, dxvk_sarek_config_path, dxvk_sarek_log_path, emit_log, emit_progress,
    inspect_prefix, resolve_runner, write_prefix_manifest, PrefixManifest, ResolvedRunner,
    WineContext, PREFIX_SCHEMA_VERSION,
};

pub const DXVK_SAREK_COMPONENT: &str = "dxvk-sarek-1.10.x";
const DXVK_SAREK_DLLS: [&str; 5] = [
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
    Sarek,
}

impl DxvkProvision {
    pub fn for_runner(runner: &ResolvedRunner) -> Self {
        if runner.is_proton() {
            Self::Runner
        } else if runner.is_wine_7_16() {
            Self::Sarek
        } else {
            Self::Winetricks
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
    let result = async {
        setup_resolved_prefix(app, &ctx.prefix, &ctx.resolved, requirements).await?;
        write_runtime_manifest(ctx, requirements)
    }
    .await;

    if result.is_err() && clean_managed_start && root.exists() && !root.is_symlink() {
        let _ = shutdown_runner(&ctx.prefix, &ctx.resolved).await;
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
    result
}

pub async fn setup_resolved_prefix(
    app: &AppHandle,
    prefix: &str,
    resolved: &ResolvedRunner,
    requirements: RuntimeRequirements,
) -> Result<(), String> {
    emit_progress(app, "Creando entorno aislado...", 40)?;

    if prefix_has_state(prefix) {
        shutdown_runner(prefix, resolved).await?;
    }
    std::fs::create_dir_all(prefix).map_err(|e| e.to_string())?;

    emit_progress(app, "Inicializando entorno...", 45)?;
    run_logged_command_ok(
        app,
        resolved.create_prefix_command(prefix),
        "inicialización del prefix",
    )
    .await?;

    emit_progress(app, "Preparando Wine Gecko...", 50)?;
    install_gecko_for_runner(app, prefix, resolved).await?;

    emit_progress(app, "Preparando gráficos...", 55)?;
    match requirements.dxvk {
        DxvkProvision::Runner => emit_log(app, "DXVK administrado por Proton/UMU.")?,
        DxvkProvision::Winetricks => run_winetricks(app, prefix, resolved, &["dxvk"]).await?,
        DxvkProvision::Sarek => install_dxvk_sarek(app, prefix)?,
    }

    emit_progress(app, "Instalando vcredist_2019...", 65)?;
    run_winetricks(app, prefix, resolved, &["vcrun2019"]).await?;

    emit_progress(app, "Instalando d3dx9...", 75)?;
    run_winetricks(app, prefix, resolved, &["d3dx9"]).await?;

    if requirements.webview2 {
        if !resolved.supports_winetricks_verb("webview2") {
            return Err(
                "Este cliente requiere WebView2, pero el winetricks del runner no ofrece ese componente"
                    .to_string(),
            );
        }
        emit_progress(app, "Instalando Microsoft Edge WebView2...", 82)?;
        run_winetricks(app, prefix, resolved, &["webview2"]).await?;
    }

    emit_progress(app, "Instalando corefonts...", 88)?;
    run_winetricks(app, prefix, resolved, &["corefonts"]).await?;

    configure_ui_font_fallback(app, prefix, resolved).await?;

    emit_progress(app, "Configurando audio...", 96)?;
    audio::ensure_audio_driver(Some(app), prefix, resolved).await?;

    emit_progress(app, "¡Listo!", 100)?;
    Ok(())
}

pub async fn reset_runtime_prefix(
    app: &AppHandle,
    ctx: &WineContext,
    requirements: RuntimeRequirements,
) -> Result<(), String> {
    emit_progress(app, "Preparando reconstrucción del entorno...", 40)?;
    if prefix_has_state(&ctx.prefix) {
        shutdown_existing_prefix_for_reset(app, ctx).await?;
    }

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
        setup_resolved_prefix(app, &ctx.prefix, &ctx.resolved, requirements).await?;
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
            if root.exists() {
                let _ = std::fs::remove_dir_all(root);
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

async fn shutdown_existing_prefix_for_reset(
    app: &AppHandle,
    ctx: &WineContext,
) -> Result<(), String> {
    if ro_tools_linux::find_prefix_processes(&ctx.prefix).is_empty() {
        emit_log(
            app,
            "El entorno ya estaba detenido; se puede rearmar sin invocar su runner anterior.",
        )?;
        return Ok(());
    }

    let health = inspect_prefix(&ctx.prefix);
    let recorded = health.manifest.as_ref().filter(|manifest| {
        manifest.schema_version == PREFIX_SCHEMA_VERSION && manifest.runner_kind != "unknown"
    });

    if let Some(manifest) = recorded {
        match resolve_runner(&manifest.runner_path) {
            Ok(runner) if runner.kind_label() == manifest.runner_kind => {
                emit_log(
                    app,
                    format!(
                        "Deteniendo el entorno con su runner original: {}",
                        manifest.runner_path
                    ),
                )?;
                return shutdown_runner(&ctx.prefix, &runner).await;
            }
            Ok(_) => {
                return Err(format!(
                    "El runner registrado {} ya no coincide con su tipo; cierra todos los procesos del entorno antes de rearmarlo",
                    manifest.runner_path
                ));
            }
            Err(error) if !ro_tools_linux::find_prefix_processes(&ctx.prefix).is_empty() => {
                return Err(format!(
                    "Hay procesos activos en el entorno y no se pudo resolver su runner original: {error}"
                ));
            }
            Err(_) => {
                emit_log(
                    app,
                    "El runner original ya no existe y no hay procesos activos; se omitió su apagado.",
                )?;
                return Ok(());
            }
        }
    }

    let active = ro_tools_linux::find_prefix_processes(&ctx.prefix);
    if active.is_empty() {
        emit_log(
            app,
            "El entorno legacy no tiene procesos activos; se puede rearmar sin ejecutar un runner desconocido.",
        )?;
        Ok(())
    } else {
        Err(format!(
            "El entorno no registra qué runner lo creó y aún tiene {} proceso(s) activo(s). Ciérralos antes de rearmar",
            active.len()
        ))
    }
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

async fn configure_ui_font_fallback(
    app: &AppHandle,
    prefix: &str,
    runner: &ResolvedRunner,
) -> Result<(), String> {
    emit_progress(app, "Configurando fuentes de interfaz...", 93)?;
    for family in ["Segoe UI", "Segoe UI Semibold"] {
        let command = runner.builtin_command(
            prefix,
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
        );
        run_logged_command_ok(app, command, "configuración de fuente Segoe UI").await?;
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
    match requirements.dxvk {
        DxvkProvision::Sarek => components.push(DXVK_SAREK_COMPONENT.to_string()),
        DxvkProvision::Runner | DxvkProvision::Winetricks => components.push("dxvk".to_string()),
    }
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

fn install_dxvk_sarek(app: &AppHandle, prefix: &str) -> Result<(), String> {
    if !managed_dxvk_sarek_ready() {
        return Err(
            "El runtime administrado no contiene DXVK-Sarek 1.10.x completo para x86 y x86_64"
                .to_string(),
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

    let source = managed_dxvk_sarek_root();
    let targets = match prefix_arch {
        "win64" => vec![
            (source.join("x86_64-windows"), windows.join("system32")),
            (source.join("i386-windows"), windows.join("syswow64")),
        ],
        "win32" => vec![(source.join("i386-windows"), windows.join("system32"))],
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
        for dll in DXVK_SAREK_DLLS {
            install_runtime_file(&source_dir.join(dll), &target_dir.join(dll))?;
        }
    }

    let config = dxvk_sarek_config_path(prefix);
    let logs = dxvk_sarek_log_path(prefix);
    let cache = dxvk_sarek_cache_path(prefix);
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
        "# RO-Launcher · Wine 7.16 legacy\nd3d9.forceSamplerTypeSpecConstants = True\n",
    )
    .map_err(|error| format!("No se pudo escribir dxvk.conf: {error}"))?;

    emit_log(
        app,
        format!(
            "DXVK-Sarek 1.10.x instalado para {prefix_arch}; Vulkan activo y old WoW64 preservado."
        ),
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
    prefix_path: &str,
    runner: &ResolvedRunner,
    packages: &[&str],
) -> Result<(), String> {
    let command = runner.winetricks_command(prefix_path, packages.iter().copied());
    run_logged_command_ok(app, command, "winetricks").await
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

async fn shutdown_runner(prefix: &str, runner: &ResolvedRunner) -> Result<(), String> {
    let status = runner
        .shutdown_command(prefix)
        .status()
        .await
        .map_err(|error| format!("No se pudo detener {}: {error}", runner.kind_label()))?;
    // `wineserver -k` puede devolver 1 cuando la sesión ya terminó entre la detección y
    // el comando. El apagado es idempotente: si no queda ningún proceso del prefix, se logró
    // el estado solicitado aunque el ejecutable haya informado un código distinto de cero.
    let active_processes = ro_tools_linux::find_prefix_processes(prefix).len();
    if !shutdown_is_complete(status.success(), active_processes) {
        return Err(format!(
            "{} no pudo detener el entorno (código {})",
            runner.kind_label(),
            status.code().unwrap_or(-1)
        ));
    }
    Ok(())
}

fn shutdown_is_complete(status_success: bool, active_processes: usize) -> bool {
    status_success || active_processes == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
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
    fn shutdown_is_idempotent_when_the_prefix_is_already_stopped() {
        assert!(shutdown_is_complete(false, 0));
        assert!(shutdown_is_complete(true, 1));
        assert!(!shutdown_is_complete(false, 1));
    }
}
