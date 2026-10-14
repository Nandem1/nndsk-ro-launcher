//! Crash recovery for managed prefix provisioning and reset (F-05).
//!
//! # Evidence
//!
//! Recovery never journals intent. It reads fixed-name evidence written by this protocol:
//!
//! * `R`: the canonical managed root (`location.path`).
//! * `B`: one fixed sibling, `<parent>/.<name>.reset-backup`, holding the pre-reset prefix. There
//!   is never more than one; a new reset refuses to start while it exists.
//! * `P`: `R/.ro-launcher-provisioning`, **rollback authority**. Written durably right after the
//!   launcher creates `R` and before any provisioning write. It records the location and, for a
//!   v3 path, the transaction's full fingerprint, never instructions.
//! * `C`: `R/.ro-launcher-commit-pending`, the same record after `rename(P, C)` + `fsync(R)`. The
//!   transaction performs this **pre-commit revocation** before the manifest can be written. `C`
//!   carries no destructive authority: from here on the commit may have happened.
//! * `M`: the prefix manifest (`R/.ro-launcher-prefix.json`, v2 or v3), the **only** commit record.
//! * `T`: `B/.ro-launcher-removing-manifest` or `B/.ro-launcher-removing-legacy`, the backup's own
//!   ownership evidence after `rename(M_B or legacy, T)` + `fsync(B)`. This **restoration-authority
//!   invalidation** precedes the first deleted payload byte. `T` is never restorable.
//!
//! Invariants that hold by construction:
//!
//! 1. `P` and `M` never coexist: `P` is renamed to `C` before `M` is written, and
//!    `commit_prefix_manifest` refuses a root that still holds `P`. "`P` without `M`" therefore
//!    proves the root was never committed, even if `M` is later lost or damaged.
//! 2. `C` without a valid `M` is never discarded or rolled back. Recovery preserves `R` and `B` and
//!    fails closed: deleting a possibly committed root is worse than keeping both.
//! 3. A backup with `T` is never restored, and is deleted only while `R` is verifiably committed.
//! 4. A v3 path encodes only 24 hex digits of the fingerprint. Every full-identity claim present
//!    for a v3 location must be byte-identical: the caller's current desired identity (only when it
//!    addresses this exact path), the transaction identity in `P`/`C`, `M` of `R`, and `M`/`T` of
//!    `B`. The path suffix is a shape check, never identity authority.
//!
//! # Root classification (`RootState`)
//!
//! | State           | Evidence                                                             |
//! | --------------- | -------------------------------------------------------------------- |
//! | `Absent`        | `R` does not exist                                                   |
//! | `Empty`         | `R` is a real directory without entries                              |
//! | `PendingMarker` | `R` contains only the fixed temporary of `P` (killed writing `P`)    |
//! | `Provisioning`  | regular `P` for this location/identity; no `C`, `M`, legacy, install |
//! | `CommitPending` | regular `C` for this location/identity; no `M`                       |
//! | `Committed`     | valid `M` for this location/identity; a stale `P`/`C` must agree     |
//! | `LegacyValid`   | legacy marker + complete Wine structure; no fixed evidence; not v3   |
//! | `Foreign`       | anything else, including symlinks and data without launcher evidence |
//!
//! # Backup classification (`BackupState`)
//!
//! | State        | Evidence                                                                    |
//! | ------------ | --------------------------------------------------------------------------- |
//! | `Absent`     | `B` does not exist                                                          |
//! | `Empty`      | `B` is a real empty directory (holds no data; removed in every state)       |
//! | `Removing`   | exactly one regular `T`; a manifest `T` must match location and identity    |
//! | `Restorable` | no `T`; valid `M` for this location/identity, or legacy marker + structure  |
//! | `Foreign`    | symlink, file, other filesystem, protected installation, or no evidence     |
//!
//! Every `B` other than `Absent`/`Empty` must be a real directory on the parent's filesystem
//! without game-installation protection. `T` dominates: once present, no other evidence in `B`
//! can make it `Restorable`.
//!
//! # Recovery table (`plan`)
//!
//! | R \ B          | Absent                  | Restorable                         | Removing   | Foreign |
//! | -------------- | ----------------------- | ---------------------------------- | ---------- | ------- |
//! | Absent / Empty | nothing (or one legacy) | restore `B -> R`                   | refuse     | refuse  |
//! | PendingMarker  | discard ours            | discard ours, restore `B`          | refuse     | refuse  |
//! | Provisioning   | discard ours            | discard ours, restore `B`          | refuse     | refuse  |
//! | CommitPending  | refuse                  | refuse                             | refuse     | refuse  |
//! | Committed      | drop stale `P`/`C`      | drop stale, invalidate, remove `B` | remove `B` | refuse  |
//! | LegacyValid    | nothing                 | refuse                             | refuse     | refuse  |
//! | Foreign        | nothing\*               | refuse                             | refuse     | refuse  |
//!
//! \* refuse if `R` still holds fixed transaction evidence: a pending transaction is never
//! reported as reconciled.
//!
//! "refuse" preserves every path and returns an actionable error. An empty `B` is removed first and
//! then treated as absent. Every destructive transition is self-stabilizing under SIGKILL:
//!
//! * discarding `R` deletes everything except `P` first, then `P`, then `R`; a kill leaves either
//!   `Provisioning` (P survives) or `Empty`;
//! * removing `B` renames its evidence to `T` first, deletes everything except `T`, then `T`, then
//!   `B`; a kill leaves `Restorable` (before the rename), `Removing` or `Empty`, all of which the
//!   committed row completes and no other row restores;
//! * restoring is one `renameat2(RENAME_NOREPLACE)`; a kill leaves `R` committed with `B` absent.
//!
//! Recovery is idempotent: every post-recovery state maps to "nothing". It runs only for managed
//! locations, under the exclusive `OperationGuard("prefix", R)`, and refuses destructive actions
//! while any process uses the prefix. Custom prefixes are never classified destructively.
//!
//! Durability: every create/rename/unlink is followed by an fsync of its directory. This is
//! qualified against process death (SIGKILL); power loss is implemented-but-unqualified.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use ro_tools_linux::find_prefix_processes;
use serde::{Deserialize, Serialize};

use crate::utils::{
    backup_path, ensure_managed_path_safe, inspect_prefix, is_v3_managed_prefix_path,
    isolated_prefix_root, manifest_matches_stored_location, parse_stored_prefix_manifest,
    rename_noreplace, sync_dir, sync_filesystem, v3_path_digest_prefix24, ManifestCommitError,
    PrefixLocation, PrefixScope, StoredPrefixManifest, COMMIT_PENDING_MARKER, INSTALLATION_MARKER,
    LEGACY_PREFIX_MARKER, PREFIX_MARKER, PROVISIONING_MARKER, PROVISIONING_MARKER_TMP,
};

const MARKER_SCHEMA: u32 = 2;
const MAX_MARKER_BYTES: u64 = 16 * 1024;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const RESET_BACKUP_SUFFIX: &str = "reset-backup";
/// `T` when the backup was owned through its manifest.
const BACKUP_REMOVING_MANIFEST: &str = ".ro-launcher-removing-manifest";
/// `T` when the backup was owned through the legacy configured marker.
const BACKUP_REMOVING_LEGACY: &str = ".ro-launcher-removing-legacy";

/// User-facing message when recovery replaced the root under an operation that already validated
/// the previous state. Retrying re-evaluates runner, manifest and protection from scratch.
pub const RECOVERED_RETRY_MSG: &str =
    "Se recuperó una preparación interrumpida del entorno; vuelve a intentarlo";

/// Test-only rendezvous for SIGKILL fault injection. Compiles to nothing outside `cfg(test)`.
macro_rules! checkpoint {
    ($name:expr) => {
        #[cfg(test)]
        $crate::tools::prefix::recovery::fault::reached($name);
    };
}

/// Contents of `P` and, after revocation, of `C`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProvisioningMarker {
    schema_version: u32,
    scope: PrefixScope,
    server_id: Option<String>,
    prefix_path: String,
    /// The transaction's full 64-hex v3 fingerprint. Present exactly when the location is a
    /// fingerprint-addressed v3 path; `None` for shared, legacy isolated and v2 paths, whose
    /// identity is the path itself. Always validated against the location and every other claim.
    prefix_fingerprint: Option<String>,
    created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RootState {
    Absent,
    Empty,
    PendingMarker,
    Provisioning,
    CommitPending,
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
    #[cfg(test)]
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

/// Full-identity rule of a fingerprint-addressed v3 location for one classification pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct V3Identity {
    /// The 24 hex digits the path exposes: a shape check only.
    suffix: String,
    /// The full digest every claim must equal. `None` only when no claim exists besides the one
    /// being classified (a backup found alone is judged on its own durable evidence).
    anchor: Option<String>,
}

impl V3Identity {
    fn check(&self, digest: &str) -> Result<(), String> {
        if !is_full_digest(digest) || v3_path_digest_prefix24(digest) != self.suffix {
            return Err("la identidad v3 no coincide con la ruta".into());
        }
        match &self.anchor {
            Some(anchor) if anchor != digest => Err(
                "la identidad v3 completa no coincide con la evidencia de la transacción".into(),
            ),
            _ => Ok(()),
        }
    }
}

fn is_full_digest(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// A managed root that recovery may act on. Construction is the safety boundary: custom and
/// unexpected paths can never reach the destructive helpers below.
pub(crate) struct ManagedPrefix<'a> {
    location: &'a PrefixLocation,
    root: PathBuf,
    parent: PathBuf,
    backup: PathBuf,
    /// The caller's current desired full fingerprint, kept only when it addresses this exact v3
    /// path. A transaction for another fingerprint is reconciled on its own durable evidence.
    expected: Option<String>,
}

impl<'a> ManagedPrefix<'a> {
    pub(crate) fn new(location: &'a PrefixLocation, desired: Option<&str>) -> Result<Self, String> {
        if !location.managed || location.scope == PrefixScope::Custom {
            return Err(
                "Por seguridad, el launcher no recupera ni elimina WINEPREFIX personalizados"
                    .to_string(),
            );
        }
        ensure_managed_path_safe(location)?;
        Self::at(location, desired)
    }

    /// Temp-dir tests only: skips the managed-path check that binds real paths to `$HOME`.
    #[cfg(test)]
    pub(crate) fn for_test(location: &'a PrefixLocation) -> Self {
        assert!(location.managed && location.scope != PrefixScope::Custom);
        Self::at(location, None).unwrap()
    }

    fn at(location: &'a PrefixLocation, desired: Option<&str>) -> Result<Self, String> {
        let root = PathBuf::from(&location.path);
        let parent = root
            .parent()
            .ok_or_else(|| "La ruta del entorno no tiene directorio padre".to_string())?
            .to_path_buf();
        let backup = reset_backup_path(&root)?;
        let expected = v3_path_suffix(location).and_then(|suffix| {
            desired
                .filter(|digest| is_full_digest(digest))
                .filter(|digest| v3_path_digest_prefix24(digest) == suffix)
                .map(str::to_string)
        });
        Ok(Self {
            location,
            root,
            parent,
            backup,
            expected,
        })
    }

    pub(crate) fn backup(&self) -> &Path {
        &self.backup
    }

    /// The v3 identity rule for this pass. The anchor is the first shape-valid claim among the
    /// caller's expected digest, the in-root transaction marker and the root's manifest; every
    /// other claim (including the backup) must then equal it.
    fn identity(&self) -> Option<V3Identity> {
        let suffix = v3_path_suffix(self.location)?.to_string();
        let shape_ok =
            |digest: &String| is_full_digest(digest) && v3_path_digest_prefix24(digest) == suffix;
        let anchor = self
            .expected
            .clone()
            .or_else(|| {
                [PROVISIONING_MARKER, COMMIT_PENDING_MARKER]
                    .iter()
                    .filter_map(|name| read_marker(&self.root.join(name)).ok())
                    .find_map(|marker| marker.prefix_fingerprint)
                    .filter(shape_ok)
            })
            .or_else(|| {
                read_manifest(&self.root.join(PREFIX_MARKER))
                    .ok()
                    .and_then(|manifest| manifest.prefix_fingerprint_digest().map(str::to_string))
                    .filter(shape_ok)
            });
        Some(V3Identity { suffix, anchor })
    }

    pub(crate) fn classify_root(&self) -> Result<RootState, String> {
        classify_root(&self.root, self.location, self.identity().as_ref())
    }

    #[cfg(test)]
    pub(crate) fn classify_backup(&self) -> Result<BackupState, String> {
        classify_backup(
            &self.backup,
            &self.parent,
            self.location,
            self.identity().as_ref(),
        )
    }

    /// Reconcile whatever an interrupted transaction left behind. Caller holds the exclusive
    /// prefix guard. Safe to run any number of times.
    pub(crate) fn recover(&self) -> Result<RecoveryOutcome, String> {
        let identity = self.identity();
        let root_state = classify_root(&self.root, self.location, identity.as_ref())?;
        let mut backup_state =
            classify_backup(&self.backup, &self.parent, self.location, identity.as_ref())?;
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
                        classify_backup(path, &self.parent, self.location, identity.as_ref())
                            == Ok(BackupState::Restorable)
                    })
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            };
        let action = plan(&root_state, &backup_state, &legacy_restorable)
            .map_err(|reason| self.ambiguity(&reason))?;
        if action == RecoveryAction::None && root_holds_transaction_evidence(&self.root) {
            // Fixed evidence of an unfinished transaction that no row can act on safely. Never
            // report it as reconciled: binding resolution steers here precisely to resolve it.
            return Err(self.ambiguity(&format!(
                "el entorno conserva evidencia de una transacción sin confirmar ({root_state:?})"
            )));
        }

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
        self.execute(&action, &root_state, &backup_state)?;
        Ok(RecoveryOutcome {
            action,
            removed_empty_backup,
            notes,
        })
    }

    fn execute(
        &self,
        action: &RecoveryAction,
        root_state: &RootState,
        backup_state: &BackupState,
    ) -> Result<(), String> {
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
                    for name in [
                        PROVISIONING_MARKER,
                        COMMIT_PENDING_MARKER,
                        PROVISIONING_MARKER_TMP,
                    ] {
                        remove_file_if_present(&self.root.join(name)).map_err(|error| {
                            self.io("retirar el marcador de preparación", &error)
                        })?;
                    }
                    sync_dir(&self.root)
                        .map_err(|error| self.io("sincronizar el entorno confirmado", &error))?;
                }
                checkpoint!("recovery:marker-removed");
                if *backup {
                    self.remove_backup(backup_state)?;
                }
                Ok(())
            }
            RecoveryAction::RestoreLegacyBackup(path) => {
                self.clear_non_data_root(root_state)?;
                self.restore_from(path)
            }
        }
    }

    /// Remove the old prefix after the new one committed. Restoration authority is invalidated
    /// durably before the first payload byte is deleted, so no interruption can leave a damaged
    /// backup that still classifies as `Restorable`.
    fn remove_backup(&self, state: &BackupState) -> Result<(), String> {
        let io =
            |error: std::io::Error| self.io("retirar el respaldo del entorno anterior", &error);
        let tombstone = match state {
            BackupState::Restorable => self.invalidate_restoration_authority()?,
            BackupState::Removing => {
                backup_tombstone(&self.backup).map_err(io)?.ok_or_else(|| {
                    self.ambiguity("el respaldo en retiro perdió su evidencia de retiro")
                })?
            }
            other => return Err(self.ambiguity(&format!("respaldo inesperado: {other:?}"))),
        };
        delete_tree_last(
            &self.backup,
            tombstone,
            [
                "cleanup:payload-partial",
                "cleanup:tombstone-only",
                "cleanup:tombstone-removed",
            ],
        )
        .map_err(io)?;
        self.sync_parent()
    }

    /// `Restorable -> Removing`: one atomic rename of the backup's ownership evidence to `T`,
    /// then `fsync(B)`. Returns the tombstone name that now owns the cleanup.
    fn invalidate_restoration_authority(&self) -> Result<&'static str, String> {
        let io =
            |error: std::io::Error| self.io("invalidar el respaldo del entorno anterior", &error);
        if is_protected(&self.backup).map_err(io)? {
            return Err(self.ambiguity("el respaldo contiene una instalación de juego protegida"));
        }
        let (source, tombstone) =
            if entry_kind(&self.backup.join(PREFIX_MARKER)).map_err(io)? == EntryKind::File {
                (PREFIX_MARKER, BACKUP_REMOVING_MANIFEST)
            } else {
                (LEGACY_PREFIX_MARKER, BACKUP_REMOVING_LEGACY)
            };
        checkpoint!("cleanup:authority-revoking");
        rename_noreplace(&self.backup.join(source), &self.backup.join(tombstone)).map_err(io)?;
        sync_dir(&self.backup).map_err(io)?;
        checkpoint!("cleanup:authority-revoked");
        Ok(tombstone)
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

    /// The transaction identity `P` records: the full digest for a v3 path (which it must
    /// address), nothing for paths whose identity is the path itself.
    fn transaction_identity(&self, desired: &str) -> Result<Option<String>, String> {
        let Some(suffix) = v3_path_suffix(self.location) else {
            return Ok(None);
        };
        if !is_full_digest(desired) || v3_path_digest_prefix24(desired) != suffix {
            return Err(format!(
                "La identidad deseada no corresponde a la ruta v3 {}; no se preparará",
                self.root.display()
            ));
        }
        Ok(Some(desired.to_string()))
    }

    /// Create (or adopt an already empty) canonical root and make `P` durable before any
    /// provisioning write. From this point a crash is recoverable from disk alone.
    pub(crate) fn begin_provisioning(&self, fingerprint: &str) -> Result<(), String> {
        let prefix_fingerprint = self.transaction_identity(fingerprint)?;
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
            prefix_fingerprint,
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

    /// Pre-commit boundary: `rename(P, C)` + `fsync(R)`. After this no evidence in `R` authorizes
    /// deleting it, because the manifest may be written at any moment.
    pub(crate) fn revoke_rollback_authority(&self) -> Result<(), String> {
        checkpoint!("commit:authority-revoking");
        rename_noreplace(
            &self.root.join(PROVISIONING_MARKER),
            &self.root.join(COMMIT_PENDING_MARKER),
        )
        .map_err(|error| self.io("revocar la autorización de reversión", &error))?;
        sync_dir(&self.root)
            .map_err(|error| self.io("sincronizar la revocación de reversión", &error))?;
        checkpoint!("commit:authority-revoked");
        Ok(())
    }

    /// In-process only: `C -> P`. The only argument is the writer's own proof that the manifest
    /// rename never happened in this transaction ([`ManifestNeverRenamed`]); disk evidence about
    /// `M` is deliberately not consulted, because a manifest that was renamed and then lost looks
    /// exactly like one that was never written. Recovery after a crash never performs this
    /// transition. A no-op when the revocation itself never renamed `P` to `C`.
    fn reinstate_rollback_authority(&self, _proof: ManifestNeverRenamed) -> Result<(), String> {
        let io = |error: std::io::Error| self.io("restituir la autorización de reversión", &error);
        if entry_kind(&self.root.join(COMMIT_PENDING_MARKER)).map_err(io)? != EntryKind::File
            || entry_kind(&self.root.join(PROVISIONING_MARKER)).map_err(io)? != EntryKind::Missing
        {
            return Ok(());
        }
        rename_noreplace(
            &self.root.join(COMMIT_PENDING_MARKER),
            &self.root.join(PROVISIONING_MARKER),
        )
        .map_err(io)?;
        sync_dir(&self.root).map_err(io)?;
        checkpoint!("rollback:authority-reinstated");
        Ok(())
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

    /// `P` authorizes this deletion and is removed last, so an interruption leaves
    /// `Provisioning` (rerun) or `Empty`.
    fn discard_partial_root(&self) -> Result<(), String> {
        delete_tree_last(
            &self.root,
            PROVISIONING_MARKER,
            [
                "discard:payload-partial",
                "discard:marker-only",
                "discard:marker-removed",
            ],
        )
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

const ROOT_EVIDENCE: [&str; 3] = [
    PROVISIONING_MARKER,
    PROVISIONING_MARKER_TMP,
    COMMIT_PENDING_MARKER,
];

/// Read-only: `R` holds a fixed transaction marker (`P`, its temporary, or `C`).
fn root_holds_transaction_evidence(root: &Path) -> bool {
    ROOT_EVIDENCE
        .iter()
        .any(|name| root.join(name).symlink_metadata().is_ok())
}

/// Read-only: fixed-name evidence written by this protocol (backup or in-root marker).
fn fixed_residue_present(root: &Path) -> bool {
    reset_backup_path(root).is_ok_and(|backup| backup.symlink_metadata().is_ok())
        || root_holds_transaction_evidence(root)
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

/// Read-only verdict on one managed isolated root of a server, for binding resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionGate {
    /// No fixed transaction evidence (`P`, its temporary, `C`, or a backup holding data).
    NoTransactionEvidence,
    /// Transaction evidence, but `R` holds a valid commit record for this exact location and full
    /// identity, and the backup (if any) is launcher-owned: only cleanup remains.
    CommittedValid,
    /// An interrupted transaction without a valid commit record. Recovery here decides.
    PendingTransaction,
    /// Evidence that cannot be classified safely. Recovery here fails closed.
    AmbiguousTransaction(String),
}

impl TransactionGate {
    /// Resolution must stay on this root (and let recovery decide) instead of choosing another
    /// candidate. Only the absence of evidence or a validated commit lets it move on.
    pub fn blocks_resolution(&self) -> bool {
        matches!(
            self,
            Self::PendingTransaction | Self::AmbiguousTransaction(_)
        )
    }
}

/// Read-only: classify `root`, an isolated managed root of `server_id`, for binding resolution.
///
/// Mere presence of a manifest entry never clears transaction evidence: a corrupt, nonregular,
/// symlinked or foreign manifest, or one whose full v3 identity contradicts the transaction
/// record, is not a commit. Classification reuses the recovery classifiers, so "committed" here
/// means exactly what recovery would accept. An empty backup holds no data and never steers
/// resolution. Pre-F-05 timestamped residue never steers resolution either; it is only restored
/// when recovery runs at its own root and finds exactly one valid candidate.
pub fn transaction_gate(server_id: &str, root: &Path) -> TransactionGate {
    let backup_holds_data = reset_backup_path(root).is_ok_and(|backup| {
        backup.symlink_metadata().is_ok_and(|meta| {
            !meta.is_dir()
                || meta.file_type().is_symlink()
                || fs::read_dir(&backup).map_or(true, |mut entries| entries.next().is_some())
        })
    });
    if !backup_holds_data && !root_holds_transaction_evidence(root) {
        return TransactionGate::NoTransactionEvidence;
    }
    let location = PrefixLocation {
        path: root.to_string_lossy().into_owned(),
        scope: PrefixScope::Isolated,
        managed: true,
        server_id: Some(server_id.to_string()),
    };
    let prefix = match ManagedPrefix::at(&location, None) {
        Ok(prefix) => prefix,
        Err(reason) => return TransactionGate::AmbiguousTransaction(reason),
    };
    let identity = prefix.identity();
    match classify_root(&prefix.root, &location, identity.as_ref()) {
        Ok(RootState::Committed { .. }) => {
            match classify_backup(&prefix.backup, &prefix.parent, &location, identity.as_ref()) {
                Ok(BackupState::Foreign(reason)) => TransactionGate::AmbiguousTransaction(reason),
                Err(reason) => TransactionGate::AmbiguousTransaction(reason),
                Ok(_) => TransactionGate::CommittedValid,
            }
        }
        Ok(
            RootState::Absent
            | RootState::Empty
            | RootState::PendingMarker
            | RootState::Provisioning
            | RootState::CommitPending,
        ) => TransactionGate::PendingTransaction,
        Ok(RootState::LegacyValid) => TransactionGate::AmbiguousTransaction(
            "el entorno actual es legacy y además existe un respaldo".into(),
        ),
        Ok(RootState::Foreign(reason)) | Err(reason) => {
            TransactionGate::AmbiguousTransaction(reason)
        }
    }
}

/// Read-only discovery of interrupted transactions of `server_id` under every v3 fingerprint.
/// Only names produced by the managed naming contract are considered: `<token>-v3-<24 hex>` for
/// this server's token, or its fixed `.<name>.reset-backup` sibling. Candidates without fixed
/// evidence are skipped before anything is opened; the rest are classified by
/// [`transaction_gate`]. Nothing is modified. A pending or ambiguous candidate is reported, and
/// must still pass full evidence validation in recovery before anything is changed.
pub fn pending_v3_transactions(server_id: &str) -> Result<Vec<String>, String> {
    let directory = isolated_prefix_root();
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(discovery_io(&directory, error)),
    };
    let backup_suffix = format!(".{RESET_BACKUP_SUFFIX}");
    let mut roots = BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|error| discovery_io(&directory, error))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let root_name = name
            .strip_prefix('.')
            .and_then(|rest| rest.strip_suffix(&backup_suffix))
            .unwrap_or(name);
        let candidate = directory.join(root_name);
        if is_v3_managed_prefix_path(&candidate, server_id) {
            roots.insert(candidate);
        }
    }
    Ok(roots
        .into_iter()
        .filter(|root| transaction_gate(server_id, root).blocks_resolution())
        .map(|root| root.to_string_lossy().into_owned())
        .collect())
}

fn discovery_io(directory: &Path, error: std::io::Error) -> String {
    format!(
        "No se pudo revisar {} en busca de preparaciones interrumpidas; no se eligió ningún entorno: {error}",
        directory.display()
    )
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
        (R::CommitPending, _) => Err(
            "la confirmación del entorno nuevo pudo haber comenzado y no puede probarse que no terminó"
                .into(),
        ),
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

fn classify_root(
    root: &Path,
    location: &PrefixLocation,
    identity: Option<&V3Identity>,
) -> Result<RootState, String> {
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
    let kind = |name: &str| entry_kind(&root.join(name)).map_err(io);
    let pending_tmp = kind(PROVISIONING_MARKER_TMP)?;
    if names.len() == 1
        && names[0] == PROVISIONING_MARKER_TMP
        && pending_tmp == EntryKind::File
        && root
            .join(PROVISIONING_MARKER_TMP)
            .symlink_metadata()
            .map_err(io)?
            .len()
            <= MAX_MARKER_BYTES
    {
        return Ok(RootState::PendingMarker);
    }
    let provisioning = kind(PROVISIONING_MARKER)?;
    let commit_pending = kind(COMMIT_PENDING_MARKER)?;
    if provisioning != EntryKind::Missing && commit_pending != EntryKind::Missing {
        return Ok(RootState::Foreign(
            "combina autorización de reversión con una confirmación pendiente".into(),
        ));
    }
    // The single transaction marker present, if any: (name, may-authorize-rollback).
    let marker = match (provisioning, commit_pending) {
        (EntryKind::Missing, EntryKind::Missing) => None,
        (EntryKind::File, _) => Some((PROVISIONING_MARKER, true)),
        (_, EntryKind::File) => Some((COMMIT_PENDING_MARKER, false)),
        _ => {
            return Ok(RootState::Foreign(
                "el marcador de preparación no es un archivo regular".into(),
            ))
        }
    };
    let validated_marker = |name: &str| -> Result<(), String> {
        let marker = read_marker(&root.join(name))?;
        validate_marker(&marker, location, identity)
    };

    match kind(PREFIX_MARKER)? {
        EntryKind::File => {
            if let Err(reason) = read_manifest(&root.join(PREFIX_MARKER))
                .and_then(|manifest| committed_manifest(&manifest, location, identity))
            {
                return Ok(RootState::Foreign(reason));
            }
            // A stale marker must describe this same transaction; a contradiction is not cleaned.
            if let Some((name, _)) = marker {
                if let Err(reason) = validated_marker(name) {
                    return Ok(RootState::Foreign(format!(
                        "el marcador residual contradice el manifiesto: {reason}"
                    )));
                }
            }
            return Ok(RootState::Committed {
                stale_marker: marker.is_some() || pending_tmp != EntryKind::Missing,
            });
        }
        EntryKind::Missing => {}
        _ => {
            return Ok(RootState::Foreign(
                "el manifiesto no es un archivo regular".into(),
            ))
        }
    }
    match marker {
        Some((name, rollback_authority)) => {
            if let Err(reason) = validated_marker(name) {
                return Ok(RootState::Foreign(reason));
            }
            if !rollback_authority {
                return Ok(RootState::CommitPending);
            }
            if is_protected(root).map_err(io)? {
                return Ok(RootState::Foreign(
                    "contiene una instalación de juego protegida".into(),
                ));
            }
            if kind(LEGACY_PREFIX_MARKER)? != EntryKind::Missing {
                return Ok(RootState::Foreign(
                    "combina un marcador legacy con uno de preparación".into(),
                ));
            }
            return Ok(RootState::Provisioning);
        }
        None if pending_tmp != EntryKind::Missing => {
            return Ok(RootState::Foreign(
                "contiene un marcador temporal de preparación junto a otros datos".into(),
            ))
        }
        None => {}
    }
    if kind(LEGACY_PREFIX_MARKER)? == EntryKind::File
        && root
            .to_str()
            .is_some_and(|path| inspect_prefix(path).structure_ok)
    {
        if identity.is_some() {
            return Ok(RootState::Foreign(
                "un marcador legacy no prueba la identidad de una ruta v3".into(),
            ));
        }
        return Ok(RootState::LegacyValid);
    }
    Ok(RootState::Foreign(
        "contiene datos sin manifiesto ni marcador del launcher".into(),
    ))
}

/// The tombstone that owns a backup's removal, if any. Two tombstones is an error.
fn backup_tombstone(backup: &Path) -> std::io::Result<Option<&'static str>> {
    let manifest = entry_kind(&backup.join(BACKUP_REMOVING_MANIFEST))?;
    let legacy = entry_kind(&backup.join(BACKUP_REMOVING_LEGACY))?;
    Ok(match (manifest, legacy) {
        (EntryKind::Missing, EntryKind::Missing) => None,
        (_, EntryKind::Missing) => Some(BACKUP_REMOVING_MANIFEST),
        (EntryKind::Missing, _) => Some(BACKUP_REMOVING_LEGACY),
        _ => {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "el respaldo tiene dos evidencias de retiro",
            ))
        }
    })
}

fn classify_backup(
    backup: &Path,
    parent: &Path,
    location: &PrefixLocation,
    identity: Option<&V3Identity>,
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
    // Removal evidence dominates: once present, nothing else can make this backup restorable.
    let tombstone = match backup_tombstone(backup) {
        Ok(tombstone) => tombstone,
        Err(error) if error.kind() == ErrorKind::InvalidData => {
            return Ok(BackupState::Foreign(error.to_string()))
        }
        Err(error) => return Err(io(error)),
    };
    if let Some(name) = tombstone {
        if entry_kind(&backup.join(name)).map_err(io)? != EntryKind::File {
            return Ok(BackupState::Foreign(
                "su evidencia de retiro no es un archivo regular".into(),
            ));
        }
        if name == BACKUP_REMOVING_MANIFEST {
            if let Err(reason) = read_manifest(&backup.join(name))
                .and_then(|manifest| committed_manifest(&manifest, location, identity))
            {
                return Ok(BackupState::Foreign(reason));
            }
        } else if identity.is_some() {
            return Ok(BackupState::Foreign(
                "un marcador legacy no prueba la identidad de una ruta v3".into(),
            ));
        }
        return Ok(BackupState::Removing);
    }
    match entry_kind(&backup.join(PREFIX_MARKER)).map_err(io)? {
        EntryKind::File => {
            return Ok(
                match read_manifest(&backup.join(PREFIX_MARKER))
                    .and_then(|manifest| committed_manifest(&manifest, location, identity))
                {
                    Ok(()) => BackupState::Restorable,
                    Err(reason) => BackupState::Foreign(reason),
                },
            )
        }
        EntryKind::Missing => {}
        _ => {
            return Ok(BackupState::Foreign(
                "su manifiesto no es un archivo regular".into(),
            ))
        }
    }
    match entry_kind(&backup.join(LEGACY_PREFIX_MARKER)).map_err(io)? {
        EntryKind::File
            if backup
                .to_str()
                .is_some_and(|path| inspect_prefix(path).structure_ok) =>
        {
            Ok(if identity.is_some() {
                BackupState::Foreign(
                    "un marcador legacy no prueba la identidad de una ruta v3".into(),
                )
            } else {
                BackupState::Restorable
            })
        }
        EntryKind::File => Ok(BackupState::Foreign(
            "tiene marcador legacy pero su estructura está incompleta".into(),
        )),
        EntryKind::Missing => Ok(BackupState::Foreign(
            "no contiene evidencia del launcher".into(),
        )),
        _ => Ok(BackupState::Foreign(
            "su marcador legacy no es un archivo regular".into(),
        )),
    }
}

/// The manifest commits a prefix for `location`: same scope/server and, for a v3 path, a full
/// digest that satisfies the identity rule (shape against the path, equality against the anchor).
fn committed_manifest(
    manifest: &StoredPrefixManifest,
    location: &PrefixLocation,
    identity: Option<&V3Identity>,
) -> Result<(), String> {
    if !manifest_matches_stored_location(manifest, location) {
        return Err("el manifiesto pertenece a otro servidor o ámbito".into());
    }
    if let Some(identity) = identity {
        let digest = manifest.prefix_fingerprint_digest().ok_or_else(|| {
            "una ruta v3 exige un manifiesto v3 con identidad completa".to_string()
        })?;
        identity.check(digest)?;
    }
    Ok(())
}

fn validate_marker(
    marker: &ProvisioningMarker,
    location: &PrefixLocation,
    identity: Option<&V3Identity>,
) -> Result<(), String> {
    if marker.schema_version != MARKER_SCHEMA
        || marker.scope != location.scope
        || marker.server_id != location.server_id
        || Path::new(&marker.prefix_path) != Path::new(&location.path)
    {
        return Err("el marcador de preparación pertenece a otra ubicación".into());
    }
    match (identity, marker.prefix_fingerprint.as_deref()) {
        (None, None) => Ok(()),
        (Some(identity), Some(digest)) => identity.check(digest),
        _ => Err("la identidad del marcador no corresponde al tipo de ruta".into()),
    }
}

fn v3_path_suffix(location: &PrefixLocation) -> Option<&str> {
    let server_id = location.server_id.as_deref()?;
    if location.scope != PrefixScope::Isolated
        || !is_v3_managed_prefix_path(Path::new(&location.path), server_id)
    {
        return None;
    }
    Path::new(&location.path)
        .file_name()?
        .to_str()?
        .rsplit_once("-v3-")
        .map(|(_, suffix)| suffix)
}

/// Bounded, no-follow read of a small launcher-owned file.
///
/// `O_NONBLOCK` keeps `open` itself from waiting on a nonregular object (a FIFO with no writer
/// blocks a plain `open(O_RDONLY)` before any type check could run); it has no effect on regular
/// files. The type is then verified on the open descriptor, never on the path, so there is no
/// path/type race. Only regular files are ever read.
fn read_bounded(path: &Path, limit: u64, label: &str) -> Result<String, String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| format!("no se pudo leer {label}: {error}"))?;
    let meta = file
        .metadata()
        .map_err(|error| format!("no se pudo leer {label}: {error}"))?;
    if !meta.is_file() {
        return Err(format!("{label} no es un archivo regular"));
    }
    if meta.len() > limit {
        return Err(format!("{label} es demasiado grande"));
    }
    let mut text = String::new();
    file.take(limit + 1)
        .read_to_string(&mut text)
        .map_err(|_| format!("{label} está dañado"))?;
    if text.len() as u64 > limit {
        return Err(format!("{label} es demasiado grande"));
    }
    Ok(text)
}

fn read_marker(path: &Path) -> Result<ProvisioningMarker, String> {
    let json = read_bounded(path, MAX_MARKER_BYTES, "el marcador de preparación")?;
    serde_json::from_str(&json).map_err(|_| "el marcador de preparación está dañado".to_string())
}

fn read_manifest(path: &Path) -> Result<StoredPrefixManifest, String> {
    let json = read_bounded(path, MAX_MANIFEST_BYTES, "el manifiesto")?;
    parse_stored_prefix_manifest(&json)
        .map_err(|_| "el manifiesto está dañado o usa un schema incompatible".to_string())
}

fn remove_file_if_present(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Delete `dir`, keeping the entry that authorizes the deletion (`last`) until every other entry
/// is gone. Only the authorizing evidence may survive an interruption, never restorable evidence:
/// callers pass `P` for a partial root and `T` for an invalidated backup. Symlinks are removed,
/// never followed; `remove_dir_all` does not follow symlinks inside the tree.
#[cfg_attr(not(test), allow(unused_variables))]
fn delete_tree_last(dir: &Path, last: &str, points: [&'static str; 3]) -> std::io::Result<()> {
    let mut first = true;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_name() == last {
            continue;
        }
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
        if first {
            first = false;
            checkpoint!(points[0]);
        }
    }
    sync_dir(dir)?;
    checkpoint!(points[1]);
    remove_file_if_present(&dir.join(last))?;
    sync_dir(dir)?;
    checkpoint!(points[2]);
    fs::remove_dir(dir)
}

/// What a managed transaction needs from its provisioning engine. Production wraps a supervised
/// `RunnerOperation`; tests inject an engine that writes representative files.
pub(crate) trait ProvisioningWork {
    async fn provision(&mut self) -> Result<(), String>;
    /// End execution and wait until no process uses the prefix. Must be repeatable.
    async fn quiesce(&mut self) -> Result<(), String>;
    /// Write the final manifest. The commit point is its successful rename; the error says on
    /// which side of it the failure happened.
    fn commit(&mut self) -> Result<(), ManifestCommitError>;
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
/// Order: `[rename(R, B)]`, create `R` and a durable `P`, provision, quiesce, `syncfs`, then the
/// pre-commit revocation `rename(P, C)` with `fsync(R)`, then the manifest (commit); recovery
/// finally drops `C` and removes `B`. Ordinary failures are reconciled by the same `recover` that
/// runs after a crash, so there is exactly one destructive rollback implementation. The only extra
/// step is in-process: when the writer proves the manifest rename never happened, `C -> P`
/// re-grants rollback authority before `recover`. Once the rename succeeded it never does.
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
    // Rollback authority after a failure is decided by what this process did, never by what is
    // later observed on disk. It starts `Held` (P), and becomes `Reinstatable` only through the
    // writer's proof that the manifest rename never happened.
    let mut authority = Authority::Held;
    let result = async {
        prefix.begin_provisioning(fingerprint)?;
        work.provision().await?;
        work.quiesce().await?;
        prefix.sync_payload()?;
        // From here the root may become committed at any moment: rollback authority goes first.
        // A failed revocation never reached the manifest writer.
        authority = Authority::Reinstatable(ManifestNeverRenamed(()));
        prefix.revoke_rollback_authority()?;
        work.commit().map_err(|error| match error {
            ManifestCommitError::BeforeRename(message) => message,
            ManifestCommitError::AfterRename(message) => {
                authority = Authority::Revoked;
                message
            }
        })
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
            if let Authority::Reinstatable(proof) = authority {
                if let Err(error) = prefix.reinstate_rollback_authority(proof) {
                    return Err(format!("{primary}. {error}"));
                }
            }
            match prefix.recover() {
                Ok(outcome) if outcome.action == RecoveryAction::None => Err(primary),
                Ok(outcome) => Err(format!("{primary}. {}", outcome.summary())),
                Err(recovery) => Err(format!("{primary}. {recovery}")),
            }
        }
    }
}

/// The writer's proof that the manifest rename never happened in this transaction. Created only
/// by `run_managed_transaction` before revocation and destroyed by an `AfterRename` failure.
pub(crate) struct ManifestNeverRenamed(());

/// In-process rollback authority of one managed transaction.
enum Authority {
    /// `P` was never revoked: ordinary recovery may discard the partial root.
    Held,
    /// `P` may have become `C`, but the manifest rename provably never happened.
    Reinstatable(ManifestNeverRenamed),
    /// The manifest rename succeeded. Rollback can never be reactivated by this process.
    Revoked,
}

#[cfg(test)]
pub(crate) mod fault {
    //! SIGKILL rendezvous. Only test binaries contain this module or read its variable.
    use std::io::{BufRead, Write};
    use std::sync::OnceLock;

    pub(crate) const CHECKPOINT_ENV: &str = "RO_F05_TEST_CHECKPOINT";
    pub(crate) const REACHED: &str = "F05-CHECKPOINT";

    pub(crate) fn reached(name: &str) {
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
