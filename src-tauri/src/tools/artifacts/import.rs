use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use crate::utils::OperationGuard;
use crate::utils::{emit_log, emit_progress, report};
use tauri::AppHandle;

use super::descriptor::{
    catalog_descriptor, ArtifactDescriptor, LOCAL_MANAGED_RUNNER_ID, MANAGED_RUNNER_ID,
    PREVIOUS_MANAGED_RUNNER_ID,
};
#[cfg(test)]
use super::{artifact_cache_state_at, ArtifactCacheState};
use super::{extract, fetch, install, runtime_dir, unique_suffix};

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
    [
        MANAGED_RUNNER_ID,
        PREVIOUS_MANAGED_RUNNER_ID,
        LOCAL_MANAGED_RUNNER_ID,
    ]
    .into_iter()
    .filter_map(catalog_descriptor)
    .find(|descriptor| descriptor.expected_size == metadata.len())
    .copied()
    .ok_or_else(|| "El tamaño no coincide con un paquete conocido de nndsk-ro-proton".to_string())
}

fn import_archive_at(
    descriptor: &ArtifactDescriptor,
    source: &Path,
    destination: &Path,
) -> Result<PathBuf, String> {
    let _operation = OperationGuard::acquire("runtime", destination)?;
    // Reject special/truncated input before any recovery mutation. Snapshot reopens and verifies it.
    open_regular_archive(source, descriptor.expected_size)?;
    std::fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    require_runtime_directory(destination)?;
    install::needs_install(descriptor, destination, install::policy(descriptor))?;
    import_archive_locked(descriptor, source, destination)
}

/// Caller owns the runtime operation lease, including across blocking extraction.
pub(super) fn import_archive_locked(
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
    let policy = install::policy(descriptor);
    // The owner swept BEFORE acquisition. Sweeping now could delete its live download wrapper.
    install::recover(descriptor, destination)?;
    let needs_install = install::inspect_target(descriptor, destination, policy)?;
    let unique = unique_suffix();
    let snapshot_directory = destination.join(format!(
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
    install::create_work(descriptor, &snapshot_directory)?;
    cleanup.snapshot_directory = Some((*descriptor, snapshot_directory.clone()));
    let snapshot = snapshot_directory.join(descriptor.archive.archive_name());
    snapshot_archive(input, &snapshot, descriptor, &mut cleanup)?;
    if !needs_install {
        return Ok(destination.join(descriptor.id));
    }
    create_owned_staging(descriptor, &staging, &mut cleanup)?;
    extract::extract_created_archive(descriptor, &snapshot, &staging)?;
    #[cfg(test)]
    install::checkpoint("extracted")?;
    let extracted = staging.join(descriptor.archive.archive_root());
    install::prepare(descriptor, &extracted)?;
    install::activate(descriptor, &extracted, destination, policy)?;
    Ok(destination.join(descriptor.id))
}

/// All artifact classes use this owned task. Dropping an IPC or preparation waiter cannot free
/// its guard while download or extraction is still using private paths.
pub(crate) async fn download_managed_runtime(
    app: &AppHandle,
    descriptor: &'static ArtifactDescriptor,
    operation: OperationGuard,
    progress_start: u32,
    progress_end: u32,
) -> Result<(), String> {
    let app = app.clone();
    tokio::spawn(async move {
        download_locked(&app, descriptor, operation, progress_start, progress_end).await
    })
    .await
    .map_err(|error| format!("Falló la instalación del runtime: {error}"))?
}

async fn download_locked(
    app: &AppHandle,
    descriptor: &'static ArtifactDescriptor,
    operation: OperationGuard,
    progress_start: u32,
    progress_end: u32,
) -> Result<(), String> {
    let destination = runtime_dir();
    std::fs::create_dir_all(&destination).map_err(|error| error.to_string())?;
    require_runtime_directory(&destination)?;
    if !install::needs_install(descriptor, &destination, install::policy(descriptor))? {
        return Ok(());
    }
    emit_log(
        app,
        format!(
            "Descargando {}: {}.",
            descriptor.id,
            super::artifact_cache_state_at(descriptor, &destination.join(descriptor.id)).reason()
        ),
    )?;
    let directory = destination.join(format!(
        ".{}.download-{}-{}",
        descriptor.id,
        std::process::id(),
        unique_suffix()
    ));
    let cleanup = DownloadDirectory::create(descriptor, &directory)?;
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
    finish_download(descriptor, operation, cleanup, archive, destination).await
}

async fn finish_download(
    descriptor: ArtifactDescriptor,
    operation: OperationGuard,
    cleanup: DownloadDirectory,
    archive: PathBuf,
    destination: PathBuf,
) -> Result<(), String> {
    // The blocking owner receives the guard AND the downloaded archive, not a future-owned Drop.
    tokio::task::spawn_blocking(move || {
        let _operation = operation;
        let _cleanup = cleanup;
        import_archive_locked(&descriptor, &archive, &destination).map(|_| ())
    })
    .await
    .map_err(|error| format!("Falló la instalación del runtime: {error}"))?
}

fn require_runtime_directory(path: &Path) -> Result<(), String> {
    if !path
        .symlink_metadata()
        .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
    {
        return Err(
            "El directorio de runtimes no puede ser un enlace ni otro tipo de archivo".into(),
        );
    }
    Ok(())
}

struct DownloadDirectory {
    path: PathBuf,
    descriptor: ArtifactDescriptor,
}

impl DownloadDirectory {
    fn create(descriptor: &ArtifactDescriptor, path: &Path) -> Result<Self, String> {
        install::create_work(descriptor, path)?;
        Ok(Self {
            path: path.to_path_buf(),
            descriptor: *descriptor,
        })
    }
}

impl Drop for DownloadDirectory {
    fn drop(&mut self) {
        install::clean_work(&self.descriptor, &self.path);
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
    snapshot_directory: Option<(ArtifactDescriptor, PathBuf)>,
    staging: Option<(ArtifactDescriptor, PathBuf)>,
}

fn create_owned_staging(
    descriptor: &ArtifactDescriptor,
    path: &Path,
    cleanup: &mut ImportCleanup,
) -> Result<(), String> {
    install::create_work(descriptor, path)?;
    cleanup.staging = Some((*descriptor, path.to_path_buf()));
    Ok(())
}

impl Drop for ImportCleanup {
    fn drop(&mut self) {
        if let Some(snapshot) = &self.snapshot {
            if let Err(error) = std::fs::remove_file(snapshot) {
                report(
                    "artifacts.cleanup",
                    format!("{}: {error}", snapshot.display()),
                );
            }
        }
        if let Some((descriptor, staging)) = &self.staging {
            install::clean_work(descriptor, staging);
        }
        if let Some((descriptor, directory)) = &self.snapshot_directory {
            install::clean_work(descriptor, directory);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::descriptor::{
        ArchiveLayout, ArtifactKind, ArtifactSource, ExpectedDigest, PayloadValidatorId,
    };
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
            assert!(create_owned_staging(&fixture.descriptor, &staging, &mut cleanup).is_err());
        }
        assert_eq!(std::fs::read(staging.join("sentinel")).unwrap(), b"keep");
    }

    #[test]
    fn private_download_collision_preserves_other_owners_and_uses_private_mode() {
        use std::os::unix::fs::PermissionsExt;
        let fixture = Fixture::new();
        let owned = fixture.root.join("download-owned");
        {
            let cleanup = DownloadDirectory::create(&fixture.descriptor, &owned).unwrap();
            assert_eq!(
                std::fs::metadata(&owned).unwrap().permissions().mode() & 0o777,
                0o700
            );
            std::fs::write(owned.join("sentinel"), b"keep").unwrap();
            assert!(DownloadDirectory::create(&fixture.descriptor, &owned).is_err());
            assert_eq!(std::fs::read(owned.join("sentinel")).unwrap(), b"keep");
            drop(cleanup);
        }
        assert!(!owned.exists());
        let foreign = fixture.root.join("foreign-download");
        std::fs::create_dir(&foreign).unwrap();
        std::fs::write(foreign.join("sentinel"), b"keep").unwrap();
        assert!(DownloadDirectory::create(&fixture.descriptor, &foreign).is_err());
        assert_eq!(std::fs::read(foreign.join("sentinel")).unwrap(), b"keep");
    }

    #[tokio::test]
    async fn cancelled_waiter_does_not_release_the_blocking_owners_lease_or_files() {
        let fixture = Fixture::new();
        let destination = fixture.destination.clone();
        std::fs::create_dir_all(&destination).unwrap();
        let operation = OperationGuard::acquire("runtime", &destination).unwrap();
        let directory = destination.join("owned-download");
        let cleanup = DownloadDirectory::create(&fixture.descriptor, &directory).unwrap();
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
    async fn downloaded_archive_owner_uses_the_same_import_activation_and_keeps_its_live_wrapper() {
        let fixture = Fixture::new();
        std::fs::create_dir_all(&fixture.destination).unwrap();
        let operation = OperationGuard::acquire("runtime", &fixture.destination).unwrap();
        install::needs_install(
            &fixture.descriptor,
            &fixture.destination,
            install::policy(&fixture.descriptor),
        )
        .unwrap();
        let directory = fixture
            .destination
            .join(format!(".{}.download-123-456", fixture.descriptor.id));
        let cleanup = DownloadDirectory::create(&fixture.descriptor, &directory).unwrap();
        let archive = directory.join(fixture.descriptor.archive.archive_name());
        std::fs::copy(&fixture.archive, &archive).unwrap();
        finish_download(
            fixture.descriptor,
            operation,
            cleanup,
            archive,
            fixture.destination.clone(),
        )
        .await
        .unwrap();
        let root = fixture.destination.join(fixture.descriptor.id);
        assert_eq!(
            artifact_cache_state_at(&fixture.descriptor, &root),
            ArtifactCacheState::Ready
        );
        fixture.no_temps();
        assert!(OperationGuard::acquire("runtime", &fixture.destination).is_ok());
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
            let marker = std::fs::read(installed.join(MARKER_FILE)).unwrap();
            let identity = std::fs::read(installed.join("nndsk-runtime.json")).unwrap();
            assert_eq!(
                import_archive_at(&descriptor, &source, &root).unwrap(),
                *installed
            );
            assert_eq!(std::fs::read(installed.join(MARKER_FILE)).unwrap(), marker);
            assert_eq!(
                std::fs::read(installed.join("nndsk-runtime.json")).unwrap(),
                identity
            );
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
