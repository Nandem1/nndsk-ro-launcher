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

use crate::utils::{write_prefix_manifest, PrefixManifest, PREFIX_SCHEMA_VERSION};

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
                out.insert(rel, format!("file:{:?}", fs::read(&path).unwrap()));
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
    make_provisioning(&fixture, "NEW");
    create_structure(&fixture.root());
    write_prefix_manifest(
        &fixture.location.path,
        &manifest_for(&fixture.location, Some("srv"), "NEW"),
    )
    .unwrap();
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
    make_provisioning(&fixture, "NEW");
    create_structure(&fixture.root());
    write_prefix_manifest(
        &fixture.location.path,
        &manifest_for(&fixture.location, Some("srv"), "NEW"),
    )
    .unwrap();
    assert_eq!(
        fixture.recover_twice().action,
        RecoveryAction::CompleteCommit {
            stale_marker: true,
            backup: false
        }
    );
    assert_eq!(payload(&fixture.root()), "NEW");

    // Legacy backup whose structure was already partly removed: only its marker survives.
    let fixture = Fixture::new("cleanup-removing");
    make_legacy(&fixture.backup(), "OLD");
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

    // Root missing while the backup is only a removal remnant.
    let fixture = Fixture::new("amb-removing");
    make_legacy(&fixture.backup(), "OLD");
    fs::remove_dir_all(fixture.backup().join("drive_c")).unwrap();
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
    assert_eq!(fixture.recover().unwrap().action, RecoveryAction::None);
    make_committed(&fixture.backup(), &fixture.location, "OLD");
    assert_refused_and_preserved(&fixture);
}

#[test]
fn protected_installation_in_partial_root_is_never_discarded() {
    let fixture = Fixture::new("protected-root");
    make_provisioning(&fixture, "PARTIAL");
    fs::write(fixture.root().join(INSTALLATION_MARKER), "{}").unwrap();
    let before = snapshot(&fixture.dir);
    assert!(matches!(
        fixture.prefix().classify_root().unwrap(),
        RootState::Foreign(_)
    ));
    assert_eq!(fixture.recover().unwrap().action, RecoveryAction::None);
    assert_eq!(snapshot(&fixture.dir), before);

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
    assert!(ManagedPrefix::new(&custom).is_err());
    assert_eq!(
        super::super::recover_interrupted_prefix(&custom).unwrap(),
        RecoveryOutcome::untouched()
    );
    assert_eq!(
        super::super::recover_with_guard_held(&custom).unwrap(),
        RecoveryOutcome::untouched()
    );
    assert_eq!(
        super::super::recover_residue_with_guard_held(&custom).unwrap(),
        RecoveryOutcome::untouched()
    );
    let unmanaged_isolated = PrefixLocation {
        managed: false,
        ..fixture.location.clone()
    };
    assert!(ManagedPrefix::new(&unmanaged_isolated).is_err());
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
    assert!(transaction_residue_present(&root));
    assert!(uncommitted_transaction_present(&root));
    make_provisioning(&fixture, "NEW");
    assert!(uncommitted_transaction_present(&root));
    write_prefix_manifest(
        &fixture.location.path,
        &manifest_for(&fixture.location, Some("srv"), "NEW"),
    )
    .unwrap();
    assert!(transaction_residue_present(&root));
    assert!(
        !uncommitted_transaction_present(&root),
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
    assert!(
        !uncommitted_transaction_present(&fixture.root()),
        "old-build residue never steers binding resolution"
    );
    make_committed(&fixture.root(), &fixture.location, "CUR");
    assert!(!transaction_residue_present(&fixture.root()));
}

// ---------------------------------------------------------------------------------------------
// Transaction orchestration with an injected provisioning engine (no Wine)

#[derive(Default)]
struct FileWork {
    location: Option<PrefixLocation>,
    tag: &'static str,
    fail_provision: bool,
    fail_commit: bool,
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

    fn commit(&mut self) -> Result<(), String> {
        if self.fail_commit {
            return Err("disco lleno".into());
        }
        let location = self.location.as_ref().unwrap();
        write_prefix_manifest(
            &location.path,
            &manifest_for(location, location.server_id.as_deref(), self.tag),
        )
    }
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
        work.fail_commit = failure == "commit";
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
                super::super::recover_with_guard_held(&location)
                    .unwrap()
                    .action,
                RecoveryAction::None
            );
            let prefix = ManagedPrefix::new(&location).unwrap();
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
        "recover" => {
            let guard = crate::utils::OperationGuard::acquire("prefix", Path::new(&location.path));
            let Ok(_guard) = guard else {
                child_print("busy");
                return;
            };
            let first = super::super::recover_with_guard_held(&location).unwrap();
            child_print(format!("{:?}", first.action));
            let second = super::super::recover_with_guard_held(&location).unwrap();
            assert_eq!(
                second.action,
                RecoveryAction::None,
                "recovery is not idempotent"
            );
            child_print("idempotent");
        }
        "supervised-reset" | "supervised-recover" => supervised_scenario(scenario, &location).await,
        "binding" => binding_scenario(),
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
            "recovery:evidence-only",
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
        // Old backup reduced to its manifest while being removed after commit.
        (
            "recovery:evidence-only",
            "commit:manifest-durable",
            RootState::Committed {
                stale_marker: false,
            },
            BackupState::Restorable,
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
}

impl ProvisioningWork for SupervisedWork<'_> {
    async fn provision(&mut self) -> Result<(), String> {
        let op = self.op.as_ref().unwrap();
        child_print(format!(
            "supervisor {}",
            op.lease().supervisor_identity().pid
        ));
        // Representative writes performed by a supervised child process, not by the test.
        let script = "mkdir -p \"$WINEPREFIX/drive_c/windows/system32\" \"$WINEPREFIX/dosdevices\" && \
                      ln -s ../drive_c \"$WINEPREFIX/dosdevices/c:\" && \
                      echo reg > \"$WINEPREFIX/system.reg\" && echo reg > \"$WINEPREFIX/user.reg\" && \
                      printf NEW > \"$WINEPREFIX/payload.txt\"";
        let code = op
            .run(
                sh_invocation(&self.ctx.prefix, script),
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

    fn commit(&mut self) -> Result<(), String> {
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
    let runner_dir = PathBuf::from(std::env::var("HOME").unwrap()).join("fake-runner/bin");
    fs::create_dir_all(&runner_dir).unwrap();
    for name in ["wine", "wineserver"] {
        let path = runner_dir.join(name);
        fs::write(&path, b"#!/bin/sh\nexit 0\n").unwrap();
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
    if scenario == "supervised-reset" {
        // Production order: session first (bootstrap recovery gate), then the exclusive guard.
        let op = RunnerOperation::begin(None, &sessions, &game, &ctx, &anchor)
            .await
            .unwrap();
        let _guard =
            crate::utils::OperationGuard::acquire("prefix", Path::new(&ctx.prefix)).unwrap();
        let prefix = ManagedPrefix::new(location).unwrap();
        let mut work = SupervisedWork {
            ctx: &ctx,
            op: Some(op),
            cleanup: None,
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

#[test]
fn sigkill_during_real_supervised_provisioning_recovers_old_prefix() {
    if crate::tools::runner_sessions::workspace_debug_sessiond().is_none() {
        panic!("build ro-sessiond first (cargo build -p ro-sessiond)");
    }
    let home = Home::new("supervised");
    home.make_old();
    let seen = home
        .pause_at("supervised-reset", "provision:partial")
        .sigkill();
    let supervisor: u32 = seen
        .iter()
        .find_map(|line| line.split_once(&format!("{RESULT} supervisor ")))
        .map(|(_, pid)| pid.trim().parse().unwrap())
        .expect("child reported its supervisor");
    wait_for_exit(supervisor);
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
    let outcome = super::super::recover_interrupted_prefix(&interrupted.location).unwrap();
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
    let error = super::super::recover_interrupted_prefix(&v3_location).unwrap_err();
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
    let error = super::super::recover_interrupted_prefix(location).unwrap_err();
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
    let error = super::super::recover_interrupted_prefix(&isolated).unwrap_err();
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
