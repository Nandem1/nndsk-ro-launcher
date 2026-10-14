//! Receipt-bearing preparation, one atomic activation, and cleanup-only recovery.
//! All writers/recovery callers own OperationGuard("runtime", runtime).
//! F absent/S absent: missing. F Ready/S absent: installed.
//! F absent/S owned: discard S, retry. F owned/S owned: F wins, discard S.
//! F foreign: preserve everything. S foreign: preserve; refuse a new activation.
//! No state gives S restoration authority, before or after the commit.

use std::path::{Path, PathBuf};

use crate::utils::{rename_noreplace, report, sync_dir};

use super::descriptor::{ArtifactDescriptor, ArtifactKind};
use super::receipt::{self, MARKER_FILE};
use super::{artifact_cache_state_at, ArtifactCacheState};

/// The plan permits repairing only supporting artifacts with positive receipt identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExistingTargetPolicy {
    Refuse,
    ReplaceNotReady,
}

pub(super) fn policy(descriptor: &ArtifactDescriptor) -> ExistingTargetPolicy {
    match descriptor.kind {
        ArtifactKind::UmuLauncher | ArtifactKind::Dxvk => ExistingTargetPolicy::ReplaceNotReady,
        _ => ExistingTargetPolicy::Refuse,
    }
}

/// A renamed receipt is a tombstone, never a Ready receipt or restoration record.
const CLEANUP: &str = ".ro-launcher-artifact-cleanup";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tree {
    Absent,
    Ready,
    OwnedIncomplete,
    Cleanup,
    Foreign,
}

pub(super) fn activation_slot(descriptor: &ArtifactDescriptor, runtime: &Path) -> PathBuf {
    runtime.join(format!(".{}.activation", descriptor.id))
}

fn exists(path: &Path) -> Result<bool, String> {
    match path.symlink_metadata() {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}

fn matching_receipt(descriptor: &ArtifactDescriptor, path: &Path) -> bool {
    receipt::parse_stored_receipt(path)
        .ok()
        .flatten()
        .is_some_and(|stored| receipt::stored_identity_matches_descriptor(descriptor, &stored))
}

fn classify(descriptor: &ArtifactDescriptor, root: &Path) -> Result<Tree, String> {
    let metadata = match root.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Tree::Absent),
        Err(error) => return Err(error.to_string()),
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Ok(Tree::Foreign);
    }
    if exists(&root.join(CLEANUP))? {
        return Ok(
            if !exists(&root.join(MARKER_FILE))?
                && matching_receipt(descriptor, &root.join(CLEANUP))
            {
                Tree::Cleanup
            } else {
                Tree::Foreign
            },
        );
    }
    if !matching_receipt(descriptor, &root.join(MARKER_FILE)) {
        return Ok(Tree::Foreign);
    }
    Ok(
        if artifact_cache_state_at(descriptor, root) == ArtifactCacheState::Ready {
            Tree::Ready
        } else {
            Tree::OwnedIncomplete
        },
    )
}

fn ambiguity(path: &Path) -> String {
    format!(
        "{} contiene datos ajenos o ambiguos; se conservó sin sustituirlo. Revisa esta ruta antes de reintentar",
        path.display()
    )
}

/// Idempotent recovery. The canonical generation always wins. No rollback, including errors.
pub(super) fn recover(descriptor: &ArtifactDescriptor, runtime: &Path) -> Result<(), String> {
    let final_dir = runtime.join(descriptor.id);
    let slot = activation_slot(descriptor, runtime);
    let final_state = classify(descriptor, &final_dir)?;
    let slot_state = classify(descriptor, &slot)?;
    if matches!(final_state, Tree::Foreign | Tree::Cleanup) {
        return Err(ambiguity(&final_dir));
    }
    match slot_state {
        Tree::Absent => Ok(()),
        Tree::Foreign if final_state == Tree::Ready => {
            report("artifacts.recovery", ambiguity(&slot));
            Ok(())
        }
        Tree::Foreign => Err(ambiguity(&slot)),
        _ => cleanup_owned(descriptor, &slot),
    }
}

/// Sweep only before acquisition: the active owner's work wrappers must never be swept.
pub(super) fn needs_install(
    descriptor: &ArtifactDescriptor,
    runtime: &Path,
    policy: ExistingTargetPolicy,
) -> Result<bool, String> {
    recover(descriptor, runtime)?;
    sweep_residue(descriptor, runtime)?;
    inspect_target(descriptor, runtime, policy)
}

pub(super) fn inspect_target(
    descriptor: &ArtifactDescriptor,
    runtime: &Path,
    policy: ExistingTargetPolicy,
) -> Result<bool, String> {
    let final_dir = runtime.join(descriptor.id);
    match classify(descriptor, &final_dir)? {
        Tree::Ready => {
            if let Err(error) = receipt::elevate_v1(descriptor, &final_dir) {
                report("artifacts.receipt", &error);
            }
            Ok(false)
        }
        Tree::Absent => Ok(true),
        Tree::OwnedIncomplete if policy == ExistingTargetPolicy::ReplaceNotReady => Ok(true),
        _ => Err(ambiguity(&final_dir)),
    }
}

/// Extracted payload is private and owned by the caller. Never write ownership into final.
pub(super) fn prepare(descriptor: &ArtifactDescriptor, extracted: &Path) -> Result<(), String> {
    if !extracted
        .symlink_metadata()
        .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
        || !super::payload::payload_ready(descriptor, extracted)
        || exists(&extracted.join(MARKER_FILE))?
        || exists(&extracted.join(CLEANUP))?
    {
        return Err("El árbol preparado no es un payload privado válido sin receipt previo".into());
    }
    #[cfg(test)]
    checkpoint("before_receipt")?;
    receipt::write_receipt(descriptor, extracted)?;
    if artifact_cache_state_at(descriptor, extracted) != ArtifactCacheState::Ready {
        return Err("El árbol preparado no superó la clasificación Ready".into());
    }
    #[cfg(test)]
    checkpoint("prepared_ready")?;
    Ok(())
}

/// The sole entry point from both local imports and online acquisition.
pub(super) fn activate(
    descriptor: &ArtifactDescriptor,
    extracted: &Path,
    runtime: &Path,
    policy: ExistingTargetPolicy,
) -> Result<(), String> {
    if artifact_cache_state_at(descriptor, extracted) != ArtifactCacheState::Ready {
        return Err("No se puede activar un árbol que no esté Ready".into());
    }
    let final_dir = runtime.join(descriptor.id);
    let slot = activation_slot(descriptor, runtime);
    recover(descriptor, runtime)?;
    match classify(descriptor, &final_dir)? {
        Tree::Ready => return Ok(()),
        Tree::Absent => {}
        Tree::OwnedIncomplete if policy == ExistingTargetPolicy::ReplaceNotReady => {}
        _ => return Err(ambiguity(&final_dir)),
    }
    rename_noreplace(extracted, &slot).map_err(|error| {
        format!(
            "No se pudo reservar {} sin sustituir datos: {error}",
            slot.display()
        )
    })?;
    let result = (|| {
        sync_dir(extracted.parent().ok_or("Preparación sin padre")?)
            .map_err(|error| error.to_string())?;
        sync_dir(runtime).map_err(|error| error.to_string())?;
        let replace = match classify(descriptor, &final_dir)? {
            Tree::Absent => false,
            Tree::OwnedIncomplete if policy == ExistingTargetPolicy::ReplaceNotReady => true,
            _ => return Err(ambiguity(&final_dir)),
        };
        commit(descriptor, &slot, &final_dir, replace)
    })();
    // Ordinary failure uses precisely the next process's recovery. Never exchange back.
    if let Err(error) = recover(descriptor, runtime) {
        report("artifacts.recovery", &error);
        return match result {
            Err(primary) => Err(format!("{primary}; recuperación: {error}")),
            Ok(()) => Err(error),
        };
    }
    result
}

fn commit(
    descriptor: &ArtifactDescriptor,
    slot: &Path,
    final_dir: &Path,
    replace: bool,
) -> Result<(), String> {
    if artifact_cache_state_at(descriptor, slot) != ArtifactCacheState::Ready {
        return Err("El slot preparado no está Ready".into());
    }
    if replace
        && !matches!(
            classify(descriptor, final_dir)?,
            Tree::Ready | Tree::OwnedIncomplete
        )
    {
        return Err(ambiguity(final_dir));
    }
    #[cfg(test)]
    checkpoint("before_commit")?;
    if replace {
        rename_exchange(slot, final_dir)
    } else {
        rename_noreplace(slot, final_dir)
    }
    .map_err(|error| format!("No se pudo activar {} atómicamente: {error}", descriptor.id))?;
    // COMMIT crossed. Nothing below this line may restore the exchanged-out generation.
    #[cfg(test)]
    checkpoint("committed")?;
    #[cfg(test)]
    checkpoint("parent_sync")?;
    sync_dir(final_dir.parent().ok_or("Runtime sin padre")?).map_err(|error| {
        format!(
            "{} quedó activado; falló fsync del padre: {error}",
            descriptor.id
        )
    })?;
    #[cfg(test)]
    checkpoint("after_parent_sync")?;
    Ok(())
}

fn rename_exchange(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let source = std::ffi::CString::new(source.as_os_str().as_bytes())?;
    let destination = std::ffi::CString::new(destination.as_os_str().as_bytes())?;
    // SAFETY: CStrings remain live, both paths are on the runtime filesystem. No fallback:
    // unsupported filesystems fail before changing either path.
    if unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_EXCHANGE,
        )
    } == 0
    {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// The receipt is removed from the Ready namespace BEFORE payload deletion. Its tombstone
/// survives until all other entries are gone; even partially cleaned trees stay cleanup-only.
fn cleanup_owned(descriptor: &ArtifactDescriptor, root: &Path) -> Result<(), String> {
    match classify(descriptor, root)? {
        Tree::Absent => return Ok(()),
        Tree::Ready | Tree::OwnedIncomplete => {
            rename_noreplace(&root.join(MARKER_FILE), &root.join(CLEANUP))
                .map_err(|error| error.to_string())?;
            sync_dir(root).map_err(|error| error.to_string())?;
        }
        Tree::Cleanup => {}
        Tree::Foreign => return Err(ambiguity(root)),
    }
    #[cfg(test)]
    checkpoint("cleanup_tombstone")?;
    for entry in std::fs::read_dir(root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry.file_name() == CLEANUP {
            continue;
        }
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            std::fs::remove_dir_all(entry.path())
        } else {
            std::fs::remove_file(entry.path())
        }
        .map_err(|error| error.to_string())?;
        #[cfg(test)]
        checkpoint("cleanup_entry")?;
    }
    sync_dir(root).map_err(|error| error.to_string())?;
    #[cfg(test)]
    checkpoint("cleanup_payload_gone")?;
    std::fs::remove_file(root.join(CLEANUP)).map_err(|error| error.to_string())?;
    // A kill here leaves an empty, unproven slot. Preserve it as ambiguity (Ready final is usable).
    #[cfg(test)]
    checkpoint("cleanup_evidence_gone")?;
    std::fs::remove_dir(root).map_err(|error| error.to_string())?;
    sync_dir(root.parent().ok_or("Residuo sin padre")?).map_err(|error| error.to_string())?;
    #[cfg(test)]
    checkpoint("cleanup_done")?;
    Ok(())
}

/// Private work wrappers use the SAME receipt as ownership evidence, before extraction/download.
/// It is not a Ready artifact: required payload is inside its archive root, not in the wrapper.
pub(super) fn create_work(descriptor: &ArtifactDescriptor, path: &Path) -> Result<(), String> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(path)
        .map_err(|error| error.to_string())?;
    receipt::write_receipt(descriptor, path)?;
    sync_dir(path.parent().ok_or("Staging sin padre")?).map_err(|error| error.to_string())
}

pub(super) fn clean_work(descriptor: &ArtifactDescriptor, path: &Path) {
    if let Err(error) = cleanup_owned(descriptor, path) {
        report("artifacts.cleanup", format!("{}: {error}", path.display()));
    }
}

fn exact_suffix(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix).is_some_and(|suffix| {
        let Some((pid, time)) = suffix.split_once('-') else {
            return false;
        };
        !pid.is_empty()
            && !time.is_empty()
            && pid.bytes().all(|b| b.is_ascii_digit())
            && time.bytes().all(|b| b.is_ascii_digit())
    })
}

/// Names select candidates, never grant ownership. Old receiptless residues are preserved.
fn sweep_residue(descriptor: &ArtifactDescriptor, runtime: &Path) -> Result<(), String> {
    let entries = std::fs::read_dir(runtime).map_err(|error| error.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let work = ["staging", "import", "download"]
            .iter()
            .any(|kind| exact_suffix(name, &format!(".{}.{kind}-", descriptor.id)))
            || exact_suffix(
                name,
                &format!(".{}.download-", descriptor.archive.archive_name()),
            );
        // Historical backup names contain only a timestamp. Never restore or choose newest.
        let previous = name
            .strip_prefix(&format!(".{}.previous-", descriptor.id))
            .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()));
        if !work && !previous {
            continue;
        }
        if previous
            && artifact_cache_state_at(descriptor, &runtime.join(descriptor.id))
                != ArtifactCacheState::Ready
        {
            report("artifacts.cleanup", ambiguity(&entry.path()));
            continue;
        }
        clean_work(descriptor, &entry.path());
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn checkpoint(name: &str) -> Result<(), String> {
    tests::checkpoint(name)
}

#[cfg(test)]
#[path = "activation_tests.rs"]
mod tests;
