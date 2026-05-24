use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use tokio::process::Command;

use crate::commands::check::get_prefix_path;
use crate::commands::runner::resolve_runner;
use crate::commands::settings::load_settings;
use crate::models::server::ServerConfig;

const DGVOODOO_TEMPLATE_FILES: &[&str] = &[
    "D3DImm.dll",
    "DDraw.dll",
    "dgVoodoo.conf",
    "dgVoodooCpl.exe",
];

const DGVOODOO_REQUIRED_FILES: &[&str] = &["D3DImm.dll", "DDraw.dll", "dgVoodoo.conf"];

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub found: bool,
    pub path: Option<String>,
    pub label: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DgVoodooStatus {
    pub cpl: ToolInfo,
    pub d3dimm_dll: ToolInfo,
    pub ddraw_dll: ToolInfo,
    pub conf: ToolInfo,
    pub configured: bool,
    pub needs_install: bool,
    pub can_auto_install: bool,
    pub can_uninstall: bool,
    pub issues: Vec<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstallDgVoodooResult {
    pub installed: Vec<String>,
    pub status: ServerToolsStatus,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UninstallDgVoodooResult {
    pub removed: Vec<String>,
    pub status: ServerToolsStatus,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ServerToolsStatus {
    pub game_dir: String,
    pub open_setup: ToolInfo,
    pub patcher: ToolInfo,
    pub dgvoodoo: DgVoodooStatus,
}

#[tauri::command]
pub async fn scan_server_tools(
    app: AppHandle,
    server: ServerConfig,
) -> Result<ServerToolsStatus, String> {
    let game_dir = game_dir_from_server(&server)?;
    let can_auto_install = dgvoodoo_template_dir(&app).is_ok();
    scan_game_dir(&game_dir, &server, can_auto_install)
}

#[tauri::command]
pub async fn install_dgvoodoo(
    app: AppHandle,
    server: ServerConfig,
) -> Result<InstallDgVoodooResult, String> {
    let game_dir = game_dir_from_server(&server)?;
    let installed = install_dgvoodoo_files(&app, Path::new(&game_dir))?;
    let can_auto_install = dgvoodoo_template_dir(&app).is_ok();
    let status = scan_game_dir(&game_dir, &server, can_auto_install)?;
    Ok(InstallDgVoodooResult { installed, status })
}

#[tauri::command]
pub async fn uninstall_dgvoodoo(
    app: AppHandle,
    server: ServerConfig,
) -> Result<UninstallDgVoodooResult, String> {
    let game_dir = game_dir_from_server(&server)?;
    let removed = uninstall_dgvoodoo_files(Path::new(&game_dir))?;
    let can_auto_install = dgvoodoo_template_dir(&app).is_ok();
    let status = scan_game_dir(&game_dir, &server, can_auto_install)?;
    Ok(UninstallDgVoodooResult { removed, status })
}

#[tauri::command]
pub async fn launch_server_tool(
    app: AppHandle,
    server: ServerConfig,
    tool: String,
    runner: Option<String>,
) -> Result<(), String> {
    let status = scan_server_tools(app, server.clone()).await?;
    let exe_path = match tool.as_str() {
        "opensetup" => status
            .open_setup
            .path
            .ok_or_else(|| "OpenSetup no encontrado".to_string())?,
        "patcher" => status
            .patcher
            .path
            .ok_or_else(|| "Patcher no encontrado".to_string())?,
        "dgvoodoo" => status
            .dgvoodoo
            .cpl
            .path
            .ok_or_else(|| "dgVoodoo Control Panel no encontrado".to_string())?,
        other => return Err(format!("Herramienta desconocida: {other}")),
    };

    let prefix_path = server
        .wine_prefix
        .clone()
        .unwrap_or_else(get_prefix_path);

    let runner_path = match runner.or_else(|| server.runner.clone()) {
        Some(path) if !path.is_empty() => path,
        _ => load_settings().await?.default_runner,
    };
    let resolved = resolve_runner(&runner_path)?;

    let work_dir = Path::new(&exe_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| game_dir_from_server(&server).unwrap_or_default());

    let mut cmd = Command::new(&resolved.wine_bin);
    cmd.arg(&exe_path)
        .env("WINEPREFIX", &prefix_path)
        .env("WAYLAND_DISPLAY", "")
        .env("DXVK_ASYNC", "1")
        .env("WINE_LARGE_ADDRESS_AWARE", "1")
        .current_dir(&work_dir);

    if tool == "dgvoodoo" || tool == "opensetup" {
        cmd.env("WINEDLLOVERRIDES", "d3dimm=n,b;ddraw=n,b");
    }

    if let Some(proton_libs) = &resolved.ld_library_path {
        let ld_library_path = match std::env::var("LD_LIBRARY_PATH") {
            Ok(existing) if !existing.is_empty() => format!("{proton_libs}:{existing}"),
            _ => proton_libs.clone(),
        };
        cmd.env("LD_LIBRARY_PATH", ld_library_path);
    }

    cmd.spawn()
        .map_err(|e| format!("Error al abrir la herramienta: {e}"))?;

    Ok(())
}

fn game_dir_from_server(server: &ServerConfig) -> Result<String, String> {
    let exe = Path::new(&server.executable_path);
    exe.parent()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Ruta del ejecutable inválida".to_string())
}

fn scan_game_dir(
    game_dir: &str,
    server: &ServerConfig,
    can_auto_install: bool,
) -> Result<ServerToolsStatus, String> {
    let dir = Path::new(game_dir);
    if !dir.is_dir() {
        return Err(format!("Carpeta del juego no encontrada: {game_dir}"));
    }

    let open_setup = detect_open_setup(dir);
    let patcher = detect_patcher(dir, server);
    let dgvoodoo = detect_dgvoodoo(dir, can_auto_install);

    Ok(ServerToolsStatus {
        game_dir: game_dir.to_string(),
        open_setup,
        patcher,
        dgvoodoo,
    })
}

/// Prioridad: opensetup.exe > setup.exe (muchos clientes traen ambos).
fn detect_open_setup(dir: &Path) -> ToolInfo {
    let opensetup = find_file_case_insensitive(dir, "opensetup.exe");
    let setup = find_file_case_insensitive(dir, "setup.exe");

    if let Some(path) = opensetup {
        let label = if setup.is_some() {
            format!("{} (+ setup.exe)", file_label(&path))
        } else {
            file_label(&path)
        };
        return tool_found(path, Some(label));
    }

    if let Some(path) = setup {
        let label = file_label(&path);
        return tool_found(path, Some(label));
    }

    tool_missing()
}

fn detect_patcher(dir: &Path, server: &ServerConfig) -> ToolInfo {
    if let Some(saved) = &server.patcher_path {
        let path = PathBuf::from(saved);
        if path.is_file() {
            let label = file_label(&path);
            return tool_found(path, Some(label));
        }
    }

    let mut candidates: Vec<String> = Vec::new();
    let name_norm = normalize_token(&server.name);
    if !name_norm.is_empty() {
        candidates.push(format!("{name_norm}patcher.exe"));
        candidates.push(format!("{name_norm}_patcher.exe"));
    }

    if let Some(stem) = Path::new(&server.executable_path)
        .file_stem()
        .and_then(|s| s.to_str())
    {
        let stem_norm = normalize_token(stem);
        if !stem_norm.is_empty() && stem_norm != name_norm {
            candidates.push(format!("{stem_norm}patcher.exe"));
            candidates.push(format!("{stem_norm}_patcher.exe"));
        }
    }

    for candidate in candidates {
        if let Some(path) = find_file_case_insensitive(dir, &candidate) {
            let label = file_label(&path);
            return tool_found(path, Some(label));
        }
    }

    if let Some(path) = find_matching_exe(dir, |name| {
        name.contains("patcher") && !name.ends_with(".tmp")
    }) {
        let label = file_label(&path);
        return tool_found(path, Some(label));
    }

    tool_missing()
}

fn detect_dgvoodoo(dir: &Path, can_auto_install: bool) -> DgVoodooStatus {
    let cpl = find_file_case_insensitive(dir, "dgvoodoocpl.exe")
        .map(|path| {
            let label = file_label(&path);
            tool_found(path, Some(label))
        })
        .unwrap_or_else(tool_missing);

    let d3dimm_dll = find_file_case_insensitive(dir, "d3dimm.dll")
        .map(|path| tool_found(path, None))
        .unwrap_or_else(tool_missing);

    let ddraw_dll = find_file_case_insensitive(dir, "ddraw.dll")
        .map(|path| tool_found(path, None))
        .unwrap_or_else(tool_missing);

    let conf = find_file_case_insensitive(dir, "dgvoodoo.conf")
        .map(|path| tool_found(path, None))
        .unwrap_or_else(tool_missing);

    let mut issues = Vec::new();

    if !d3dimm_dll.found {
        issues.push("Falta D3DImm.dll (wrapper de dgVoodoo)".to_string());
    }
    if !ddraw_dll.found {
        issues.push("Falta DDraw.dll (wrapper de dgVoodoo)".to_string());
    }
    if !conf.found {
        issues.push("Falta dgVoodoo.conf".to_string());
    }
    if let Some(conf_path) = &conf.path {
        if let Ok(content) = std::fs::read_to_string(conf_path) {
            validate_dgvoodoo_conf(&content, &mut issues);
        }
    }

    let configured = d3dimm_dll.found && ddraw_dll.found && conf.found && issues.is_empty();
    let needs_install = !DGVOODOO_REQUIRED_FILES.iter().all(|file| {
        find_file_case_insensitive(dir, file).is_some()
    });
    let can_uninstall = DGVOODOO_TEMPLATE_FILES.iter().any(|file| {
        find_file_case_insensitive(dir, file).is_some()
    });

    DgVoodooStatus {
        cpl,
        d3dimm_dll,
        ddraw_dll,
        conf,
        configured,
        needs_install,
        can_auto_install,
        can_uninstall,
        issues,
    }
}

fn dgvoodoo_template_dir(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let bundled = resource_dir.join("dgvoodoo");
        if template_is_complete(&bundled) {
            return Ok(bundled);
        }
    }

    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/dgvoodoo");
    if template_is_complete(&dev) {
        return Ok(dev);
    }

    Err("Plantilla dgVoodoo no encontrada en el launcher".to_string())
}

fn template_is_complete(dir: &Path) -> bool {
    dir.is_dir()
        && DGVOODOO_TEMPLATE_FILES
            .iter()
            .all(|file| dir.join(file).is_file())
}

fn install_dgvoodoo_files(app: &AppHandle, game_dir: &Path) -> Result<Vec<String>, String> {
    let template_dir = dgvoodoo_template_dir(app)?;
    let mut installed = Vec::new();

    for file in DGVOODOO_TEMPLATE_FILES {
        if find_file_case_insensitive(game_dir, file).is_some() {
            continue;
        }

        let source = template_dir.join(file);
        let dest = game_dir.join(file);
        std::fs::copy(&source, &dest)
            .map_err(|e| format!("No se pudo copiar {file}: {e}"))?;
        installed.push((*file).to_string());
    }

    Ok(installed)
}

fn uninstall_dgvoodoo_files(game_dir: &Path) -> Result<Vec<String>, String> {
    let mut removed = Vec::new();

    for file in DGVOODOO_TEMPLATE_FILES {
        let Some(path) = find_file_case_insensitive(game_dir, file) else {
            continue;
        };

        std::fs::remove_file(&path).map_err(|e| {
            format!(
                "No se pudo eliminar {}: {e}",
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(file)
            )
        })?;
        removed.push(
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file)
                .to_string(),
        );
    }

    if removed.is_empty() {
        return Err("No hay archivos dgVoodoo para desinstalar".to_string());
    }

    Ok(removed)
}

fn validate_dgvoodoo_conf(content: &str, issues: &mut Vec<String>) {
    if conf_value_anywhere(content, "Version").is_none() {
        issues.push("dgVoodoo.conf no parece válido (falta Version)".to_string());
    }

    if let Some(output_api) = conf_value(content, "General", "OutputAPI") {
        let api = output_api.to_ascii_lowercase();
        if api.is_empty() || api == "disabled" {
            issues.push("OutputAPI no está configurado en dgVoodoo.conf".to_string());
        }
    } else {
        issues.push("OutputAPI no definido en dgVoodoo.conf".to_string());
    }

    if let Some(pass_through) = conf_value(content, "DirectX", "DisableAndPassThru") {
        if pass_through.eq_ignore_ascii_case("true") {
            issues.push(
                "DisableAndPassThru está activo — dgVoodoo no interceptará DirectX".to_string(),
            );
        }
    }
}

fn conf_value_anywhere(content: &str, key: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('[') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim().eq_ignore_ascii_case(key) {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

fn conf_value(content: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let current = &line[1..line.len() - 1];
            in_section = current.eq_ignore_ascii_case(section);
            continue;
        }
        if in_section {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim().eq_ignore_ascii_case(key) {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

fn find_file_case_insensitive(dir: &Path, filename: &str) -> Option<PathBuf> {
    let target = filename.to_ascii_lowercase();
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case(&target))
            {
                return Some(path);
            }
        }
    }
    None
}

fn find_matching_exe(dir: &Path, predicate: impl Fn(&str) -> bool) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut matches: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| predicate(&n.to_ascii_lowercase()))
        })
        .collect();

    matches.sort_by_key(|p| p.file_name().map(|n| n.to_ascii_lowercase()));
    matches.into_iter().next()
}

fn normalize_token(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("desconocido")
        .to_string()
}

fn tool_found(path: PathBuf, label: Option<String>) -> ToolInfo {
    ToolInfo {
        found: true,
        path: Some(path.to_string_lossy().to_string()),
        label,
    }
}

fn tool_missing() -> ToolInfo {
    ToolInfo {
        found: false,
        path: None,
        label: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_osro_sample_folder() {
        let server = ServerConfig {
            id: "test".to_string(),
            name: "OsRO MR".to_string(),
            executable_path: "/home/nndsk/Downloads/OsRO MR Full v4.3/OsRO MR Full v4.3/OldschoolRO [MR]/OsRO Midrate.exe".to_string(),
            patcher_path: None,
            wine_prefix: None,
            runner: None,
        };

        let game_dir = game_dir_from_server(&server).unwrap();
        let dev_template =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/dgvoodoo");
        let can_auto_install = template_is_complete(&dev_template);
        let status = scan_game_dir(&game_dir, &server, can_auto_install).unwrap();

        assert!(status.open_setup.found);
        assert!(status.patcher.found);
        assert!(status.dgvoodoo.d3dimm_dll.found);
        assert!(status.dgvoodoo.ddraw_dll.found);
        assert!(status.dgvoodoo.conf.found);
        assert!(status.dgvoodoo.cpl.found);
        assert!(status.dgvoodoo.configured, "{:?}", status.dgvoodoo.issues);
    }

    #[test]
    fn open_setup_prioritizes_opensetup_when_both_exist() {
        let dir = std::env::temp_dir().join(format!(
            "ro-launcher-opensetup-test-{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("setup.exe"), b"").unwrap();
        std::fs::write(dir.join("opensetup.exe"), b"").unwrap();

        let info = detect_open_setup(&dir);
        let path = info.path.expect("debe encontrar opensetup");
        assert!(
            path.to_ascii_lowercase().contains("opensetup.exe"),
            "path inesperado: {path}"
        );
        assert_eq!(
            info.label.as_deref(),
            Some("opensetup.exe (+ setup.exe)")
        );

        let _ = std::fs::remove_file(dir.join("setup.exe"));
        let _ = std::fs::remove_file(dir.join("opensetup.exe"));
        let _ = std::fs::remove_dir(&dir);
    }
}
