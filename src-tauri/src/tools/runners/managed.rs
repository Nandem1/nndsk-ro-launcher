use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use tauri::AppHandle;
use tokio::io::AsyncWriteExt;

use crate::utils::{app_data_dir, emit_log, emit_progress, replace_json, OperationGuard};

pub const MANAGED_RUNNER_ID: &str = "ro-proton-cachyos-11.0-20260702-slr";
pub const MANAGED_RUNNER_LABEL: &str = "proton-cachyos-11.0-20260702-slr-x86_64";

const RUNTIME_SCHEMA: u32 = 1;
const RUNTIME_DIR: &str = "runtime";
const MARKER_FILE: &str = ".ro-launcher-runtime.json";

const PROTON_ARCHIVE_NAME: &str = "proton-cachyos-11.0-20260702-slr-x86_64.tar.xz";
const PROTON_ARCHIVE_ROOT: &str = "proton-cachyos-11.0-20260702-slr-x86_64";
const PROTON_URL: &str = "https://github.com/CachyOS/proton-cachyos/releases/download/cachyos-11.0-20260702-slr/proton-cachyos-11.0-20260702-slr-x86_64.tar.xz";
const PROTON_SHA512: &str = "c8a050077b1d420e5b691dc487eaa998fe03b99b7e05e6ee3e16c8d4bd9f4c9ff5d9f80e5f6cd1a3f6bb5194bf1481fca9f91999f710d505b68ad97aa5592c7b";
const PROTON_SIZE: u64 = 328_233_608;

const UMU_ID: &str = "umu-launcher-1.4.0";
const UMU_ARCHIVE_NAME: &str = "umu-launcher-1.4.0-zipapp.tar";
const UMU_ARCHIVE_ROOT: &str = "umu";
const UMU_URL: &str = "https://github.com/Open-Wine-Components/umu-launcher/releases/download/1.4.0/umu-launcher-1.4.0-zipapp.tar";
const UMU_SHA256: &str = "138ce4b8843608a257d4bee88191ca78a989778bcefd8abb3c1d1aaac3ac6fb8";
const UMU_SIZE: u64 = 430_080;

#[derive(Clone, Copy)]
enum ArchiveKind {
    Tar,
    TarXz,
}

#[derive(Clone, Copy)]
enum ExpectedDigest {
    Sha256(&'static str),
    Sha512(&'static str),
}

#[derive(Clone, Copy)]
struct Artifact {
    id: &'static str,
    archive_name: &'static str,
    archive_root: &'static str,
    url: &'static str,
    digest: ExpectedDigest,
    size: u64,
    kind: ArchiveKind,
    executable: &'static str,
}

const UMU_ARTIFACT: Artifact = Artifact {
    id: UMU_ID,
    archive_name: UMU_ARCHIVE_NAME,
    archive_root: UMU_ARCHIVE_ROOT,
    url: UMU_URL,
    digest: ExpectedDigest::Sha256(UMU_SHA256),
    size: UMU_SIZE,
    kind: ArchiveKind::Tar,
    executable: "umu-run",
};

const PROTON_ARTIFACT: Artifact = Artifact {
    id: MANAGED_RUNNER_ID,
    archive_name: PROTON_ARCHIVE_NAME,
    archive_root: PROTON_ARCHIVE_ROOT,
    url: PROTON_URL,
    digest: ExpectedDigest::Sha512(PROTON_SHA512),
    size: PROTON_SIZE,
    kind: ArchiveKind::TarXz,
    executable: "proton",
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeMarker {
    schema_version: u32,
    artifact_id: String,
    digest: String,
}

pub fn managed_runtime_dir() -> PathBuf {
    app_data_dir().join(RUNTIME_DIR)
}

pub fn managed_runner_root() -> PathBuf {
    managed_runtime_dir().join(MANAGED_RUNNER_ID)
}

pub fn managed_proton_path() -> PathBuf {
    managed_runner_root().join("proton")
}

pub fn managed_umu_root() -> PathBuf {
    managed_runtime_dir().join(UMU_ID)
}

pub fn managed_umu_path() -> PathBuf {
    managed_umu_root().join("umu-run")
}

pub fn managed_runtime_ready() -> bool {
    artifact_ready(&UMU_ARTIFACT) && artifact_ready(&PROTON_ARTIFACT)
}

pub async fn ensure_managed_runtime(app: &AppHandle) -> Result<(), String> {
    let runtime_dir = managed_runtime_dir();
    std::fs::create_dir_all(&runtime_dir).map_err(|error| {
        format!(
            "No se pudo crear el directorio del runtime {}: {error}",
            runtime_dir.display()
        )
    })?;
    let _operation = OperationGuard::acquire("runtime", &runtime_dir)?;

    ensure_artifact(app, &UMU_ARTIFACT, 1, 5).await?;
    ensure_artifact(app, &PROTON_ARTIFACT, 6, 38).await?;
    emit_log(
        app,
        format!(
            "Runtime Ragnarok listo: {}",
            managed_runner_root().display()
        ),
    )?;
    Ok(())
}

async fn ensure_artifact(
    app: &AppHandle,
    artifact: &Artifact,
    progress_start: u32,
    progress_end: u32,
) -> Result<(), String> {
    if artifact_ready(artifact) {
        return Ok(());
    }

    let runtime_dir = managed_runtime_dir();
    let final_dir = runtime_dir.join(artifact.id);
    let unique = unique_suffix();
    let download_path = runtime_dir.join(format!(
        ".{}.download-{}-{unique}",
        artifact.archive_name,
        std::process::id()
    ));
    let staging_dir = runtime_dir.join(format!(
        ".{}.staging-{}-{unique}",
        artifact.id,
        std::process::id()
    ));

    let result = async {
        emit_log(app, format!("Descargando {}...", artifact.id))?;
        download_verified(
            app,
            artifact,
            &download_path,
            progress_start,
            progress_end.saturating_sub(2),
        )
        .await?;
        emit_progress(
            app,
            &format!("Verificando {}...", artifact.id),
            progress_end - 1,
        )?;

        let artifact_copy = *artifact;
        let download_copy = download_path.clone();
        let staging_copy = staging_dir.clone();
        tokio::task::spawn_blocking(move || {
            extract_artifact(&artifact_copy, &download_copy, &staging_copy)
        })
        .await
        .map_err(|error| format!("Falló la tarea de extracción: {error}"))??;

        emit_progress(app, &format!("Instalando {}...", artifact.id), progress_end)?;
        install_extracted(artifact, &staging_dir, &final_dir)
    }
    .await;
    let _ = std::fs::remove_file(&download_path);
    let _ = std::fs::remove_dir_all(&staging_dir);
    result
}

async fn download_verified(
    app: &AppHandle,
    artifact: &Artifact,
    destination: &Path,
    progress_start: u32,
    progress_end: u32,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent("nndsk-ro-launcher")
        .build()
        .map_err(|error| format!("No se pudo preparar HTTP: {error}"))?;
    let mut response = client
        .get(artifact.url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| format!("No se pudo descargar {}: {error}", artifact.id))?;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .await
        .map_err(|error| format!("No se pudo crear {}: {error}", destination.display()))?;

    let mut downloaded = 0_u64;
    let mut last_progress = progress_start.saturating_sub(1);
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("La descarga de {} se interrumpió: {error}", artifact.id))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|error| format!("No se pudo guardar {}: {error}", artifact.id))?;
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        if downloaded > artifact.size {
            return Err(format!(
                "La descarga de {} supera el tamaño oficial esperado",
                artifact.id
            ));
        }
        let span = progress_end.saturating_sub(progress_start);
        let progress = progress_start
            + ((downloaded.min(artifact.size) * u64::from(span)) / artifact.size) as u32;
        if progress != last_progress {
            emit_progress(
                app,
                &format!(
                    "Descargando {} · {} / {} MB",
                    artifact.id,
                    downloaded / 1_048_576,
                    artifact.size / 1_048_576
                ),
                progress,
            )?;
            last_progress = progress;
        }
    }
    file.flush()
        .await
        .map_err(|error| format!("No se pudo finalizar {}: {error}", artifact.id))?;
    file.sync_all()
        .await
        .map_err(|error| format!("No se pudo finalizar {}: {error}", artifact.id))?;
    drop(file);

    if downloaded != artifact.size {
        return Err(format!(
            "Descarga incompleta de {}: {} bytes de {}",
            artifact.id, downloaded, artifact.size
        ));
    }
    let actual = digest_file(destination, artifact.digest)?;
    if actual != expected_digest(artifact.digest) {
        return Err(format!(
            "El checksum de {} no coincide; el archivo descargado no se instalará",
            artifact.id
        ));
    }
    Ok(())
}

fn extract_artifact(
    artifact: &Artifact,
    archive_path: &Path,
    staging_dir: &Path,
) -> Result<(), String> {
    std::fs::create_dir(staging_dir)
        .map_err(|error| format!("No se pudo crear staging: {error}"))?;
    let file = File::open(archive_path)
        .map_err(|error| format!("No se pudo abrir {}: {error}", archive_path.display()))?;
    match artifact.kind {
        ArchiveKind::Tar => {
            let mut archive = tar::Archive::new(BufReader::new(file));
            archive
                .unpack(staging_dir)
                .map_err(|error| format!("No se pudo extraer {}: {error}", artifact.id))?;
        }
        ArchiveKind::TarXz => {
            let decoder = xz2::read::XzDecoder::new(BufReader::new(file));
            let mut archive = tar::Archive::new(decoder);
            archive
                .unpack(staging_dir)
                .map_err(|error| format!("No se pudo extraer {}: {error}", artifact.id))?;
        }
    }

    let extracted = staging_dir.join(artifact.archive_root);
    let executable = extracted.join(artifact.executable);
    if !extracted.is_dir() || !is_executable(&executable) {
        return Err(format!(
            "El artefacto {} no contiene {}",
            artifact.id,
            executable.display()
        ));
    }
    Ok(())
}

fn install_extracted(
    artifact: &Artifact,
    staging_dir: &Path,
    final_dir: &Path,
) -> Result<(), String> {
    let extracted = staging_dir.join(artifact.archive_root);
    let backup = final_dir.with_file_name(format!(".{}.previous-{}", artifact.id, unique_suffix()));
    let had_previous = final_dir.exists();
    if had_previous {
        std::fs::rename(final_dir, &backup)
            .map_err(|error| format!("No se pudo preservar el runtime anterior: {error}"))?;
    }

    let result = (|| {
        std::fs::rename(&extracted, final_dir)
            .map_err(|error| format!("No se pudo activar {}: {error}", artifact.id))?;
        replace_json(
            &final_dir.join(MARKER_FILE),
            &RuntimeMarker {
                schema_version: RUNTIME_SCHEMA,
                artifact_id: artifact.id.to_string(),
                digest: expected_digest(artifact.digest).to_string(),
            },
        )?;
        if !artifact_ready(artifact) {
            return Err(format!(
                "La instalación de {} no superó la validación",
                artifact.id
            ));
        }
        Ok(())
    })();

    match result {
        Ok(()) => {
            if had_previous {
                let _ = std::fs::remove_dir_all(backup);
            }
            Ok(())
        }
        Err(error) => {
            if final_dir.exists() {
                let _ = std::fs::remove_dir_all(final_dir);
            }
            if had_previous {
                let _ = std::fs::rename(&backup, final_dir);
            }
            Err(error)
        }
    }
}

fn artifact_ready(artifact: &Artifact) -> bool {
    let root = managed_runtime_dir().join(artifact.id);
    if root
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return false;
    }
    let marker = std::fs::read(root.join(MARKER_FILE))
        .ok()
        .and_then(|content| serde_json::from_slice::<RuntimeMarker>(&content).ok());
    marker.is_some_and(|marker| {
        marker.schema_version == RUNTIME_SCHEMA
            && marker.artifact_id == artifact.id
            && marker.digest == expected_digest(artifact.digest)
            && artifact_payload_ready(artifact, &root)
    })
}

fn artifact_payload_ready(artifact: &Artifact, root: &Path) -> bool {
    if !is_executable(&root.join(artifact.executable)) {
        return false;
    }
    if artifact.id != MANAGED_RUNNER_ID {
        return true;
    }

    let dxvk = root.join("files/lib/wine/dxvk");
    let dxvk_ready = ["x86_64-windows", "i386-windows"].iter().all(|arch| {
        ["d3d9.dll", "d3d11.dll", "dxgi.dll"]
            .iter()
            .all(|dll| dxvk.join(arch).join(dll).is_file())
    });
    let vkd3d = root.join("files/lib/vkd3d");
    let vkd3d_ready = ["x86_64-windows", "i386-windows"].iter().all(|arch| {
        [
            "libvkd3d-1.dll",
            "libvkd3d-shader-1.dll",
            "libvkd3d-utils-1.dll",
        ]
        .iter()
        .all(|dll| vkd3d.join(arch).join(dll).is_file())
    });

    is_executable(&root.join("files/bin/wine")) && dxvk_ready && vkd3d_ready
}

fn digest_file(path: &Path, expected: ExpectedDigest) -> Result<String, String> {
    let file = File::open(path)
        .map_err(|error| format!("No se pudo verificar {}: {error}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0_u8; 128 * 1024];
    match expected {
        ExpectedDigest::Sha256(_) => {
            let mut hasher = Sha256::new();
            loop {
                let read = reader
                    .read(&mut buffer)
                    .map_err(|error| error.to_string())?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
            }
            Ok(format!("{:x}", hasher.finalize()))
        }
        ExpectedDigest::Sha512(_) => {
            let mut hasher = Sha512::new();
            loop {
                let read = reader
                    .read(&mut buffer)
                    .map_err(|error| error.to_string())?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
            }
            Ok(format!("{:x}", hasher.finalize()))
        }
    }
}

fn expected_digest(digest: ExpectedDigest) -> &'static str {
    match digest {
        ExpectedDigest::Sha256(value) | ExpectedDigest::Sha512(value) => value,
    }
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.is_file()
        && path
            .metadata()
            .is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_proton_is_newer_than_the_broken_dxvk_snapshot() {
        assert_eq!(MANAGED_RUNNER_ID, "ro-proton-cachyos-11.0-20260702-slr");
        assert_eq!(
            MANAGED_RUNNER_LABEL,
            "proton-cachyos-11.0-20260702-slr-x86_64"
        );
        assert_eq!(PROTON_SHA512.len(), 128);
        assert_eq!(UMU_SHA256.len(), 64);
    }

    #[test]
    fn verifies_file_digests_before_installing() {
        let path = std::env::temp_dir().join(format!(
            "ro-launcher-runtime-digest-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        std::fs::write(&path, b"ragnarok").unwrap();

        assert_eq!(
            digest_file(&path, ExpectedDigest::Sha256("")).unwrap(),
            "ac3160b0a933ac03d7fb269baf8443e65936aa4322881e30c60443d7dda152d5"
        );
        assert_eq!(
            digest_file(&path, ExpectedDigest::Sha512("")).unwrap(),
            "aa4b54d2454a2f9c866028e9e37d373ec81b9710a68dfe418bd792d5081364ac2a6ef49e326e2df16725c7b0f27171f944bcd81dfbbc7855803d71791fad1b52"
        );

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn proton_readiness_requires_both_dxvk_architectures_and_d3d11() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "ro-launcher-runtime-payload-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        for executable in ["proton", "files/bin/wine"] {
            let path = root.join(executable);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"runner").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert!(!artifact_payload_ready(&PROTON_ARTIFACT, &root));

        for arch in ["x86_64-windows", "i386-windows"] {
            let dxvk = root.join("files/lib/wine/dxvk").join(arch);
            let vkd3d = root.join("files/lib/vkd3d").join(arch);
            std::fs::create_dir_all(&dxvk).unwrap();
            std::fs::create_dir_all(&vkd3d).unwrap();
            for dll in ["d3d9.dll", "d3d11.dll", "dxgi.dll"] {
                std::fs::write(dxvk.join(dll), b"dxvk").unwrap();
            }
            for dll in [
                "libvkd3d-1.dll",
                "libvkd3d-shader-1.dll",
                "libvkd3d-utils-1.dll",
            ] {
                std::fs::write(vkd3d.join(dll), b"vkd3d").unwrap();
            }
        }

        assert!(artifact_payload_ready(&PROTON_ARTIFACT, &root));
        std::fs::remove_dir_all(root).unwrap();
    }
}
