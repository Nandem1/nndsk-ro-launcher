use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use sha2::{Digest, Sha256, Sha512};
use tauri::AppHandle;
use tokio::io::AsyncWriteExt;

use super::descriptor::{ArtifactDescriptor, ExpectedDigest};
use super::source::{catalog_url, request_url_allowed};
use crate::utils::emit_progress;

pub(crate) async fn download_verified(
    app: &AppHandle,
    descriptor: &ArtifactDescriptor,
    destination: &Path,
    progress_start: u32,
    progress_end: u32,
) -> Result<(), String> {
    let url = catalog_url(descriptor);
    if !request_url_allowed(descriptor, url) {
        return Err(format!(
            "La URL de {} no está permitida por el catálogo",
            descriptor.id
        ));
    }

    let client = reqwest::Client::builder()
        .user_agent("nndsk-ro-launcher")
        .build()
        .map_err(|error| format!("No se pudo preparar HTTP: {error}"))?;
    let mut response = client
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| format!("No se pudo descargar {}: {error}", descriptor.id))?;
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
        .map_err(|error| format!("La descarga de {} se interrumpió: {error}", descriptor.id))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|error| format!("No se pudo guardar {}: {error}", descriptor.id))?;
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        if downloaded > descriptor.expected_size {
            return Err(format!(
                "La descarga de {} supera el tamaño oficial esperado",
                descriptor.id
            ));
        }
        let span = progress_end.saturating_sub(progress_start);
        let progress = progress_start
            + ((downloaded.min(descriptor.expected_size) * u64::from(span))
                / descriptor.expected_size) as u32;
        if progress != last_progress {
            emit_progress(
                app,
                &format!(
                    "Descargando {} · {} / {} MB",
                    descriptor.id,
                    downloaded / 1_048_576,
                    descriptor.expected_size / 1_048_576
                ),
                progress,
            )?;
            last_progress = progress;
        }
    }
    file.flush()
        .await
        .map_err(|error| format!("No se pudo finalizar {}: {error}", descriptor.id))?;
    file.sync_all()
        .await
        .map_err(|error| format!("No se pudo finalizar {}: {error}", descriptor.id))?;
    drop(file);

    if downloaded != descriptor.expected_size {
        return Err(format!(
            "Descarga incompleta de {}: {} bytes de {}",
            descriptor.id, downloaded, descriptor.expected_size
        ));
    }
    verify_archive_file(destination, descriptor.expected_size, descriptor.digest)?;
    Ok(())
}

pub(crate) fn verify_archive_file(
    path: &Path,
    expected_size: u64,
    digest: ExpectedDigest,
) -> Result<(), String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("No se pudo verificar {}: {error}", path.display()))?;
    if metadata.len() != expected_size {
        return Err(format!(
            "Tamaño inválido en {}: {} bytes de {}",
            path.display(),
            metadata.len(),
            expected_size
        ));
    }
    let actual = digest_file(path, digest)?;
    let expected = super::descriptor::expected_digest_hex(digest);
    if actual != expected {
        return Err(format!(
            "El checksum de {} no coincide; el archivo descargado no se instalará",
            path.display()
        ));
    }
    Ok(())
}

pub(crate) fn digest_file(path: &Path, expected: ExpectedDigest) -> Result<String, String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::artifacts::descriptor::ExpectedDigest;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_suffix() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
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
    fn verify_archive_file_rejects_truncated_and_wrong_digest() {
        let path = std::env::temp_dir().join(format!(
            "ro-launcher-verify-archive-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        std::fs::write(&path, b"short").unwrap();
        assert!(verify_archive_file(&path, 10, ExpectedDigest::Sha256("")).is_err());
        std::fs::write(&path, b"0123456789").unwrap();
        assert!(verify_archive_file(&path, 10, ExpectedDigest::Sha256("00")).is_err());
        assert!(verify_archive_file(
            &path,
            10,
            ExpectedDigest::Sha256(
                "84d89877f0d4041efb6bf91a16f0248f2fd573e6af05c19f96bedb9f882f7882",
            )
        )
        .is_ok());
        std::fs::remove_file(path).unwrap();
    }
}
