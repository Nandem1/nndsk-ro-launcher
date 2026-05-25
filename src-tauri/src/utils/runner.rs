use std::path::{Path, PathBuf};

use crate::utils::prefix::effective_prefix;
use crate::utils::settings::effective_runner;

pub const PROTON_CACHYOS_SLR: &str = "proton-cachyos-slr";

pub struct ResolvedRunner {
    pub wine_bin: String,
    pub ld_library_path: Option<String>,
}

/// WINEPREFIX + runner resueltos para un servidor (o defaults globales).
pub struct WineContext {
    pub prefix: String,
    pub resolved: ResolvedRunner,
}

pub async fn resolve_wine_context(
    wine_prefix: Option<String>,
    runner: Option<String>,
) -> Result<WineContext, String> {
    Ok(WineContext {
        prefix: effective_prefix(wine_prefix),
        resolved: resolve_effective_runner(runner).await?,
    })
}

pub fn proton_runner_id(folder_name: &str) -> String {
    format!("proton-{folder_name}")
}

pub fn proton_search_dirs(home: &str) -> [String; 3] {
    [
        "/usr/share/steam/compatibilitytools.d".to_string(),
        format!("{home}/.local/share/Steam/compatibilitytools.d"),
        format!("{home}/.steam/root/compatibilitytools.d"),
    ]
}

pub fn canonical_runner_path(path: &Path) -> Option<PathBuf> {
    std::fs::canonicalize(path).ok()
}

pub async fn resolve_effective_runner(
    override_path: Option<String>,
) -> Result<ResolvedRunner, String> {
    let path = effective_runner(override_path).await?;
    resolve_runner(&path)
}

pub fn resolve_runner(runner_path: &str) -> Result<ResolvedRunner, String> {
    let path = Path::new(runner_path);

    if path.file_name().and_then(|name| name.to_str()) == Some("proton") {
        let proton_dir = path
            .parent()
            .ok_or_else(|| format!("Ruta Proton inválida: {runner_path}"))?;
        let files_dir = proton_dir.join("files");
        let wine_bin = files_dir.join("bin").join("wine");

        if !wine_bin.exists() {
            return Err(format!(
                "No se encontró el wine de Proton en {}",
                wine_bin.display()
            ));
        }

        let lib64 = files_dir.join("lib64");
        let lib = files_dir.join("lib");
        let ld_library_path = format!("{}:{}", lib64.display(), lib.display());

        return Ok(ResolvedRunner {
            wine_bin: wine_bin.to_string_lossy().to_string(),
            ld_library_path: Some(ld_library_path),
        });
    }

    if !path.exists() {
        return Err(format!("Runner no encontrado: {runner_path}"));
    }

    Ok(ResolvedRunner {
        wine_bin: runner_path.to_string(),
        ld_library_path: None,
    })
}
