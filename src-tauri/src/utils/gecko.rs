use std::path::{Path, PathBuf};
use tauri::AppHandle;

use crate::tools::artifacts::{
    download_pinned_https, pinned_https_url_allowed, verify_archive_file, ExpectedDigest,
    PinnedHttpsFile,
};
use crate::tools::runner_sessions::RunnerOperation;
use crate::utils::{app_data_dir, emit_log, emit_log_opt};

const GECKO_VERSION: &str = "2.47.4";
const GECKO_BASE_URL: &str = "https://dl.winehq.org/wine/wine-gecko";

const GECKO_X86_64: PinnedHttpsFile = PinnedHttpsFile {
    id: "wine-gecko-2.47.4-x86_64",
    url: "https://dl.winehq.org/wine/wine-gecko/2.47.4/wine-gecko-2.47.4-x86_64.msi",
    expected_size: 53_898_752,
    digest: ExpectedDigest::Sha256(
        "e590b7d988a32d6aa4cf1d8aa3aa3d33766fdd4cf4c89c2dcc2095ecb28d066f",
    ),
};

const GECKO_X86: PinnedHttpsFile = PinnedHttpsFile {
    id: "wine-gecko-2.47.4-x86",
    url: "https://dl.winehq.org/wine/wine-gecko/2.47.4/wine-gecko-2.47.4-x86.msi",
    expected_size: 55_187_968,
    digest: ExpectedDigest::Sha256(
        "26cecc47706b091908f7f814bddb074c61beb8063318e9efc5a7f789857793d6",
    ),
};

const GECKO_MSIS: &[PinnedHttpsFile] = &[GECKO_X86_64, GECKO_X86];

pub fn find_system_gecko_msis() -> Vec<PathBuf> {
    let search_dirs = [
        Path::new("/usr/share/wine/gecko"),
        Path::new("/usr/share/wine/wine/gecko"),
    ];
    collect_verified_catalog_msis(&search_dirs)
}

fn verified_system_msi(spec: &PinnedHttpsFile, search_dirs: &[&Path]) -> Option<PathBuf> {
    let file_name = gecko_file_name(spec);
    for dir in search_dirs {
        let path = dir.join(file_name);
        if verify_gecko_msi(&path, spec).is_ok() {
            return Some(path);
        }
    }
    None
}

fn collect_verified_catalog_msis(search_dirs: &[&Path]) -> Vec<PathBuf> {
    GECKO_MSIS
        .iter()
        .filter_map(|spec| verified_system_msi(spec, search_dirs))
        .collect()
}

fn gecko_file_name(spec: &PinnedHttpsFile) -> &'static str {
    spec.url
        .rsplit('/')
        .next()
        .expect("catalog gecko URL includes a file name")
}

fn verify_gecko_msi(path: &Path, spec: &PinnedHttpsFile) -> Result<(), String> {
    verify_archive_file(path, spec.expected_size, spec.digest)
}

pub fn check_gecko_installed(prefix_path: &str) -> bool {
    gecko_has_runtime(&format!("{prefix_path}/drive_c/windows/system32/gecko"))
        || gecko_has_runtime(&format!("{prefix_path}/drive_c/windows/syswow64/gecko"))
}

fn gecko_has_runtime(gecko_dir: &str) -> bool {
    dir_contains_xul(Path::new(gecko_dir))
}

fn dir_contains_xul(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }

    if dir.join("xul.dll").is_file() {
        return true;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && dir_contains_xul(&path) {
            return true;
        }
    }

    false
}

fn gecko_cache_dir() -> PathBuf {
    app_data_dir().join("cache/gecko")
}

async fn ensure_cached_gecko_msis(app: &AppHandle) -> Result<Vec<PathBuf>, String> {
    let system_msis = find_system_gecko_msis();
    let cache_dir = gecko_cache_dir();
    std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;

    let mut msis = Vec::new();
    for spec in GECKO_MSIS {
        let file_name = gecko_file_name(spec);
        if let Some(system) = system_msis
            .iter()
            .find(|path| path.file_name().and_then(|name| name.to_str()) == Some(file_name))
        {
            emit_log_opt(
                Some(app),
                format!(
                    "Wine Gecko ({file_name}): instalador verificado del sistema; sin descarga."
                ),
            );
            msis.push(system.clone());
            continue;
        }
        let dest = cache_dir.join(file_name);
        if verify_gecko_msi(&dest, spec).is_ok() {
            emit_log_opt(
                Some(app),
                format!("Wine Gecko ({file_name}): caché verificada; sin descarga."),
            );
            msis.push(dest);
            continue;
        }
        if dest.exists() {
            let _ = std::fs::remove_file(&dest);
        }
        if !pinned_https_url_allowed(spec, spec.url)
            || !spec.url.starts_with(GECKO_BASE_URL)
            || !spec.url.contains(GECKO_VERSION)
        {
            return Err(format!(
                "La URL de {} no está permitida por el catálogo",
                spec.id
            ));
        }
        emit_log(app, format!("Descargando Wine Gecko ({file_name})..."))?;
        if let Err(error) = download_pinned_https(spec, &dest).await {
            let _ = std::fs::remove_file(&dest);
            return Err(error);
        }
        msis.push(dest);
    }

    Ok(msis)
}

pub async fn install_gecko_for_runner(app: &AppHandle, op: &RunnerOperation) -> Result<(), String> {
    let ctx = op.ctx();
    let prefix_path = &ctx.prefix;
    let runner = &ctx.resolved;

    if runner.is_proton() {
        return Ok(());
    }

    if check_gecko_installed(prefix_path) {
        emit_log_opt(
            Some(app),
            "Wine Gecko ya está instalado en este entorno; sin descarga.",
        );
        return Ok(());
    }

    let msis = ensure_cached_gecko_msis(app).await?;

    emit_log(app, "Instalando Wine Gecko en el prefix...")?;

    for spec in GECKO_MSIS {
        let msi = msis
            .iter()
            .find(|path| {
                path.file_name().and_then(|name| name.to_str()) == Some(gecko_file_name(spec))
            })
            .ok_or_else(|| format!("Falta el instalador Gecko {}", spec.id))?;
        verify_gecko_msi(msi, spec)?;
        let msi_str = msi.to_string_lossy();
        op.run_ok(
            runner.builtin_invocation(prefix_path, "msiexec", ["/i", msi_str.as_ref(), "/qn"])?,
            &format!("wine msiexec {msi_str}"),
        )
        .await?;
    }

    if !check_gecko_installed(prefix_path) {
        return Err(
            "Wine Gecko no quedó instalado en el prefix. Intenta rearmar el WINEPREFIX."
                .to_string(),
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ro-launcher-gecko-{}-{}",
            std::process::id(),
            suffix
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn gecko_catalog_is_exact_https_winehq() {
        for spec in GECKO_MSIS {
            assert!(pinned_https_url_allowed(spec, spec.url));
            assert!(spec.url.starts_with(GECKO_BASE_URL));
            assert!(spec.url.contains(GECKO_VERSION));
            assert!(gecko_file_name(spec).ends_with(".msi"));
            assert!(!pinned_https_url_allowed(
                spec,
                &spec.url.replace("https://", "http://")
            ));
            assert!(!pinned_https_url_allowed(
                spec,
                "https://evil.example/x.msi"
            ));
        }
    }

    #[test]
    fn system_gecko_ignores_extra_and_unverified_msis() {
        let dir = unique_dir();
        std::fs::write(dir.join("evil.msi"), b"not-gecko").unwrap();
        std::fs::write(dir.join(gecko_file_name(&GECKO_X86_64)), b"wrong-bytes").unwrap();
        assert!(collect_verified_catalog_msis(&[dir.as_path()]).is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cached_gecko_with_wrong_digest_is_rejected() {
        let dir = unique_dir();
        let dest = dir.join(gecko_file_name(&GECKO_X86));
        std::fs::write(&dest, b"cached-poison").unwrap();
        assert!(verify_gecko_msi(&dest, &GECKO_X86).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
