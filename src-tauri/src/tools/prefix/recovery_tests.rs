//! F-05 recovery tests.
//!
//! In-process tests only touch unique temp directories through `ManagedPrefix::for_test`; they
//! never resolve `$HOME`-based managed paths. Everything that needs the real managed-path checks,
//! cross-process locks, SIGKILL or the real `ro-sessiond` runs in a child test process with a
//! disposable `HOME`.

use super::*;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::utils::test_hooks::ManifestFault;
use crate::utils::{
    write_prefix_manifest, ManifestCommitError, PrefixManifest, PREFIX_SCHEMA_VERSION,
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    dir: PathBuf,
    location: PrefixLocation,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "ro-f05-{label}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(dir.join("prefixes")).unwrap();
        let location = PrefixLocation {
            path: dir.join("prefixes/pfx").to_string_lossy().into_owned(),
            scope: PrefixScope::Isolated,
            managed: true,
            server_id: Some("srv".into()),
        };
        Self { dir, location }
    }

    fn root(&self) -> PathBuf {
        PathBuf::from(&self.location.path)
    }

    fn backup(&self) -> PathBuf {
        self.dir.join("prefixes/.pfx.reset-backup")
    }

    fn prefix(&self) -> ManagedPrefix<'_> {
        ManagedPrefix::for_test(&self.location)
    }

    fn recover(&self) -> Result<RecoveryOutcome, String> {
        self.prefix().recover()
    }

    /// Recover, then prove the result is a fixed point.
    fn recover_twice(&self) -> RecoveryOutcome {
        let first = self.recover().unwrap();
        let before = snapshot(&self.dir);
        let second = self.recover().unwrap();
        assert_eq!(second.action, RecoveryAction::None, "second recovery acted");
        assert!(!second.removed_empty_backup);
        assert_eq!(snapshot(&self.dir), before, "second recovery changed bytes");
        first
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn create_structure(root: &Path) {
    fs::create_dir_all(root.join("drive_c/windows/system32")).unwrap();
    fs::create_dir_all(root.join("dosdevices")).unwrap();
    fs::write(root.join("system.reg"), "reg").unwrap();
    fs::write(root.join("user.reg"), "reg").unwrap();
    std::os::unix::fs::symlink("../drive_c", root.join("dosdevices/c:")).unwrap();
}

fn manifest_for(location: &PrefixLocation, server: Option<&str>, tag: &str) -> PrefixManifest {
    PrefixManifest {
        schema_version: PREFIX_SCHEMA_VERSION,
        scope: location.scope,
        server_id: server.map(str::to_string),
        runner_kind: "wine".into(),
        runner_path: "/opt/fake/bin/wine".into(),
        components: vec![tag.to_string()],
    }
}

fn make_committed(dir: &Path, location: &PrefixLocation, tag: &str) {
    create_structure(dir);
    fs::write(dir.join("payload.txt"), tag).unwrap();
    write_prefix_manifest(
        dir.to_str().unwrap(),
        &manifest_for(location, location.server_id.as_deref(), tag),
    )
    .unwrap();
}

fn make_legacy(dir: &Path, tag: &str) {
    create_structure(dir);
    fs::write(dir.join("payload.txt"), tag).unwrap();
    fs::write(dir.join(LEGACY_PREFIX_MARKER), "configured").unwrap();
}

fn make_provisioning(fixture: &Fixture, tag: &str) {
    fixture.prefix().begin_provisioning("fp").unwrap();
    fs::create_dir_all(fixture.root().join("drive_c/windows")).unwrap();
    fs::write(fixture.root().join("payload.txt"), tag).unwrap();
}

/// Provisioned root past the pre-commit boundary: `C` instead of `P`, no manifest yet.
fn make_commit_pending(fixture: &Fixture, tag: &str) {
    make_provisioning(fixture, tag);
    create_structure(&fixture.root());
    fixture.prefix().revoke_rollback_authority().unwrap();
}

/// What the transaction leaves when killed right after the commit record became durable.
fn make_committed_with_stale_marker(fixture: &Fixture, tag: &str) {
    make_commit_pending(fixture, tag);
    write_prefix_manifest(
        &fixture.location.path,
        &manifest_for(
            &fixture.location,
            fixture.location.server_id.as_deref(),
            tag,
        ),
    )
    .unwrap();
}

fn payload(dir: &Path) -> String {
    fs::read_to_string(dir.join("payload.txt")).unwrap()
}

fn exists(path: &Path) -> bool {
    path.symlink_metadata().is_ok()
}

/// Byte-level snapshot without following symlinks.
fn snapshot(dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(base).unwrap().display().to_string();
            let meta = path.symlink_metadata().unwrap();
            if meta.file_type().is_symlink() {
                out.insert(
                    rel,
                    format!("link:{}", fs::read_link(&path).unwrap().display()),
                );
            } else if meta.is_dir() {
                out.insert(rel, "dir".into());
                walk(base, &path, out);
            } else {
                // Unreadable files are compared by metadata; their bytes are checked separately.
                let bytes = fs::read(&path).map_or_else(
                    |_| format!("unreadable:{}:{:o}", meta.len(), meta.permissions().mode()),
                    |bytes| format!("{bytes:?}"),
                );
                out.insert(rel, format!("file:{bytes}"));
            }
        }
    }
    walk(dir, dir, &mut out);
    out
}

fn assert_refused_and_preserved(fixture: &Fixture) {
    let before = snapshot(&fixture.dir);
    let error = fixture.recover().unwrap_err();
    assert!(
        error.contains("no puede resolverse con seguridad"),
        "{error}"
    );
    assert!(
        error.contains(&fixture.backup().display().to_string()),
        "{error}"
    );
    assert_eq!(snapshot(&fixture.dir), before, "refusal modified data");
}

// ---------------------------------------------------------------------------------------------
// Pure transition table

#[test]
fn plan_table_is_total_and_matches_the_documented_rows() {
    use BackupState as B;
    use RootState as R;
    let roots = [
        R::Absent,
        R::Empty,
        R::PendingMarker,
        R::Provisioning,
        R::CommitPending,
        R::Committed {
            stale_marker: false,
        },
        R::Committed { stale_marker: true },
        R::LegacyValid,
        R::Foreign("x".into()),
    ];
    // `Empty` is removed before planning; `plan` refuses it defensively.
    let backups = [
        B::Absent,
        B::Empty,
        B::Restorable,
        B::Removing,
        B::Foreign("y".into()),
    ];
    for root in &roots {
        for backup in &backups {
            let action = plan(root, backup, &[]);
            let expected = match (root, backup) {
                (_, B::Foreign(_) | B::Empty) => None,
                // Commit may have happened: never discarded, never rolled back.
                (R::CommitPending, _) => None,
                (R::Absent | R::Empty, B::Absent) => Some(RecoveryAction::None),
                (R::Absent | R::Empty | R::PendingMarker, B::Restorable) => {
                    Some(RecoveryAction::RestoreBackup)
                }
                (R::PendingMarker | R::Provisioning, B::Absent) => {
                    Some(RecoveryAction::DiscardPartial)
                }
                (R::Provisioning, B::Restorable) => Some(RecoveryAction::DiscardPartialAndRestore),
                (R::Absent | R::Empty | R::PendingMarker | R::Provisioning, B::Removing) => None,
                (
                    R::Committed {
                        stale_marker: false,
                    },
                    B::Absent,
                ) => Some(RecoveryAction::None),
                (R::Committed { stale_marker }, B::Absent) => {
                    Some(RecoveryAction::CompleteCommit {
                        stale_marker: *stale_marker,
                        backup: false,
                    })
                }
                (R::Committed { stale_marker }, _) => Some(RecoveryAction::CompleteCommit {
                    stale_marker: *stale_marker,
                    backup: true,
                }),
                (R::LegacyValid | R::Foreign(_), B::Absent) => Some(RecoveryAction::None),
                (R::LegacyValid | R::Foreign(_), _) => None,
            };
            match expected {
                Some(expected) => {
                    assert_eq!(action.as_ref(), Ok(&expected), "{root:?} x {backup:?}")
                }
                None => assert!(action.is_err(), "{root:?} x {backup:?} must refuse"),
            }
        }
    }
    // Committed always wins over any verifiable backup: never a restore row.
    for backup in [B::Restorable, B::Removing] {
        let action = plan(&R::Committed { stale_marker: true }, &backup, &[]).unwrap();
        assert!(matches!(
            action,
            RecoveryAction::CompleteCommit { backup: true, .. }
        ));
    }
    // Exactly one restorable legacy backup is restored; two are never chosen between.
    let one = [PathBuf::from("/a")];
    let two = [PathBuf::from("/a"), PathBuf::from("/b")];
    assert_eq!(
        plan(&R::Absent, &B::Absent, &one),
        Ok(RecoveryAction::RestoreLegacyBackup(PathBuf::from("/a")))
    );
    assert_eq!(plan(&R::Absent, &B::Absent, &two), Ok(RecoveryAction::None));
}

// ---------------------------------------------------------------------------------------------
// Filesystem state table: every reachable state, its action, its final state, idempotence

#[test]
fn state_stable_absent_and_committed_take_no_action() {
    let fixture = Fixture::new("stable");
    assert_eq!(fixture.recover_twice().action, RecoveryAction::None);
    make_committed(&fixture.root(), &fixture.location, "OLD");
    assert_eq!(
        fixture.prefix().classify_root().unwrap(),
        RootState::Committed {
            stale_marker: false
        }
    );
    assert_eq!(fixture.recover_twice().action, RecoveryAction::None);
    assert_eq!(payload(&fixture.root()), "OLD");
}

#[test]
fn state_root_missing_with_backup_restores_old_prefix() {
    for root_kind in ["absent", "empty", "pending-marker"] {
        let fixture = Fixture::new("restore");
        make_committed(&fixture.backup(), &fixture.location, "OLD");
        match root_kind {
            "empty" => fs::create_dir(fixture.root()).unwrap(),
            "pending-marker" => {
                fs::create_dir(fixture.root()).unwrap();
                fs::write(fixture.root().join(PROVISIONING_MARKER_TMP), "{\"partial").unwrap();
            }
            _ => {}
        }
        let outcome = fixture.recover_twice();
        assert_eq!(outcome.action, RecoveryAction::RestoreBackup, "{root_kind}");
        assert!(outcome.restored_previous());
        assert_eq!(payload(&fixture.root()), "OLD");
        assert!(!exists(&fixture.backup()));
        assert_eq!(
            fixture.prefix().classify_root().unwrap(),
            RootState::Committed {
                stale_marker: false
            }
        );
    }
}

#[test]
fn state_uncommitted_replacement_loses_to_old_backup() {
    let fixture = Fixture::new("discard-restore");
    make_committed(&fixture.backup(), &fixture.location, "OLD");
    make_provisioning(&fixture, "NEW-PARTIAL");
    assert_eq!(
        fixture.prefix().classify_root().unwrap(),
        RootState::Provisioning
    );
    let outcome = fixture.recover_twice();
    assert_eq!(outcome.action, RecoveryAction::DiscardPartialAndRestore);
    assert_eq!(payload(&fixture.root()), "OLD");
    assert!(!exists(&fixture.root().join(PROVISIONING_MARKER)));
    assert!(!exists(&fixture.backup()));
}

/// Central anti-rollback invariant: a committed manifest beats the backup and a stale marker.
#[test]
fn state_committed_new_prefix_always_wins() {
    let fixture = Fixture::new("commit-wins");
    make_committed(&fixture.backup(), &fixture.location, "OLD");
    make_committed_with_stale_marker(&fixture, "NEW");
    assert_eq!(
        fixture.prefix().classify_root().unwrap(),
        RootState::Committed { stale_marker: true }
    );
    let outcome = fixture.recover_twice();
    assert_eq!(
        outcome.action,
        RecoveryAction::CompleteCommit {
            stale_marker: true,
            backup: true
        }
    );
    assert!(!outcome.root_replaced());
    assert_eq!(payload(&fixture.root()), "NEW");
    assert!(!exists(&fixture.root().join(PROVISIONING_MARKER)));
    assert!(!exists(&fixture.root().join(COMMIT_PENDING_MARKER)));
    assert!(!exists(&fixture.backup()), "old backup must be cleaned");
}

#[test]
fn state_commit_cleanup_variants_complete_without_rollback() {
    // Backup present, marker already removed.
    let fixture = Fixture::new("cleanup-backup");
    make_committed(&fixture.backup(), &fixture.location, "OLD");
    make_committed(&fixture.root(), &fixture.location, "NEW");
    assert_eq!(
        fixture.recover_twice().action,
        RecoveryAction::CompleteCommit {
            stale_marker: false,
            backup: true
        }
    );
    assert_eq!(payload(&fixture.root()), "NEW");
    assert!(!exists(&fixture.backup()));

    // Stale marker only (initial setup killed after commit).
    let fixture = Fixture::new("cleanup-marker");
    make_committed_with_stale_marker(&fixture, "NEW");
    assert_eq!(
        fixture.recover_twice().action,
        RecoveryAction::CompleteCommit {
            stale_marker: true,
            backup: false
        }
    );
    assert_eq!(payload(&fixture.root()), "NEW");

    // Legacy backup whose restoration authority was invalidated and payload partly removed.
    let fixture = Fixture::new("cleanup-removing");
    make_legacy(&fixture.backup(), "OLD");
    fs::rename(
        fixture.backup().join(LEGACY_PREFIX_MARKER),
        fixture.backup().join(BACKUP_REMOVING_LEGACY),
    )
    .unwrap();
    fs::remove_dir_all(fixture.backup().join("drive_c")).unwrap();
    assert_eq!(
        fixture.prefix().classify_backup().unwrap(),
        BackupState::Removing
    );
    make_committed(&fixture.root(), &fixture.location, "NEW");
    assert_eq!(
        fixture.recover_twice().action,
        RecoveryAction::CompleteCommit {
            stale_marker: false,
            backup: true
        }
    );
    assert!(!exists(&fixture.backup()));

    // An empty backup holds no data and is removed in any state.
    let fixture = Fixture::new("cleanup-empty");
    make_committed(&fixture.root(), &fixture.location, "NEW");
    fs::create_dir(fixture.backup()).unwrap();
    let outcome = fixture.recover_twice();
    assert!(outcome.removed_empty_backup);
    assert_eq!(outcome.action, RecoveryAction::None);
    assert!(!exists(&fixture.backup()));
}

#[test]
fn state_interrupted_initial_setup_is_discarded_and_location_reusable() {
    let fixture = Fixture::new("initial");
    make_provisioning(&fixture, "PARTIAL");
    let outcome = fixture.recover_twice();
    assert_eq!(outcome.action, RecoveryAction::DiscardPartial);
    assert!(!exists(&fixture.root()));
    // Killed while the marker itself was being written: only the fixed temp file exists.
    fs::create_dir(fixture.root()).unwrap();
    fs::write(fixture.root().join(PROVISIONING_MARKER_TMP), "{").unwrap();
    assert_eq!(
        fixture.prefix().classify_root().unwrap(),
        RootState::PendingMarker
    );
    assert_eq!(
        fixture.recover_twice().action,
        RecoveryAction::DiscardPartial
    );
    assert!(!exists(&fixture.root()));
    // The location is ready for a clean provisioning attempt.
    fixture.prefix().begin_provisioning("fp").unwrap();
    assert_eq!(
        fixture.prefix().classify_root().unwrap(),
        RootState::Provisioning
    );
}

#[test]
fn state_empty_root_and_legacy_valid_root_are_left_alone() {
    let fixture = Fixture::new("empty");
    fs::create_dir(fixture.root()).unwrap();
    assert_eq!(fixture.recover_twice().action, RecoveryAction::None);
    assert!(
        fixture.root().is_dir(),
        "an empty root without backup is not deleted"
    );

    let fixture = Fixture::new("legacy-valid");
    make_legacy(&fixture.root(), "LEGACY");
    assert_eq!(
        fixture.prefix().classify_root().unwrap(),
        RootState::LegacyValid
    );
    assert_eq!(fixture.recover_twice().action, RecoveryAction::None);
    assert_eq!(payload(&fixture.root()), "LEGACY");
}

#[test]
fn state_legacy_valid_backup_is_restorable() {
    // Reset supports legacy prefixes, so the fixed backup may hold one.
    let fixture = Fixture::new("legacy-backup");
    make_legacy(&fixture.backup(), "LEGACY");
    assert_eq!(
        fixture.prefix().classify_backup().unwrap(),
        BackupState::Restorable
    );
    assert_eq!(
        fixture.recover_twice().action,
        RecoveryAction::RestoreBackup
    );
    assert_eq!(payload(&fixture.root()), "LEGACY");
}

#[test]
fn legacy_timestamped_residue_is_never_deleted() {
    let fixture = Fixture::new("legacy-residue");
    let first = fixture.dir.join("prefixes/.pfx.reset-backup-123-456");
    make_committed(&first, &fixture.location, "OLD-1");
    // Exactly one valid legacy backup and no root: restore it.
    let outcome = fixture.recover_twice();
    assert_eq!(
        outcome.action,
        RecoveryAction::RestoreLegacyBackup(first.clone())
    );
    assert_eq!(payload(&fixture.root()), "OLD-1");

    // Two candidates: never choose, preserve both, report.
    let fixture = Fixture::new("legacy-two");
    let a = fixture.dir.join("prefixes/.pfx.reset-backup-1-10");
    let b = fixture.dir.join("prefixes/.pfx.reset-backup-2-20");
    make_committed(&a, &fixture.location, "A");
    make_committed(&b, &fixture.location, "B");
    let before = snapshot(&fixture.dir);
    let outcome = fixture.recover().unwrap();
    assert_eq!(outcome.action, RecoveryAction::None);
    assert!(outcome.notes[0].contains(".pfx.reset-backup-1-10"));
    assert!(outcome.notes[0].contains(".pfx.reset-backup-2-20"));
    assert_eq!(snapshot(&fixture.dir), before);

    // With a committed root the residue is only reported.
    let fixture = Fixture::new("legacy-committed");
    make_committed(&fixture.root(), &fixture.location, "CUR");
    let a = fixture.dir.join("prefixes/.pfx.reset-backup-1-10");
    make_committed(&a, &fixture.location, "A");
    let before = snapshot(&fixture.dir);
    let outcome = fixture.recover().unwrap();
    assert_eq!(outcome.action, RecoveryAction::None);
    assert_eq!(outcome.notes.len(), 1);
    assert_eq!(snapshot(&fixture.dir), before);
}

// ---------------------------------------------------------------------------------------------
// Unknown data, ambiguity and object types: fail closed, preserve every byte

/// Being under the managed prefixes directory is not ownership.
#[test]
fn unknown_directory_at_managed_path_is_never_adopted_or_deleted() {
    let fixture = Fixture::new("unknown");
    fs::create_dir_all(fixture.root().join("drive_c")).unwrap();
    fs::write(fixture.root().join("user-save.dat"), b"\x00precious\xff").unwrap();
    fs::write(fixture.root().join("drive_c/notes.txt"), "mine").unwrap();
    let before = snapshot(&fixture.dir);
    assert!(matches!(
        fixture.prefix().classify_root().unwrap(),
        RootState::Foreign(_)
    ));
    assert_eq!(fixture.recover().unwrap().action, RecoveryAction::None);
    assert_eq!(snapshot(&fixture.dir), before);
    assert!(fixture.prefix().begin_provisioning("fp").is_err());
    assert!(fixture.prefix().preserve_for_reset().is_err());
    assert_eq!(snapshot(&fixture.dir), before);
}

#[test]
fn ambiguous_root_states_with_backup_refuse() {
    // Unknown root + valid backup.
    let fixture = Fixture::new("amb-unknown");
    make_committed(&fixture.backup(), &fixture.location, "OLD");
    fs::create_dir(fixture.root()).unwrap();
    fs::write(fixture.root().join("unknown"), "x").unwrap();
    assert_refused_and_preserved(&fixture);

    // Partial root without a provisioning marker (looks like ours, is not proven).
    let fixture = Fixture::new("amb-nomarker");
    make_committed(&fixture.backup(), &fixture.location, "OLD");
    create_structure(&fixture.root());
    assert_refused_and_preserved(&fixture);

    // Malformed marker.
    let fixture = Fixture::new("amb-badmarker");
    make_committed(&fixture.backup(), &fixture.location, "OLD");
    fs::create_dir(fixture.root()).unwrap();
    fs::write(fixture.root().join(PROVISIONING_MARKER), "{not json").unwrap();
    fs::write(fixture.root().join("payload.txt"), "x").unwrap();
    assert_refused_and_preserved(&fixture);

    // Marker for another server/location.
    let fixture = Fixture::new("amb-othermarker");
    make_committed(&fixture.backup(), &fixture.location, "OLD");
    let other = PrefixLocation {
        server_id: Some("other".into()),
        ..fixture.location.clone()
    };
    ManagedPrefix::for_test(&other)
        .begin_provisioning("fp")
        .unwrap();
    assert!(matches!(
        fixture.prefix().classify_root().unwrap(),
        RootState::Foreign(_)
    ));
    assert_refused_and_preserved(&fixture);

    // Committed manifest of another managed location.
    let fixture = Fixture::new("amb-othermanifest");
    make_committed(&fixture.backup(), &fixture.location, "OLD");
    create_structure(&fixture.root());
    write_prefix_manifest(
        &fixture.location.path,
        &manifest_for(&fixture.location, Some("other"), "NEW"),
    )
    .unwrap();
    assert_refused_and_preserved(&fixture);

    // Legacy-valid root + backup (unreachable under protocol).
    let fixture = Fixture::new("amb-legacy");
    make_committed(&fixture.backup(), &fixture.location, "OLD");
    make_legacy(&fixture.root(), "LEGACY");
    assert_refused_and_preserved(&fixture);

    // A legacy backup with incomplete structure and no removal evidence is not ours to judge.
    let fixture = Fixture::new("amb-legacy-partial");
    make_legacy(&fixture.backup(), "OLD");
    fs::remove_dir_all(fixture.backup().join("drive_c")).unwrap();
    assert!(matches!(
        fixture.prefix().classify_backup().unwrap(),
        BackupState::Foreign(_)
    ));
    assert_refused_and_preserved(&fixture);

    // Root missing while the backup is only a removal remnant.
    let fixture = Fixture::new("amb-removing");
    make_committed(&fixture.backup(), &fixture.location, "OLD");
    fixture.prefix().invalidate_restoration_authority().unwrap();
    assert_eq!(
        fixture.prefix().classify_backup().unwrap(),
        BackupState::Removing
    );
    assert_refused_and_preserved(&fixture);
}

#[test]
fn ambiguous_backup_objects_refuse_even_with_committed_root() {
    type Build = fn(&Fixture);
    let builders: [(&str, Build); 6] = [
        ("symlink", |f| {
            let target = f.dir.join("elsewhere");
            make_committed(&target, &f.location, "OLD");
            std::os::unix::fs::symlink(&target, f.backup()).unwrap();
        }),
        ("file", |f| fs::write(f.backup(), "file").unwrap()),
        ("no-evidence", |f| {
            fs::create_dir(f.backup()).unwrap();
            fs::write(f.backup().join("user-data"), "mine").unwrap();
        }),
        ("other-server", |f| {
            create_structure(&f.backup());
            write_prefix_manifest(
                f.backup().to_str().unwrap(),
                &manifest_for(&f.location, Some("other"), "OLD"),
            )
            .unwrap();
        }),
        ("protected-installation", |f| {
            make_committed(&f.backup(), &f.location, "OLD");
            fs::write(f.backup().join(INSTALLATION_MARKER), "{}").unwrap();
        }),
        ("manifest-symlink", |f| {
            create_structure(&f.backup());
            std::os::unix::fs::symlink("/etc/hostname", f.backup().join(PREFIX_MARKER)).unwrap();
        }),
    ];
    for (label, build) in builders {
        for root in ["absent", "committed", "provisioning"] {
            let fixture = Fixture::new("amb-backup");
            build(&fixture);
            match root {
                "committed" => make_committed(&fixture.root(), &fixture.location, "NEW"),
                "provisioning" => make_provisioning(&fixture, "PARTIAL"),
                _ => {}
            }
            assert!(
                matches!(
                    fixture.prefix().classify_backup().unwrap(),
                    BackupState::Foreign(_)
                ),
                "{label}"
            );
            assert_refused_and_preserved(&fixture);
        }
    }
}

#[test]
fn symlinked_root_and_marker_are_foreign() {
    let fixture = Fixture::new("root-symlink");
    let target = fixture.dir.join("target");
    make_committed(&target, &fixture.location, "TARGET");
    std::os::unix::fs::symlink(&target, fixture.root()).unwrap();
    assert!(matches!(
        fixture.prefix().classify_root().unwrap(),
        RootState::Foreign(_)
    ));
    assert_eq!(fixture.recover().unwrap().action, RecoveryAction::None);
    make_committed(&fixture.backup(), &fixture.location, "OLD");
    assert_refused_and_preserved(&fixture);
    assert_eq!(payload(&target), "TARGET");

    let fixture = Fixture::new("marker-symlink");
    let elsewhere = fixture.dir.join("marker.json");
    ManagedPrefix::for_test(&fixture.location)
        .begin_provisioning("fp")
        .unwrap();
    fs::rename(fixture.root().join(PROVISIONING_MARKER), &elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, fixture.root().join(PROVISIONING_MARKER)).unwrap();
    fs::write(fixture.root().join("payload.txt"), "x").unwrap();
    assert!(matches!(
        fixture.prefix().classify_root().unwrap(),
        RootState::Foreign(_)
    ));
    // Fixed transaction evidence that cannot be verified is refused, never reported as resolved.
    assert_refused_and_preserved(&fixture);
    make_committed(&fixture.backup(), &fixture.location, "OLD");
    assert_refused_and_preserved(&fixture);
}

#[test]
fn protected_installation_in_partial_root_is_never_discarded() {
    let fixture = Fixture::new("protected-root");
    make_provisioning(&fixture, "PARTIAL");
    fs::write(fixture.root().join(INSTALLATION_MARKER), "{}").unwrap();
    assert!(matches!(
        fixture.prefix().classify_root().unwrap(),
        RootState::Foreign(_)
    ));
    assert_refused_and_preserved(&fixture);

    // Reset never moves a protected root aside, even when called directly.
    let fixture = Fixture::new("protected-reset");
    make_committed(&fixture.root(), &fixture.location, "GAME");
    fs::write(fixture.root().join(INSTALLATION_MARKER), "{}").unwrap();
    let before = snapshot(&fixture.dir);
    assert!(fixture.prefix().preserve_for_reset().is_err());
    assert_eq!(snapshot(&fixture.dir), before);
}

#[test]
fn custom_prefix_is_never_recovered_destructively() {
    let fixture = Fixture::new("custom");
    let custom = PrefixLocation {
        scope: PrefixScope::Custom,
        managed: false,
        ..fixture.location.clone()
    };
    // Provisioning-looking root, a fixed-name backup and a legacy residue around a custom path.
    fs::create_dir(fixture.root()).unwrap();
    fs::write(
        fixture.root().join(PROVISIONING_MARKER),
        serde_json::json!({"schemaVersion":1,"scope":"custom","serverId":"srv","prefixPath":fixture.location.path,"prefixFingerprint":"fp","createdAtMs":1}).to_string(),
    )
    .unwrap();
    fs::write(fixture.root().join("user.reg"), "user data").unwrap();
    make_committed(&fixture.backup(), &fixture.location, "OLD");
    make_committed(
        &fixture.dir.join("prefixes/.pfx.reset-backup-1-2"),
        &fixture.location,
        "L",
    );
    let before = snapshot(&fixture.dir);
    assert!(ManagedPrefix::new(&custom, None).is_err());
    assert_eq!(
        super::super::recover_interrupted_prefix(&custom, None).unwrap(),
        RecoveryOutcome::untouched()
    );
    assert_eq!(
        super::super::recover_with_guard_held(&custom, None).unwrap(),
        RecoveryOutcome::untouched()
    );
    assert_eq!(
        super::super::recover_residue_with_guard_held(&custom, None).unwrap(),
        RecoveryOutcome::untouched()
    );
    let unmanaged_isolated = PrefixLocation {
        managed: false,
        ..fixture.location.clone()
    };
    assert!(ManagedPrefix::new(&unmanaged_isolated, None).is_err());
    assert_eq!(
        snapshot(&fixture.dir),
        before,
        "custom data must be byte-identical"
    );
}

#[test]
fn live_process_using_the_prefix_blocks_destructive_recovery() {
    let fixture = Fixture::new("live");
    make_committed(&fixture.backup(), &fixture.location, "OLD");
    make_provisioning(&fixture, "PARTIAL");
    let mut user = Command::new("/bin/sleep")
        .arg("30")
        .env("WINEPREFIX", &fixture.location.path)
        .spawn()
        .unwrap();
    // The process is visible once its environ is readable.
    let pid = user.id();
    while ro_tools_linux::find_prefix_processes(&fixture.location.path)
        .iter()
        .all(|identity| identity.pid != pid)
    {
        std::thread::yield_now();
    }
    let before = snapshot(&fixture.dir);
    let error = fixture.recover().unwrap_err();
    assert!(error.contains("proceso(s) usando el entorno"), "{error}");
    assert_eq!(snapshot(&fixture.dir), before);
    user.kill().unwrap();
    user.wait().unwrap();
    assert_eq!(
        fixture.recover_twice().action,
        RecoveryAction::DiscardPartialAndRestore
    );
    assert_eq!(payload(&fixture.root()), "OLD");
}

#[test]
fn health_never_treats_provisioning_marker_as_ready() {
    let fixture = Fixture::new("health");
    make_provisioning(&fixture, "PARTIAL");
    create_structure(&fixture.root());
    let health = inspect_prefix(&fixture.location.path);
    assert!(health.structure_ok);
    assert!(health.provisioning);
    assert!(!health.configured);
    assert!(health
        .issues
        .iter()
        .any(|issue| issue.contains("se interrumpió")));
    // Even a legacy marker cannot make an uncommitted root ready.
    fs::write(fixture.root().join(LEGACY_PREFIX_MARKER), "configured").unwrap();
    assert!(!inspect_prefix(&fixture.location.path).configured);
    fs::remove_file(fixture.root().join(LEGACY_PREFIX_MARKER)).unwrap();
    // Past the pre-commit boundary it is still not ready, and says it needs review.
    fixture.prefix().revoke_rollback_authority().unwrap();
    let health = inspect_prefix(&fixture.location.path);
    assert!(health.provisioning && !health.configured);
    assert!(health
        .issues
        .iter()
        .any(|issue| issue.contains("no puede probarse")));
    // The manifest commits it, stale marker or not.
    write_prefix_manifest(
        &fixture.location.path,
        &manifest_for(&fixture.location, Some("srv"), "NEW"),
    )
    .unwrap();
    let health = inspect_prefix(&fixture.location.path);
    assert!(health.configured);
    assert!(!health.provisioning);
}

#[test]
fn residue_predicates_are_read_only_and_precise() {
    let fixture = Fixture::new("residue");
    let root = fixture.root();
    assert!(!transaction_residue_present(&root));
    make_committed(&root, &fixture.location, "OLD");
    assert!(!transaction_residue_present(&root));
    fs::rename(&root, fixture.backup()).unwrap();
    let gate = |root: &Path| transaction_gate("srv", root);
    assert!(transaction_residue_present(&root));
    assert_eq!(gate(&root), TransactionGate::PendingTransaction);
    make_provisioning(&fixture, "NEW");
    assert_eq!(gate(&root), TransactionGate::PendingTransaction);
    fixture.prefix().revoke_rollback_authority().unwrap();
    assert_eq!(
        gate(&root),
        TransactionGate::PendingTransaction,
        "C is still uncommitted"
    );
    write_prefix_manifest(
        &fixture.location.path,
        &manifest_for(&fixture.location, Some("srv"), "NEW"),
    )
    .unwrap();
    assert!(transaction_residue_present(&root));
    assert_eq!(
        gate(&root),
        TransactionGate::CommittedValid,
        "committed root resolves normally"
    );
    // Legacy residue counts only when the root holds no data.
    let fixture = Fixture::new("residue-legacy");
    make_committed(
        &fixture.dir.join("prefixes/.pfx.reset-backup-1-2"),
        &fixture.location,
        "L",
    );
    assert!(transaction_residue_present(&fixture.root()));
    assert_eq!(
        gate(&fixture.root()),
        TransactionGate::NoTransactionEvidence,
        "old-build residue never steers binding resolution"
    );
    make_committed(&fixture.root(), &fixture.location, "CUR");
    assert!(!transaction_residue_present(&fixture.root()));
    // An empty backup holds no data and never steers resolution.
    let fixture = Fixture::new("residue-empty-backup");
    fs::create_dir(fixture.backup()).unwrap();
    assert!(transaction_residue_present(&fixture.root()));
    assert_eq!(
        gate(&fixture.root()),
        TransactionGate::NoTransactionEvidence
    );
}

/// Only a valid commit record clears fixed transaction evidence; classification is read-only.
#[test]
fn transaction_gate_never_treats_an_invalid_manifest_as_a_commit() {
    for evidence in ["P", "C", "B"] {
        for how in [
            "corrupt",
            "directory",
            "symlink",
            "unreadable",
            "foreign-server",
        ] {
            let fixture = Fixture::new("gate");
            match evidence {
                "P" => make_provisioning(&fixture, "NEW"),
                "C" => make_commit_pending(&fixture, "NEW"),
                _ => {
                    make_committed(&fixture.backup(), &fixture.location, "OLD");
                    fs::create_dir(fixture.root()).unwrap();
                    fs::write(fixture.root().join("payload.txt"), "NEW").unwrap();
                }
            }
            let manifest = fixture.root().join(PREFIX_MARKER);
            let valid =
                serde_json::to_vec_pretty(&manifest_for(&fixture.location, Some("srv"), "NEW"))
                    .unwrap();
            match how {
                "corrupt" => fs::write(&manifest, b"{\"schemaVersion\":2,").unwrap(),
                "directory" => fs::create_dir(&manifest).unwrap(),
                "symlink" => {
                    let elsewhere = fixture.dir.join("elsewhere.json");
                    fs::write(&elsewhere, &valid).unwrap();
                    std::os::unix::fs::symlink(&elsewhere, &manifest).unwrap();
                }
                "unreadable" => {
                    fs::write(&manifest, &valid).unwrap();
                    fs::set_permissions(&manifest, fs::Permissions::from_mode(0o000)).unwrap();
                }
                _ => fs::write(
                    &manifest,
                    serde_json::to_vec_pretty(&manifest_for(&fixture.location, Some("other"), "X"))
                        .unwrap(),
                )
                .unwrap(),
            }
            let before = snapshot(&fixture.dir);
            let gate = transaction_gate("srv", &fixture.root());
            assert!(
                matches!(gate, TransactionGate::AmbiguousTransaction(_)),
                "{evidence} {how}: {gate:?}"
            );
            assert!(gate.blocks_resolution());
            assert_eq!(
                snapshot(&fixture.dir),
                before,
                "{evidence} {how}: gate mutated"
            );
            if how == "unreadable" {
                fs::set_permissions(&manifest, fs::Permissions::from_mode(0o600)).unwrap();
            }
        }
    }
    // A valid commit record with a launcher-owned backup only needs cleanup.
    let fixture = Fixture::new("gate-committed");
    make_committed(&fixture.backup(), &fixture.location, "OLD");
    make_committed_with_stale_marker(&fixture, "NEW");
    assert_eq!(
        transaction_gate("srv", &fixture.root()),
        TransactionGate::CommittedValid
    );
    // ...but not when the backup cannot be verified.
    fs::remove_file(fixture.backup().join(PREFIX_MARKER)).unwrap();
    assert!(transaction_gate("srv", &fixture.root()).blocks_resolution());
}

// ---------------------------------------------------------------------------------------------
// Transaction orchestration with an injected provisioning engine (no Wine)

#[derive(Default)]
struct FileWork {
    location: Option<PrefixLocation>,
    tag: &'static str,
    /// Commit a v3 manifest with this full digest instead of a v2 manifest.
    v3_digest: Option<String>,
    fail_provision: bool,
    /// Fail the real manifest writer at this step, on either side of its rename.
    manifest_fault: Option<ManifestFault>,
    /// After an `AfterRename` failure, the manifest disappears before error handling sees it.
    lose_manifest_after_rename: bool,
    /// Write a binary user-data sentinel into the new root while provisioning.
    sentinel: bool,
    fail_quiesce: bool,
    quiesced: u32,
}

impl FileWork {
    fn new(location: &PrefixLocation, tag: &'static str) -> Self {
        Self {
            location: Some(location.clone()),
            tag,
            ..Default::default()
        }
    }

    fn root(&self) -> PathBuf {
        PathBuf::from(&self.location.as_ref().unwrap().path)
    }
}

impl ProvisioningWork for FileWork {
    async fn provision(&mut self) -> Result<(), String> {
        let root = self.root();
        create_structure(&root);
        fs::write(root.join("payload.txt"), self.tag).unwrap();
        if self.sentinel {
            fs::write(root.join("user-save.dat"), USER_SENTINEL).unwrap();
        }
        fault::reached("provision:partial");
        if self.fail_provision {
            return Err("winetricks falló".into());
        }
        Ok(())
    }

    async fn quiesce(&mut self) -> Result<(), String> {
        self.quiesced += 1;
        if self.fail_quiesce {
            return Err("quedan procesos".into());
        }
        Ok(())
    }

    fn commit(&mut self) -> Result<(), ManifestCommitError> {
        if let Some(fault) = self.manifest_fault {
            crate::utils::test_hooks::arm(fault);
        }
        let location = self.location.as_ref().unwrap();
        let result = match &self.v3_digest {
            Some(digest) => write_v3_manifest(&location.path, location, digest, self.tag),
            None => write_prefix_manifest(
                &location.path,
                &manifest_for(location, location.server_id.as_deref(), self.tag),
            ),
        };
        if self.lose_manifest_after_rename
            && matches!(result, Err(ManifestCommitError::AfterRename(_)))
        {
            fs::remove_file(Path::new(&location.path).join(PREFIX_MARKER)).unwrap();
            sync_dir(Path::new(&location.path)).unwrap();
            fault::reached("commit:manifest-lost");
        }
        result
    }
}

fn write_v3_manifest(
    dir: &str,
    location: &PrefixLocation,
    digest: &str,
    tag: &str,
) -> Result<(), ManifestCommitError> {
    crate::utils::write_prefix_manifest_v3(
        dir,
        &crate::utils::PrefixManifestV3 {
            schema_version: crate::utils::PREFIX_SCHEMA_V3,
            scope: location.scope,
            server_id: location.server_id.clone(),
            runner_kind: "wine".into(),
            runner_path: "/opt/fake/bin/wine".into(),
            components: vec![tag.to_string()],
            prefix_fingerprint: crate::utils::PrefixFingerprintEnvelope {
                schema_version: 1,
                algorithm: "sha256".into(),
                digest: digest.to_string(),
            },
        },
    )
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

#[test]
fn reset_transaction_commits_new_and_cleans_everything() {
    let fixture = Fixture::new("tx-reset");
    make_committed(&fixture.root(), &fixture.location, "OLD");
    let mut work = FileWork::new(&fixture.location, "NEW");
    let committed = block_on(run_managed_transaction(
        &fixture.prefix(),
        TransactionKind::Reset,
        "fp",
        &mut work,
    ))
    .unwrap();
    assert_eq!(
        committed.cleanup.unwrap().action,
        RecoveryAction::CompleteCommit {
            stale_marker: true,
            backup: true
        }
    );
    assert_eq!(payload(&fixture.root()), "NEW");
    assert!(!exists(&fixture.backup()));
    assert!(!exists(&fixture.root().join(PROVISIONING_MARKER)));
    assert_eq!(fixture.recover_twice().action, RecoveryAction::None);
}

#[test]
fn ordinary_failures_use_the_same_recovery_state_machine() {
    for failure in ["provision", "commit"] {
        let fixture = Fixture::new("tx-fail");
        make_committed(&fixture.root(), &fixture.location, "OLD");
        let mut work = FileWork::new(&fixture.location, "NEW");
        work.fail_provision = failure == "provision";
        work.manifest_fault = (failure == "commit").then_some(ManifestFault::TempWrite);
        let error = block_on(run_managed_transaction(
            &fixture.prefix(),
            TransactionKind::Reset,
            "fp",
            &mut work,
        ))
        .err()
        .unwrap();
        assert!(
            error.contains("restaurado correctamente"),
            "{failure}: {error}"
        );
        assert!(work.quiesced >= 1, "failure path must quiesce first");
        assert_eq!(payload(&fixture.root()), "OLD");
        assert!(!exists(&fixture.backup()));
        assert_eq!(fixture.recover_twice().action, RecoveryAction::None);
    }
    // Initial setup failure discards only the marker-proven root.
    let fixture = Fixture::new("tx-initial-fail");
    let mut work = FileWork::new(&fixture.location, "NEW");
    work.fail_provision = true;
    let error = block_on(run_managed_transaction(
        &fixture.prefix(),
        TransactionKind::InitialSetup,
        "fp",
        &mut work,
    ))
    .err()
    .unwrap();
    assert!(error.contains("entorno incompleto"), "{error}");
    assert!(!exists(&fixture.root()));
}

#[test]
fn failed_quiescence_preserves_everything_for_later_recovery() {
    let fixture = Fixture::new("tx-quiesce");
    make_committed(&fixture.root(), &fixture.location, "OLD");
    let mut work = FileWork::new(&fixture.location, "NEW");
    work.fail_quiesce = true;
    let error = block_on(run_managed_transaction(
        &fixture.prefix(),
        TransactionKind::Reset,
        "fp",
        &mut work,
    ))
    .err()
    .unwrap();
    assert!(error.contains("Limpieza pendiente"), "{error}");
    assert_eq!(
        fixture.prefix().classify_root().unwrap(),
        RootState::Provisioning
    );
    assert_eq!(payload(&fixture.backup()), "OLD");
    assert_eq!(
        fixture.recover_twice().action,
        RecoveryAction::DiscardPartialAndRestore
    );
    assert_eq!(payload(&fixture.root()), "OLD");
}

#[test]
fn reset_refuses_a_second_backup_and_never_overwrites_it() {
    let fixture = Fixture::new("tx-second-backup");
    make_committed(&fixture.root(), &fixture.location, "CUR");
    make_committed(&fixture.backup(), &fixture.location, "PENDING");
    let before = snapshot(&fixture.dir);
    let mut work = FileWork::new(&fixture.location, "NEW");
    let error = block_on(run_managed_transaction(
        &fixture.prefix(),
        TransactionKind::Reset,
        "fp",
        &mut work,
    ))
    .err()
    .unwrap();
    assert!(error.contains("respaldo pendiente"), "{error}");
    assert_eq!(snapshot(&fixture.dir), before);
}

#[test]
fn reset_backup_name_is_fixed_and_sibling() {
    let root = Path::new("/data/ro-launcher/prefixes/abc-v3-0123");
    assert_eq!(
        reset_backup_path(root).unwrap(),
        PathBuf::from("/data/ro-launcher/prefixes/.abc-v3-0123.reset-backup")
    );
    assert_eq!(
        reset_backup_path(root).unwrap(),
        reset_backup_path(root).unwrap()
    );
}

// ---------------------------------------------------------------------------------------------
// Real SIGKILL fault injection in child processes with a disposable HOME

const CHILD_ENV: &str = "RO_F05_CHILD";
const SCENARIO_ENV: &str = "RO_F05_SCENARIO";
const CHILD_TEST: &str = "tools::prefix::recovery::tests::f05_child_entry";
const RESULT: &str = "F05-RESULT";

struct Home {
    dir: PathBuf,
}

impl Home {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "ro-f05-home-{label}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(dir.join(".local/share/ro-launcher")).unwrap();
        Self { dir }
    }
    /// The shared managed prefix of this HOME (`prefix_path()` inside the child).
    fn root(&self) -> PathBuf {
        self.dir.join(".local/share/ro-launcher/prefix")
    }
    fn backup(&self) -> PathBuf {
        self.dir
            .join(".local/share/ro-launcher/.prefix.reset-backup")
    }
    fn location(&self) -> PrefixLocation {
        PrefixLocation {
            path: self.root().to_string_lossy().into_owned(),
            scope: PrefixScope::Shared,
            managed: true,
            server_id: None,
        }
    }
    fn classify(&self) -> (RootState, BackupState) {
        let location = self.location();
        let prefix = ManagedPrefix::for_test(&location);
        (
            prefix.classify_root().unwrap(),
            prefix.classify_backup().unwrap(),
        )
    }
    fn make_old(&self) {
        make_committed(&self.root(), &self.location(), "OLD");
        fs::write(self.root().join("save.bin"), b"\x01OLD-SAVE\xfe").unwrap();
    }

    fn command(&self, scenario: &str, checkpoint: Option<&str>) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", CHILD_TEST, "--nocapture", "--test-threads=1"])
            .env_clear()
            .env("HOME", &self.dir)
            .env("XDG_DATA_HOME", self.dir.join(".local/share"))
            .env("TMPDIR", std::env::temp_dir())
            .env("PATH", "/usr/bin:/bin")
            .env(CHILD_ENV, "1")
            .env(SCENARIO_ENV, scenario)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Ok(target) = std::env::var("CARGO_TARGET_DIR") {
            command.env("CARGO_TARGET_DIR", target);
        }
        if let Some(checkpoint) = checkpoint {
            command.env(fault::CHECKPOINT_ENV, checkpoint);
        }
        command
    }

    /// Run a scenario to completion; returns its `F05-RESULT` lines.
    fn run(&self, scenario: &str) -> Vec<String> {
        let output = self.command(scenario, None).output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        assert!(
            output.status.success(),
            "{scenario} failed\n{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        stdout
            .lines()
            .filter_map(|line| {
                line.split_once(RESULT)
                    .map(|(_, rest)| rest.trim().to_string())
            })
            .collect()
    }

    /// Start a scenario and block until it reaches `checkpoint` (pipe rendezvous, no sleeps).
    fn pause_at(&self, scenario: &str, checkpoint: &str) -> Paused {
        let mut child = self.command(scenario, Some(checkpoint)).spawn().unwrap();
        let watchdog = Watchdog::arm(child.id());
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut seen = Vec::new();
        loop {
            let mut line = String::new();
            let read = stdout.read_line(&mut line).unwrap();
            if read == 0 {
                let mut stderr = String::new();
                child
                    .stderr
                    .take()
                    .unwrap()
                    .read_to_string(&mut stderr)
                    .unwrap();
                panic!(
                    "{scenario} exited before {checkpoint}\n{}\n{stderr}",
                    seen.join("")
                );
            }
            if line.contains(&format!("{} {checkpoint}", fault::REACHED)) {
                break;
            }
            seen.push(line);
        }
        Paused {
            child,
            stdout,
            seen,
            _watchdog: watchdog,
        }
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

struct Paused {
    child: Child,
    stdout: BufReader<std::process::ChildStdout>,
    seen: Vec<String>,
    _watchdog: Watchdog,
}

impl Paused {
    fn sigkill(mut self) -> Vec<String> {
        unsafe { libc::kill(self.child.id() as i32, libc::SIGKILL) };
        let status = self.child.wait().unwrap();
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(status.signal(), Some(libc::SIGKILL));
        self.seen
    }

    fn resume(mut self) -> Vec<String> {
        let mut stdin = self.child.stdin.take().unwrap();
        writeln!(stdin, "continue").unwrap();
        drop(stdin);
        let mut rest = String::new();
        self.stdout.read_to_string(&mut rest).unwrap();
        assert!(self.child.wait().unwrap().success(), "{rest}");
        rest.lines()
            .filter_map(|line| line.split_once(RESULT).map(|(_, r)| r.trim().to_string()))
            .collect()
    }
}

/// Kills a stuck child so a broken rendezvous fails the test instead of hanging CI.
struct Watchdog(Option<std::sync::mpsc::Sender<()>>);

impl Watchdog {
    fn arm(pid: u32) -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        std::thread::spawn(move || {
            if rx.recv_timeout(Duration::from_secs(120))
                == Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            {
                unsafe { libc::kill(pid as i32, libc::SIGKILL) };
            }
        });
        Self(Some(tx))
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {
        if let Some(tx) = self.0.take() {
            let _ = tx.send(());
        }
    }
}

fn child_print(line: impl std::fmt::Display) {
    let mut out = std::io::stdout().lock();
    writeln!(out, "\n{RESULT} {line}").unwrap();
    out.flush().unwrap();
}

/// Child entrypoint. A no-op in the normal suite.
#[test]
fn f05_child_entry() {
    if std::env::var_os(CHILD_ENV).is_none() {
        return;
    }
    let scenario = std::env::var(SCENARIO_ENV).unwrap();
    block_on(child_scenario(&scenario));
}

async fn child_scenario(scenario: &str) {
    let location = PrefixLocation {
        path: crate::utils::prefix_path(),
        scope: PrefixScope::Shared,
        managed: true,
        server_id: None,
    };
    match scenario {
        "reset" | "initial" => {
            let _guard =
                crate::utils::OperationGuard::acquire("prefix", Path::new(&location.path)).unwrap();
            assert_eq!(
                super::super::recover_with_guard_held(&location, None)
                    .unwrap()
                    .action,
                RecoveryAction::None
            );
            let prefix = ManagedPrefix::new(&location, None).unwrap();
            let mut work = FileWork::new(&location, "NEW");
            let kind = if scenario == "reset" {
                TransactionKind::Reset
            } else {
                TransactionKind::InitialSetup
            };
            let committed = run_managed_transaction(&prefix, kind, "fp", &mut work)
                .await
                .unwrap();
            child_print(format!("committed {:?}", committed.cleanup.unwrap().action));
        }
        "reset-lost" | "initial-lost" => {
            // The manifest rename succeeds, the directory fsync fails, and the manifest vanishes
            // before the transaction's error handling observes it.
            let _guard =
                crate::utils::OperationGuard::acquire("prefix", Path::new(&location.path)).unwrap();
            let prefix = ManagedPrefix::new(&location, None).unwrap();
            let mut work = FileWork::new(&location, "NEW");
            work.manifest_fault = Some(ManifestFault::DirSync);
            work.lose_manifest_after_rename = true;
            work.sentinel = true;
            let kind = if scenario == "reset-lost" {
                TransactionKind::Reset
            } else {
                TransactionKind::InitialSetup
            };
            let error = run_managed_transaction(&prefix, kind, "fp", &mut work)
                .await
                .err()
                .expect("a lost manifest never reports success");
            child_print(format!(
                "failed {}",
                error.contains("no puede resolverse con seguridad")
            ));
        }
        "recover" => {
            let guard = crate::utils::OperationGuard::acquire("prefix", Path::new(&location.path));
            let Ok(_guard) = guard else {
                child_print("busy");
                return;
            };
            match super::super::recover_with_guard_held(&location, None) {
                Ok(first) => {
                    child_print(format!("{:?}", first.action));
                    let second = super::super::recover_with_guard_held(&location, None).unwrap();
                    assert_eq!(
                        second.action,
                        RecoveryAction::None,
                        "recovery is not idempotent"
                    );
                    child_print("idempotent");
                }
                Err(error) => {
                    child_print(format!(
                        "refused {}",
                        error.contains("no puede resolverse con seguridad")
                    ));
                    let again = super::super::recover_with_guard_held(&location, None);
                    child_print(format!(
                        "refused-again {}",
                        again.is_err_and(|error| error.contains("no puede resolverse"))
                    ));
                }
            }
        }
        "supervised-reset" | "supervised-reset-running" | "supervised-recover" => {
            supervised_scenario(scenario, &location).await
        }
        "binding" => binding_scenario(),
        "collision" => collision_scenario(),
        "fp-reset" => fingerprint_reset_scenario().await,
        "fp-resolve" => fingerprint_resolve_scenario().await,
        "fp-resolve-blocked" => fingerprint_blocked_scenario().await,
        "multi-pending" => multiple_pending_scenario(),
        "path-safety" => path_safety_scenario(&location),
        other => panic!("unknown scenario {other}"),
    }
}

fn kill_and_classify(home: &Home, scenario: &str, checkpoint: &str) -> (RootState, BackupState) {
    home.pause_at(scenario, checkpoint).sigkill();
    home.classify()
}

fn recover_in_new_process(home: &Home) -> String {
    let lines = home.run("recover");
    assert_eq!(
        lines.last().map(String::as_str),
        Some("idempotent"),
        "{lines:?}"
    );
    lines[0].clone()
}

fn assert_old_restored(home: &Home) {
    assert_eq!(payload(&home.root()), "OLD");
    assert_eq!(
        fs::read(home.root().join("save.bin")).unwrap(),
        b"\x01OLD-SAVE\xfe"
    );
    assert!(!exists(&home.backup()));
    assert!(!exists(&home.root().join(PROVISIONING_MARKER)));
}

fn assert_new_committed(home: &Home) {
    assert_eq!(payload(&home.root()), "NEW");
    assert!(!exists(&home.root().join("save.bin")));
    assert!(!exists(&home.backup()));
    assert!(!exists(&home.root().join(PROVISIONING_MARKER)));
}

/// Every reset checkpoint, killed with SIGKILL while holding the prefix guard, then recovered by a
/// new process that must acquire the same guard (kernel released the fcntl lock).
#[test]
fn sigkill_reset_checkpoints_recover_to_old_or_committed_new() {
    use BackupState as B;
    use RootState as R;
    let cases: [(&str, R, B, &str, bool); 6] = [
        (
            "reset:backup-preserved",
            R::Absent,
            B::Restorable,
            "RestoreBackup",
            false,
        ),
        (
            "provision:root-created",
            R::Empty,
            B::Restorable,
            "RestoreBackup",
            false,
        ),
        (
            "provision:marker-durable",
            R::Provisioning,
            B::Restorable,
            "DiscardPartialAndRestore",
            false,
        ),
        (
            "provision:partial",
            R::Provisioning,
            B::Restorable,
            "DiscardPartialAndRestore",
            false,
        ),
        (
            "commit:manifest-durable",
            R::Committed { stale_marker: true },
            B::Restorable,
            "CompleteCommit { stale_marker: true, backup: true }",
            true,
        ),
        (
            "recovery:marker-removed",
            R::Committed {
                stale_marker: false,
            },
            B::Restorable,
            "CompleteCommit { stale_marker: false, backup: true }",
            true,
        ),
    ];
    for (checkpoint, root, backup, action, new_wins) in cases {
        let home = Home::new("kill-reset");
        home.make_old();
        let observed = kill_and_classify(&home, "reset", checkpoint);
        assert_eq!(observed, (root, backup), "post-kill state at {checkpoint}");
        assert_eq!(recover_in_new_process(&home), action, "{checkpoint}");
        if new_wins {
            assert_new_committed(&home);
        } else {
            assert_old_restored(&home);
        }
    }
}

#[test]
fn sigkill_initial_setup_is_discarded_then_provisioning_succeeds() {
    let home = Home::new("kill-initial");
    let observed = kill_and_classify(&home, "initial", "provision:partial");
    assert_eq!(observed, (RootState::Provisioning, BackupState::Absent));
    assert_eq!(recover_in_new_process(&home), "DiscardPartial");
    assert!(!exists(&home.root()));
    let lines = home.run("initial");
    assert_eq!(
        lines,
        ["committed CompleteCommit { stale_marker: true, backup: false }"]
    );
    assert_new_committed(&home);
    // Commit is durable before marker cleanup: kill exactly there on a fresh location.
    let home = Home::new("kill-initial-commit");
    let observed = kill_and_classify(&home, "initial", "commit:manifest-durable");
    assert_eq!(
        observed,
        (
            RootState::Committed { stale_marker: true },
            BackupState::Absent
        )
    );
    assert_eq!(
        recover_in_new_process(&home),
        "CompleteCommit { stale_marker: true, backup: false }"
    );
    assert_new_committed(&home);
}

/// Recovery's own destructive steps are killed too; the next recovery still converges.
#[test]
fn sigkill_during_recovery_is_self_stabilizing() {
    let cases: [(&str, &str, RootState, BackupState, &str, bool); 4] = [
        // Partial root reduced to its marker while being discarded.
        (
            "discard:marker-only",
            "provision:partial",
            RootState::Provisioning,
            BackupState::Restorable,
            "DiscardPartialAndRestore",
            false,
        ),
        // Partial root gone, backup not yet restored.
        (
            "recovery:partial-removed",
            "provision:partial",
            RootState::Absent,
            BackupState::Restorable,
            "RestoreBackup",
            false,
        ),
        // Restore rename done, parent fsync not yet.
        (
            "recovery:backup-restored",
            "provision:partial",
            RootState::Committed {
                stale_marker: false,
            },
            BackupState::Absent,
            "None",
            false,
        ),
        // Old backup reduced to its removal evidence while being removed after commit.
        (
            "cleanup:tombstone-only",
            "commit:manifest-durable",
            RootState::Committed {
                stale_marker: false,
            },
            BackupState::Removing,
            "CompleteCommit { stale_marker: false, backup: true }",
            true,
        ),
    ];
    for (recovery_checkpoint, reset_checkpoint, root, backup, action, new_wins) in cases {
        let home = Home::new("kill-recovery");
        home.make_old();
        home.pause_at("reset", reset_checkpoint).sigkill();
        home.pause_at("recover", recovery_checkpoint).sigkill();
        assert_eq!(
            home.classify(),
            (root, backup),
            "{recovery_checkpoint} after {reset_checkpoint}"
        );
        assert_eq!(recover_in_new_process(&home), action);
        if new_wins {
            assert_new_committed(&home);
        } else {
            assert_old_restored(&home);
        }
    }
}

/// Two recoveries race: the guard admits one; the other changes nothing; the result equals one run.
#[test]
fn concurrent_recovery_is_serialized_by_the_prefix_guard() {
    let home = Home::new("concurrent");
    home.make_old();
    home.pause_at("reset", "provision:partial").sigkill();
    let first = home.pause_at("recover", "recovery:partial-removed");
    let before = snapshot(&home.dir);
    assert_eq!(home.run("recover"), ["busy"]);
    assert_eq!(
        snapshot(&home.dir),
        before,
        "the rejected recovery modified data"
    );
    assert_eq!(first.resume(), ["DiscardPartialAndRestore", "idempotent"]);
    assert_old_restored(&home);
}

// ---------------------------------------------------------------------------------------------
// Real supervised fixture: ro-sessiond + inert runner, killed mid-provisioning

struct SupervisedWork<'a> {
    ctx: &'a crate::utils::WineContext,
    op: Option<crate::tools::runner_sessions::RunnerOperation>,
    cleanup: Option<crate::tools::runner_sessions::RunnerCleanup>,
    /// The supervised child stays alive (exec sleep) while the launcher is killed.
    keep_running: bool,
}

impl ProvisioningWork for SupervisedWork<'_> {
    async fn provision(&mut self) -> Result<(), String> {
        let op = self.op.as_ref().unwrap();
        child_print(format!(
            "supervisor {}",
            op.lease().supervisor_identity().pid
        ));
        // Representative writes performed by a supervised child process, not by the test.
        let mut script = "mkdir -p \"$WINEPREFIX/drive_c/windows/system32\" \"$WINEPREFIX/dosdevices\" && \
                      ln -s ../drive_c \"$WINEPREFIX/dosdevices/c:\" && \
                      echo reg > \"$WINEPREFIX/system.reg\" && echo reg > \"$WINEPREFIX/user.reg\" && \
                      printf NEW > \"$WINEPREFIX/payload.txt\""
            .to_string();
        if self.keep_running {
            // The direct child of ro-sessiond becomes `sleep`, still inside the prefix session.
            script.push_str(" && printf $$ > \"$WINEPREFIX/child.pid\" && exec sleep 300");
            let pid_file = Path::new(&self.ctx.prefix).join("child.pid");
            let run = op.run(
                sh_invocation(&self.ctx.prefix, &script),
                "provisión supervisada",
            );
            let started = async {
                // Bounded poll for a condition the supervised child produces, not a timing guess.
                for _ in 0..3000 {
                    if fs::read_to_string(&pid_file).is_ok_and(|pid| !pid.is_empty()) {
                        child_print(format!(
                            "running-child {}",
                            fs::read_to_string(&pid_file).unwrap()
                        ));
                        return;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                panic!("supervised child never started");
            };
            tokio::select! {
                result = run => panic!("supervised child ended early: {result:?}"),
                () = started => fault::reached("provision:child-running"),
            }
            unreachable!("the parent kills this process at provision:child-running");
        }
        let code = op
            .run(
                sh_invocation(&self.ctx.prefix, &script),
                "provisión supervisada",
            )
            .await?;
        assert_eq!(code, 0);
        fault::reached("provision:partial");
        Ok(())
    }

    async fn quiesce(&mut self) -> Result<(), String> {
        if let Some(op) = self.op.take() {
            self.cleanup = Some(op.into_cleanup());
        }
        match &self.cleanup {
            Some(cleanup) => cleanup.quiesce().await,
            None => Ok(()),
        }
    }

    fn commit(&mut self) -> Result<(), ManifestCommitError> {
        write_prefix_manifest(
            &self.ctx.prefix,
            &manifest_for(&self.ctx.location, None, "NEW"),
        )
    }
}

fn sh_invocation(prefix: &str, script: &str) -> crate::utils::RunnerInvocation {
    crate::utils::RunnerInvocation {
        program: PathBuf::from("/bin/sh"),
        args: vec!["-c".into(), script.into()],
        cwd: PathBuf::from(prefix),
        env: vec![("WINEPREFIX".into(), Some(prefix.into()))],
    }
}

fn inert_runner_context(location: &PrefixLocation) -> crate::utils::WineContext {
    inert_runner_context_with(location, b"#!/bin/sh\nexit 0\n")
}

/// Inert runner at a fixed path. A different `wine` body is the same runner path with different
/// observed material, so the desired prefix fingerprint changes (an in-place runner update).
fn inert_runner_context_with(location: &PrefixLocation, wine: &[u8]) -> crate::utils::WineContext {
    let runner_dir = PathBuf::from(std::env::var("HOME").unwrap()).join("fake-runner/bin");
    fs::create_dir_all(&runner_dir).unwrap();
    for (name, body) in [
        ("wine", wine),
        ("wineserver", b"#!/bin/sh\nexit 0\n".as_slice()),
    ] {
        let path = runner_dir.join(name);
        fs::write(&path, body).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let resolved = crate::utils::resolve_runner(runner_dir.join("wine").to_str().unwrap()).unwrap();
    let probe = crate::tools::runtime::probe_runner(&resolved);
    let identity =
        crate::tools::runtime::resolve_prefix_binding(None, &resolved, &probe, false).unwrap();
    assert_eq!(
        identity.location, *location,
        "shared binding must be the managed path"
    );
    crate::utils::WineContext {
        prefix: location.path.clone(),
        location: location.clone(),
        resolved,
        identity,
        probe,
    }
}

async fn supervised_scenario(scenario: &str, location: &PrefixLocation) {
    use crate::tools::runner_sessions::{RunnerOperation, RunnerSessionRegistry};
    let ctx = inert_runner_context(location);
    let sidecar = crate::tools::runner_sessions::workspace_debug_sessiond()
        .expect("build ro-sessiond first (cargo build -p ro-sessiond)");
    let sessions = RunnerSessionRegistry::with_sidecar_for_test(sidecar);
    let game = crate::state::GameProcessHandle::new();
    let anchor = crate::tools::runtime::session_anchor_from_context(&ctx);
    if scenario.starts_with("supervised-reset") {
        // Production order: session first (bootstrap recovery gate), then the exclusive guard.
        let op = RunnerOperation::begin(None, &sessions, &game, &ctx, &anchor)
            .await
            .unwrap();
        let _guard =
            crate::utils::OperationGuard::acquire("prefix", Path::new(&ctx.prefix)).unwrap();
        let prefix = ManagedPrefix::new(location, None).unwrap();
        let mut work = SupervisedWork {
            ctx: &ctx,
            op: Some(op),
            cleanup: None,
            keep_running: scenario == "supervised-reset-running",
        };
        run_managed_transaction(&prefix, TransactionKind::Reset, "fp", &mut work)
            .await
            .unwrap();
        child_print("unexpected-commit");
        return;
    }
    // supervised-recover: a fresh launcher process. Supervisor bootstrap reconciles first and
    // makes the caller start over; the retried operation then runs on the restored prefix.
    let error = RunnerOperation::begin(None, &sessions, &game, &ctx, &anchor)
        .await
        .err()
        .expect("bootstrap must reject the pre-recovery operation");
    child_print(format!("bootstrap {error}"));
    if error != RECOVERED_RETRY_MSG {
        // Fail closed: nothing supervised may run, and asking again changes nothing.
        let again = RunnerOperation::begin(None, &sessions, &game, &ctx, &anchor)
            .await
            .err()
            .expect("an unresolved transaction keeps blocking supervised programs");
        child_print(format!("bootstrap-again {}", again == error));
        assert!(sessions.shutdown_all().await.is_empty());
        return;
    }
    let op = RunnerOperation::begin(None, &sessions, &game, &ctx, &anchor)
        .await
        .unwrap();
    let code = op
        .run(
            sh_invocation(
                &ctx.prefix,
                "test \"$(cat \"$WINEPREFIX/payload.txt\")\" = OLD",
            ),
            "verificación supervisada",
        )
        .await
        .unwrap();
    child_print(format!("supervised-read-old {code}"));
    op.quiesce().await.unwrap();
    assert!(sessions.shutdown_all().await.is_empty());
}

fn wait_for_exit(pid: u32) {
    // Bounded wait for a condition (PR_SET_PDEATHSIG delivery), not a timing assumption.
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while Path::new(&format!("/proc/{pid}")).exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "supervisor {pid} outlived its launcher"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Real `ro-sessiond`, real launcher test process, real SIGKILL while a supervised child is still
/// running in the prefix. PDEATHSIG must take down the supervisor and, through it, the child;
/// only then may a fresh process recover. The runner itself is an inert fixture, not Wine.
#[test]
fn sigkill_during_real_supervised_provisioning_recovers_old_prefix() {
    if crate::tools::runner_sessions::workspace_debug_sessiond().is_none() {
        panic!("build ro-sessiond first (cargo build -p ro-sessiond)");
    }
    let home = Home::new("supervised");
    home.make_old();
    let seen = home
        .pause_at("supervised-reset-running", "provision:child-running")
        .sigkill();
    let reported = |label: &str| -> u32 {
        seen.iter()
            .find_map(|line| line.split_once(&format!("{RESULT} {label} ")))
            .map(|(_, pid)| pid.trim().parse().unwrap())
            .unwrap_or_else(|| panic!("child reported no {label}: {seen:?}"))
    };
    let supervisor = reported("supervisor");
    let child = reported("running-child");
    wait_for_exit(supervisor);
    wait_for_exit(child);
    assert!(
        ro_tools_linux::find_prefix_processes(&home.location().path).is_empty(),
        "no supervised process survives the launcher"
    );
    assert_eq!(
        home.classify(),
        (RootState::Provisioning, BackupState::Restorable)
    );
    assert_eq!(
        payload(&home.root()),
        "NEW",
        "the supervised child wrote the partial root"
    );
    let lines = home.run("supervised-recover");
    assert_eq!(lines[0], format!("bootstrap {RECOVERED_RETRY_MSG}"));
    assert_eq!(lines[1], "supervised-read-old 0");
    assert_old_restored(&home);
}

/// Same real supervisor, killed after the pre-commit revocation: the fresh process must refuse to
/// run anything on the prefix, and must not delete or restore either side.
#[test]
fn sigkill_after_supervised_precommit_revocation_blocks_and_preserves() {
    if crate::tools::runner_sessions::workspace_debug_sessiond().is_none() {
        panic!("build ro-sessiond first (cargo build -p ro-sessiond)");
    }
    let home = Home::new("supervised-precommit");
    home.make_old();
    let seen = home
        .pause_at("supervised-reset", "commit:authority-revoked")
        .sigkill();
    let supervisor: u32 = seen
        .iter()
        .find_map(|line| line.split_once(&format!("{RESULT} supervisor ")))
        .map(|(_, pid)| pid.trim().parse().unwrap())
        .expect("child reported its supervisor");
    wait_for_exit(supervisor);
    assert_eq!(
        home.classify(),
        (RootState::CommitPending, BackupState::Restorable)
    );
    let before = snapshot(&home.dir);
    let lines = home.run("supervised-recover");
    assert!(
        lines[0].contains("no puede resolverse con seguridad"),
        "{lines:?}"
    );
    assert_eq!(lines[1], "bootstrap-again true");
    assert_eq!(
        snapshot_without_runtime(&home.dir),
        strip_runtime(before),
        "a refused supervised start modified prefix data"
    );
}

/// Snapshot of prefix data only: the supervised scenario also writes its inert runner and the
/// launcher's own runtime files under HOME, which are not transaction evidence.
fn snapshot_without_runtime(dir: &Path) -> BTreeMap<String, String> {
    strip_runtime(snapshot(dir))
}

fn strip_runtime(snapshot: BTreeMap<String, String>) -> BTreeMap<String, String> {
    snapshot
        .into_iter()
        .filter(|(path, _)| {
            path.starts_with(".local/share/ro-launcher/prefix")
                || path.starts_with(".local/share/ro-launcher/.prefix")
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Binding resolution must not walk away from an interrupted transaction

fn binding_scenario() {
    use crate::tools::runtime::{probe_runner, resolve_prefix_binding};
    use crate::utils::{
        write_prefix_manifest_v3, PrefixFingerprintEnvelope, PrefixManifestV3, PREFIX_SCHEMA_V3,
    };
    let location = PrefixLocation {
        path: crate::utils::prefix_path(),
        scope: PrefixScope::Shared,
        managed: true,
        server_id: None,
    };
    let ctx = inert_runner_context(&location);
    let server: crate::models::server::ServerConfig = serde_json::from_value(serde_json::json!({
        "id": "srv", "name": "srv", "executablePath": "/unused/game.exe", "prefixMode": "isolated"
    }))
    .unwrap();
    let resolve = || {
        resolve_prefix_binding(
            Some(&server),
            &ctx.resolved,
            &probe_runner(&ctx.resolved),
            false,
        )
        .unwrap()
    };
    let fresh = resolve();
    let digest = fresh.desired_fingerprint.digest.hex_digest();
    let v3 = crate::utils::isolated_prefix_path_v3("srv", &digest);
    assert_eq!(fresh.location.path, v3);
    let v3_location = fresh.location.clone();
    // A committed v3 prefix (OLD) and, for the same runner, a matching legacy v2 prefix.
    create_structure(Path::new(&v3));
    fs::write(Path::new(&v3).join("payload.txt"), "OLD").unwrap();
    let v3_manifest = |digest: &str| PrefixManifestV3 {
        schema_version: PREFIX_SCHEMA_V3,
        scope: PrefixScope::Isolated,
        server_id: Some("srv".into()),
        runner_kind: ctx.resolved.kind_label().into(),
        runner_path: ctx.resolved.runner_path().to_string_lossy().into_owned(),
        components: vec![],
        prefix_fingerprint: PrefixFingerprintEnvelope {
            schema_version: 1,
            algorithm: "sha256".into(),
            digest: digest.to_string(),
        },
    };
    write_prefix_manifest_v3(&v3, &v3_manifest(&digest)).unwrap();
    let runner = fs::canonicalize(ctx.resolved.runner_path()).unwrap();
    let v2 = crate::utils::isolated_prefix_path_for_runner("srv", runner.to_str().unwrap());
    create_structure(Path::new(&v2));
    write_prefix_manifest(
        &v2,
        &PrefixManifest {
            schema_version: PREFIX_SCHEMA_VERSION,
            scope: PrefixScope::Isolated,
            server_id: Some("srv".into()),
            runner_kind: ctx.resolved.kind_label().into(),
            runner_path: runner.to_string_lossy().into_owned(),
            components: vec![],
        },
    )
    .unwrap();
    let committed = resolve();
    child_print(format!(
        "committed {:?} {}",
        committed.status,
        committed.location.path == v3
    ));
    // Reset of the v3 prefix killed right after the old root moved to the fixed backup.
    let backup = reset_backup_path(Path::new(&v3)).unwrap();
    fs::rename(&v3, &backup).unwrap();
    // Without F-05 routing this resolved to the other (v2) prefix and built from there.
    let interrupted = resolve();
    child_print(format!(
        "interrupted {:?} {} {}",
        interrupted.status,
        interrupted.location.path == v3,
        interrupted.ineligible_reasons().is_none()
    ));
    let outcome =
        super::super::recover_interrupted_prefix(&interrupted.location, Some(&digest)).unwrap();
    child_print(format!("recovered {:?}", outcome.action));
    let after = resolve();
    child_print(format!(
        "after {:?} {}",
        after.status,
        after.location.path == v3
    ));
    assert_eq!(payload(Path::new(&v3)), "OLD");
    // The v3 commit check binds the manifest digest to the fingerprint-addressed path.
    fs::rename(&v3, &backup).unwrap();
    write_prefix_manifest_v3(backup.to_str().unwrap(), &v3_manifest(&"f".repeat(64))).unwrap();
    let error = super::super::recover_interrupted_prefix(&v3_location, None).unwrap_err();
    child_print(format!("foreign-digest {}", error.contains("identidad v3")));
}

#[test]
fn binding_stays_on_an_interrupted_transaction_instead_of_switching_prefix() {
    let home = Home::new("binding");
    let lines = home.run("binding");
    assert_eq!(
        lines,
        [
            "committed V3Verified true",
            "interrupted Unknown true true",
            "recovered RestoreBackup",
            "after V3Verified true",
            "foreign-digest true",
        ]
    );
}

fn path_safety_scenario(location: &PrefixLocation) {
    // Symlinked managed root with a valid-looking backup beside it: refused before classification.
    let root = Path::new(&location.path);
    let target = root.parent().unwrap().join("elsewhere");
    make_committed(&target, location, "TARGET");
    std::os::unix::fs::symlink(&target, root).unwrap();
    make_committed(&reset_backup_path(root).unwrap(), location, "OLD");
    let error = super::super::recover_interrupted_prefix(location, None).unwrap_err();
    child_print(format!("root-symlink {}", error.contains("symlink")));
    // Symlinked isolated prefixes root: no isolated recovery at all.
    let isolated_root = crate::utils::isolated_prefix_root();
    let real = isolated_root.parent().unwrap().join("real-prefixes");
    fs::create_dir_all(&real).unwrap();
    std::os::unix::fs::symlink(&real, &isolated_root).unwrap();
    let isolated = PrefixLocation {
        path: crate::utils::isolated_prefix_path("srv"),
        scope: PrefixScope::Isolated,
        managed: true,
        server_id: Some("srv".into()),
    };
    make_committed(
        &reset_backup_path(Path::new(&isolated.path)).unwrap(),
        &isolated,
        "OLD",
    );
    let error = super::super::recover_interrupted_prefix(&isolated, None).unwrap_err();
    child_print(format!(
        "isolated-root-symlink {}",
        error.contains("symlink")
    ));
}

#[test]
fn recovery_entry_point_enforces_managed_path_safety() {
    let home = Home::new("path-safety");
    let before_target = home.dir.join(".local/share/ro-launcher/elsewhere");
    assert_eq!(
        home.run("path-safety"),
        ["root-symlink true", "isolated-root-symlink true"]
    );
    assert_eq!(payload(&before_target), "TARGET");
    assert_eq!(payload(&home.backup()), "OLD");
    assert_eq!(
        payload(
            &home
                .dir
                .join(".local/share/ro-launcher/real-prefixes")
                .join(format!(
                    ".{}.reset-backup",
                    Path::new(&isolated_path_name()).display()
                ))
        ),
        "OLD"
    );
}

fn isolated_path_name() -> String {
    // Same FNV-1a token as `isolated_prefix_path("srv")`, computed without $HOME.
    let mut hash = 0xcbf29ce484222325u64;
    for byte in b"srv" {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

// ---------------------------------------------------------------------------------------------
// P1: rollback authority is revoked before the commit can happen

const USER_SENTINEL: &[u8] = b"\x00user-save-after-commit\xff";

/// Ways the commit record can disappear or break after a successful commit.
const MANIFEST_DAMAGE: [&str; 5] = ["removed", "corrupt", "directory", "symlink", "unreadable"];

fn damage_manifest(root: &Path, how: &str) {
    let manifest = root.join(PREFIX_MARKER);
    match how {
        "removed" => fs::remove_file(&manifest).unwrap(),
        "corrupt" => fs::write(&manifest, b"{\"schemaVersion\":2,").unwrap(),
        "directory" => {
            fs::remove_file(&manifest).unwrap();
            fs::create_dir(&manifest).unwrap();
        }
        "symlink" => {
            let elsewhere = root.parent().unwrap().join("moved-manifest.json");
            fs::rename(&manifest, &elsewhere).unwrap();
            std::os::unix::fs::symlink(&elsewhere, &manifest).unwrap();
        }
        "unreadable" => fs::set_permissions(&manifest, fs::Permissions::from_mode(0o000)).unwrap(),
        other => panic!("unknown damage {other}"),
    }
}

#[test]
fn manifest_never_commits_a_root_that_still_holds_rollback_authority() {
    let fixture = Fixture::new("no-commit-over-p");
    make_provisioning(&fixture, "NEW");
    create_structure(&fixture.root());
    let error = write_prefix_manifest(
        &fixture.location.path,
        &manifest_for(&fixture.location, Some("srv"), "NEW"),
    )
    .unwrap_err();
    assert!(
        matches!(&error, ManifestCommitError::BeforeRename(message) if message.contains("autorización de reversión")),
        "{error:?}"
    );
    assert!(!exists(&fixture.root().join(PREFIX_MARKER)));
    assert_eq!(
        fixture.prefix().classify_root().unwrap(),
        RootState::Provisioning
    );
    // After revocation the same write commits, and nothing can authorize deleting it any more.
    fixture.prefix().revoke_rollback_authority().unwrap();
    assert!(!exists(&fixture.root().join(PROVISIONING_MARKER)));
    assert_eq!(
        fixture.prefix().classify_root().unwrap(),
        RootState::CommitPending
    );
}

/// The independent P1 schedule, in-process: commit, stale marker cleanup never ran, user data is
/// added, the manifest is lost or damaged. Nothing may be discarded or rolled back.
#[test]
fn lost_or_damaged_manifest_after_commit_is_never_discarded() {
    for how in MANIFEST_DAMAGE {
        for with_backup in [false, true] {
            let fixture = Fixture::new("p1-damage");
            if with_backup {
                make_committed(&fixture.backup(), &fixture.location, "OLD");
            }
            make_committed_with_stale_marker(&fixture, "NEW");
            fs::write(fixture.root().join("user-save.dat"), USER_SENTINEL).unwrap();
            damage_manifest(&fixture.root(), how);
            let state = fixture.prefix().classify_root().unwrap();
            if how == "removed" {
                assert_eq!(state, RootState::CommitPending, "{how}");
            } else {
                assert!(matches!(state, RootState::Foreign(_)), "{how}: {state:?}");
            }
            assert_refused_and_preserved(&fixture);
            assert_refused_and_preserved(&fixture);
            if how == "unreadable" {
                fs::set_permissions(
                    fixture.root().join(PREFIX_MARKER),
                    fs::Permissions::from_mode(0o600),
                )
                .unwrap();
            }
            assert_eq!(
                fs::read(fixture.root().join("user-save.dat")).unwrap(),
                USER_SENTINEL,
                "{how} with_backup={with_backup}"
            );
            assert_eq!(payload(&fixture.root()), "NEW");
            if with_backup {
                assert_eq!(payload(&fixture.backup()), "OLD");
            }
        }
    }
}

/// Run one transaction whose manifest writer fails at `fault`; returns the transaction error.
fn failed_commit(
    fixture: &Fixture,
    kind: TransactionKind,
    fault: ManifestFault,
    lose_manifest_after_rename: bool,
) -> String {
    let mut work = FileWork::new(&fixture.location, "NEW");
    work.manifest_fault = Some(fault);
    work.lose_manifest_after_rename = lose_manifest_after_rename;
    work.sentinel = true;
    block_on(run_managed_transaction(
        &fixture.prefix(),
        kind,
        "fp",
        &mut work,
    ))
    .err()
    .unwrap()
}

#[test]
fn manifest_writer_reports_its_commit_stage() {
    let cases = [
        (ManifestFault::TempWrite, false),
        (ManifestFault::Rename, false),
        (ManifestFault::DirSync, true),
    ];
    for (fault, renamed) in cases {
        let fixture = Fixture::new("commit-stage");
        make_commit_pending(&fixture, "NEW");
        crate::utils::test_hooks::arm(fault);
        let error = write_prefix_manifest(
            &fixture.location.path,
            &manifest_for(&fixture.location, Some("srv"), "NEW"),
        )
        .unwrap_err();
        assert_eq!(
            matches!(error, ManifestCommitError::AfterRename(_)),
            renamed,
            "{fault:?}: {error:?}"
        );
        assert_eq!(exists(&fixture.root().join(PREFIX_MARKER)), renamed);
        let temporaries = fs::read_dir(fixture.root())
            .unwrap()
            .flatten()
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".ro-launcher-prefix.tmp")
            })
            .count();
        assert_eq!(temporaries, 0, "{fault:?} left a temporary manifest");
    }
}

/// A/B: the writer proves the manifest rename never happened, so `C -> P` re-grants rollback
/// and the old prefix wins exactly as before the pre-commit boundary existed.
#[test]
fn pre_rename_commit_failure_reinstates_authority_and_restores_old() {
    for fault in [ManifestFault::TempWrite, ManifestFault::Rename] {
        let fixture = Fixture::new("tx-prerename");
        make_committed(&fixture.root(), &fixture.location, "OLD");
        fs::write(fixture.root().join("save.bin"), b"\x01OLD-SAVE\xfe").unwrap();
        let error = failed_commit(&fixture, TransactionKind::Reset, fault, false);
        assert!(error.contains("os error 5"), "{fault:?}: {error}");
        assert!(
            error.contains("restaurado correctamente"),
            "{fault:?}: {error}"
        );
        assert_eq!(payload(&fixture.root()), "OLD");
        assert_eq!(
            fs::read(fixture.root().join("save.bin")).unwrap(),
            b"\x01OLD-SAVE\xfe"
        );
        assert!(!exists(&fixture.root().join("user-save.dat")));
        assert!(!exists(&fixture.backup()));
        assert_eq!(fixture.recover_twice().action, RecoveryAction::None);

        let fixture = Fixture::new("tx-prerename-initial");
        let error = failed_commit(&fixture, TransactionKind::InitialSetup, fault, false);
        assert!(error.contains("entorno incompleto"), "{fault:?}: {error}");
        assert!(!exists(&fixture.root()));
    }
}

/// C: the rename succeeded and the directory fsync failed. NEW is committed and authoritative.
#[test]
fn post_rename_fsync_failure_never_reinstates_authority() {
    let fixture = Fixture::new("tx-postrename-eio");
    make_committed(&fixture.root(), &fixture.location, "OLD");
    let error = failed_commit(
        &fixture,
        TransactionKind::Reset,
        ManifestFault::DirSync,
        false,
    );
    assert!(error.contains("os error 5"), "{error}");
    assert!(error.contains("ya estaba confirmado"), "{error}");
    assert_eq!(payload(&fixture.root()), "NEW");
    assert_eq!(
        fs::read(fixture.root().join("user-save.dat")).unwrap(),
        USER_SENTINEL
    );
    assert!(!exists(&fixture.backup()));
    assert!(!exists(&fixture.root().join(PROVISIONING_MARKER)));
    assert!(!exists(&fixture.root().join(COMMIT_PENDING_MARKER)));
    assert_eq!(fixture.recover_twice().action, RecoveryAction::None);
}

/// D: the rename succeeded, the directory fsync failed, and the manifest vanished before error
/// handling observed it. Disk now looks exactly like "never renamed"; only the writer's commit
/// stage knows better. Rollback is never reactivated: `R` and `B` are preserved, fail closed.
#[test]
fn post_rename_manifest_loss_never_reactivates_rollback() {
    for kind in [TransactionKind::Reset, TransactionKind::InitialSetup] {
        let fixture = Fixture::new("tx-postrename-lost");
        if kind == TransactionKind::Reset {
            make_committed(&fixture.root(), &fixture.location, "OLD");
            fs::write(fixture.root().join("save.bin"), b"\x01OLD-SAVE\xfe").unwrap();
        }
        let error = failed_commit(&fixture, kind, ManifestFault::DirSync, true);
        assert!(error.contains("os error 5"), "{kind:?}: {error}");
        assert!(
            error.contains("no puede resolverse con seguridad"),
            "{kind:?}: {error}"
        );
        assert!(!error.contains("restaurado"), "{kind:?}: {error}");
        assert!(!error.contains("entorno incompleto"), "{kind:?}: {error}");
        assert!(!exists(&fixture.root().join(PREFIX_MARKER)));
        assert!(
            !exists(&fixture.root().join(PROVISIONING_MARKER)),
            "{kind:?}: rollback authority was reactivated after the rename"
        );
        assert!(exists(&fixture.root().join(COMMIT_PENDING_MARKER)));
        assert_eq!(
            fixture.prefix().classify_root().unwrap(),
            RootState::CommitPending
        );
        assert_eq!(payload(&fixture.root()), "NEW");
        assert_eq!(
            fs::read(fixture.root().join("user-save.dat")).unwrap(),
            USER_SENTINEL
        );
        if kind == TransactionKind::Reset {
            assert_eq!(payload(&fixture.backup()), "OLD");
            assert_eq!(
                fs::read(fixture.backup().join("save.bin")).unwrap(),
                b"\x01OLD-SAVE\xfe"
            );
        }
        assert_refused_and_preserved(&fixture);
        assert_refused_and_preserved(&fixture);
    }
}

// ---------------------------------------------------------------------------------------------
// P2-B: a backup loses restoration authority before its first payload byte is deleted

/// Removal stages a SIGKILL can leave behind, reproduced with the production transition.
fn removing_stage(fixture: &Fixture, stage: &str) {
    match fixture.prefix().invalidate_restoration_authority().unwrap() {
        BACKUP_REMOVING_MANIFEST | BACKUP_REMOVING_LEGACY => {}
        other => panic!("unexpected tombstone {other}"),
    }
    let backup = fixture.backup();
    match stage {
        "invalidated" => {}
        "partial" => fs::remove_dir_all(backup.join("drive_c")).unwrap(),
        "tombstone-only" => {
            for entry in fs::read_dir(&backup).unwrap().flatten() {
                let name = entry.file_name();
                if name == BACKUP_REMOVING_MANIFEST || name == BACKUP_REMOVING_LEGACY {
                    continue;
                }
                if entry.file_type().unwrap().is_dir() {
                    fs::remove_dir_all(entry.path()).unwrap();
                } else {
                    fs::remove_file(entry.path()).unwrap();
                }
            }
        }
        other => panic!("unknown stage {other}"),
    }
}

#[test]
fn removing_backup_is_never_restorable_and_needs_a_committed_root() {
    for legacy in [false, true] {
        for stage in ["invalidated", "partial", "tombstone-only"] {
            for root in ["committed", "manifest-lost", "manifest-corrupt", "absent"] {
                let fixture = Fixture::new("removing");
                if legacy {
                    make_legacy(&fixture.backup(), "OLD");
                } else {
                    make_committed(&fixture.backup(), &fixture.location, "OLD");
                }
                assert_eq!(
                    fixture.prefix().classify_backup().unwrap(),
                    BackupState::Restorable
                );
                removing_stage(&fixture, stage);
                let label = format!("legacy={legacy} {stage} {root}");
                assert_eq!(
                    fixture.prefix().classify_backup().unwrap(),
                    BackupState::Removing,
                    "{label}"
                );
                // Restoration evidence under its normal name is gone before any payload is.
                assert!(!exists(&fixture.backup().join(PREFIX_MARKER)), "{label}");
                if root != "absent" {
                    make_committed(&fixture.root(), &fixture.location, "NEW");
                }
                match root {
                    "manifest-lost" => fs::remove_file(fixture.root().join(PREFIX_MARKER)).unwrap(),
                    "manifest-corrupt" => {
                        fs::write(fixture.root().join(PREFIX_MARKER), "{").unwrap()
                    }
                    _ => {}
                }
                if root == "committed" {
                    assert_eq!(
                        fixture.recover_twice().action,
                        RecoveryAction::CompleteCommit {
                            stale_marker: false,
                            backup: true
                        },
                        "{label}"
                    );
                    assert_eq!(payload(&fixture.root()), "NEW");
                    assert!(!exists(&fixture.backup()), "{label}");
                } else {
                    // Damaged OLD is never restored: preserved, and the condition reported.
                    assert_refused_and_preserved(&fixture);
                    assert!(exists(&fixture.backup()), "{label}");
                }
            }
        }
    }
}

#[test]
fn removal_evidence_is_validated_before_cleanup() {
    type Build = fn(&Fixture);
    let builders: [(&str, Build); 4] = [
        ("two-tombstones", |f| {
            make_committed(&f.backup(), &f.location, "OLD");
            f.prefix().invalidate_restoration_authority().unwrap();
            fs::write(f.backup().join(BACKUP_REMOVING_LEGACY), "configured").unwrap();
        }),
        ("tombstone-symlink", |f| {
            create_structure(&f.backup());
            std::os::unix::fs::symlink("/etc/hostname", f.backup().join(BACKUP_REMOVING_MANIFEST))
                .unwrap();
        }),
        ("tombstone-other-server", |f| {
            create_structure(&f.backup());
            write_prefix_manifest(
                f.backup().to_str().unwrap(),
                &manifest_for(&f.location, Some("other"), "OLD"),
            )
            .unwrap();
            fs::rename(
                f.backup().join(PREFIX_MARKER),
                f.backup().join(BACKUP_REMOVING_MANIFEST),
            )
            .unwrap();
        }),
        ("tombstone-protected", |f| {
            make_committed(&f.backup(), &f.location, "OLD");
            f.prefix().invalidate_restoration_authority().unwrap();
            fs::write(f.backup().join(INSTALLATION_MARKER), "{}").unwrap();
        }),
    ];
    for (label, build) in builders {
        let fixture = Fixture::new("tombstone");
        build(&fixture);
        make_committed(&fixture.root(), &fixture.location, "NEW");
        assert!(
            matches!(
                fixture.prefix().classify_backup().unwrap(),
                BackupState::Foreign(_)
            ),
            "{label}"
        );
        assert_refused_and_preserved(&fixture);
    }
    // A protected backup is never invalidated, even if called directly.
    let fixture = Fixture::new("protected-invalidate");
    make_committed(&fixture.backup(), &fixture.location, "OLD");
    fs::write(fixture.backup().join(INSTALLATION_MARKER), "{}").unwrap();
    let before = snapshot(&fixture.dir);
    assert!(fixture.prefix().invalidate_restoration_authority().is_err());
    assert_eq!(snapshot(&fixture.dir), before);
}

/// Committed NEW plus a protected OLD backup: the backup is never deleted, and the condition is
/// reported rather than hidden.
#[test]
fn committed_root_never_deletes_a_protected_backup() {
    let fixture = Fixture::new("committed-protected-backup");
    make_committed(&fixture.backup(), &fixture.location, "OLD");
    fs::write(fixture.backup().join(INSTALLATION_MARKER), "{}").unwrap();
    make_committed_with_stale_marker(&fixture, "NEW");
    assert_refused_and_preserved(&fixture);
    assert_eq!(payload(&fixture.backup()), "OLD");
}

// ---------------------------------------------------------------------------------------------
// Real SIGKILL around every authority transition

fn refuse_in_new_process(home: &Home) {
    let before = snapshot(&home.dir);
    assert_eq!(
        home.run("recover"),
        ["refused true", "refused-again true"],
        "recovery must fail closed"
    );
    assert_eq!(snapshot(&home.dir), before, "a refusal modified data");
}

/// Checkpoints that only a destructive rollback (or its re-authorization) can reach.
const DESTRUCTIVE_CHECKPOINTS: [&str; 4] = [
    "rollback:authority-reinstated",
    "discard:payload-partial",
    "recovery:partial-removed",
    "recovery:backup-restored",
];

/// Run `scenario` armed at `checkpoint` with stdin closed: reaching the checkpoint aborts the
/// child, so a successful exit proves that the process never reached it.
fn run_never_reaching(home: &Home, scenario: &str, checkpoint: &str) -> Vec<String> {
    let output = home
        .command(scenario, Some(checkpoint))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success() && !stdout.contains(fault::REACHED),
        "{scenario} reached {checkpoint}\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    stdout
        .lines()
        .filter_map(|line| {
            line.split_once(RESULT)
                .map(|(_, rest)| rest.trim().to_string())
        })
        .collect()
}

fn assert_lost_commit_preserved(home: &Home, reset: bool) {
    assert_eq!(home.classify().0, RootState::CommitPending);
    assert!(!exists(&home.root().join(PROVISIONING_MARKER)));
    assert!(!exists(&home.root().join(PREFIX_MARKER)));
    assert_eq!(payload(&home.root()), "NEW");
    assert_eq!(
        fs::read(home.root().join("user-save.dat")).unwrap(),
        USER_SENTINEL
    );
    if reset {
        assert_eq!(home.classify().1, BackupState::Restorable);
        assert_eq!(payload(&home.backup()), "OLD");
        assert_eq!(
            fs::read(home.backup().join("save.bin")).unwrap(),
            b"\x01OLD-SAVE\xfe"
        );
    } else {
        assert!(!exists(&home.backup()));
    }
}

/// E/F: the post-rename loss of the manifest, then SIGKILL, fresh recovery, and further
/// interrupted recovery attempts. No process ever reactivates rollback or reaches a destructive
/// step; every attempt fails closed with identical bytes.
#[test]
fn sigkill_after_post_rename_manifest_loss_never_rolls_back() {
    for (scenario, reset) in [("reset-lost", true), ("initial-lost", false)] {
        // Killed after the loss, before the transaction's own error handling ran.
        let home = Home::new("kill-lost");
        if reset {
            home.make_old();
        }
        home.pause_at(scenario, "commit:manifest-lost").sigkill();
        assert_lost_commit_preserved(&home, reset);
        let before = snapshot_without_runtime(&home.dir);
        refuse_in_new_process(&home);
        for checkpoint in DESTRUCTIVE_CHECKPOINTS {
            assert_eq!(
                run_never_reaching(&home, "recover", checkpoint),
                ["refused true", "refused-again true"],
                "{scenario} {checkpoint}"
            );
        }
        refuse_in_new_process(&home);
        assert_eq!(snapshot_without_runtime(&home.dir), before, "{scenario}");
        assert_lost_commit_preserved(&home, reset);

        // The transaction's error handling runs to completion: it never reaches C -> P, then
        // the process dies and fresh recoveries (themselves interrupted) still refuse.
        for armed in DESTRUCTIVE_CHECKPOINTS {
            let home = Home::new("lost-handled");
            if reset {
                home.make_old();
            }
            assert_eq!(
                run_never_reaching(&home, scenario, armed),
                ["failed true"],
                "{scenario} {armed}"
            );
            assert_lost_commit_preserved(&home, reset);
            let before = snapshot_without_runtime(&home.dir);
            for checkpoint in DESTRUCTIVE_CHECKPOINTS {
                run_never_reaching(&home, "recover", checkpoint);
            }
            refuse_in_new_process(&home);
            assert_eq!(snapshot_without_runtime(&home.dir), before, "{scenario}");
            assert_lost_commit_preserved(&home, reset);
        }
    }
}

#[test]
fn sigkill_around_precommit_revocation() {
    use BackupState as B;
    use RootState as R;
    // (checkpoint, state after kill, Some(action) or None for fail-closed, new wins)
    let cases: [(&str, R, Option<&str>, bool); 4] = [
        // A: before revocation, P still authorizes rollback.
        (
            "commit:authority-revoking",
            R::Provisioning,
            Some("DiscardPartialAndRestore"),
            false,
        ),
        // B: after revocation, before the manifest: commit may have happened.
        ("commit:authority-revoked", R::CommitPending, None, false),
        // C: immediately before the manifest rename.
        ("commit:manifest-rename", R::CommitPending, None, false),
        // D: immediately after the manifest rename, before its directory fsync.
        (
            "commit:manifest-renamed",
            R::Committed { stale_marker: true },
            Some("CompleteCommit { stale_marker: true, backup: true }"),
            true,
        ),
    ];
    for (checkpoint, root, action, new_wins) in cases {
        let home = Home::new("kill-precommit");
        home.make_old();
        assert_eq!(
            kill_and_classify(&home, "reset", checkpoint),
            (root, B::Restorable),
            "{checkpoint}"
        );
        match action {
            Some(action) => {
                assert_eq!(recover_in_new_process(&home), action, "{checkpoint}");
                if new_wins {
                    assert_new_committed(&home);
                } else {
                    assert_old_restored(&home);
                }
                assert_eq!(recover_in_new_process(&home), "None", "{checkpoint}");
            }
            None => {
                refuse_in_new_process(&home);
                assert_eq!(payload(&home.root()), "NEW");
                assert_eq!(payload(&home.backup()), "OLD");
                assert_eq!(
                    fs::read(home.backup().join("save.bin")).unwrap(),
                    b"\x01OLD-SAVE\xfe"
                );
            }
        }
    }
    // Initial setup past the boundary: no backup, the partial root is still never discarded.
    let home = Home::new("kill-precommit-initial");
    assert_eq!(
        kill_and_classify(&home, "initial", "commit:authority-revoked"),
        (R::CommitPending, B::Absent)
    );
    refuse_in_new_process(&home);
    assert_eq!(payload(&home.root()), "NEW");
}

/// The independent P1 reproduction with a real SIGKILL and a fresh process: committed, killed
/// before marker cleanup, user data added, manifest lost or damaged.
#[test]
fn sigkill_after_commit_then_manifest_loss_never_deletes_the_root() {
    for scenario in ["initial", "reset"] {
        for how in ["removed", "corrupt", "directory"] {
            let home = Home::new("kill-p1");
            if scenario == "reset" {
                home.make_old();
            }
            let (root, _) = kill_and_classify(&home, scenario, "commit:manifest-durable");
            assert_eq!(root, RootState::Committed { stale_marker: true });
            assert!(exists(&home.root().join(COMMIT_PENDING_MARKER)));
            assert!(
                !exists(&home.root().join(PROVISIONING_MARKER)),
                "a committed root never holds rollback authority"
            );
            fs::write(home.root().join("user-save.dat"), USER_SENTINEL).unwrap();
            damage_manifest(&home.root(), how);
            refuse_in_new_process(&home);
            assert_eq!(
                fs::read(home.root().join("user-save.dat")).unwrap(),
                USER_SENTINEL,
                "{scenario} {how}"
            );
            assert_eq!(payload(&home.root()), "NEW");
            if scenario == "reset" {
                assert_eq!(payload(&home.backup()), "OLD");
            }
        }
    }
}

#[test]
fn sigkill_during_backup_cleanup_never_restores_damaged_old() {
    use BackupState as B;
    let committed = RootState::Committed {
        stale_marker: false,
    };
    let cases: [(&str, B); 5] = [
        ("cleanup:authority-revoking", B::Restorable),
        ("cleanup:authority-revoked", B::Removing),
        ("cleanup:payload-partial", B::Removing),
        ("cleanup:tombstone-only", B::Removing),
        ("cleanup:tombstone-removed", B::Empty),
    ];
    for (checkpoint, backup) in cases {
        let home = Home::new("kill-cleanup");
        home.make_old();
        assert_eq!(
            kill_and_classify(&home, "reset", checkpoint),
            (committed.clone(), backup.clone()),
            "{checkpoint}"
        );
        if backup == B::Removing {
            assert!(!exists(&home.backup().join(PREFIX_MARKER)), "{checkpoint}");
            // The NEW commit record is lost or damaged: the damaged OLD is never restored.
            let manifest = home.root().join(PREFIX_MARKER);
            let moved = home.dir.join("moved-manifest.json");
            fs::rename(&manifest, &moved).unwrap();
            refuse_in_new_process(&home);
            fs::write(&manifest, "{").unwrap();
            refuse_in_new_process(&home);
            fs::remove_file(&manifest).unwrap();
            fs::rename(&moved, &manifest).unwrap();
        }
        let action = recover_in_new_process(&home);
        match backup {
            B::Empty => assert_eq!(action, "None", "{checkpoint}"),
            _ => assert_eq!(
                action, "CompleteCommit { stale_marker: false, backup: true }",
                "{checkpoint}"
            ),
        }
        assert_new_committed(&home);
    }
}

/// Second crashes inside recovery itself, after each new boundary.
#[test]
fn double_crash_schedules_converge_without_losing_authority() {
    // Before revocation -> recovery killed mid-discard -> recovery converges to OLD.
    let home = Home::new("double-precommit");
    home.make_old();
    home.pause_at("reset", "commit:authority-revoking")
        .sigkill();
    home.pause_at("recover", "discard:payload-partial")
        .sigkill();
    assert_eq!(
        home.classify(),
        (RootState::Provisioning, BackupState::Restorable)
    );
    home.pause_at("recover", "discard:marker-only").sigkill();
    assert_eq!(
        home.classify(),
        (RootState::Provisioning, BackupState::Restorable)
    );
    assert_eq!(recover_in_new_process(&home), "DiscardPartialAndRestore");
    assert_old_restored(&home);

    // After revocation: no recovery step is destructive, so every attempt refuses identically.
    let home = Home::new("double-revoked");
    home.make_old();
    home.pause_at("reset", "commit:authority-revoked").sigkill();
    refuse_in_new_process(&home);
    refuse_in_new_process(&home);

    // Removing backup: killed after invalidation, then twice more during its cleanup.
    let home = Home::new("double-removing");
    home.make_old();
    home.pause_at("reset", "cleanup:authority-revoked")
        .sigkill();
    home.pause_at("recover", "cleanup:payload-partial")
        .sigkill();
    assert_eq!(
        home.classify().1,
        BackupState::Removing,
        "partially deleted OLD keeps no restoration authority"
    );
    home.pause_at("recover", "cleanup:tombstone-only").sigkill();
    assert_eq!(home.classify().1, BackupState::Removing);
    assert_eq!(
        recover_in_new_process(&home),
        "CompleteCommit { stale_marker: false, backup: true }"
    );
    assert_new_committed(&home);
}

// ---------------------------------------------------------------------------------------------
// P2-A: full v3 identity, never the 24-hex path suffix

/// Distinct full digests sharing the 24-hex v3 path suffix (same pair as the identity tests).
const DIGEST_A: &str = "a23f2a940a9cb8e5110b0e454ad68a35a22af760c005bcd0ebdf47ab44330270";
const DIGEST_B: &str = "a23f2a940a9cb8e5110b0e45ffffffffffffffffffffffffffffffffffffffff";

fn collision_scenario() {
    assert!(crate::utils::digests_share_v3_path_suffix(
        DIGEST_A, DIGEST_B
    ));
    assert_ne!(DIGEST_A, DIGEST_B);
    let path = crate::utils::isolated_prefix_path_v3("srv", DIGEST_A);
    assert_eq!(path, crate::utils::isolated_prefix_path_v3("srv", DIGEST_B));
    fs::create_dir_all(crate::utils::isolated_prefix_root()).unwrap();
    let location = PrefixLocation {
        path: path.clone(),
        scope: PrefixScope::Isolated,
        managed: true,
        server_id: Some("srv".into()),
    };
    let root = PathBuf::from(&path);
    let backup = reset_backup_path(&root).unwrap();
    let clear = || {
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&backup);
    };
    let committed = |dir: &Path, digest: &str, tag: &str| {
        create_structure(dir);
        fs::write(dir.join("payload.txt"), tag).unwrap();
        write_v3_manifest(dir.to_str().unwrap(), &location, digest, tag).unwrap();
    };
    let prefix = |expected: Option<&str>| ManagedPrefix::new(&location, expected).unwrap();
    let refused = |expected: Option<&str>| {
        let before = snapshot(root.parent().unwrap());
        let refused = prefix(expected)
            .recover()
            .is_err_and(|error| error.contains("no puede resolverse con seguridad"));
        refused && snapshot(root.parent().unwrap()) == before
    };

    // Current expected A, root committed for B: never Committed for A.
    committed(&root, DIGEST_B, "B");
    child_print(format!(
        "root-b-for-a {}",
        matches!(
            prefix(Some(DIGEST_A)).classify_root().unwrap(),
            RootState::Foreign(_)
        )
    ));
    child_print(format!(
        "root-b-for-b {}",
        prefix(Some(DIGEST_B)).classify_root().unwrap()
            == RootState::Committed {
                stale_marker: false
            }
    ));
    clear();

    // Expected A, backup manifest B: never Restorable for A, never restored.
    committed(&backup, DIGEST_B, "B");
    child_print(format!(
        "backup-b-for-a {} {}",
        matches!(
            prefix(Some(DIGEST_A)).classify_backup().unwrap(),
            BackupState::Foreign(_)
        ),
        refused(Some(DIGEST_A))
    ));
    clear();

    // Committed A root + B backup: B is never deleted as A's backup, in any form.
    for tombstoned in [false, true] {
        committed(&root, DIGEST_A, "A");
        committed(&backup, DIGEST_B, "B");
        if tombstoned {
            fs::rename(
                backup.join(PREFIX_MARKER),
                backup.join(BACKUP_REMOVING_MANIFEST),
            )
            .unwrap();
        }
        child_print(format!(
            "committed-a-backup-b {tombstoned} {} {} {}",
            matches!(
                prefix(None).classify_backup().unwrap(),
                BackupState::Foreign(_)
            ),
            refused(None),
            refused(Some(DIGEST_A))
        ));
        assert_eq!(payload(&backup), "B");
        clear();
    }

    // Transaction marker A + backup B, and marker A + manifest B: fail closed.
    committed(&backup, DIGEST_B, "B");
    prefix(Some(DIGEST_A)).begin_provisioning(DIGEST_A).unwrap();
    child_print(format!("marker-a-backup-b {}", refused(None)));
    clear();
    let pending = prefix(Some(DIGEST_A));
    pending.begin_provisioning(DIGEST_A).unwrap();
    create_structure(&root);
    pending.revoke_rollback_authority().unwrap();
    write_v3_manifest(&path, &location, DIGEST_B, "B").unwrap();
    child_print(format!(
        "marker-a-manifest-b {} {}",
        matches!(prefix(None).classify_root().unwrap(), RootState::Foreign(_)),
        refused(None)
    ));
    clear();
    // A transaction for B can never even start at a path claimed for A by the caller... and a
    // transaction whose identity does not address the path is refused outright.
    child_print(format!(
        "begin-wrong-path {}",
        prefix(None)
            .begin_provisioning(&"f".repeat(64))
            .is_err_and(|error| error.contains("no corresponde a la ruta v3"))
    ));
    clear();

    // Legitimate same-full-digest behavior is preserved.
    committed(&root, DIGEST_A, "A-NEW");
    committed(&backup, DIGEST_A, "A-OLD");
    let outcome = prefix(Some(DIGEST_A)).recover().unwrap();
    child_print(format!(
        "same-digest-cleanup {:?} {}",
        outcome.action,
        !exists(&backup)
    ));
    clear();
    committed(&backup, DIGEST_A, "A-OLD");
    let outcome = prefix(Some(DIGEST_A)).recover().unwrap();
    child_print(format!(
        "same-digest-restore {:?} {}",
        outcome.action,
        payload(&root)
    ));
    clear();
    // A backup found alone, with no current claim for this path, is judged on its own evidence.
    committed(&backup, DIGEST_B, "B");
    child_print(format!(
        "own-evidence {:?}",
        prefix(None).classify_backup().unwrap()
    ));
    clear();
}

#[test]
fn v3_recovery_requires_the_exact_full_fingerprint() {
    let home = Home::new("collision");
    assert_eq!(
        home.run("collision"),
        [
            "root-b-for-a true",
            "root-b-for-b true",
            "backup-b-for-a true true",
            "committed-a-backup-b false true true true",
            "committed-a-backup-b true true true true",
            "marker-a-backup-b true",
            "marker-a-manifest-b true true",
            "begin-wrong-path true",
            "same-digest-cleanup CompleteCommit { stale_marker: false, backup: true } true",
            "same-digest-restore RestoreBackup A-OLD",
            "own-evidence Restorable",
        ]
    );
}

// ---------------------------------------------------------------------------------------------
// P2-C: a changed desired fingerprint never bypasses a pending v3 transaction

const FP_STATE: &str = "fp-state.txt";
const UPDATED_WINE: &[u8] = b"#!/bin/sh\n# runner updated in place\nexit 0\n";

fn fp_server() -> crate::models::server::ServerConfig {
    serde_json::from_value(serde_json::json!({
        "id": "srv", "name": "srv", "executablePath": "/unused/game.exe", "prefixMode": "isolated"
    }))
    .unwrap()
}

fn fp_resolve(
    ctx: &crate::utils::WineContext,
) -> Result<crate::tools::runtime::PrefixBinding, String> {
    crate::tools::runtime::resolve_prefix_binding(
        Some(&fp_server()),
        &ctx.resolved,
        &crate::tools::runtime::probe_runner(&ctx.resolved),
        false,
    )
}

fn fp_context(
    base: &crate::utils::WineContext,
    binding: crate::tools::runtime::PrefixBinding,
) -> crate::utils::WineContext {
    crate::utils::WineContext {
        prefix: binding.location.path.clone(),
        location: binding.location.clone(),
        resolved: base.resolved.clone(),
        identity: binding,
        probe: base.probe.clone(),
    }
}

fn shared_location() -> PrefixLocation {
    PrefixLocation {
        path: crate::utils::prefix_path(),
        scope: PrefixScope::Shared,
        managed: true,
        server_id: None,
    }
}

/// A valid v2 sibling for the same server and runner path: the candidate the defect selected.
fn make_v2_sibling(ctx: &crate::utils::WineContext) -> String {
    let runner = fs::canonicalize(ctx.resolved.runner_path()).unwrap();
    let v2 = crate::utils::isolated_prefix_path_for_runner("srv", runner.to_str().unwrap());
    create_structure(Path::new(&v2));
    fs::write(Path::new(&v2).join("payload.txt"), "V2").unwrap();
    write_prefix_manifest(
        &v2,
        &PrefixManifest {
            schema_version: PREFIX_SCHEMA_VERSION,
            scope: PrefixScope::Isolated,
            server_id: Some("srv".into()),
            runner_kind: ctx.resolved.kind_label().into(),
            runner_path: runner.to_string_lossy().into_owned(),
            components: vec![],
        },
    )
    .unwrap();
    v2
}

/// Committed v3 prefix for fingerprint A plus a valid v2 sibling, then a real reset of A that the
/// parent kills at a checkpoint.
async fn fingerprint_reset_scenario() {
    let ctx = inert_runner_context(&shared_location());
    let fresh = fp_resolve(&ctx).unwrap();
    let digest = fresh.desired_fingerprint.digest.hex_digest();
    let v3 = fresh.location.path.clone();
    assert_eq!(v3, crate::utils::isolated_prefix_path_v3("srv", &digest));
    create_structure(Path::new(&v3));
    fs::write(Path::new(&v3).join("payload.txt"), "OLD").unwrap();
    fs::write(Path::new(&v3).join("save.bin"), b"\x01OLD-SAVE\xfe").unwrap();
    write_v3_manifest(&v3, &fresh.location, &digest, "OLD").unwrap();
    let v2 = make_v2_sibling(&ctx);
    fs::write(
        PathBuf::from(std::env::var("HOME").unwrap()).join(FP_STATE),
        format!("{digest}\n{v3}\n{v2}\n"),
    )
    .unwrap();
    let verified = fp_resolve(&ctx).unwrap();
    assert_eq!(
        (verified.status, verified.location.path.as_str()),
        (
            crate::tools::runtime::PrefixIdentityStatus::V3Verified,
            v3.as_str()
        )
    );
    let _guard = crate::utils::OperationGuard::acquire("prefix", Path::new(&v3)).unwrap();
    let prefix = ManagedPrefix::new(&verified.location, Some(&digest)).unwrap();
    let mut work = FileWork::new(&verified.location, "NEW");
    work.v3_digest = Some(digest.clone());
    run_managed_transaction(&prefix, TransactionKind::Reset, &digest, &mut work)
        .await
        .unwrap();
    child_print("unexpected-commit");
}

/// The runner is updated in place (fingerprint A -> B) after the crash. The production gate
/// (resolution, then `RunnerOperation::begin` with its bootstrap recovery) must surface and
/// reconcile A before any fresh resolution may pick the v2 sibling.
async fn fingerprint_resolve_scenario() {
    {
        use crate::tools::runner_sessions::{RunnerOperation, RunnerSessionRegistry};
        let state =
            fs::read_to_string(PathBuf::from(std::env::var("HOME").unwrap()).join(FP_STATE))
                .unwrap();
        let mut lines = state.lines();
        let (digest_a, v3_a, v2) = (
            lines.next().unwrap().to_string(),
            lines.next().unwrap().to_string(),
            lines.next().unwrap().to_string(),
        );
        let base = inert_runner_context_with(&shared_location(), UPDATED_WINE);
        let steered = fp_resolve(&base).unwrap();
        let digest_b = steered.desired_fingerprint.digest.hex_digest();
        child_print(format!("fingerprint-changed {}", digest_b != digest_a));
        child_print(format!(
            "steered {:?} {}",
            steered.status,
            steered.location.path == v3_a
        ));
        let sidecar = crate::tools::runner_sessions::workspace_debug_sessiond()
            .expect("build ro-sessiond first (cargo build -p ro-sessiond)");
        let sessions = RunnerSessionRegistry::with_sidecar_for_test(sidecar);
        let game = crate::state::GameProcessHandle::new();
        let ctx = fp_context(&base, steered);
        let anchor = crate::tools::runtime::session_anchor_from_context(&ctx);
        let error = RunnerOperation::begin(None, &sessions, &game, &ctx, &anchor)
            .await
            .err()
            .expect("the pending transaction must stop the operation");
        child_print(format!("bootstrap {}", error == RECOVERED_RETRY_MSG));
        child_print(format!(
            "restored {} {}",
            payload(Path::new(&v3_a)),
            !exists(&reset_backup_path(Path::new(&v3_a)).unwrap())
        ));
        // Only now does a clean resolution choose B's correct candidate, never reinterpreting A.
        let after = fp_resolve(&base).unwrap();
        child_print(format!(
            "after {:?} {}",
            after.status,
            after.location.path == v2
        ));
        let ctx = fp_context(&base, after);
        let anchor = crate::tools::runtime::session_anchor_from_context(&ctx);
        let op = RunnerOperation::begin(None, &sessions, &game, &ctx, &anchor)
            .await
            .unwrap();
        op.quiesce().await.unwrap();
        child_print("v2-start ok");
        assert!(sessions.shutdown_all().await.is_empty());
    }
}

#[test]
fn changed_fingerprint_never_bypasses_a_pending_v3_transaction() {
    if crate::tools::runner_sessions::workspace_debug_sessiond().is_none() {
        panic!("build ro-sessiond first (cargo build -p ro-sessiond)");
    }
    for checkpoint in ["reset:backup-preserved", "provision:partial"] {
        let home = Home::new("fingerprint");
        home.pause_at("fp-reset", checkpoint).sigkill();
        assert_eq!(
            home.run("fp-resolve"),
            [
                "fingerprint-changed true",
                "steered Unknown true",
                "bootstrap true",
                "restored OLD true",
                "after LegacyV2RunnerMatched true",
                "v2-start ok",
            ],
            "{checkpoint}"
        );
    }
    // Double crash: the pending A transaction, a recovery killed between discard and restore,
    // then the changed-fingerprint resolution in yet another process.
    let home = Home::new("fingerprint-double");
    home.pause_at("fp-reset", "provision:partial").sigkill();
    home.pause_at("fp-resolve", "recovery:partial-removed")
        .sigkill();
    assert_eq!(
        home.run("fp-resolve"),
        [
            "fingerprint-changed true",
            "steered Unknown true",
            "bootstrap true",
            "restored OLD true",
            "after LegacyV2RunnerMatched true",
            "v2-start ok",
        ]
    );
}

/// Ways an invalid manifest can sit next to the fixed evidence of pending transaction A. None of
/// them is a commit record for A.
const INVALID_COMMIT_RECORDS: [&str; 6] = [
    "corrupt",
    "directory",
    "symlink",
    "unreadable",
    "foreign-server",
    "digest-conflict",
];

/// Raw bytes of a syntactically valid v3 manifest. Written directly, because the real writer
/// (correctly) refuses a root that still holds `P`.
fn raw_v3_manifest(server: &str, digest: &str) -> Vec<u8> {
    serde_json::to_vec_pretty(&crate::utils::PrefixManifestV3 {
        schema_version: crate::utils::PREFIX_SCHEMA_V3,
        scope: PrefixScope::Isolated,
        server_id: Some(server.into()),
        runner_kind: "wine".into(),
        runner_path: "/opt/fake/bin/wine".into(),
        components: vec![],
        prefix_fingerprint: crate::utils::PrefixFingerprintEnvelope {
            schema_version: 1,
            algorithm: "sha256".into(),
            digest: digest.to_string(),
        },
    })
    .unwrap()
}

/// Same 24-hex v3 path suffix as `digest`, different full identity.
fn conflicting_digest(digest: &str) -> String {
    let last = if digest.ends_with('0') { '1' } else { '0' };
    let conflict = format!("{}{last}", &digest[..63]);
    assert_ne!(conflict, digest);
    assert!(crate::utils::digests_share_v3_path_suffix(
        digest, &conflict
    ));
    conflict
}

fn plant_invalid_manifest(root: &Path, how: &str, digest: &str) {
    let manifest = root.join(PREFIX_MARKER);
    match how {
        "corrupt" => fs::write(&manifest, b"{\"schemaVersion\":3,").unwrap(),
        "directory" => fs::create_dir(&manifest).unwrap(),
        "symlink" => {
            let elsewhere = root.parent().unwrap().join("elsewhere-manifest.json");
            fs::write(&elsewhere, raw_v3_manifest("srv", digest)).unwrap();
            std::os::unix::fs::symlink(&elsewhere, &manifest).unwrap();
        }
        "unreadable" => {
            fs::write(&manifest, raw_v3_manifest("srv", digest)).unwrap();
            fs::set_permissions(&manifest, fs::Permissions::from_mode(0o000)).unwrap();
        }
        "foreign-server" => fs::write(&manifest, raw_v3_manifest("other", digest)).unwrap(),
        "digest-conflict" => fs::write(
            &manifest,
            raw_v3_manifest("srv", &conflicting_digest(digest)),
        )
        .unwrap(),
        other => panic!("unknown manifest {other}"),
    }
}

/// Pending transaction A plus an invalid manifest, inert runner updated in place (A -> B), and a
/// valid v2 sibling. Resolution must stay on A and the production gate must refuse; nothing may
/// start on the v2 sibling and no byte under the isolated root may change.
async fn fingerprint_blocked_scenario() {
    use crate::tools::runner_sessions::{RunnerOperation, RunnerSessionRegistry};
    let state =
        fs::read_to_string(PathBuf::from(std::env::var("HOME").unwrap()).join(FP_STATE)).unwrap();
    let mut lines = state.lines();
    let (digest_a, v3_a, v2) = (
        lines.next().unwrap().to_string(),
        lines.next().unwrap().to_string(),
        lines.next().unwrap().to_string(),
    );
    let isolated = crate::utils::isolated_prefix_root();
    let before = snapshot(&isolated);
    let base = inert_runner_context_with(&shared_location(), UPDATED_WINE);
    let steered = fp_resolve(&base).unwrap();
    child_print(format!(
        "fingerprint-changed {}",
        steered.desired_fingerprint.digest.hex_digest() != digest_a
    ));
    child_print(format!(
        "steered {:?} {} {}",
        steered.status,
        steered.location.path == v3_a,
        steered.location.path != v2
    ));
    let sidecar = crate::tools::runner_sessions::workspace_debug_sessiond()
        .expect("build ro-sessiond first (cargo build -p ro-sessiond)");
    let sessions = RunnerSessionRegistry::with_sidecar_for_test(sidecar);
    let game = crate::state::GameProcessHandle::new();
    let ctx = fp_context(&base, steered);
    let anchor = crate::tools::runtime::session_anchor_from_context(&ctx);
    let error = RunnerOperation::begin(None, &sessions, &game, &ctx, &anchor)
        .await
        .err()
        .expect("an unresolved transaction must stop the operation");
    // Recovery's ambiguity refusal, or (for a symlinked manifest) the managed-path safety check
    // that runs first on the same steered path. Either way nothing starts and nothing changes.
    child_print(format!(
        "bootstrap-refused {}",
        error.contains("no puede resolverse con seguridad")
            || error == "El manifiesto del entorno no puede ser un symlink"
    ));
    let again = fp_resolve(&base).unwrap();
    child_print(format!("still-steered {}", again.location.path == v3_a));
    child_print(format!("untouched {}", snapshot(&isolated) == before));
    assert!(sessions.shutdown_all().await.is_empty());
}

/// P2 regression: a corrupt, nonregular or foreign manifest next to A's fixed evidence never
/// hides A from discovery, so a changed fingerprint cannot silently select the v2 sibling.
#[test]
fn invalid_manifest_never_hides_a_pending_transaction_from_resolution() {
    if crate::tools::runner_sessions::workspace_debug_sessiond().is_none() {
        panic!("build ro-sessiond first (cargo build -p ro-sessiond)");
    }
    for checkpoint in ["provision:partial", "commit:authority-revoked"] {
        for how in INVALID_COMMIT_RECORDS {
            let home = Home::new("fp-invalid-m");
            home.pause_at("fp-reset", checkpoint).sigkill();
            let state = fs::read_to_string(home.dir.join(FP_STATE)).unwrap();
            let mut lines = state.lines();
            let (digest_a, v3_a) = (lines.next().unwrap(), PathBuf::from(lines.next().unwrap()));
            plant_invalid_manifest(&v3_a, how, digest_a);
            assert_eq!(
                home.run("fp-resolve-blocked"),
                [
                    "fingerprint-changed true",
                    "steered Unknown true true",
                    "bootstrap-refused true",
                    "still-steered true",
                    "untouched true",
                ],
                "{checkpoint} {how}"
            );
            if how == "unreadable" {
                fs::set_permissions(v3_a.join(PREFIX_MARKER), fs::Permissions::from_mode(0o600))
                    .unwrap();
            }
            let backup = reset_backup_path(&v3_a).unwrap();
            assert_eq!(payload(&backup), "OLD", "{checkpoint} {how}");
            assert_eq!(
                fs::read(backup.join("save.bin")).unwrap(),
                b"\x01OLD-SAVE\xfe",
                "{checkpoint} {how}"
            );
            assert_eq!(payload(&v3_a), "NEW", "{checkpoint} {how}");
        }
    }
}

fn multiple_pending_scenario() {
    let ctx = inert_runner_context(&shared_location());
    let digest = fp_resolve(&ctx)
        .unwrap()
        .desired_fingerprint
        .digest
        .hex_digest();
    let v3 = crate::utils::isolated_prefix_path_v3("srv", &digest);
    let v2 = make_v2_sibling(&ctx);
    let data_root = PathBuf::from(std::env::var("HOME").unwrap()).join(".local/share/ro-launcher");
    let unchanged = |label: &str| {
        let before = snapshot(&data_root);
        let result = fp_resolve(&ctx);
        child_print(format!(
            "{label} {} {}",
            result.is_err_and(|error| error.contains("no se elegirá ninguna")),
            snapshot(&data_root) == before
        ));
    };
    // Pending #1: A reset interrupted after moving the old root aside.
    let v3_backup = reset_backup_path(Path::new(&v3)).unwrap();
    create_structure(&v3_backup);
    write_v3_manifest(
        v3_backup.to_str().unwrap(),
        &PrefixLocation {
            path: v3.clone(),
            scope: PrefixScope::Isolated,
            managed: true,
            server_id: Some("srv".into()),
        },
        &digest,
        "A",
    )
    .unwrap();
    // Names outside the managed contract, and other servers' transactions, are never counted.
    let isolated = crate::utils::isolated_prefix_root();
    fs::create_dir_all(isolated.join(".not-a-prefix-v3-zz.reset-backup")).unwrap();
    fs::write(isolated.join(".not-a-prefix-v3-zz.reset-backup/x"), "x").unwrap();
    let other_digest = "0".repeat(64);
    let other = crate::utils::isolated_prefix_path_v3("other", &other_digest);
    let other_backup = reset_backup_path(Path::new(&other)).unwrap();
    create_structure(&other_backup);
    let resolved = fp_resolve(&ctx).unwrap();
    child_print(format!("scoped {}", resolved.location.path == v3));
    // Pending #2 under another fingerprint of the same server: never choose between them.
    // A guaranteed different first hex digit, so the second path is never the first one.
    let first = if digest.starts_with('d') { 'e' } else { 'd' };
    let digest_d = format!("{first}{}", &digest[1..]);
    assert_ne!(digest, digest_d);
    assert_ne!(crate::utils::isolated_prefix_path_v3("srv", &digest_d), v3);
    let v3_d = crate::utils::isolated_prefix_path_v3("srv", &digest_d);
    let location_d = PrefixLocation {
        path: v3_d.clone(),
        scope: PrefixScope::Isolated,
        managed: true,
        server_id: Some("srv".into()),
    };
    ManagedPrefix::new(&location_d, Some(&digest_d))
        .unwrap()
        .begin_provisioning(&digest_d)
        .unwrap();
    unchanged("two-v3");
    // A pending v2 sibling plus a pending v3 transaction: also never chosen between.
    fs::remove_dir_all(&v3_d).unwrap();
    fs::rename(&v2, reset_backup_path(Path::new(&v2)).unwrap()).unwrap();
    unchanged("v2-and-v3");
}

#[test]
fn multiple_pending_transactions_for_one_server_fail_closed() {
    let home = Home::new("multi-pending");
    assert_eq!(
        home.run("multi-pending"),
        ["scoped true", "two-v3 true true", "v2-and-v3 true true"]
    );
}
