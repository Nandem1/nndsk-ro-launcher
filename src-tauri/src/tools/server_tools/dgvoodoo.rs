use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use ro_tools_core::dgvoodoo::TEMPLATE_FILES;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

pub const INSTALL_MANIFEST: &str = ".ro-launcher-dgvoodoo.json";
const BACKUP_DIR: &str = ".ro-launcher-dgvoodoo-backup";
const MANIFEST_SCHEMA: u32 = 2;
const MUTABLE_CONFIG: &str = "dgVoodoo.conf";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallManifest {
    schema_version: u32,
    files: Vec<InstalledFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstalledFile {
    name: String,
    size: u64,
    hash: String,
    #[serde(default)]
    backup: Option<BackupFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupFile {
    stored_name: String,
    original_name: String,
    size: u64,
    hash: String,
}

#[derive(Debug)]
struct Snapshot {
    path: PathBuf,
    bytes: Option<Vec<u8>>,
}

pub fn template_dir(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let bundled = resource_dir.join("dgvoodoo");
        if template_is_complete(&bundled) {
            return Ok(bundled);
        }
    }

    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/dgvoodoo");
    if template_is_complete(&dev) {
        return Ok(dev);
    }

    Err("Plantilla dgVoodoo no encontrada en el launcher".to_string())
}

pub fn template_is_complete(dir: &Path) -> bool {
    dir.is_dir()
        && !dir.is_symlink()
        && TEMPLATE_FILES.iter().all(|file| {
            let path = dir.join(file);
            path.is_file() && !path.is_symlink()
        })
}

pub fn install_files(app: &AppHandle, game_dir: &Path) -> Result<Vec<String>, String> {
    validate_install_root(game_dir)?;
    let templates = template_dir(app)?;
    let previous = read_manifest(game_dir)?;
    let backup_root = game_dir.join(BACKUP_DIR);
    validate_optional_regular_path(&manifest_path(game_dir))?;
    validate_optional_directory(&backup_root)?;

    let mut snapshots = Vec::new();
    let mut seen_snapshots = HashSet::new();
    let backup_dir_existed = backup_root.is_dir();
    let result = (|| {
        std::fs::create_dir_all(&backup_root)
            .map_err(|error| format!("No se pudo crear el respaldo de dgVoodoo: {error}"))?;

        let mut records = Vec::new();
        let mut installed = Vec::new();
        for (index, name) in TEMPLATE_FILES.iter().enumerate() {
            let source = templates.join(name);
            let source_bytes = read_regular_file(&source)?;
            let existing = find_unique_entry_case_insensitive(game_dir, name)?;
            let target = existing.clone().unwrap_or_else(|| game_dir.join(name));
            validate_optional_regular_path(&target)?;

            let previous_record = previous.as_ref().and_then(|manifest| {
                manifest
                    .files
                    .iter()
                    .find(|record| record.name.eq_ignore_ascii_case(name))
            });
            let current_bytes = if target.exists() {
                Some(read_regular_file(&target)?)
            } else {
                None
            };
            if let (Some(record), Some(bytes)) = (previous_record, current_bytes.as_ref()) {
                if name.eq_ignore_ascii_case(MUTABLE_CONFIG) && !record_matches_bytes(record, bytes)
                {
                    // El CPL modifica legítimamente dgVoodoo.conf. Una reparación repone los
                    // binarios faltantes sin borrar esa configuración ni reemplazar su backup.
                    records.push(record.clone());
                    continue;
                }
            }
            let backup = match (previous_record, current_bytes.as_ref()) {
                (Some(record), Some(bytes)) if record_matches_bytes(record, bytes) => {
                    record.backup.clone()
                }
                // Si falta un archivo previamente administrado, se repone sin perder el respaldo
                // del original que existía antes de instalar dgVoodoo.
                (Some(record), None) => record.backup.clone(),
                (Some(_), Some(_)) => {
                    return Err(format!(
                        "{name} fue modificado después de instalarse; no se reemplazará ni se sobrescribirá su respaldo"
                    ));
                }
                (None, Some(bytes)) => {
                    let original_name = target
                        .file_name()
                        .and_then(|value| value.to_str())
                        .ok_or_else(|| format!("Nombre inválido: {}", target.display()))?;
                    let stored_name = format!("{index:02}-{name}.original");
                    let backup_path = backup_root.join(&stored_name);
                    snapshot_once(&backup_path, &mut snapshots, &mut seen_snapshots)?;
                    atomic_write_regular(&backup_path, bytes)?;
                    Some(BackupFile {
                        stored_name,
                        original_name: original_name.to_string(),
                        size: bytes.len() as u64,
                        hash: hash_bytes(bytes),
                    })
                }
                (None, None) => None,
            };

            snapshot_once(&target, &mut snapshots, &mut seen_snapshots)?;
            atomic_write_regular(&target, &source_bytes)?;
            records.push(InstalledFile {
                name: target
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or(name)
                    .to_string(),
                size: source_bytes.len() as u64,
                hash: hash_bytes(&source_bytes),
                backup,
            });
            installed.push((*name).to_string());
        }

        write_manifest(
            game_dir,
            &InstallManifest {
                schema_version: MANIFEST_SCHEMA,
                files: records,
            },
        )?;
        Ok(installed)
    })();

    if let Err(error) = result {
        rollback(&snapshots);
        if !backup_dir_existed {
            let _ = std::fs::remove_dir(&backup_root);
        }
        return Err(error);
    }
    result
}

pub fn uninstall_files(game_dir: &Path) -> Result<Vec<String>, String> {
    validate_install_root(game_dir)?;
    let manifest = read_manifest(game_dir)?.ok_or_else(|| {
        "No se puede determinar qué archivos pertenecen al launcher; no se eliminará nada"
            .to_string()
    })?;
    let backup_root = game_dir.join(BACKUP_DIR);
    validate_optional_directory(&backup_root)?;

    // Preflight completo: si el usuario modificó un wrapper no hacemos una desinstalación parcial.
    let mut operations = Vec::new();
    for owned in &manifest.files {
        let target = find_unique_entry_case_insensitive(game_dir, &owned.name)?
            .unwrap_or_else(|| game_dir.join(&owned.name));
        validate_optional_regular_path(&target)?;
        let mut preserved_config = None;
        if target.exists() {
            let bytes = read_regular_file(&target)?;
            if !record_matches_bytes(owned, &bytes) {
                if owned.name.eq_ignore_ascii_case(MUTABLE_CONFIG) {
                    preserved_config = Some((preserved_config_path(game_dir)?, bytes));
                } else {
                    return Err(format!(
                        "{} fue modificado después de instalarse; no se eliminará ningún archivo",
                        owned.name
                    ));
                }
            }
        }

        let backup = if let Some(backup) = &owned.backup {
            let path = backup_root.join(&backup.stored_name);
            let bytes = read_regular_file(&path)?;
            if bytes.len() as u64 != backup.size || hash_bytes(&bytes) != backup.hash {
                return Err(format!(
                    "El respaldo original de {} está dañado; no se eliminará ningún archivo",
                    owned.name
                ));
            }
            Some((backup.clone(), path, bytes))
        } else {
            None
        };
        operations.push((owned.clone(), target, backup, preserved_config));
    }

    let mut snapshots = Vec::new();
    let mut seen = HashSet::new();
    let result = (|| {
        let mut removed = Vec::new();
        for (owned, target, backup, preserved_config) in &operations {
            snapshot_once(target, &mut snapshots, &mut seen)?;
            if let Some((preserved_path, bytes)) = preserved_config {
                snapshot_once(preserved_path, &mut snapshots, &mut seen)?;
                atomic_write_regular(preserved_path, bytes)?;
            }
            if let Some((backup, _, bytes)) = backup {
                let original = game_dir.join(&backup.original_name);
                validate_optional_regular_path(&original)?;
                snapshot_once(&original, &mut snapshots, &mut seen)?;
                atomic_write_regular(&original, bytes)?;
                if original != *target && target.exists() {
                    std::fs::remove_file(target).map_err(|error| error.to_string())?;
                }
                removed.push(if preserved_config.is_some() {
                    format!(
                        "{} (configuración editada preservada y original restaurado)",
                        owned.name
                    )
                } else {
                    format!("{} (original restaurado)", owned.name)
                });
            } else if target.exists() {
                std::fs::remove_file(target).map_err(|error| error.to_string())?;
                removed.push(if preserved_config.is_some() {
                    format!("{} (configuración editada preservada)", owned.name)
                } else {
                    owned.name.clone()
                });
            }
        }
        std::fs::remove_file(manifest_path(game_dir)).map_err(|error| error.to_string())?;
        Ok(removed)
    })();

    if let Err(error) = result {
        rollback(&snapshots);
        return Err(format!(
            "No se pudo desinstalar dgVoodoo de forma segura: {error}"
        ));
    }

    for (_, _, backup, _) in operations {
        if let Some((_, path, _)) = backup {
            let _ = std::fs::remove_file(path);
        }
    }
    let _ = std::fs::remove_dir(&backup_root);
    result
}

pub fn verify_wrapper_files(app: &AppHandle, game_dir: &Path) -> Result<(), Vec<String>> {
    let template = template_dir(app).map_err(|error| vec![error])?;
    let mut issues = Vec::new();

    for name in ["D3DImm.dll", "DDraw.dll"] {
        let path = match find_unique_entry_case_insensitive(game_dir, name) {
            Ok(Some(path)) => path,
            Ok(None) => {
                issues.push(format!("Falta {name}"));
                continue;
            }
            Err(error) => {
                issues.push(error);
                continue;
            }
        };
        let current = match read_regular_file(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                issues.push(error);
                continue;
            }
        };
        let bundled = read_regular_file(&template.join(name))
            .is_ok_and(|bytes| hash_bytes(&bytes) == hash_bytes(&current));
        if !bundled {
            issues.push(format!(
                "{name} no coincide con el wrapper verificado del launcher; no se forzará su carga"
            ));
        }
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

pub fn entry_collision_issues(game_dir: &Path) -> Vec<String> {
    TEMPLATE_FILES
        .iter()
        .filter_map(|name| find_unique_entry_case_insensitive(game_dir, name).err())
        .collect()
}

pub fn has_install_manifest(game_dir: &Path) -> bool {
    read_manifest(game_dir).is_ok_and(|manifest| manifest.is_some())
}

pub fn install_manifest_issue(game_dir: &Path) -> Option<String> {
    read_manifest(game_dir).err()
}

fn validate_install_root(game_dir: &Path) -> Result<(), String> {
    if !game_dir.is_dir() || game_dir.is_symlink() {
        return Err("La carpeta del juego debe ser un directorio real, no un symlink".to_string());
    }
    validate_optional_regular_path(&manifest_path(game_dir))
}

fn validate_optional_directory(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("Ruta insegura para dgVoodoo: {}", path.display()));
    }
    Ok(())
}

fn validate_optional_regular_path(path: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Err(format!(
            "Se rechazó una ruta no regular o symlink: {}",
            path.display()
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn find_unique_entry_case_insensitive(
    dir: &Path,
    filename: &str,
) -> Result<Option<PathBuf>, String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|error| format!("No se pudo inspeccionar {}: {error}", dir.display()))?;
    let mut matches = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("No se pudo inspeccionar {}: {error}", dir.display()))?;
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.eq_ignore_ascii_case(filename))
        {
            matches.push(entry.path());
        }
    }
    if matches.len() > 1 {
        return Err(format!(
            "Hay varias entradas que Wine trataría como {filename}: {} y {}; resuelve la colisión antes de administrar dgVoodoo",
            matches[0].display(),
            matches[1].display()
        ));
    }
    Ok(matches.pop())
}

fn manifest_path(game_dir: &Path) -> PathBuf {
    game_dir.join(INSTALL_MANIFEST)
}

fn read_manifest(game_dir: &Path) -> Result<Option<InstallManifest>, String> {
    let path = manifest_path(game_dir);
    validate_optional_regular_path(&path)?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(&path)
        .map_err(|error| format!("No se pudo leer {}: {error}", path.display()))?;
    let manifest: InstallManifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("El manifiesto dgVoodoo está dañado: {error}"))?;
    validate_manifest(&manifest)?;
    Ok(Some(manifest))
}

fn validate_manifest(manifest: &InstallManifest) -> Result<(), String> {
    if !matches!(manifest.schema_version, 1 | MANIFEST_SCHEMA) {
        return Err(format!(
            "Schema de manifiesto dgVoodoo no compatible: {}",
            manifest.schema_version
        ));
    }
    if manifest.files.is_empty() || manifest.files.len() > TEMPLATE_FILES.len() {
        return Err("El manifiesto dgVoodoo tiene una lista de archivos inválida".to_string());
    }

    let mut seen = HashSet::new();
    for record in &manifest.files {
        validate_safe_basename(&record.name)?;
        let Some((index, canonical_name)) = TEMPLATE_FILES
            .iter()
            .enumerate()
            .find(|(_, name)| name.eq_ignore_ascii_case(&record.name))
        else {
            return Err(format!(
                "El manifiesto dgVoodoo declara un archivo ajeno: {}",
                record.name
            ));
        };
        if !seen.insert(canonical_name.to_ascii_lowercase()) {
            return Err(format!(
                "El manifiesto dgVoodoo repite el archivo {}",
                record.name
            ));
        }
        validate_hash(&record.hash)?;

        if let Some(backup) = &record.backup {
            validate_safe_basename(&backup.stored_name)?;
            validate_safe_basename(&backup.original_name)?;
            let expected_stored = format!("{index:02}-{canonical_name}.original");
            if backup.stored_name != expected_stored
                || !backup.original_name.eq_ignore_ascii_case(&record.name)
            {
                return Err(format!(
                    "El respaldo declarado para {} no pertenece a ese archivo",
                    record.name
                ));
            }
            validate_hash(&backup.hash)?;
        }
    }
    Ok(())
}

fn validate_safe_basename(name: &str) -> Result<(), String> {
    let path = Path::new(name);
    let single_component = !name.is_empty()
        && !path.is_absolute()
        && path
            .parent()
            .is_some_and(|parent| parent.as_os_str().is_empty())
        && path.file_name().is_some_and(|file_name| file_name == name);
    if !single_component || matches!(name, "." | "..") {
        return Err(format!(
            "El manifiesto dgVoodoo contiene una ruta insegura: {name}"
        ));
    }
    Ok(())
}

fn validate_hash(hash: &str) -> Result<(), String> {
    if hash.len() == 16 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err("El manifiesto dgVoodoo contiene un hash inválido".to_string())
    }
}

fn preserved_config_path(game_dir: &Path) -> Result<PathBuf, String> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    for suffix in 0..100u32 {
        let path = game_dir.join(format!(
            ".ro-launcher-dgvoodoo-user-{timestamp}-{}-{suffix}.conf",
            std::process::id()
        ));
        validate_optional_regular_path(&path)?;
        if !path.exists() {
            return Ok(path);
        }
    }
    Err("No se pudo reservar un nombre para preservar dgVoodoo.conf".to_string())
}

fn write_manifest(game_dir: &Path, manifest: &InstallManifest) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(manifest).map_err(|error| error.to_string())?;
    atomic_write_regular(&manifest_path(game_dir), &json)
        .map_err(|error| format!("No se pudo registrar la instalación de dgVoodoo: {error}"))
}

fn read_regular_file(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("No se pudo inspeccionar {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "Se rechazó una ruta no regular: {}",
            path.display()
        ));
    }
    std::fs::read(path).map_err(|error| format!("No se pudo leer {}: {error}", path.display()))
}

fn atomic_write_regular(path: &Path, bytes: &[u8]) -> Result<(), String> {
    validate_optional_regular_path(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| format!("Ruta sin directorio padre: {}", path.display()))?;
    let temporary = parent.join(format!(
        ".dgvoodoo.tmp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    std::fs::rename(&temporary, path).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        error.to_string()
    })
}

fn snapshot_once(
    path: &Path,
    snapshots: &mut Vec<Snapshot>,
    seen: &mut HashSet<PathBuf>,
) -> Result<(), String> {
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

fn record_matches_bytes(record: &InstalledFile, bytes: &[u8]) -> bool {
    bytes.len() as u64 == record.size && hash_bytes(bytes) == record.hash
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:016x}", fnv1a(bytes))
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn test_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ro-launcher-dgvoodoo-{label}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn record(name: &str) -> InstalledFile {
        InstalledFile {
            name: name.to_string(),
            size: 1,
            hash: "0000000000000000".to_string(),
            backup: None,
        }
    }

    #[test]
    fn rejects_symlink_destinations() {
        let dir = test_dir("symlink");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.join("missing"), dir.join("DDraw.dll")).unwrap();
        assert!(validate_optional_regular_path(&dir.join("DDraw.dll")).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_manifest_paths_outside_the_game_directory() {
        let manifest = InstallManifest {
            schema_version: MANIFEST_SCHEMA,
            files: vec![record("../../DDraw.dll")],
        };
        assert!(validate_manifest(&manifest).is_err());

        let mut owned = record("DDraw.dll");
        owned.backup = Some(BackupFile {
            stored_name: "../../outside".to_string(),
            original_name: "DDraw.dll".to_string(),
            size: 1,
            hash: "0000000000000000".to_string(),
        });
        assert!(validate_manifest(&InstallManifest {
            schema_version: MANIFEST_SCHEMA,
            files: vec![owned],
        })
        .is_err());
    }

    #[test]
    fn accepts_only_expected_unique_template_records() {
        let valid = InstallManifest {
            schema_version: MANIFEST_SCHEMA,
            files: vec![record("DDraw.dll")],
        };
        assert!(validate_manifest(&valid).is_ok());

        let duplicate = InstallManifest {
            schema_version: MANIFEST_SCHEMA,
            files: vec![record("DDraw.dll"), record("ddraw.DLL")],
        };
        assert!(validate_manifest(&duplicate).is_err());
    }

    #[test]
    fn rejects_case_insensitive_file_collisions() {
        let dir = test_dir("case-collision");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("DDraw.dll"), b"first").unwrap();
        std::fs::write(dir.join("ddraw.DLL"), b"second").unwrap();

        let error = find_unique_entry_case_insensitive(&dir, "ddraw.dll").unwrap_err();
        assert!(error.contains("varias entradas"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn reports_collisions_for_config_and_control_panel_too() {
        let dir = test_dir("all-case-collisions");
        std::fs::create_dir_all(&dir).unwrap();
        for name in [
            "dgVoodoo.conf",
            "DGVOODOO.CONF",
            "dgVoodooCpl.exe",
            "DGVOODOOCPL.EXE",
        ] {
            std::fs::write(dir.join(name), b"duplicate").unwrap();
        }

        let issues = entry_collision_issues(&dir);
        assert_eq!(issues.len(), 2);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn uninstall_preserves_edited_config_and_restores_original() {
        let dir = test_dir("mutable-config");
        let backup_root = dir.join(BACKUP_DIR);
        std::fs::create_dir_all(&backup_root).unwrap();
        let current = b"edited by cpl";
        let installed = b"launcher template";
        let original = b"original server config";
        std::fs::write(dir.join(MUTABLE_CONFIG), current).unwrap();

        let index = TEMPLATE_FILES
            .iter()
            .position(|name| name.eq_ignore_ascii_case(MUTABLE_CONFIG))
            .unwrap();
        let stored_name = format!("{index:02}-{MUTABLE_CONFIG}.original");
        std::fs::write(backup_root.join(&stored_name), original).unwrap();
        let manifest = InstallManifest {
            schema_version: MANIFEST_SCHEMA,
            files: vec![InstalledFile {
                name: MUTABLE_CONFIG.to_string(),
                size: installed.len() as u64,
                hash: hash_bytes(installed),
                backup: Some(BackupFile {
                    stored_name,
                    original_name: MUTABLE_CONFIG.to_string(),
                    size: original.len() as u64,
                    hash: hash_bytes(original),
                }),
            }],
        };
        write_manifest(&dir, &manifest).unwrap();

        uninstall_files(&dir).unwrap();
        assert_eq!(std::fs::read(dir.join(MUTABLE_CONFIG)).unwrap(), original);
        let preserved = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(".ro-launcher-dgvoodoo-user-"))
            })
            .unwrap();
        assert_eq!(std::fs::read(preserved).unwrap(), current);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
