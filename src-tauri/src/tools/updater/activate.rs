//! Post-verification activation only. Tauri authenticates the bytes before this call.
//! The caller holds F-01's operation gate; cross-process exclusion belongs to F-04.

use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use super::backend::BackendError;

/// Format defense, not a second authenticity check or a full ELF/filesystem parser.
/// Releases ship raw ELF64 little-endian type-2 AppImages, not updater archives.
fn validate_payload(bytes: &[u8]) -> Result<(), BackendError> {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        return Err(BackendError::Install(
            "El updater requiere un AppImage sin comprimir, no gzip".into(),
        ));
    }
    if bytes.len() <= 64
        || &bytes[..7] != b"\x7fELF\x02\x01\x01"
        || &bytes[8..11] != b"AI\x02"
        || !matches!(u16::from_le_bytes([bytes[16], bytes[17]]), 2 | 3)
        || bytes[20..24] != [1, 0, 0, 0]
        || bytes[52..54] != [64, 0]
    {
        return Err(BackendError::Install(
            "El updater requiere una cabecera AppImage ELF64 type-2 válida; no acepta tar ni otros paquetes"
                .into(),
        ));
    }
    Ok(())
}

fn temporary_path(target: &Path) -> Result<PathBuf, BackendError> {
    if !target.is_absolute() || target.file_name().is_none() {
        return Err(BackendError::Install(
            "La ruta AppImage del host debe ser absoluta y nombrar un archivo".into(),
        ));
    }
    let mut name = OsString::from(".");
    name.push(target.file_name().expect("checked filename"));
    name.push(".update-tmp");
    Ok(target.with_file_name(name))
}

fn io_error(step: &str, error: io::Error) -> BackendError {
    BackendError::Install(format!("AppImage: {step}: {error}"))
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Checkpoint {
    TempCreated,
    BeforeRename,
    AfterRename,
}

pub(super) fn activate_appimage(target: &Path, bytes: &[u8]) -> Result<(), BackendError> {
    activate_inner(
        target,
        bytes,
        #[cfg(test)]
        |_| Ok(()),
    )
}

fn activate_inner(
    target: &Path,
    bytes: &[u8],
    #[cfg(test)] mut checkpoint: impl FnMut(Checkpoint) -> io::Result<()>,
) -> Result<(), BackendError> {
    // No filesystem mutation, including residue cleanup, before format validation.
    validate_payload(bytes)?;
    let temporary = temporary_path(target)?;
    let parent = target.parent().expect("absolute file has parent");
    let metadata = fs::symlink_metadata(target).map_err(|e| io_error("inspeccionar destino", e))?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return Err(BackendError::Install(
            "El AppImage instalado debe ser un archivo regular ejecutable, no un enlace".into(),
        ));
    }
    // Open before mutation so parent path errors leave both target and residue alone.
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(parent)
        .map_err(|e| io_error("abrir directorio padre", e))?;

    // The exact sibling basename is reserved for this updater. Never truncate an
    // existing object or follow a link. Refuse foreign/non-regular residue.
    match fs::symlink_metadata(&temporary) {
        Ok(stale) => {
            // SAFETY: geteuid has no pointer arguments or preconditions.
            let owner = unsafe { libc::geteuid() };
            if !stale.is_file() || stale.nlink() != 1 || stale.uid() != owner {
                return Err(BackendError::Install(
                    "El temporal AppImage está ocupado por un objeto ajeno; se conserva".into(),
                ));
            }
            fs::remove_file(&temporary).map_err(|e| io_error("retirar temporal anterior", e))?;
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(io_error("inspeccionar temporal", e)),
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&temporary)
        .map_err(|e| io_error("crear temporal exclusivo", e))?;

    let prepared = (|| -> io::Result<()> {
        #[cfg(test)]
        checkpoint(Checkpoint::TempCreated)?;
        file.write_all(bytes)?;
        // chmod AFTER writing (writes can clear special mode bits), then fsync
        // both contents and the final permission metadata before the commit.
        file.set_permissions(metadata.permissions())?;
        file.sync_all()?;
        #[cfg(test)]
        checkpoint(Checkpoint::BeforeRename)?;
        // Single commit point. Same parent, no backup rename or copy fallback.
        fs::rename(&temporary, target)
    })();
    if let Err(error) = prepared {
        let cleanup = fs::remove_file(&temporary);
        let mut error = format!("AppImage: activación rechazada antes de rename: {error}");
        if let Err(cleanup) = cleanup {
            error.push_str(&format!("; no se pudo retirar el temporal: {cleanup}"));
        }
        return Err(BackendError::Install(error));
    }

    // New target is authoritative from here. Errors MUST NOT initiate rollback.
    #[cfg(test)]
    checkpoint(Checkpoint::AfterRename)
        .map_err(|e| io_error("nuevo AppImage activado; checkpoint posterior", e))?;
    directory
        .sync_all()
        .map_err(|e| io_error("nuevo AppImage activado; fsync del directorio falló", e))
}

#[cfg(test)]
#[path = "activate_tests.rs"]
mod tests;
