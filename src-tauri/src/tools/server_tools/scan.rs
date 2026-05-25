use std::path::{Path, PathBuf};

use ro_tools_core::dgvoodoo::{validate_conf, REQUIRED_FILES, TEMPLATE_FILES};

use crate::models::server::ServerConfig;
use crate::models::server_tools::{DgVoodooStatus, ServerToolsStatus, ToolInfo};
use crate::utils::{file_label, find_file_case_insensitive, find_matching_exe, normalize_token};

pub fn scan_game_dir(
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
pub fn detect_open_setup(dir: &Path) -> ToolInfo {
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

pub fn detect_patcher(dir: &Path, server: &ServerConfig) -> ToolInfo {
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

pub fn detect_dgvoodoo(dir: &Path, can_auto_install: bool) -> DgVoodooStatus {
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
            validate_conf(&content, &mut issues);
        }
    }

    let configured = d3dimm_dll.found && ddraw_dll.found && conf.found && issues.is_empty();
    let needs_install = !REQUIRED_FILES
        .iter()
        .all(|file| find_file_case_insensitive(dir, file).is_some());
    let can_uninstall = TEMPLATE_FILES
        .iter()
        .any(|file| find_file_case_insensitive(dir, file).is_some());

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
    use crate::tools::server_tools::dgvoodoo;

    #[test]
    fn scan_osro_sample_folder() {
        let server = ServerConfig {
            id: "test".to_string(),
            name: "OsRO MR".to_string(),
            executable_path: "/home/nndsk/Downloads/OsRO MR Full v4.3/OsRO MR Full v4.3/OldschoolRO [MR]/OsRO Midrate.exe".to_string(),
            patcher_path: None,
            wine_prefix: None,
            runner: None,
            autopot: Default::default(),
        };

        let game_dir = crate::utils::required_game_dir(&server.executable_path).unwrap();
        let dev_template =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/dgvoodoo");
        let can_auto_install = dgvoodoo::template_is_complete(&dev_template);
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
