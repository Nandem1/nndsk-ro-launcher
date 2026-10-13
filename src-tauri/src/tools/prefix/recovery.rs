//! Crash recovery for managed prefix provisioning and reset (F-05).
//!
//! # Evidence
//!
//! Recovery never journals intent. It reads three pieces of on-disk evidence:
//!
//! * `R`: the canonical managed root (`location.path`).
//! * `B`: one fixed sibling, `<parent>/.<name>.reset-backup`, holding the pre-reset prefix. There
//!   is never more than one; a new reset refuses to start while it exists.
//! * `M`: `R/.ro-launcher-provisioning`, written durably right after the launcher creates `R`.
//!   It proves "the launcher created this root for this location and has not committed it". It
//!   records the location and fingerprint, never instructions.
//! * The existing prefix manifest (`R/.ro-launcher-prefix.json`, v2 or v3) is the **only** commit
//!   record. Once it is durable for this location, the new prefix wins.
//!
//! # Root classification (`RootState`)
//!
//! | State          | Evidence                                                              |
//! | -------------- | --------------------------------------------------------------------- |
//! | `Absent`       | `R` does not exist                                                    |
//! | `Empty`        | `R` is a real directory without entries                               |
//! | `PendingMarker`| `R` contains only the fixed temporary marker (killed while writing M) |
//! | `Provisioning` | `M` is a regular file for this location; no manifest; no protection   |
//! | `Committed`    | a valid manifest for this location (stale `M` noted, not trusted)     |
//! | `LegacyValid`  | legacy configured marker plus a complete Wine structure; no `M`       |
//! | `Foreign`      | anything else, including symlinks and data without launcher evidence  |
//!
//! # Backup classification (`BackupState`)
//!
//! | State        | Evidence                                                                    |
//! | ------------ | --------------------------------------------------------------------------- |
//! | `Absent`     | `B` does not exist                                                          |
//! | `Empty`      | `B` is a real empty directory (holds no data; removed in every state)       |
//! | `Restorable` | real directory on the parent's filesystem, no game-installation protection, |
//! |              | and a valid manifest for this location or legacy marker + structure          |
//! | `Removing`   | as above but only the legacy marker survives: removal after commit, killed  |
//! | `Foreign`    | symlink, file, other filesystem, protected installation, or no evidence     |
//!
//! # Recovery table (`plan`)
//!
//! | R \ B                    | Absent                        | Restorable                | Removing          | Foreign  |
//! | ------------------------ | ----------------------------- | ------------------------- | ----------------- | -------- |
//! | Absent / Empty           | nothing (or one legacy backup)| restore `B -> R`          | refuse            | refuse   |
//! | PendingMarker            | discard ours                  | discard ours, restore `B` | refuse            | refuse   |
//! | Provisioning             | discard ours                  | discard ours, restore `B` | refuse            | refuse   |
//! | Committed                | drop stale `M`                | drop stale `M`, remove `B`| same              | refuse   |
//! | LegacyValid              | nothing                       | refuse                    | refuse            | refuse   |
//! | Foreign                  | nothing (existing refusal)    | refuse                    | refuse            | refuse   |
//!
//! "refuse" preserves every path and returns an actionable error. An empty `B` is removed first and
//! then treated as absent. Every destructive transition is self-stabilizing under SIGKILL:
//!
//! * discarding `R` deletes everything except `M` first, then `M`, then `R`; a kill leaves either
//!   `Provisioning` (M survives) or `Empty`;
//! * removing `B` keeps its manifest/legacy marker for last; a kill leaves `Restorable`/`Removing`
//!   or `Empty`, all of which the committed row completes;
//! * restoring is one `renameat2(RENAME_NOREPLACE)`; a kill leaves `R` committed with `B` absent.
//!
//! Recovery is idempotent: every post-recovery state maps to "nothing". It runs only for managed
//! locations, under the exclusive `OperationGuard("prefix", R)`, and refuses destructive actions
//! while any process uses the prefix. Custom prefixes are never classified destructively.
//!
//! Durability: every create/rename/unlink is followed by an fsync of its directory. This is
//! qualified against process death (SIGKILL); power loss is implemented-but-unqualified.

use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use ro_tools_linux::find_prefix_processes;
use serde::{Deserialize, Serialize};

use crate::utils::{
    backup_path, ensure_managed_path_safe, inspect_prefix, is_v3_managed_prefix_path,
    manifest_matches_stored_location, rename_noreplace, sync_dir, sync_filesystem,
    v3_path_digest_prefix24, PrefixLocation, PrefixScope, INSTALLATION_MARKER,
    LEGACY_PREFIX_MARKER, PREFIX_MARKER, PROVISIONING_MARKER, PROVISIONING_MARKER_TMP,
};

const MARKER_SCHEMA: u32 = 1;
const MAX_MARKER_BYTES: u64 = 16 * 1024;
const RESET_BACKUP_SUFFIX: &str = "reset-backup";

/// User-facing message when recovery replaced the root under an operation that already validated
/// the previous state. Retrying re-evaluates runner, manifest and protection from scratch.
pub const RECOVERED_RETRY_MSG: &str =
    "Se recuperó una preparación interrumpida del entorno; vuelve a intentarlo";

/// Test-only rendezvous for SIGKILL fault injection. Compiles to nothing outside `cfg(test)`.
macro_rules! checkpoint {
    ($name:literal) => {
        #[cfg(test)]
        $crate::tools::prefix::recovery::fault::reached($name);
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProvisioningMarker {
    schema_version: u32,
    scope: PrefixScope,
    server_id: Option<String>,
    prefix_path: String,
    prefix_fingerprint: String,
    created_at_ms: u64,
}

impl ProvisioningMarker {
    fn matches(&self, location: &PrefixLocation) -> bool {
        self.schema_version == MARKER_SCHEMA
            && self.scope == location.scope
            && self.server_id == location.server_id
            && Path::new(&self.prefix_path) == Path::new(&location.path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RootState {
    Absent,
    Empty,
    PendingMarker,
    Provisioning,
    Committed { stale_marker: bool },
    LegacyValid,
    Foreign(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BackupState {
    Absent,
    Empty,
    Restorable,
    Removing,
    Foreign(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryAction {
    None,
    RestoreBackup,
    DiscardPartialAndRestore,
    DiscardPartial,
    CompleteCommit { stale_marker: bool, backup: bool },
    RestoreLegacyBackup(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryOutcome {
    pub action: RecoveryAction,
    pub removed_empty_backup: bool,
    /// Evidence that needs a human (legacy residue). Callers report it; recovery stays pure.
    pub notes: Vec<String>,
}

impl RecoveryOutcome {
    pub fn untouched() -> Self {
        Self {
            action: RecoveryAction::None,
            removed_empty_backup: false,
            notes: Vec::new(),
        }
    }

    /// The canonical root now holds different data than before recovery.
    pub fn root_replaced(&self) -> bool {
        matches!(
            self.action,
            RecoveryAction::RestoreBackup
                | RecoveryAction::DiscardPartialAndRestore
                | RecoveryAction::DiscardPartial
                | RecoveryAction::RestoreLegacyBackup(_)
        )
    }

    /// The pre-reset prefix is back at the canonical root.
    pub fn restored_previous(&self) -> bool {
        matches!(
            self.action,
            RecoveryAction::RestoreBackup
                | RecoveryAction::DiscardPartialAndRestore
                | RecoveryAction::RestoreLegacyBackup(_)
        )
    }

    pub fn summary(&self) -> String {
        match &self.action {
            RecoveryAction::None => String::new(),
            RecoveryAction::RestoreBackup | RecoveryAction::DiscardPartialAndRestore => {
                "El entorno anterior fue restaurado correctamente".to_string()
            }
            RecoveryAction::DiscardPartial => {
                "Se retiró el entorno incompleto creado por el launcher".to_string()
            }
            RecoveryAction::CompleteCommit { .. } => {
                "El entorno nuevo ya estaba confirmado y se conservó".to_string()
            }
            RecoveryAction::RestoreLegacyBackup(path) => format!(
                "El entorno anterior fue restaurado desde {}",
                path.display()
            ),
        }
    }
}

/// A managed root that recovery may act on. Construction is the safety boundary: custom and
/// unexpected paths can never reach the destructive helpers below.
pub(crate) struct ManagedPrefix<'a> {
    location: &'a PrefixLocation,
    root: PathBuf,
    parent: PathBuf,
    backup: PathBuf,
}

impl<'a> ManagedPrefix<'a> {
    pub(crate) fn new(location: &'a PrefixLocation) -> Result<Self, String> {
        if !location.managed || location.scope == PrefixScope::Custom {
            return Err(
                "Por seguridad, el launcher no recupera ni elimina WINEPREFIX personalizados"
                    .to_string(),
            );
        }
        ensure_managed_path_safe(location)?;
        Self::at(location)
    }

    /// Temp-dir tests only: skips the managed-path check that binds real paths to `$HOME`.
    #[cfg(test)]
    pub(crate) fn for_test(location: &'a PrefixLocation) -> Self {
        assert!(location.managed && location.scope != PrefixScope::Custom);
        Self::at(location).unwrap()
    }

    fn at(location: &'a PrefixLocation) -> Result<Self, String> {
        let root = PathBuf::from(&location.path);
        let parent = root
            .parent()
            .ok_or_else(|| "La ruta del entorno no tiene directorio padre".to_string())?
            .to_path_buf();
        let backup = reset_backup_path(&root)?;
        Ok(Self {
            location,
            root,
            parent,
            backup,
        })
    }

    pub(crate) fn backup(&self) -> &Path {
        &self.backup
    }

    pub(crate) fn classify_root(&self) -> Result<RootState, String> {
        classify_root(&self.root, self.location)
    }

    pub(crate) fn classify_backup(&self) -> Result<BackupState, String> {
        classify_backup(&self.backup, &self.parent, self.location)
    }

    /// Reconcile whatever an interrupted transaction left behind. Caller holds the exclusive
    /// prefix guard. Safe to run any number of times.
    pub(crate) fn recover(&self) -> Result<RecoveryOutcome, String> {
        let root_state = self.classify_root()?;
        let mut backup_state = self.classify_backup()?;
        let removed_empty_backup = backup_state == BackupState::Empty;
        if removed_empty_backup {
            backup_state = BackupState::Absent;
        }
        let legacy = legacy_reset_backups(&self.root);
        let legacy_restorable: Vec<PathBuf> =
            if matches!(root_state, RootState::Absent | RootState::Empty)
                && backup_state == BackupState::Absent
            {
                legacy
                    .iter()
                    .filter(|path| {
                        classify_backup(path, &self.parent, self.location)
                            == Ok(BackupState::Restorable)
                    })
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            };
        let action = plan(&root_state, &backup_state, &legacy_restorable)
            .map_err(|reason| self.ambiguity(&reason))?;

        let mut notes = Vec::new();
        let unrestored_legacy: Vec<&PathBuf> = legacy
            .iter()
            .filter(|path| !matches!(&action, RecoveryAction::RestoreLegacyBackup(chosen) if chosen == *path))
            .collect();
        if !unrestored_legacy.is_empty() {
            notes.push(format!(
                "Se conservaron respaldos de reinicio de versiones anteriores sin tocarlos: {}",
                unrestored_legacy
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        if action == RecoveryAction::None && !removed_empty_backup {
            return Ok(RecoveryOutcome {
                action,
                removed_empty_backup,
                notes,
            });
        }
        self.ensure_quiet()?;
        if removed_empty_backup {
            fs::remove_dir(&self.backup)
                .map_err(|error| self.io("retirar el respaldo vacío", &error))?;
            self.sync_parent()?;
        }
        self.execute(&action, &root_state)?;
        Ok(RecoveryOutcome {
            action,
            removed_empty_backup,
            notes,
        })
    }

    fn execute(&self, action: &RecoveryAction, root_state: &RootState) -> Result<(), String> {
        match action {
            RecoveryAction::None => Ok(()),
            RecoveryAction::RestoreBackup => {
                self.clear_non_data_root(root_state)?;
                self.restore_from(&self.backup.clone())
            }
            RecoveryAction::DiscardPartialAndRestore => {
                self.discard_partial_root()?;
                checkpoint!("recovery:partial-removed");
                self.restore_from(&self.backup.clone())
            }
            RecoveryAction::DiscardPartial => {
                if *root_state == RootState::PendingMarker {
                    self.clear_non_data_root(root_state)
                } else {
                    self.discard_partial_root()
                }
            }
            RecoveryAction::CompleteCommit {
                stale_marker,
                backup,
            } => {
                if *stale_marker {
                    for name in [PROVISIONING_MARKER, PROVISIONING_MARKER_TMP] {
                        remove_file_if_present(&self.root.join(name)).map_err(|error| {
                            self.io("retirar el marcador de preparación", &error)
                        })?;
                    }
                    sync_dir(&self.root)
                        .map_err(|error| self.io("sincronizar el entorno confirmado", &error))?;
                }
                checkpoint!("recovery:marker-removed");
                if *backup {
                    remove_tree_evidence_last(&self.backup, &[PREFIX_MARKER, LEGACY_PREFIX_MARKER])
                        .map_err(|error| {
                            self.io("retirar el respaldo del entorno anterior", &error)
                        })?;
                    self.sync_parent()?;
                }
                Ok(())
            }
            RecoveryAction::RestoreLegacyBackup(path) => {
                self.clear_non_data_root(root_state)?;
                self.restore_from(path)
            }
        }
    }

    /// Prepare the canonical root for a reset: move a committed or legacy-valid prefix to the
    /// single fixed backup. Returns whether a prefix was preserved.
    pub(crate) fn preserve_for_reset(&self) -> Result<bool, String> {
        match self.classify_root()? {
            RootState::Absent => return Ok(false),
            RootState::Empty => {
                fs::remove_dir(&self.root)
                    .map_err(|error| self.io("retirar el directorio vacío", &error))?;
                self.sync_parent()?;
                return Ok(false);
            }
            RootState::Committed {
                stale_marker: false,
            }
            | RootState::LegacyValid => {}
            other => {
                return Err(format!(
                    "El entorno {} no está en un estado confirmado ({other:?}); no se reconstruirá",
                    self.root.display()
                ))
            }
        }
        if entry_kind(&self.backup).map_err(|error| self.io("revisar el respaldo", &error))?
            != EntryKind::Missing
        {
            return Err(self.ambiguity("ya existe un respaldo pendiente"));
        }
        if is_protected(&self.root).map_err(|error| self.io("revisar la protección", &error))? {
            return Err("Este entorno contiene una instalación de juego y no puede reconstruirse automáticamente".into());
        }
        rename_noreplace(&self.root, &self.backup)
            .map_err(|error| self.io("preservar el entorno anterior", &error))?;
        self.sync_parent()?;
        checkpoint!("reset:backup-preserved");
        Ok(true)
    }

    /// Create (or adopt an already empty) canonical root and make `M` durable before any
    /// provisioning write. From this point a crash is recoverable from disk alone.
    pub(crate) fn begin_provisioning(&self, fingerprint: &str) -> Result<(), String> {
        fs::create_dir_all(&self.parent)
            .map_err(|error| self.io("crear el directorio de entornos", &error))?;
        match self.classify_root()? {
            RootState::Absent => fs::create_dir(&self.root)
                .map_err(|error| self.io("crear el entorno", &error))?,
            RootState::Empty => {}
            other => {
                return Err(format!(
                    "El entorno {} ya contiene datos ({other:?}); no se iniciará una preparación limpia",
                    self.root.display()
                ))
            }
        }
        self.sync_parent()?;
        checkpoint!("provision:root-created");
        let marker = ProvisioningMarker {
            schema_version: MARKER_SCHEMA,
            scope: self.location.scope,
            server_id: self.location.server_id.clone(),
            prefix_path: self.location.path.clone(),
            prefix_fingerprint: fingerprint.to_string(),
            created_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_millis() as u64)
                .unwrap_or_default(),
        };
        let json = serde_json::to_vec_pretty(&marker).map_err(|error| error.to_string())?;
        let temporary = self.root.join(PROVISIONING_MARKER_TMP);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|error| self.io("crear el marcador de preparación", &error))?;
        file.write_all(&json)
            .and_then(|_| file.sync_all())
            .map_err(|error| self.io("escribir el marcador de preparación", &error))?;
        drop(file);
        rename_noreplace(&temporary, &self.root.join(PROVISIONING_MARKER))
            .map_err(|error| self.io("activar el marcador de preparación", &error))?;
        sync_dir(&self.root)
            .map_err(|error| self.io("sincronizar el marcador de preparación", &error))?;
        checkpoint!("provision:marker-durable");
        Ok(())
    }

    /// Flush provisioned payload before the manifest can commit it.
    pub(crate) fn sync_payload(&self) -> Result<(), String> {
        sync_filesystem(&self.root).map_err(|error| self.io("sincronizar el entorno nuevo", &error))
    }

    fn clear_non_data_root(&self, root_state: &RootState) -> Result<(), String> {
        match root_state {
            RootState::Absent => Ok(()),
            RootState::Empty => {
                fs::remove_dir(&self.root)
                    .map_err(|error| self.io("retirar el directorio vacío", &error))?;
                self.sync_parent()
            }
            RootState::PendingMarker => {
                remove_file_if_present(&self.root.join(PROVISIONING_MARKER_TMP))
                    .map_err(|error| self.io("retirar el marcador temporal", &error))?;
                fs::remove_dir(&self.root)
                    .map_err(|error| self.io("retirar el entorno incompleto", &error))?;
                self.sync_parent()
            }
            other => Err(format!("estado inesperado del entorno: {other:?}")),
        }
    }

    fn discard_partial_root(&self) -> Result<(), String> {
        remove_tree_evidence_last(&self.root, &[PROVISIONING_MARKER])
            .map_err(|error| self.io("retirar el entorno incompleto", &error))?;
        self.sync_parent()
    }

    fn restore_from(&self, source: &Path) -> Result<(), String> {
        rename_noreplace(source, &self.root)
            .map_err(|error| self.io("restaurar el entorno anterior", &error))?;
        checkpoint!("recovery:backup-restored");
        self.sync_parent()
    }

    fn ensure_quiet(&self) -> Result<(), String> {
        let active = find_prefix_processes(&self.location.path).len();
        if active == 0 {
            Ok(())
        } else {
            Err(format!(
                "Hay {active} proceso(s) usando el entorno {}; ciérralos para recuperar la preparación interrumpida. No se modificó nada",
                self.root.display()
            ))
        }
    }

    fn sync_parent(&self) -> Result<(), String> {
        sync_dir(&self.parent)
            .map_err(|error| self.io("sincronizar el directorio de entornos", &error))
    }

    fn io(&self, action: &str, error: &std::io::Error) -> String {
        format!(
            "No se pudo {action} durante la recuperación de {}: {error}. Los datos restantes se conservaron",
            self.root.display()
        )
    }

    fn ambiguity(&self, reason: &str) -> String {
        format!(
            "El entorno {} tiene un estado de preparación interrumpida que no puede resolverse con seguridad ({reason}). Se conservaron {} y {} sin modificarlos; revísalos antes de volver a preparar",
            self.root.display(),
            self.root.display(),
            self.backup.display()
        )
    }
}

/// The single fixed reset backup for a root. Same parent, so restore is one same-fs rename.
pub(crate) fn reset_backup_path(root: &Path) -> Result<PathBuf, String> {
    let parent = root
        .parent()
        .ok_or_else(|| "La ruta del entorno no tiene directorio padre".to_string())?;
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "La ruta del entorno no tiene nombre válido".to_string())?;
    Ok(parent.join(format!(".{name}.{RESET_BACKUP_SUFFIX}")))
}

/// `.<name>.reset-backup-<pid>-<millis>` written by builds before F-05. Never deleted.
fn legacy_reset_backups(root: &Path) -> Vec<PathBuf> {
    let (Some(parent), Some(name)) = (root.parent(), root.file_name().and_then(|n| n.to_str()))
    else {
        return Vec::new();
    };
    let prefix = format!(".{name}.{RESET_BACKUP_SUFFIX}-");
    let Ok(entries) = fs::read_dir(parent) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| {
            entry.file_name().to_str().is_some_and(|file| {
                file.strip_prefix(&prefix)
                    .and_then(|rest| rest.split_once('-'))
                    .is_some_and(|(pid, millis)| {
                        !pid.is_empty()
                            && !millis.is_empty()
                            && pid.bytes().all(|b| b.is_ascii_digit())
                            && millis.bytes().all(|b| b.is_ascii_digit())
                    })
            })
        })
        .map(|entry| entry.path())
        .collect();
    found.sort();
    found
}

/// Read-only: fixed-name evidence written by this protocol (backup, marker, marker temp).
fn fixed_residue_present(root: &Path) -> bool {
    let present = |path: PathBuf| path.symlink_metadata().is_ok();
    reset_backup_path(root).is_ok_and(present)
        || present(root.join(PROVISIONING_MARKER))
        || present(root.join(PROVISIONING_MARKER_TMP))
}

/// Read-only: does `root` carry any transaction evidence that recovery would act on? Lets hot
/// paths (supervisor bootstrap) skip classification entirely for ordinary prefixes. Legacy
/// timestamped backups only matter while the root holds no data.
pub fn transaction_residue_present(root: &Path) -> bool {
    if fixed_residue_present(root) {
        return true;
    }
    let root_holds_data = root.symlink_metadata().is_ok_and(|meta| {
        !meta.is_dir() || fs::read_dir(root).is_ok_and(|mut entries| entries.next().is_some())
    });
    !root_holds_data && !legacy_reset_backups(root).is_empty()
}

/// Read-only: an interrupted transaction of this protocol left `root` without a commit record.
/// Binding resolution keeps pointing here so recovery, not a fresh candidate path, decides what
/// survives. Pre-F-05 timestamped residue never steers resolution; it is only ever restored when
/// recovery runs at its own root and finds exactly one valid candidate.
pub fn uncommitted_transaction_present(root: &Path) -> bool {
    root.join(PREFIX_MARKER).symlink_metadata().is_err() && fixed_residue_present(root)
}

/// The transition table. Pure, so every row is unit-tested without touching the filesystem.
pub(crate) fn plan(
    root: &RootState,
    backup: &BackupState,
    legacy_restorable: &[PathBuf],
) -> Result<RecoveryAction, String> {
    use BackupState as B;
    use RootState as R;
    match (root, backup) {
        (_, B::Empty) => Err("respaldo vacío sin retirar".into()),
        (_, B::Foreign(reason)) => Err(format!("respaldo no verificable: {reason}")),
        (R::Absent | R::Empty | R::PendingMarker, B::Restorable) => {
            Ok(RecoveryAction::RestoreBackup)
        }
        (R::Absent | R::Empty | R::PendingMarker | R::Provisioning, B::Removing) => {
            Err("el respaldo estaba siendo retirado pero no hay un entorno nuevo confirmado".into())
        }
        (R::Absent | R::Empty, B::Absent) => Ok(match legacy_restorable {
            [only] => RecoveryAction::RestoreLegacyBackup(only.clone()),
            _ => RecoveryAction::None,
        }),
        (R::PendingMarker, B::Absent) => Ok(RecoveryAction::DiscardPartial),
        (R::Provisioning, B::Restorable) => Ok(RecoveryAction::DiscardPartialAndRestore),
        (R::Provisioning, B::Absent) => Ok(RecoveryAction::DiscardPartial),
        (R::Committed { stale_marker }, B::Absent) => Ok(if *stale_marker {
            RecoveryAction::CompleteCommit {
                stale_marker: true,
                backup: false,
            }
        } else {
            RecoveryAction::None
        }),
        (R::Committed { stale_marker }, B::Restorable | B::Removing) => {
            Ok(RecoveryAction::CompleteCommit {
                stale_marker: *stale_marker,
                backup: true,
            })
        }
        (R::LegacyValid, B::Absent) => Ok(RecoveryAction::None),
        (R::LegacyValid, _) => {
            Err("el entorno actual es legacy y además existe un respaldo".into())
        }
        (R::Foreign(_), B::Absent) => Ok(RecoveryAction::None),
        (R::Foreign(reason), _) => Err(format!("el entorno actual no es verificable: {reason}")),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryKind {
    Missing,
    Directory,
    File,
    Other,
}

/// Never follows symlinks: a symlink is always `Other`.
fn entry_kind(path: &Path) -> std::io::Result<EntryKind> {
    match path.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => Ok(EntryKind::Other),
        Ok(meta) if meta.is_dir() => Ok(EntryKind::Directory),
        Ok(meta) if meta.is_file() => Ok(EntryKind::File),
        Ok(_) => Ok(EntryKind::Other),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(EntryKind::Missing),
        Err(error) => Err(error),
    }
}

fn entry_names(dir: &Path) -> std::io::Result<Vec<OsString>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)? {
        names.push(entry?.file_name());
    }
    Ok(names)
}

fn is_protected(dir: &Path) -> std::io::Result<bool> {
    let marker = dir.join(INSTALLATION_MARKER);
    for candidate in [marker.clone(), backup_path(&marker)] {
        if entry_kind(&candidate)? != EntryKind::Missing {
            return Ok(true);
        }
    }
    Ok(false)
}

fn classify_io(path: &Path, error: std::io::Error) -> String {
    format!(
        "No se pudo inspeccionar {} para recuperar la preparación; no se modificó nada: {error}",
        path.display()
    )
}

fn classify_root(root: &Path, location: &PrefixLocation) -> Result<RootState, String> {
    let io = |error| classify_io(root, error);
    match entry_kind(root).map_err(io)? {
        EntryKind::Missing => return Ok(RootState::Absent),
        EntryKind::Directory => {}
        _ => {
            return Ok(RootState::Foreign(
                "la ruta del entorno no es un directorio real".into(),
            ))
        }
    }
    let names = entry_names(root).map_err(io)?;
    if names.is_empty() {
        return Ok(RootState::Empty);
    }
    if names.len() == 1
        && names[0] == PROVISIONING_MARKER_TMP
        && entry_kind(&root.join(PROVISIONING_MARKER_TMP)).map_err(io)? == EntryKind::File
        && root
            .join(PROVISIONING_MARKER_TMP)
            .symlink_metadata()
            .map_err(io)?
            .len()
            <= MAX_MARKER_BYTES
    {
        return Ok(RootState::PendingMarker);
    }
    let marker = root.join(PROVISIONING_MARKER);
    let marker_kind = entry_kind(&marker).map_err(io)?;
    match entry_kind(&root.join(PREFIX_MARKER)).map_err(io)? {
        EntryKind::File => {
            return Ok(match committed_manifest(root, location) {
                Ok(()) => RootState::Committed {
                    stale_marker: marker_kind != EntryKind::Missing
                        || entry_kind(&root.join(PROVISIONING_MARKER_TMP)).map_err(io)?
                            != EntryKind::Missing,
                },
                Err(reason) => RootState::Foreign(reason),
            })
        }
        EntryKind::Missing => {}
        _ => {
            return Ok(RootState::Foreign(
                "el manifiesto no es un archivo regular".into(),
            ))
        }
    }
    match marker_kind {
        EntryKind::File => {
            let marker = match read_marker(&marker) {
                Ok(marker) => marker,
                Err(reason) => return Ok(RootState::Foreign(reason)),
            };
            if !marker.matches(location) {
                return Ok(RootState::Foreign(
                    "el marcador de preparación pertenece a otra ubicación".into(),
                ));
            }
            if is_protected(root).map_err(io)? {
                return Ok(RootState::Foreign(
                    "contiene una instalación de juego protegida".into(),
                ));
            }
            if entry_kind(&root.join(LEGACY_PREFIX_MARKER)).map_err(io)? != EntryKind::Missing {
                return Ok(RootState::Foreign(
                    "combina un marcador legacy con uno de preparación".into(),
                ));
            }
            return Ok(RootState::Provisioning);
        }
        EntryKind::Missing => {}
        _ => {
            return Ok(RootState::Foreign(
                "el marcador de preparación no es un archivo regular".into(),
            ))
        }
    }
    if entry_kind(&root.join(LEGACY_PREFIX_MARKER)).map_err(io)? == EntryKind::File
        && root
            .to_str()
            .is_some_and(|path| inspect_prefix(path).structure_ok)
    {
        return Ok(RootState::LegacyValid);
    }
    Ok(RootState::Foreign(
        "contiene datos sin manifiesto ni marcador del launcher".into(),
    ))
}

fn classify_backup(
    backup: &Path,
    parent: &Path,
    location: &PrefixLocation,
) -> Result<BackupState, String> {
    let io = |error| classify_io(backup, error);
    match entry_kind(backup).map_err(io)? {
        EntryKind::Missing => return Ok(BackupState::Absent),
        EntryKind::Directory => {}
        _ => return Ok(BackupState::Foreign("no es un directorio real".into())),
    }
    let backup_dev = backup.symlink_metadata().map_err(io)?.dev();
    let parent_dev = parent.metadata().map_err(io)?.dev();
    if backup_dev != parent_dev {
        return Ok(BackupState::Foreign(
            "está en otro sistema de archivos".into(),
        ));
    }
    if entry_names(backup).map_err(io)?.is_empty() {
        return Ok(BackupState::Empty);
    }
    if is_protected(backup).map_err(io)? {
        return Ok(BackupState::Foreign(
            "contiene una instalación de juego protegida".into(),
        ));
    }
    match entry_kind(&backup.join(PREFIX_MARKER)).map_err(io)? {
        EntryKind::File => {
            return Ok(match committed_manifest(backup, location) {
                Ok(()) => BackupState::Restorable,
                Err(reason) => BackupState::Foreign(reason),
            })
        }
        EntryKind::Missing => {}
        _ => {
            return Ok(BackupState::Foreign(
                "su manifiesto no es un archivo regular".into(),
            ))
        }
    }
    match entry_kind(&backup.join(LEGACY_PREFIX_MARKER)).map_err(io)? {
        EntryKind::File => Ok(
            if backup
                .to_str()
                .is_some_and(|path| inspect_prefix(path).structure_ok)
            {
                BackupState::Restorable
            } else {
                BackupState::Removing
            },
        ),
        EntryKind::Missing => Ok(BackupState::Foreign(
            "no contiene evidencia del launcher".into(),
        )),
        _ => Ok(BackupState::Foreign(
            "su marcador legacy no es un archivo regular".into(),
        )),
    }
}

/// The manifest at `dir` commits a prefix for `location`: v2/v3, same scope/server, and for a
/// fingerprint-addressed v3 path the digest the path encodes.
fn committed_manifest(dir: &Path, location: &PrefixLocation) -> Result<(), String> {
    let path = dir
        .to_str()
        .ok_or_else(|| "la ruta del entorno no es UTF-8".to_string())?;
    let manifest = inspect_prefix(path)
        .manifest
        .ok_or_else(|| "el manifiesto está dañado o usa un schema incompatible".to_string())?;
    if !manifest_matches_stored_location(&manifest, location) {
        return Err("el manifiesto pertenece a otro servidor o ámbito".into());
    }
    if let Some(suffix) = v3_path_suffix(location) {
        let matches = manifest
            .prefix_fingerprint_digest()
            .is_some_and(|digest| v3_path_digest_prefix24(digest) == suffix);
        if !matches {
            return Err("la identidad v3 del manifiesto no coincide con la ruta".into());
        }
    }
    Ok(())
}

fn v3_path_suffix(location: &PrefixLocation) -> Option<&str> {
    let server_id = location.server_id.as_deref()?;
    if !is_v3_managed_prefix_path(Path::new(&location.path), server_id) {
        return None;
    }
    Path::new(&location.path)
        .file_name()?
        .to_str()?
        .rsplit_once("-v3-")
        .map(|(_, suffix)| suffix)
}

fn read_marker(path: &Path) -> Result<ProvisioningMarker, String> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| format!("no se pudo leer el marcador de preparación: {error}"))?;
    let len = file
        .metadata()
        .map_err(|error| format!("no se pudo leer el marcador de preparación: {error}"))?
        .len();
    if len > MAX_MARKER_BYTES {
        return Err("el marcador de preparación es demasiado grande".into());
    }
    let mut json = String::new();
    file.read_to_string(&mut json)
        .map_err(|_| "el marcador de preparación está dañado".to_string())?;
    serde_json::from_str(&json).map_err(|_| "el marcador de preparación está dañado".to_string())
}

fn remove_file_if_present(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Delete `dir` so a kill at any point leaves either the evidence files or an empty directory.
/// `remove_dir_all` does not follow symlinks inside the tree.
fn remove_tree_evidence_last(dir: &Path, evidence: &[&str]) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if evidence.iter().any(|name| entry.file_name() == *name) {
            continue;
        }
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    checkpoint!("recovery:evidence-only");
    for name in evidence {
        remove_file_if_present(&dir.join(name))?;
    }
    sync_dir(dir)?;
    fs::remove_dir(dir)
}

/// What a managed transaction needs from its provisioning engine. Production wraps a supervised
/// `RunnerOperation`; tests inject an engine that writes representative files.
pub(crate) trait ProvisioningWork {
    async fn provision(&mut self) -> Result<(), String>;
    /// End execution and wait until no process uses the prefix. Must be repeatable.
    async fn quiesce(&mut self) -> Result<(), String>;
    /// Write the final manifest: the commit point.
    fn commit(&mut self) -> Result<(), String>;
    fn annotate_failure(&self, primary: String) -> String {
        primary
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransactionKind {
    InitialSetup,
    Reset,
}

/// Outcome of a committed transaction. Cleanup after the commit never rolls it back.
pub(crate) struct CommittedTransaction {
    pub(crate) cleanup: Result<RecoveryOutcome, String>,
}

/// One managed provisioning transaction. Preconditions: exclusive prefix guard held, recovery
/// already returned a stable state, and (for reset) the old prefix is quiesced.
///
/// Ordinary failures are reconciled by the same `recover` that runs after a crash, so there is
/// exactly one destructive rollback implementation.
pub(crate) async fn run_managed_transaction<W: ProvisioningWork>(
    prefix: &ManagedPrefix<'_>,
    kind: TransactionKind,
    fingerprint: &str,
    work: &mut W,
) -> Result<CommittedTransaction, String> {
    if kind == TransactionKind::Reset {
        if let Err(primary) = prefix.preserve_for_reset() {
            // A refusal moved nothing and must not trigger any reconciliation. Only when the
            // rename already happened (a later durability step failed) is the old prefix
            // restored, through the same state machine as after a crash.
            let moved = prefix.classify_root() == Ok(RootState::Absent)
                && entry_kind(prefix.backup()).is_ok_and(|kind| kind != EntryKind::Missing);
            if !moved {
                return Err(primary);
            }
            return Err(match prefix.recover() {
                Ok(outcome) if outcome.action == RecoveryAction::None => primary,
                Ok(outcome) => format!("{primary}. {}", outcome.summary()),
                Err(recovery) => format!("{primary}. {recovery}"),
            });
        }
    }
    let result = async {
        prefix.begin_provisioning(fingerprint)?;
        work.provision().await?;
        work.quiesce().await?;
        prefix.sync_payload()?;
        work.commit()
    }
    .await;
    match result {
        Ok(()) => {
            checkpoint!("commit:manifest-durable");
            Ok(CommittedTransaction {
                cleanup: prefix.recover(),
            })
        }
        Err(primary) => {
            let primary = work.annotate_failure(primary);
            if let Err(cleanup) = work.quiesce().await {
                return Err(format!(
                    "{primary}\nLimpieza pendiente: {cleanup}; el entorno se conservó y se recuperará antes de la próxima operación"
                ));
            }
            match prefix.recover() {
                Ok(outcome) if outcome.action == RecoveryAction::None => Err(primary),
                Ok(outcome) => Err(format!("{primary}. {}", outcome.summary())),
                Err(recovery) => Err(format!("{primary}. {recovery}")),
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod fault {
    //! SIGKILL rendezvous. Only test binaries contain this module or read its variable.
    use std::io::{BufRead, Write};
    use std::sync::OnceLock;

    pub(crate) const CHECKPOINT_ENV: &str = "RO_F05_TEST_CHECKPOINT";
    pub(crate) const REACHED: &str = "F05-CHECKPOINT";

    pub(crate) fn reached(name: &'static str) {
        static TARGET: OnceLock<Option<String>> = OnceLock::new();
        let target = TARGET.get_or_init(|| std::env::var(CHECKPOINT_ENV).ok());
        if target.as_deref() != Some(name) {
            return;
        }
        {
            let mut out = std::io::stdout().lock();
            let _ = writeln!(out, "\n{REACHED} {name}");
            let _ = out.flush();
        }
        // The parent either SIGKILLs this process or answers "continue".
        let mut line = String::new();
        match std::io::stdin().lock().read_line(&mut line) {
            Ok(_) if line.trim() == "continue" => {}
            _ => std::process::abort(),
        }
    }
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod tests;
