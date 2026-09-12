use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::models::runner::RunnerInfo;
use crate::tools::runners::{managed_proton_path, MANAGED_RUNNER_ID, MANAGED_RUNNER_LABEL};
use crate::utils::{app_data_dir, discovered_system_wines, is_executable_file, resolve_runner};

/// Expone primero el runtime administrado y después los runners compatibles ya instalados.
///
/// La ruta administrada se devuelve aunque todavía no se haya descargado: la instalación se
/// realiza bajo demanda al preparar el entorno del primer servidor.
pub fn discover_runners() -> Result<Vec<RunnerInfo>, String> {
    let managed_path = managed_proton_path();
    let mut runners = vec![RunnerInfo {
        id: MANAGED_RUNNER_ID.to_string(),
        name: MANAGED_RUNNER_LABEL.to_string(),
        path: managed_path.to_string_lossy().to_string(),
    }];
    let mut seen = HashSet::from([path_key(&managed_path)]);

    for path in installed_runner_paths() {
        let Some(path) = normalize_runner_path(path) else {
            continue;
        };
        if !seen.insert(path_key(&path)) {
            continue;
        }

        let path_string = path.to_string_lossy().to_string();
        runners.push(RunnerInfo {
            id: format!("external:{path_string}"),
            name: runner_name(&path),
            path: path_string,
        });
    }

    Ok(runners)
}

fn installed_runner_paths() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = discovered_system_wines()
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    paths.push(PathBuf::from("/opt/wine-cachyos/bin/wine"));

    // Runners portables administrados por el usuario/launcher. Sólo se inspecciona un nivel y
    // nunca se ejecuta ni modifica su contenido durante el descubrimiento.
    collect_portable_wine_paths(&app_data_dir().join("runners"), &mut paths);

    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return paths;
    };

    for root in [
        home.join(".local/share/Steam/compatibilitytools.d"),
        home.join(".steam/root/compatibilitytools.d"),
        home.join(".steam/steam/compatibilitytools.d"),
    ] {
        collect_child_runner_paths(&root, &mut paths);
    }

    for root in [
        home.join(".local/share/Steam/steamapps/common"),
        home.join(".steam/root/steamapps/common"),
    ] {
        collect_child_runner_paths(&root, &mut paths);
    }

    paths
}

fn collect_portable_wine_paths(root: &Path, paths: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            paths.push(path.join("bin/wine"));
        }
    }
}

fn collect_child_runner_paths(root: &Path, paths: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            paths.push(path.join("proton"));
        }
    }
}

fn normalize_runner_path(path: PathBuf) -> Option<PathBuf> {
    if !is_executable_file(&path) {
        return None;
    }
    let path = std::fs::canonicalize(&path).unwrap_or(path);
    resolve_runner(path.to_string_lossy().as_ref())
        .ok()
        .map(|_| path)
}

fn path_key(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

fn runner_name(path: &Path) -> String {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if file_name.starts_with("wine") {
        if let Some(root) = path.parent().and_then(Path::parent) {
            if root.starts_with(app_data_dir().join("runners")) {
                let name = root
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Wine portable");
                let mode = if root.join("lib/wine/i386-unix").is_dir() {
                    "old WoW64"
                } else {
                    "WoW64 desconocido"
                };
                return format!("{name} · portable · {mode}");
            }
        }
        if path.starts_with("/opt/wine-cachyos") {
            return "Wine CachyOS".to_string();
        }
        return "Wine del sistema".to_string();
    }

    let directory = path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("Proton");
    let version = path
        .parent()
        .and_then(|parent| std::fs::read_to_string(parent.join("version")).ok())
        .and_then(|content| {
            content
                .lines()
                .next()
                .map(str::trim)
                .filter(|version| !version.is_empty())
                .map(str::to_owned)
        });

    match version {
        Some(version) => format!("{directory} · {version}"),
        None => directory.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_the_managed_runtime_first() {
        let runners = discover_runners().unwrap();
        assert!(!runners.is_empty());
        assert_eq!(runners[0].id, MANAGED_RUNNER_ID);
        assert_eq!(runners[0].path, managed_proton_path().to_string_lossy());
    }
}
