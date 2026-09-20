//! Harness descartable Fase 6A — pin D7VK v2.2, install/restore en scratch. No autoridad productiva.

use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::tools::artifacts::catalog_descriptor;
use crate::utils::{replace_json, OperationGuard};

pub(crate) const SPIKE_ARTIFACT_ID: &str = "d7vk-2.2";
pub(crate) const SPIKE_VERSION: &str = "2.2";
pub(crate) const SPIKE_URL: &str =
    "https://github.com/WinterSnowfall/d7vk/releases/download/v2.2/d7vk-v2.2.zip";
pub(crate) const SPIKE_SIZE: u64 = 3_375_635;
pub(crate) const SPIKE_SHA256: &str =
    "1a9ffe3639ceb5e2fb1ccb25ef388354b4476bb26bc7edf88a8dc59f2774df38";
pub(crate) const SPIKE_LICENSE: &str = "zlib";
pub(crate) const SPIKE_MANIFEST: &str = ".ro-launcher-d7vk-spike.json";
pub(crate) const SPIKE_BACKUP_DIR: &str = ".ro-launcher-d7vk-spike-backup";
pub(crate) const SPIKE_COMPONENT: &str = "game-dir/d7vk-spike";
pub(crate) const WINE_DDRAW_DELEGATE: &str = "ddraw_.dll";
const DGVOODOO_MANIFEST: &str = ".ro-launcher-dgvoodoo.json";
const PREFIX_MARKER: &str = ".ro-launcher-prefix.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpikeDescriptor {
    pub id: &'static str,
    pub version: &'static str,
    pub source_url: &'static str,
    pub expected_size: u64,
    pub sha256_hex: &'static str,
    pub license: &'static str,
}

pub(crate) fn spike_descriptor() -> SpikeDescriptor {
    SpikeDescriptor {
        id: SPIKE_ARTIFACT_ID,
        version: SPIKE_VERSION,
        source_url: SPIKE_URL,
        expected_size: SPIKE_SIZE,
        sha256_hex: SPIKE_SHA256,
        license: SPIKE_LICENSE,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SpikeDeployment {
    GameDirSideBySide,
    PrefixOwnedDdrawUnderscore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SpikeError {
    UrlNotHttpsExact,
    SizeMismatch { actual: u64, expected: u64 },
    DigestMismatch,
    ZipUnsafeEntry,
    MissingRequiredPayload,
    CollisionDgVoodoo,
    MissingDelegableDdraw,
    ScratchPathForbidden,
    Interrupted(&'static str),
    RestoreFailed(String),
    DdrawUnderscoreAlreadyExists,
    Io(String),
    Lock(String),
}

impl std::fmt::Display for SpikeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UrlNotHttpsExact => write!(f, "URL no coincide con el pin HTTPS"),
            Self::SizeMismatch { actual, expected } => {
                write!(f, "tamaño {actual} != {expected}")
            }
            Self::DigestMismatch => write!(f, "digest del archive no coincide"),
            Self::ZipUnsafeEntry => write!(f, "entrada zip insegura"),
            Self::MissingRequiredPayload => write!(f, "payload ddraw.dll x86 ausente"),
            Self::CollisionDgVoodoo => write!(f, "colisión con overlay dgVoodoo o prefix"),
            Self::MissingDelegableDdraw => write!(f, "falta ddraw.dll delegable en el prefix"),
            Self::ScratchPathForbidden => write!(f, "scratch no permitido"),
            Self::Interrupted(stage) => write!(f, "interrupción inyectada: {stage}"),
            Self::RestoreFailed(msg) => write!(f, "restore falló: {msg}"),
            Self::DdrawUnderscoreAlreadyExists => write!(f, "ddraw_.dll ya existe"),
            Self::Io(msg) => write!(f, "{msg}"),
            Self::Lock(msg) => write!(f, "{msg}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SpikeManifest {
    pub schema_version: u32,
    pub artifact_id: String,
    pub deployment: SpikeDeployment,
    pub files: Vec<SpikeFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SpikeFile {
    pub name: String,
    pub relative_source: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PinnedLayout {
    pub archive_root: String,
    pub x86_relative_source: String,
    pub x64_relative_source: Option<String>,
    pub x86_payload_sha256: String,
    pub x86_payload_size: u64,
}

pub(crate) fn pinned_layout_from_fixture(json: &str) -> Result<PinnedLayout, String> {
    serde_json::from_str(json).map_err(|error| error.to_string())
}

pub(crate) fn url_allowed(url: &str) -> bool {
    url == SPIKE_URL && url.starts_with("https://")
}

pub(crate) fn verify_archive(path: &Path) -> Result<(), SpikeError> {
    let metadata = std::fs::metadata(path).map_err(|error| SpikeError::Io(error.to_string()))?;
    let actual = metadata.len();
    if actual != SPIKE_SIZE {
        return Err(SpikeError::SizeMismatch {
            actual,
            expected: SPIKE_SIZE,
        });
    }
    let bytes = std::fs::read(path).map_err(|error| SpikeError::Io(error.to_string()))?;
    let digest = hex_sha256(&bytes);
    if digest != SPIKE_SHA256 {
        return Err(SpikeError::DigestMismatch);
    }
    Ok(())
}

pub(crate) fn extract_zip_safe(zip_path: &Path, staging: &Path) -> Result<(), SpikeError> {
    use std::fs::File;

    std::fs::create_dir_all(staging).map_err(|error| SpikeError::Io(error.to_string()))?;
    let file = File::open(zip_path).map_err(|error| SpikeError::Io(error.to_string()))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| SpikeError::Io(error.to_string()))?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| SpikeError::Io(error.to_string()))?;
        let Some(enclosed) = entry.enclosed_name() else {
            return Err(SpikeError::ZipUnsafeEntry);
        };
        if !path_is_valid_utf8(&enclosed) {
            return Err(SpikeError::ZipUnsafeEntry);
        }
        if normalize_relative_path(&enclosed).is_err() {
            return Err(SpikeError::ZipUnsafeEntry);
        }
        if entry.is_symlink() {
            return Err(SpikeError::ZipUnsafeEntry);
        }
        let out_path = staging.join(enclosed);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)
                .map_err(|error| SpikeError::Io(error.to_string()))?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| SpikeError::Io(error.to_string()))?;
            }
            let mut buffer = Vec::new();
            entry
                .read_to_end(&mut buffer)
                .map_err(|error| SpikeError::Io(error.to_string()))?;
            atomic_write_regular(&out_path, &buffer)?;
        }
    }
    Ok(())
}

pub(crate) fn payload_ready(extracted_root: &Path, layout: &PinnedLayout) -> bool {
    let path = extracted_root.join(&layout.x86_relative_source);
    is_regular_file(&path)
        && path.metadata().map(|m| m.len()).ok() == Some(layout.x86_payload_size)
        && file_sha256_hex(&path).as_deref() == Some(layout.x86_payload_sha256.as_str())
}

pub(crate) fn pe_machine(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() < 0x40 {
        return None;
    }
    let e_lfanew = u32::from_le_bytes(bytes[0x3c..0x40].try_into().ok()?) as usize;
    if bytes.len() < e_lfanew + 6 {
        return None;
    }
    let machine = u16::from_le_bytes(bytes[e_lfanew + 4..e_lfanew + 6].try_into().ok()?);
    match machine {
        0x014c => Some("x86"),
        0x8664 => Some("x86_64"),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstallFault {
    None,
    AfterDllWrite,
    AfterManifestWrite,
}

thread_local! {
    static INSTALL_FAULT: std::cell::Cell<InstallFault> = const { std::cell::Cell::new(InstallFault::None) };
}

pub(crate) fn set_install_fault(fault: InstallFault) {
    INSTALL_FAULT.set(fault);
}

pub(crate) fn clear_install_fault() {
    INSTALL_FAULT.set(InstallFault::None);
}

fn current_fault() -> InstallFault {
    INSTALL_FAULT.get()
}

fn inject_fault(fault: InstallFault) -> Result<(), SpikeError> {
    if current_fault() == fault {
        return Err(SpikeError::Interrupted(match fault {
            InstallFault::AfterDllWrite => "after-dll-write",
            InstallFault::AfterManifestWrite => "after-manifest-write",
            InstallFault::None => "none",
        }));
    }
    Ok(())
}

#[derive(Debug)]
struct Snapshot {
    path: PathBuf,
    bytes: Option<Vec<u8>>,
}

struct ScratchGuard {
    path: PathBuf,
}

impl ScratchGuard {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for ScratchGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

pub(crate) fn validate_scratch(scratch: &Path) -> Result<(), SpikeError> {
    if !scratch.is_dir() || scratch.is_symlink() {
        return Err(SpikeError::ScratchPathForbidden);
    }
    if scratch.join(PREFIX_MARKER).exists() {
        return Err(SpikeError::ScratchPathForbidden);
    }
    if scratch.join(DGVOODOO_MANIFEST).exists() {
        return Err(SpikeError::CollisionDgVoodoo);
    }
    let has_ddraw = find_case_insensitive(scratch, "ddraw.dll").is_some();
    let has_d3dimm = find_case_insensitive(scratch, "d3dimm.dll").is_some()
        || find_case_insensitive(scratch, "D3DImm.dll").is_some();
    if has_ddraw && has_d3dimm {
        return Err(SpikeError::CollisionDgVoodoo);
    }
    Ok(())
}

pub(crate) fn install_side_by_side(
    scratch_game_dir: &Path,
    extracted: &Path,
    layout: &PinnedLayout,
) -> Result<SpikeManifest, SpikeError> {
    let _lock =
        OperationGuard::acquire("d7vk-spike", scratch_game_dir).map_err(SpikeError::Lock)?;
    validate_scratch(scratch_game_dir)?;
    let source = extracted.join(&layout.x86_relative_source);
    if !is_regular_file(&source) {
        return Err(SpikeError::MissingRequiredPayload);
    }
    let payload = read_regular_file(&source)?;
    if pe_machine(&payload) != Some("x86") {
        return Err(SpikeError::MissingRequiredPayload);
    }

    let mut snapshots = Vec::new();
    let mut seen = HashSet::new();
    let backup_root = scratch_game_dir.join(SPIKE_BACKUP_DIR);
    std::fs::create_dir_all(&backup_root).map_err(|error| SpikeError::Io(error.to_string()))?;

    let target = scratch_game_dir.join("ddraw.dll");
    let result = (|| {
        let backup = if target.exists() {
            let bytes = read_regular_file(&target)?;
            let stored = backup_root.join("00-ddraw.dll.original");
            snapshot_once(&target, &mut snapshots, &mut seen)?;
            snapshot_once(&stored, &mut snapshots, &mut seen)?;
            atomic_write_regular(&stored, &bytes)?;
            Some((stored, bytes))
        } else {
            snapshot_once(&target, &mut snapshots, &mut seen)?;
            None
        };

        atomic_write_regular(&target, &payload)?;
        inject_fault(InstallFault::AfterDllWrite)?;

        let manifest = SpikeManifest {
            schema_version: 1,
            artifact_id: SPIKE_ARTIFACT_ID.to_string(),
            deployment: SpikeDeployment::GameDirSideBySide,
            files: vec![SpikeFile {
                name: "ddraw.dll".to_string(),
                relative_source: layout.x86_relative_source.clone(),
                size: payload.len() as u64,
                sha256: hex_sha256(&payload),
            }],
        };
        write_manifest(scratch_game_dir, &manifest)?;
        inject_fault(InstallFault::AfterManifestWrite)?;
        let _ = backup;
        Ok(manifest)
    })();

    match result {
        Ok(manifest) => Ok(manifest),
        Err(error) => {
            rollback(&snapshots);
            let _ = std::fs::remove_file(scratch_game_dir.join(SPIKE_MANIFEST));
            Err(error)
        }
    }
}

pub(crate) fn install_prefix_owned(
    scratch_prefix: &Path,
    extracted: &Path,
    arch_dir: &str,
    layout: &PinnedLayout,
) -> Result<SpikeManifest, SpikeError> {
    let _lock = OperationGuard::acquire("d7vk-spike", scratch_prefix).map_err(SpikeError::Lock)?;
    if scratch_prefix.join(PREFIX_MARKER).exists() {
        return Err(SpikeError::ScratchPathForbidden);
    }

    let windows = scratch_prefix.join("drive_c/windows").join(arch_dir);
    let wine_ddraw = windows.join("ddraw.dll");
    let wine_delegate = windows.join(WINE_DDRAW_DELEGATE);
    let d7vk_ddraw = windows.join("ddraw.dll");

    if !is_regular_file(&wine_ddraw) {
        return Err(SpikeError::MissingDelegableDdraw);
    }
    if wine_delegate.exists() {
        return Err(SpikeError::DdrawUnderscoreAlreadyExists);
    }

    let source = extracted.join(&layout.x86_relative_source);
    let payload = read_regular_file(&source)?;

    let mut snapshots = Vec::new();
    let mut seen = HashSet::new();

    let result = (|| {
        snapshot_once(&wine_ddraw, &mut snapshots, &mut seen)?;
        snapshot_once(&wine_delegate, &mut snapshots, &mut seen)?;
        snapshot_once(&d7vk_ddraw, &mut snapshots, &mut seen)?;

        std::fs::rename(&wine_ddraw, &wine_delegate)
            .map_err(|error| SpikeError::Io(error.to_string()))?;
        inject_fault(InstallFault::AfterDllWrite)?;

        atomic_write_regular(&d7vk_ddraw, &payload)?;
        inject_fault(InstallFault::AfterManifestWrite)?;

        let manifest = SpikeManifest {
            schema_version: 1,
            artifact_id: SPIKE_ARTIFACT_ID.to_string(),
            deployment: SpikeDeployment::PrefixOwnedDdrawUnderscore,
            files: vec![SpikeFile {
                name: "ddraw.dll".to_string(),
                relative_source: layout.x86_relative_source.clone(),
                size: payload.len() as u64,
                sha256: hex_sha256(&payload),
            }],
        };
        write_manifest(scratch_prefix, &manifest)?;
        Ok(manifest)
    })();

    match result {
        Ok(manifest) => Ok(manifest),
        Err(error) => {
            rollback_prefix_owned(&windows, &snapshots);
            Err(error)
        }
    }
}

pub(crate) fn uninstall_restore(root: &Path) -> Result<(), SpikeError> {
    let manifest_path = root.join(SPIKE_MANIFEST);
    if !manifest_path.exists() {
        return Ok(());
    }
    let manifest = read_manifest(root)?;
    match manifest.deployment {
        SpikeDeployment::GameDirSideBySide => restore_side_by_side(root, &manifest),
        SpikeDeployment::PrefixOwnedDdrawUnderscore => restore_prefix_owned(root, &manifest),
    }
}

fn restore_side_by_side(root: &Path, manifest: &SpikeManifest) -> Result<(), SpikeError> {
    let target = root.join("ddraw.dll");
    let backup = root.join(SPIKE_BACKUP_DIR).join("00-ddraw.dll.original");
    if backup.is_file() {
        let bytes = read_regular_file(&backup)?;
        atomic_write_regular(&target, &bytes)?;
        let _ = std::fs::remove_file(&backup);
    } else if target.exists() {
        std::fs::remove_file(&target).map_err(|error| SpikeError::Io(error.to_string()))?;
    }
    let _ = std::fs::remove_file(root.join(SPIKE_MANIFEST));
    if root.join(SPIKE_BACKUP_DIR).exists() {
        let _ = std::fs::remove_dir_all(root.join(SPIKE_BACKUP_DIR));
    }
    let _ = manifest;
    Ok(())
}

fn restore_prefix_owned(root: &Path, manifest: &SpikeManifest) -> Result<(), SpikeError> {
    let arch_dir = "syswow64";
    let windows = root.join("drive_c/windows").join(arch_dir);
    let wine_ddraw = windows.join("ddraw.dll");
    let wine_delegate = windows.join(WINE_DDRAW_DELEGATE);

    if wine_delegate.exists() {
        if wine_ddraw.exists() {
            std::fs::remove_file(&wine_ddraw)
                .map_err(|error| SpikeError::RestoreFailed(error.to_string()))?;
        }
        std::fs::rename(&wine_delegate, &wine_ddraw)
            .map_err(|error| SpikeError::RestoreFailed(error.to_string()))?;
    } else if wine_ddraw.exists() {
        std::fs::remove_file(&wine_ddraw)
            .map_err(|error| SpikeError::RestoreFailed(error.to_string()))?;
    }
    let _ = std::fs::remove_file(root.join(SPIKE_MANIFEST));
    let _ = manifest;
    Ok(())
}

fn rollback_prefix_owned(windows: &Path, snapshots: &[Snapshot]) {
    rollback(snapshots);
    let wine_ddraw = windows.join("ddraw.dll");
    let wine_delegate = windows.join(WINE_DDRAW_DELEGATE);
    if wine_ddraw.exists() && wine_delegate.exists() {
        let _ = std::fs::remove_file(&wine_ddraw);
        let _ = std::fs::rename(&wine_delegate, &wine_ddraw);
    }
}

fn write_manifest(root: &Path, manifest: &SpikeManifest) -> Result<(), SpikeError> {
    replace_json(&root.join(SPIKE_MANIFEST), manifest).map_err(SpikeError::Io)
}

fn read_manifest(root: &Path) -> Result<SpikeManifest, SpikeError> {
    let bytes = read_regular_file(&root.join(SPIKE_MANIFEST))?;
    serde_json::from_slice(&bytes).map_err(|error| SpikeError::Io(error.to_string()))
}

fn find_case_insensitive(dir: &Path, name: &str) -> Option<PathBuf> {
    let expected = name.to_ascii_lowercase();
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let file_name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if file_name == expected {
            return Some(entry.path());
        }
    }
    None
}

fn path_is_valid_utf8(path: &Path) -> bool {
    path.to_str().is_some()
}

fn normalize_relative_path(path: &Path) -> Result<PathBuf, SpikeError> {
    if path.is_absolute() {
        return Err(SpikeError::ZipUnsafeEntry);
    }
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return Err(SpikeError::ZipUnsafeEntry);
                }
            }
            Component::RootDir | Component::Prefix(_) => return Err(SpikeError::ZipUnsafeEntry),
        }
    }
    Ok(out)
}

fn is_regular_file(path: &Path) -> bool {
    path.symlink_metadata()
        .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
}

fn read_regular_file(path: &Path) -> Result<Vec<u8>, SpikeError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|error| SpikeError::Io(error.to_string()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(SpikeError::Io(format!(
            "ruta no regular: {}",
            path.display()
        )));
    }
    std::fs::read(path).map_err(|error| SpikeError::Io(error.to_string()))
}

fn atomic_write_regular(path: &Path, bytes: &[u8]) -> Result<(), SpikeError> {
    if path.exists() {
        let metadata =
            std::fs::symlink_metadata(path).map_err(|error| SpikeError::Io(error.to_string()))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(SpikeError::Io(format!(
                "ruta no regular: {}",
                path.display()
            )));
        }
    }
    let parent = path
        .parent()
        .ok_or_else(|| SpikeError::Io("ruta sin padre".to_string()))?;
    std::fs::create_dir_all(parent).map_err(|error| SpikeError::Io(error.to_string()))?;
    let temporary = parent.join(format!(
        ".d7vk-spike.tmp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| SpikeError::Io(error.to_string()))?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|error| SpikeError::Io(error.to_string()))?;
    }
    std::fs::rename(&temporary, path).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        SpikeError::Io(error.to_string())
    })
}

fn snapshot_once(
    path: &Path,
    snapshots: &mut Vec<Snapshot>,
    seen: &mut HashSet<PathBuf>,
) -> Result<(), SpikeError> {
    if !seen.insert(path.to_path_buf()) {
        return Ok(());
    }
    let bytes = if path.exists() {
        Some(read_regular_file(path)?)
    } else {
        None
    };
    snapshots.push(Snapshot {
        path: path.to_path_buf(),
        bytes,
    });
    Ok(())
}

fn rollback(snapshots: &[Snapshot]) {
    for snapshot in snapshots.iter().rev() {
        match &snapshot.bytes {
            Some(bytes) => {
                let _ = atomic_write_regular(&snapshot.path, bytes);
            }
            None => {
                let _ = std::fs::remove_file(&snapshot.path);
            }
        }
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{:02x}", byte)).collect()
}

fn file_sha256_hex(path: &Path) -> Option<String> {
    read_regular_file(path).ok().map(|bytes| hex_sha256(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn layout() -> PinnedLayout {
        pinned_layout_from_fixture(include_str!(
            "../../../../contract-fixtures/d7vk-spike-v2.2.json"
        ))
        .expect("golden layout")
    }

    fn scratch_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ro-launcher-d7vk-spike-{}-{}-{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }

    fn write_zip(entries: &[(&str, &[u8])]) -> PathBuf {
        let path = scratch_dir("zip");
        std::fs::create_dir_all(&path).unwrap();
        let zip_path = path.join("test.zip");
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut writer = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (name, bytes) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
        zip_path
    }

    fn minimal_pe_x86() -> Vec<u8> {
        let mut bytes = vec![0u8; 0x80];
        bytes[0] = b'M';
        bytes[1] = b'Z';
        bytes[0x3c..0x40].copy_from_slice(&64u32.to_le_bytes());
        bytes[64] = b'P';
        bytes[65] = b'E';
        bytes[68..70].copy_from_slice(&0x014cu16.to_le_bytes());
        bytes
    }

    fn extracted_stub(layout: &PinnedLayout) -> (ScratchGuard, PathBuf) {
        let root = scratch_dir("extracted");
        std::fs::create_dir_all(&root).unwrap();
        let payload_path = root.join(&layout.x86_relative_source);
        std::fs::create_dir_all(payload_path.parent().unwrap()).unwrap();
        std::fs::write(&payload_path, minimal_pe_x86()).unwrap();
        (ScratchGuard::new(root.clone()), root)
    }

    #[test]
    fn golden_matches_spike_constants() {
        let golden: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../contract-fixtures/d7vk-spike-v2.2.json"
        ))
        .unwrap();
        assert_eq!(golden["artifactId"], SPIKE_ARTIFACT_ID);
        assert_eq!(golden["expectedSize"], SPIKE_SIZE);
        assert_eq!(golden["sha256"].as_str(), Some(SPIKE_SHA256));
        assert_eq!(golden["sourceUrl"].as_str(), Some(SPIKE_URL));
        let descriptor = spike_descriptor();
        assert_eq!(descriptor.expected_size, SPIKE_SIZE);
        assert_eq!(descriptor.sha256_hex, SPIKE_SHA256);
    }

    #[test]
    fn url_allowed_rejects_mutations() {
        assert!(url_allowed(SPIKE_URL));
        assert!(!url_allowed(&SPIKE_URL.replace("https://", "http://")));
        assert!(!url_allowed(&format!("{SPIKE_URL}?x=1")));
        assert!(!url_allowed("https://evil.example/x"));
    }

    #[test]
    fn catalog_still_has_three_artifacts() {
        assert!(catalog_descriptor("d7vk-2.2").is_none());
        assert_eq!(crate::tools::artifacts::catalog_all().len(), 3);
    }

    #[test]
    fn verify_archive_rejects_size_and_digest() {
        let dir = scratch_dir("verify");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("small.zip");
        std::fs::write(&path, b"tiny").unwrap();
        assert!(matches!(
            verify_archive(&path),
            Err(SpikeError::SizeMismatch { .. })
        ));
        std::fs::write(&path, vec![0u8; SPIKE_SIZE as usize]).unwrap();
        assert!(matches!(
            verify_archive(&path),
            Err(SpikeError::DigestMismatch)
        ));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn extract_zip_rejects_traversal() {
        let zip_path = write_zip(&[("../escape.dll", b"x")]);
        let staging = scratch_dir("staging-bad");
        assert!(matches!(
            extract_zip_safe(&zip_path, &staging),
            Err(SpikeError::ZipUnsafeEntry)
        ));
        let _ = std::fs::remove_dir_all(staging);
        let _ = std::fs::remove_file(zip_path);
    }

    #[test]
    fn extract_zip_accepts_layout_paths() {
        let layout = layout();
        let zip_path = write_zip(&[(layout.x86_relative_source.as_str(), b"stub-ddraw")]);
        let staging = scratch_dir("staging-ok");
        extract_zip_safe(&zip_path, &staging).unwrap();
        assert!(is_regular_file(&staging.join(&layout.x86_relative_source)));
        let _ = std::fs::remove_dir_all(staging);
        let _ = std::fs::remove_file(zip_path);
    }

    #[test]
    fn pe_machine_reads_x86_header() {
        assert_eq!(pe_machine(&minimal_pe_x86()), Some("x86"));
    }

    #[test]
    fn install_side_by_side_and_restore() {
        let layout = layout();
        let (_extract_guard, extracted) = extracted_stub(&layout);
        let scratch = scratch_dir("game");
        std::fs::create_dir_all(&scratch).unwrap();
        let _guard = ScratchGuard::new(scratch.clone());

        install_side_by_side(&scratch, &extracted, &layout).unwrap();
        assert!(scratch.join("ddraw.dll").is_file());
        assert!(scratch.join(SPIKE_MANIFEST).is_file());

        uninstall_restore(&scratch).unwrap();
        assert!(!scratch.join("ddraw.dll").exists());
        assert!(!scratch.join(SPIKE_MANIFEST).exists());
    }

    #[test]
    fn install_restores_prior_ddraw() {
        let layout = layout();
        let (_extract_guard, extracted) = extracted_stub(&layout);
        let scratch = scratch_dir("game-prior");
        std::fs::create_dir_all(&scratch).unwrap();
        let _guard = ScratchGuard::new(scratch.clone());
        std::fs::write(scratch.join("ddraw.dll"), b"original").unwrap();

        install_side_by_side(&scratch, &extracted, &layout).unwrap();
        uninstall_restore(&scratch).unwrap();
        assert_eq!(
            std::fs::read(scratch.join("ddraw.dll")).unwrap(),
            b"original"
        );
    }

    #[test]
    fn interrupt_after_dll_write_rolls_back() {
        let layout = layout();
        let (_extract_guard, extracted) = extracted_stub(&layout);
        let scratch = scratch_dir("interrupt");
        std::fs::create_dir_all(&scratch).unwrap();
        let _guard = ScratchGuard::new(scratch.clone());

        set_install_fault(InstallFault::AfterDllWrite);
        assert!(matches!(
            install_side_by_side(&scratch, &extracted, &layout),
            Err(SpikeError::Interrupted(_))
        ));
        clear_install_fault();
        assert!(!scratch.join(SPIKE_MANIFEST).exists());
    }

    #[test]
    fn collision_dgvoodoo_manifest_rejected() {
        let layout = layout();
        let (_extract_guard, extracted) = extracted_stub(&layout);
        let scratch = scratch_dir("dg-collision");
        std::fs::create_dir_all(&scratch).unwrap();
        let _guard = ScratchGuard::new(scratch.clone());
        std::fs::write(scratch.join(DGVOODOO_MANIFEST), b"{}").unwrap();
        assert!(matches!(
            install_side_by_side(&scratch, &extracted, &layout),
            Err(SpikeError::CollisionDgVoodoo)
        ));
    }

    #[test]
    fn prefix_owned_install_and_restore() {
        let layout = layout();
        let (_extract_guard, extracted) = extracted_stub(&layout);
        let scratch = scratch_dir("prefix");
        let windows = scratch.join("drive_c/windows/syswow64");
        std::fs::create_dir_all(&windows).unwrap();
        std::fs::write(windows.join("ddraw.dll"), b"wine-ddraw").unwrap();
        let _guard = ScratchGuard::new(scratch.clone());

        install_prefix_owned(&scratch, &extracted, "syswow64", &layout).unwrap();
        assert!(windows.join(WINE_DDRAW_DELEGATE).is_file());
        assert!(windows.join("ddraw.dll").is_file());

        uninstall_restore(&scratch).unwrap();
        assert!(!windows.join(WINE_DDRAW_DELEGATE).exists());
        assert_eq!(
            std::fs::read(windows.join("ddraw.dll")).unwrap(),
            b"wine-ddraw"
        );
    }

    #[test]
    fn prefix_owned_requires_delegable_ddraw() {
        let layout = layout();
        let (_extract_guard, extracted) = extracted_stub(&layout);
        let scratch = scratch_dir("prefix-missing");
        std::fs::create_dir_all(scratch.join("drive_c/windows/syswow64")).unwrap();
        let _guard = ScratchGuard::new(scratch.clone());
        assert!(matches!(
            install_prefix_owned(&scratch, &extracted, "syswow64", &layout),
            Err(SpikeError::MissingDelegableDdraw)
        ));
    }

    #[test]
    fn scratch_forbidden_with_prefix_marker() {
        let layout = layout();
        let (_extract_guard, extracted) = extracted_stub(&layout);
        let scratch = scratch_dir("prefix-marker");
        std::fs::create_dir_all(&scratch).unwrap();
        let _guard = ScratchGuard::new(scratch.clone());
        std::fs::write(scratch.join(PREFIX_MARKER), b"{}").unwrap();
        assert!(matches!(
            install_side_by_side(&scratch, &extracted, &layout),
            Err(SpikeError::ScratchPathForbidden)
        ));
    }

    #[test]
    fn payload_ready_rejects_symlink_dll() {
        let layout = layout();
        let root = scratch_dir("symlink-payload");
        std::fs::create_dir_all(root.join("d7vk-v2.2/x32")).unwrap();
        std::fs::write(root.join("d7vk-v2.2/x32/real.dll"), b"x").unwrap();
        std::os::unix::fs::symlink("real.dll", root.join(&layout.x86_relative_source)).unwrap();
        assert!(!payload_ready(&root, &layout));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    #[ignore = "requires network download of d7vk-v2.2.zip"]
    fn pinned_release_zip_matches_fixture() {
        let zip_path = PathBuf::from("/tmp/ro-launcher-d7vk-spike/d7vk-v2.2.zip");
        verify_archive(&zip_path).expect("pinned archive");
        let layout = layout();
        let staging = scratch_dir("pinned-extract");
        extract_zip_safe(&zip_path, &staging).unwrap();
        assert!(payload_ready(&staging, &layout));
        let bytes = std::fs::read(staging.join(&layout.x86_relative_source)).unwrap();
        assert_eq!(pe_machine(&bytes), Some("x86"));
        let _ = std::fs::remove_dir_all(staging);
    }
}
