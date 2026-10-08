use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use crate::utils::OperationGuard;

use super::descriptor::{catalog_descriptor, ArtifactDescriptor, MANAGED_RUNNER_ID};
use super::{
    artifact_cache_state_at, extract, fetch, install, runtime_dir, unique_suffix,
    ArtifactCacheState,
};

/// Import the single checksum-pinned preservation build, not an arbitrary runner archive.
/// The blocking owner retains both the runtime lock and cleanup if the caller is cancelled.
pub(crate) async fn import_managed_runtime_archive(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("El archivo del runtime requiere una ruta absoluta".to_string());
    }
    let source = path.to_path_buf();
    let destination = runtime_dir();
    let descriptor = *catalog_descriptor(MANAGED_RUNNER_ID)
        .ok_or_else(|| "El runtime principal no está en el catálogo".to_string())?;
    tokio::task::spawn_blocking(move || {
        import_archive_at(&descriptor, &source, &destination).map(|root| root.join("proton"))
    })
    .await
    .map_err(|error| format!("Falló la tarea de importación: {error}"))?
}

fn import_archive_at(
    descriptor: &ArtifactDescriptor,
    source: &Path,
    destination: &Path,
) -> Result<PathBuf, String> {
    let _operation = OperationGuard::acquire("runtime", destination)?;
    let input = open_regular_archive(source, descriptor.expected_size)?;
    std::fs::create_dir_all(destination)
        .map_err(|error| format!("No se pudo preparar el directorio de runtimes: {error}"))?;
    if !destination
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
    {
        return Err(
            "El directorio de runtimes no puede ser un enlace ni otro tipo de archivo".to_string(),
        );
    }
    let unique = unique_suffix();
    let snapshot = destination.join(format!(
        ".{}.import-{}-{unique}",
        descriptor.id,
        std::process::id()
    ));
    let staging = destination.join(format!(
        ".{}.staging-{}-{unique}",
        descriptor.id,
        std::process::id()
    ));
    let mut cleanup = ImportCleanup::default();
    snapshot_archive(input, &snapshot, descriptor, &mut cleanup)?;

    let final_dir = destination.join(descriptor.id);
    if artifact_cache_state_at(descriptor, &final_dir) == ArtifactCacheState::Ready {
        return Ok(final_dir);
    }
    match final_dir.symlink_metadata() {
        Ok(_) => {
            return Err(format!("{} ya existe pero no coincide con el runtime validado; se conservó sin sustituirlo", final_dir.display()));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("No se pudo comprobar el destino: {error}")),
    }
    create_owned_staging(&staging, &mut cleanup)?;
    extract::extract_created_archive(descriptor, &snapshot, &staging)?;
    install::install_new_extracted(descriptor, &staging, &final_dir, |descriptor| {
        artifact_cache_state_at(descriptor, &final_dir) == ArtifactCacheState::Ready
    })?;
    Ok(final_dir)
}

fn open_regular_archive(path: &Path, expected_size: u64) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| format!("No se pudo abrir un archivo regular del runtime: {error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("No se pudo comprobar el archivo del runtime: {error}"))?;
    if !metadata.is_file() {
        return Err(
            "El runtime debe ser un archivo regular, no un enlace, dispositivo ni FIFO".to_string(),
        );
    }
    if metadata.len() != expected_size {
        return Err(format!(
            "Tamaño inválido del runtime: {} bytes de {expected_size}",
            metadata.len()
        ));
    }
    Ok(file)
}

fn snapshot_archive(
    input: File,
    snapshot: &Path,
    descriptor: &ArtifactDescriptor,
    cleanup: &mut ImportCleanup,
) -> Result<(), String> {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(snapshot)
        .map_err(|error| format!("No se pudo crear el snapshot del runtime: {error}"))?;
    cleanup.snapshot = Some(snapshot.to_path_buf());
    let copied = std::io::copy(
        &mut input.take(descriptor.expected_size.saturating_add(1)),
        &mut output,
    )
    .map_err(|error| format!("No se pudo copiar el runtime: {error}"))?;
    output
        .flush()
        .and_then(|_| output.sync_all())
        .map_err(|error| format!("No se pudo finalizar el snapshot del runtime: {error}"))?;
    drop(output);
    if copied != descriptor.expected_size {
        return Err("El archivo del runtime cambió de tamaño durante la importación".to_string());
    }
    fetch::verify_archive_file(snapshot, descriptor.expected_size, descriptor.digest)
}

#[derive(Default)]
struct ImportCleanup {
    snapshot: Option<PathBuf>,
    staging: Option<PathBuf>,
}

fn create_owned_staging(path: &Path, cleanup: &mut ImportCleanup) -> Result<(), String> {
    std::fs::create_dir(path)
        .map_err(|error| format!("No se pudo crear staging propio: {error}"))?;
    cleanup.staging = Some(path.to_path_buf());
    Ok(())
}

impl Drop for ImportCleanup {
    fn drop(&mut self) {
        if let Some(snapshot) = &self.snapshot {
            let _ = std::fs::remove_file(snapshot);
        }
        if let Some(staging) = &self.staging {
            let _ = std::fs::remove_dir_all(staging);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::descriptor::{
        ArchiveLayout, ArtifactKind, ArtifactSource, ExpectedDigest, PayloadValidatorId,
    };
    use super::super::install::{InstallFault, INSTALL_FAULT};
    use super::super::receipt::{
        parse_stored_receipt, stored_identity_matches_descriptor, MARKER_FILE,
    };
    use super::*;
    use crate::tools::runtime::ArtifactArchitecture;
    use sha2::{Digest, Sha256};

    struct Fixture {
        root: PathBuf,
        archive: PathBuf,
        destination: PathBuf,
        descriptor: ArtifactDescriptor,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "ro-launcher-import-{}-{}",
                std::process::id(),
                unique_suffix()
            ));
            std::fs::create_dir_all(&root).unwrap();
            let archive = root.join("runtime.tar.zst");
            let mut tar = tar::Builder::new(Vec::new());
            let mut header = tar::Header::new_gnu();
            header.set_path("fixture/umu-run").unwrap();
            header.set_size(7);
            header.set_mode(0o755);
            header.set_cksum();
            tar.append(&header, &b"fixture"[..]).unwrap();
            let bytes = zstd::stream::encode_all(&tar.into_inner().unwrap()[..], 1).unwrap();
            std::fs::write(&archive, &bytes).unwrap();
            // Test-only descriptors need static digest storage just like catalog entries.
            let digest = Box::leak(format!("{:x}", Sha256::digest(&bytes)).into_boxed_str());
            let descriptor = ArtifactDescriptor {
                id: "import-fixture",
                kind: ArtifactKind::UmuLauncher,
                version: "test",
                source: ArtifactSource::LocalOnly {
                    source_commit: "fixture",
                },
                expected_size: bytes.len() as u64,
                digest: ExpectedDigest::Sha256(digest),
                archive: ArchiveLayout::TarZst {
                    archive_name: "runtime.tar.zst",
                    root: "fixture",
                },
                platform: "linux-x86_64",
                architectures: &[ArtifactArchitecture::X86_64],
                recipe_revision: 1,
                payload: PayloadValidatorId::UmuZipapp,
            };
            Self {
                archive,
                destination: root.join("runtime"),
                root,
                descriptor,
            }
        }

        fn import(&self) -> Result<PathBuf, String> {
            import_archive_at(&self.descriptor, &self.archive, &self.destination)
        }
        fn no_temps(&self) {
            if self.destination.exists() {
                assert!(!std::fs::read_dir(&self.destination)
                    .unwrap()
                    .any(|entry| entry
                        .unwrap()
                        .file_name()
                        .to_string_lossy()
                        .starts_with('.')));
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn import_is_verified_transactional_and_idempotent() {
        use std::os::unix::fs::MetadataExt;
        let fixture = Fixture::new();
        let installed = fixture.import().unwrap();
        let inode = installed.metadata().unwrap().ino();
        let receipt = parse_stored_receipt(&installed.join(MARKER_FILE))
            .unwrap()
            .unwrap();
        assert!(stored_identity_matches_descriptor(
            &fixture.descriptor,
            &receipt
        ));
        assert_eq!(fixture.import().unwrap().metadata().unwrap().ino(), inode);
        fixture.no_temps();
        std::fs::write(
            &fixture.archive,
            vec![0; fixture.descriptor.expected_size as usize],
        )
        .unwrap();
        assert!(fixture.import().unwrap_err().contains("checksum"));
        assert_eq!(installed.metadata().unwrap().ino(), inode);
        fixture.no_temps();
    }

    #[test]
    fn import_rejects_symlinks_truncation_and_unknown_existing_targets() {
        let fixture = Fixture::new();
        let alias = fixture.root.join("alias.tar.zst");
        std::os::unix::fs::symlink(&fixture.archive, &alias).unwrap();
        assert!(import_archive_at(&fixture.descriptor, &alias, &fixture.destination).is_err());
        let installed = fixture.destination.join(fixture.descriptor.id);
        std::fs::create_dir_all(&installed).unwrap();
        std::fs::write(installed.join("sentinel"), b"keep").unwrap();
        assert!(fixture.import().unwrap_err().contains("se conservó"));
        assert_eq!(std::fs::read(installed.join("sentinel")).unwrap(), b"keep");
        std::fs::write(&fixture.archive, b"short").unwrap();
        assert!(fixture.import().unwrap_err().contains("Tamaño inválido"));
        fixture.no_temps();
    }

    #[test]
    fn import_rolls_back_marker_failure_and_can_retry() {
        for fault in [
            InstallFault::AfterActivateRename,
            InstallFault::AfterMarkerWrite,
        ] {
            let fixture = Fixture::new();
            INSTALL_FAULT.set(fault);
            assert!(fixture.import().unwrap_err().contains("fault inyectado"));
            assert!(!fixture.destination.join(fixture.descriptor.id).exists());
            fixture.no_temps();
            assert!(fixture.import().is_ok());
            fixture.no_temps();
        }
    }

    #[test]
    fn checksum_valid_but_invalid_archive_does_not_activate_or_leave_temps() {
        let mut fixture = Fixture::new();
        let bytes = zstd::stream::encode_all(&b"not a tar archive"[..], 1).unwrap();
        std::fs::write(&fixture.archive, &bytes).unwrap();
        fixture.descriptor.expected_size = bytes.len() as u64;
        fixture.descriptor.digest = ExpectedDigest::Sha256(Box::leak(
            format!("{:x}", Sha256::digest(&bytes)).into_boxed_str(),
        ));
        assert!(fixture.import().is_err());
        assert!(!fixture.destination.join(fixture.descriptor.id).exists());
        fixture.no_temps();
    }

    #[test]
    fn first_install_never_replaces_a_racing_destination() {
        let fixture = Fixture::new();
        let staging = fixture.root.join("staging");
        extract::extract_archive(&fixture.descriptor, &fixture.archive, &staging).unwrap();
        let final_dir = fixture.destination.join(fixture.descriptor.id);
        std::fs::create_dir_all(&final_dir).unwrap();
        std::fs::write(final_dir.join("sentinel"), b"keep").unwrap();
        assert!(
            install::install_new_extracted(&fixture.descriptor, &staging, &final_dir, |_| true)
                .is_err()
        );
        assert_eq!(std::fs::read(final_dir.join("sentinel")).unwrap(), b"keep");
        assert!(staging
            .join(fixture.descriptor.archive.archive_root())
            .exists());
    }

    #[test]
    fn snapshot_collision_is_never_claimed_or_removed_by_cleanup() {
        let fixture = Fixture::new();
        let snapshot = fixture.root.join("already-owned");
        std::fs::write(&snapshot, b"keep").unwrap();
        {
            let mut cleanup = ImportCleanup::default();
            let input =
                open_regular_archive(&fixture.archive, fixture.descriptor.expected_size).unwrap();
            assert!(snapshot_archive(input, &snapshot, &fixture.descriptor, &mut cleanup).is_err());
        }
        assert_eq!(std::fs::read(snapshot).unwrap(), b"keep");
    }

    #[test]
    fn staging_collision_is_never_claimed_or_removed_by_cleanup() {
        let fixture = Fixture::new();
        let staging = fixture.root.join("already-owned");
        std::fs::create_dir(&staging).unwrap();
        std::fs::write(staging.join("sentinel"), b"keep").unwrap();
        {
            let mut cleanup = ImportCleanup::default();
            assert!(create_owned_staging(&staging, &mut cleanup).is_err());
        }
        assert_eq!(std::fs::read(staging.join("sentinel")).unwrap(), b"keep");
    }

    #[tokio::test]
    async fn public_import_rejects_relative_paths_before_selecting_appdata() {
        assert!(import_managed_runtime_archive(Path::new("runtime.tar.zst"))
            .await
            .unwrap_err()
            .contains("ruta absoluta"));
    }

    #[test]
    fn import_respects_existing_runtime_operation_and_rejects_destination_symlink() {
        let fixture = Fixture::new();
        let guard = OperationGuard::acquire("runtime", &fixture.destination).unwrap();
        assert!(fixture
            .import()
            .unwrap_err()
            .contains("Ya hay una operación runtime"));
        drop(guard);
        std::fs::create_dir_all(fixture.root.join("elsewhere")).unwrap();
        std::os::unix::fs::symlink(fixture.root.join("elsewhere"), &fixture.destination).unwrap();
        assert!(fixture.import().is_err());
        assert_eq!(
            std::fs::read_dir(fixture.root.join("elsewhere"))
                .unwrap()
                .count(),
            0
        );
    }

    #[test]
    #[ignore = "requires RO_LAUNCHER_TEST_RUNTIME_ARCHIVE, validates the assembled artifact without executing Wine"]
    fn accepted_runtime_archive_imports_with_real_manifest_and_module_hashes() {
        let source = PathBuf::from(
            std::env::var_os("RO_LAUNCHER_TEST_RUNTIME_ARCHIVE")
                .expect("set the accepted runtime archive path explicitly"),
        );
        let root = std::env::temp_dir().join(format!(
            "ro-launcher-accepted-import-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        let descriptor = catalog_descriptor(MANAGED_RUNNER_ID).unwrap();
        let result = import_archive_at(descriptor, &source, &root);
        if let Ok(installed) = &result {
            assert_eq!(
                artifact_cache_state_at(descriptor, installed),
                ArtifactCacheState::Ready
            );
            assert!(installed.join("proton").is_file());
            assert!(!std::fs::read_dir(&root).unwrap().any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with('.')));
        }
        if root.exists() {
            std::fs::remove_dir_all(root).unwrap();
        }
        result.expect("the exact accepted archive must pass the install pipeline");
    }
}
