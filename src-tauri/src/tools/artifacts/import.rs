use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use crate::utils::emit_progress;
use crate::utils::OperationGuard;
use tauri::AppHandle;

use super::descriptor::{
    catalog_descriptor, ArtifactDescriptor, LOCAL_MANAGED_RUNNER_ID, MANAGED_RUNNER_ID,
};
use super::{
    artifact_cache_state_at, extract, fetch, install, runtime_dir, unique_suffix,
    ArtifactCacheState,
};

/// Import known checksum-pinned packages, not an arbitrary runner archive.
/// The blocking owner retains both the runtime lock and cleanup if the caller is cancelled.
pub(crate) async fn import_managed_runtime_archive(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("El archivo del runtime requiere una ruta absoluta".to_string());
    }
    let source = path.to_path_buf();
    let destination = runtime_dir();
    let descriptor = known_runtime_descriptor(&source)?;
    tokio::task::spawn_blocking(move || {
        import_archive_at(&descriptor, &source, &destination).map(|root| root.join("proton"))
    })
    .await
    .map_err(|error| format!("Falló la tarea de importación: {error}"))?
}

fn known_runtime_descriptor(path: &Path) -> Result<ArtifactDescriptor, String> {
    let metadata = path
        .symlink_metadata()
        .map_err(|error| format!("No se pudo comprobar el paquete del runtime: {error}"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(
            "El runtime debe ser un archivo regular, no un enlace, dispositivo ni FIFO".to_string(),
        );
    }
    [MANAGED_RUNNER_ID, LOCAL_MANAGED_RUNNER_ID]
        .into_iter()
        .filter_map(catalog_descriptor)
        .find(|descriptor| descriptor.expected_size == metadata.len())
        .copied()
        .ok_or_else(|| {
            "El tamaño no coincide con un paquete conocido de nndsk-ro-proton".to_string()
        })
}

fn import_archive_at(
    descriptor: &ArtifactDescriptor,
    source: &Path,
    destination: &Path,
) -> Result<PathBuf, String> {
    let _operation = OperationGuard::acquire("runtime", destination)?;
    import_archive_locked(descriptor, source, destination)
}

/// Caller owns the runtime operation lease, including across blocking extraction.
fn import_archive_locked(
    descriptor: &ArtifactDescriptor,
    source: &Path,
    destination: &Path,
) -> Result<PathBuf, String> {
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

/// The caller transfers its existing lease; no recursive lock acquisition.
/// Cancellation cannot remove files still owned by the blocking importer.
pub(crate) async fn download_managed_runtime(
    app: &AppHandle,
    descriptor: &'static ArtifactDescriptor,
    operation: OperationGuard,
    progress_start: u32,
    progress_end: u32,
) -> Result<(), String> {
    if !matches!(
        descriptor.kind,
        super::ArtifactKind::NndskRoProton | super::ArtifactKind::WineTkg716
    ) {
        return Err("La descarga transaccional requiere un runner del catálogo".to_string());
    }
    let destination = runtime_dir();
    let final_dir = destination.join(descriptor.id);
    if artifact_cache_state_at(descriptor, &final_dir) == ArtifactCacheState::Ready {
        return Ok(());
    }
    require_missing_destination(&final_dir)?;
    let directory = destination.join(format!(
        ".{}.download-{}-{}",
        descriptor.id,
        std::process::id(),
        unique_suffix()
    ));
    let cleanup = DownloadDirectory::create(&directory)?;
    let archive = directory.join(descriptor.archive.archive_name());
    fetch::download_verified(
        app,
        descriptor,
        &archive,
        progress_start,
        progress_end.saturating_sub(2),
    )
    .await?;
    emit_progress(
        app,
        &format!("Verificando e instalando {}...", descriptor.id),
        progress_end,
    )?;
    let descriptor = *descriptor;
    tokio::task::spawn_blocking(move || {
        let _operation = operation;
        let _cleanup = cleanup;
        import_archive_locked(&descriptor, &archive, &destination).map(|_| ())
    })
    .await
    .map_err(|error| format!("Falló la instalación del runtime: {error}"))?
}

fn require_missing_destination(path: &Path) -> Result<(), String> {
    match path.symlink_metadata() {
        Ok(_) => Err(format!(
            "{} ya existe pero no coincide con el runtime; se conservó sin sustituirlo",
            path.display()
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("No se pudo comprobar el destino: {error}")),
    }
}

struct DownloadDirectory(PathBuf);

impl DownloadDirectory {
    fn create(path: &Path) -> Result<Self, String> {
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(path)
            .map_err(|error| format!("No se pudo crear descarga privada: {error}"))?;
        Ok(Self(path.to_path_buf()))
    }
}

impl Drop for DownloadDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
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

    #[test]
    fn private_download_collision_preserves_other_owners_and_uses_private_mode() {
        use std::os::unix::fs::PermissionsExt;
        let fixture = Fixture::new();
        let owned = fixture.root.join("download-owned");
        {
            let cleanup = DownloadDirectory::create(&owned).unwrap();
            assert_eq!(
                std::fs::metadata(&owned).unwrap().permissions().mode() & 0o777,
                0o700
            );
            std::fs::write(owned.join("sentinel"), b"keep").unwrap();
            assert!(DownloadDirectory::create(&owned).is_err());
            assert_eq!(std::fs::read(owned.join("sentinel")).unwrap(), b"keep");
            drop(cleanup);
        }
        assert!(!owned.exists());
        let foreign = fixture.root.join("foreign-download");
        std::fs::create_dir(&foreign).unwrap();
        std::fs::write(foreign.join("sentinel"), b"keep").unwrap();
        assert!(DownloadDirectory::create(&foreign).is_err());
        assert_eq!(std::fs::read(foreign.join("sentinel")).unwrap(), b"keep");
        assert!(require_missing_destination(&foreign).is_err());
    }

    #[tokio::test]
    async fn cancelled_waiter_does_not_release_the_blocking_owners_lease_or_files() {
        let fixture = Fixture::new();
        let destination = fixture.destination.clone();
        std::fs::create_dir_all(&destination).unwrap();
        let operation = OperationGuard::acquire("runtime", &destination).unwrap();
        let directory = destination.join("owned-download");
        let cleanup = DownloadDirectory::create(&directory).unwrap();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        let waiter = tokio::spawn(async move {
            tokio::task::spawn_blocking(move || {
                let _operation = operation;
                let _cleanup = cleanup;
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                drop(_cleanup);
                drop(_operation);
                done_tx.send(()).unwrap();
            })
            .await
            .unwrap();
        });
        started_rx.await.unwrap();
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        assert!(OperationGuard::acquire("runtime", &destination).is_err());
        assert!(directory.exists());
        release_tx.send(()).unwrap();
        done_rx.await.unwrap();
        assert!(!directory.exists());
        assert!(OperationGuard::acquire("runtime", &destination).is_ok());
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
        let descriptor = known_runtime_descriptor(&source).unwrap();
        let result = import_archive_at(&descriptor, &source, &root);
        if let Ok(installed) = &result {
            assert_eq!(
                artifact_cache_state_at(&descriptor, installed),
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

    #[test]
    #[ignore = "requires RO_LAUNCHER_TEST_WINE716_ARCHIVE and multilib, imports to an isolated directory; only executes --version"]
    fn wine716_real_archive_is_reusable_and_keeps_legacy_sync_and_layout() {
        use crate::utils::{resolve_runner, WineSyncMode};
        let fixture = Fixture::new();
        let source = PathBuf::from(
            std::env::var_os("RO_LAUNCHER_TEST_WINE716_ARCHIVE")
                .expect("set exact Wine 7.16 TkG archive"),
        );
        let descriptor = catalog_descriptor(super::super::descriptor::MANAGED_WINE716_ID).unwrap();
        let installed = import_archive_at(descriptor, &source, &fixture.destination).unwrap();
        assert_eq!(
            artifact_cache_state_at(descriptor, &installed),
            ArtifactCacheState::Ready
        );
        let marker = std::fs::read(installed.join(MARKER_FILE)).unwrap();
        assert_eq!(
            import_archive_at(descriptor, &source, &fixture.destination).unwrap(),
            installed
        );
        assert_eq!(std::fs::read(installed.join(MARKER_FILE)).unwrap(), marker);
        let runner = resolve_runner(installed.join("bin/wine").to_str().unwrap()).unwrap();
        assert_eq!(
            runner.reported_version().as_deref(),
            Some("wine-7.16.r0.gaa2eb6ee ( TkG Staging Esync Fsync )")
        );
        assert!(runner.is_wine_7_16());
        assert_eq!(runner.wine_sync_mode(), WineSyncMode::Fsync);
        assert!(installed.join("lib/wine/i386-unix/ntdll.so").is_file());
        assert!(installed.join("lib/wine/x86_64-unix/ntdll.so").is_file());
        assert!(!installed.join("proton").exists());
    }
}
