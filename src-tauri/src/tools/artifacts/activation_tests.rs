use super::*;
use crate::tools::artifacts::{descriptor::*, receipt::*, unique_suffix};
use crate::utils::{replace_json, OperationGuard};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Barrier};
use std::time::Duration;

enum HookAction {
    Pause,
    Fail,
    Race(PathBuf),
    Barrier(Arc<Barrier>),
    Rendezvous(
        tokio::sync::oneshot::Sender<()>,
        std::sync::mpsc::Receiver<()>,
    ),
}
struct Hook {
    name: String,
    hits: usize,
    action: HookAction,
}
thread_local! { static HOOK: RefCell<Option<Hook>> = const { RefCell::new(None) }; }

fn hook(name: &str, hits: usize, action: HookAction) {
    HOOK.set(Some(Hook {
        name: name.into(),
        hits,
        action,
    }));
}

pub(super) fn checkpoint(name: &str) -> Result<(), String> {
    let selected = HOOK.with(|cell| {
        let mut hook = cell.borrow_mut();
        let selected = hook.as_mut()?;
        if selected.name != name {
            return None;
        }
        selected.hits -= 1;
        if selected.hits != 0 {
            return None;
        }
        hook.take()
    });
    let Some(selected) = selected else {
        return Ok(());
    };
    match selected.action {
        HookAction::Pause => {
            println!("F06_CHECKPOINT:{name}");
            std::io::stdout().flush().unwrap();
            let mut byte = [0];
            std::io::stdin().read_exact(&mut byte).unwrap();
            Ok(())
        }
        HookAction::Fail => Err(format!("injected {name}")),
        HookAction::Race(path) => {
            // Empty directory is the high-value race: ordinary rename would silently replace it.
            std::fs::create_dir(path).unwrap();
            Ok(())
        }
        HookAction::Barrier(barrier) => {
            barrier.wait();
            barrier.wait();
            Ok(())
        }
        HookAction::Rendezvous(started, release) => {
            started.send(()).unwrap();
            release.recv().unwrap();
            Ok(())
        }
    }
}

struct Fixture {
    root: PathBuf,
    runtime: PathBuf,
    archive: PathBuf,
    descriptor: ArtifactDescriptor,
}

fn payload(root: &Path, sentinel: &[u8]) {
    for arch in ["x32", "x64"] {
        std::fs::create_dir_all(root.join(arch)).unwrap();
        for dll in super::super::payload::DXVK_DLLS {
            std::fs::write(root.join(arch).join(dll), sentinel).unwrap();
        }
    }
    std::fs::write(root.join("sentinel"), sentinel).unwrap();
}

fn descriptor(archive: &Path) -> ArtifactDescriptor {
    let bytes = std::fs::read(archive).unwrap();
    let mut d = *catalog_descriptor(MANAGED_DXVK_ID).unwrap();
    d.archive = ArchiveLayout::Tar {
        archive_name: "fixture.tar",
        root: "dxvk-2.6.2",
    };
    d.expected_size = bytes.len() as u64;
    d.digest = ExpectedDigest::Sha256(Box::leak(
        format!("{:x}", Sha256::digest(bytes)).into_boxed_str(),
    ));
    d
}

impl Fixture {
    fn new(old: bool) -> Self {
        let root =
            std::env::temp_dir().join(format!("ro-f06-{}-{}", std::process::id(), unique_suffix()));
        let runtime = root.join("runtime");
        std::fs::create_dir_all(&runtime).unwrap();
        let archive = root.join("fixture.tar");
        let source = root.join("source");
        payload(&source, b"NEW");
        let mut tar = tar::Builder::new(std::fs::File::create(&archive).unwrap());
        tar.append_dir_all("dxvk-2.6.2", &source).unwrap();
        tar.finish().unwrap();
        let descriptor = descriptor(&archive);
        let fixture = Self {
            root,
            runtime,
            archive,
            descriptor,
        };
        if old {
            fixture.ready(&fixture.final_dir(), b"OLD");
        }
        fixture
    }
    fn final_dir(&self) -> PathBuf {
        self.runtime.join(self.descriptor.id)
    }
    fn slot(&self) -> PathBuf {
        activation_slot(&self.descriptor, &self.runtime)
    }
    fn ready(&self, path: &Path, sentinel: &[u8]) {
        payload(path, sentinel);
        write_receipt(&self.descriptor, path).unwrap();
        assert_eq!(
            artifact_cache_state_at(&self.descriptor, path),
            ArtifactCacheState::Ready
        );
    }
    fn assert_final(&self, winner: &[u8]) {
        assert_eq!(
            artifact_cache_state_at(&self.descriptor, &self.final_dir()),
            ArtifactCacheState::Ready
        );
        assert_eq!(
            std::fs::read(self.final_dir().join("sentinel")).unwrap(),
            winner
        );
    }
    fn fresh(&self) {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tools::artifacts::install::tests::subprocess",
                "--nocapture",
            ])
            .env("F06_ROOT", &self.root)
            .env("F06_ACTION", "recover")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "fresh recovery failed: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("F06_RECOVERED"));
    }
    fn kill_at(&self, action: &str, name: &str, hits: usize) {
        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tools::artifacts::install::tests::subprocess",
                "--nocapture",
            ])
            .env("F06_ROOT", &self.root)
            .env("F06_ACTION", action)
            .env("F06_CHECKPOINT", name)
            .env("F06_HITS", hits.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut child = ReapedChild(child);
        let stdout = child.0.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let expected = format!("F06_CHECKPOINT:{name}");
        let reader = std::thread::spawn(move || {
            let mut captured = String::new();
            for line in BufReader::new(stdout).lines() {
                let line = line.unwrap();
                captured.push_str(&line);
                captured.push('\n');
                if line == expected {
                    tx.send(Ok(())).unwrap();
                    return;
                }
            }
            let _ = tx.send(Err(captured));
        });
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(20))
                .expect("child checkpoint timed out"),
            Ok(())
        );
        assert!(
            OperationGuard::acquire("runtime", &self.runtime).is_err(),
            "child must own runtime lock at every kill"
        );
        child.0.kill().unwrap();
        assert_eq!(child.0.wait().unwrap().signal(), Some(libc::SIGKILL));
        reader.join().unwrap();
        assert!(
            OperationGuard::acquire("runtime", &self.runtime).is_ok(),
            "kernel must release lock without unlink"
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}
struct ReapedChild(Child);
impl Drop for ReapedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn subprocess() {
    let Some(root) = std::env::var_os("F06_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let runtime = root.join("runtime");
    let archive = root.join("fixture.tar");
    let d = descriptor(&archive);
    let _guard = OperationGuard::acquire("runtime", &runtime).unwrap();
    let action = std::env::var("F06_ACTION").unwrap();
    let slot = activation_slot(&d, &runtime);
    let final_dir = runtime.join(d.id);
    // Complete Ready/Ready exchange qualifications exercise the same production commit primitive;
    // automatic repair separately tests the plan's non-Ready-only supporting-artifact policy.
    if action == "replace_ready" {
        payload(&slot, b"NEW");
        write_receipt(&d, &slot).unwrap();
    }
    if let Ok(name) = std::env::var("F06_CHECKPOINT") {
        hook(
            &name,
            std::env::var("F06_HITS").unwrap().parse().unwrap(),
            HookAction::Pause,
        );
    }
    match action.as_str() {
        "install" => {
            super::super::import::import_archive_locked(&d, &archive, &runtime).unwrap();
        }
        "replace_ready" => {
            commit(&d, &slot, &final_dir, true).unwrap();
            recover(&d, &runtime).unwrap();
        }
        "recover" => {
            needs_install(&d, &runtime, policy(&d)).unwrap();
            println!(
                "F06_RECOVERED:{:?}",
                artifact_cache_state_at(&d, &final_dir)
            );
        }
        "elevate" => {
            elevate_v1(&d, &final_dir).unwrap();
        }
        _ => panic!("unknown child action"),
    }
}

#[test]
fn sigkill_first_install_receipt_and_commit_boundaries() {
    for (name, hits, committed) in [
        ("extracted", 1, false),
        ("before_receipt", 1, false),
        ("receipt_temp_synced", 3, false),
        ("receipt_renamed", 3, false),
        ("prepared_ready", 1, false),
        ("before_commit", 1, false),
        ("committed", 1, true),
        ("parent_sync", 1, true),
        ("after_parent_sync", 1, true),
        ("cleanup_tombstone", 1, true),
    ] {
        let fixture = Fixture::new(false);
        fixture.kill_at("install", name, hits);
        if committed {
            fixture.assert_final(b"NEW");
        } else {
            assert!(!fixture.final_dir().exists());
        }
        fixture.fresh();
        fixture.fresh();
        if committed {
            fixture.assert_final(b"NEW");
        } else {
            assert!(!fixture.final_dir().exists());
        }
        assert!(!fixture.slot().exists());
        assert_eq!(
            std::fs::read_dir(&fixture.runtime).unwrap().count(),
            usize::from(committed)
        );
        println!(
            "first/{name}: canonical={}",
            if committed { "NEW Ready" } else { "absent" }
        );
    }
}

#[test]
fn sigkill_replacement_exchange_and_cleanup_boundaries() {
    for name in [
        "before_commit",
        "committed",
        "parent_sync",
        "after_parent_sync",
        "cleanup_tombstone",
        "cleanup_entry",
        "cleanup_payload_gone",
        "cleanup_evidence_gone",
        "cleanup_done",
    ] {
        let fixture = Fixture::new(true);
        fixture.kill_at("replace_ready", name, 1);
        let winner = if name == "before_commit" {
            b"OLD"
        } else {
            b"NEW"
        };
        fixture.assert_final(winner);
        if name == "committed" {
            assert_eq!(
                std::fs::read(fixture.slot().join("sentinel")).unwrap(),
                b"OLD"
            );
        }
        fixture.fresh();
        fixture.fresh();
        fixture.assert_final(winner);
        if name == "cleanup_evidence_gone" {
            assert!(fixture.slot().is_dir()); // no ownership proof: preserve, never restore
            assert_eq!(std::fs::read_dir(fixture.slot()).unwrap().count(), 0);
        } else {
            assert!(!fixture.slot().exists());
        }
        println!(
            "replacement/{name}: canonical={} Ready",
            String::from_utf8_lossy(winner)
        );
    }
}

#[test]
fn sigkill_recovery_and_double_interruption_converge() {
    for initial in ["before_commit", "committed"] {
        for cleanup in ["cleanup_tombstone", "cleanup_entry", "cleanup_payload_gone"] {
            let fixture = Fixture::new(true);
            fixture.kill_at("replace_ready", initial, 1);
            fixture.kill_at("recover", cleanup, 1);
            fixture.kill_at("recover", "cleanup_payload_gone", 1);
            fixture.fresh();
            fixture.fresh();
            fixture.assert_final(if initial == "committed" {
                b"NEW"
            } else {
                b"OLD"
            });
            assert!(!fixture.slot().exists());
        }
    }
    let fixture = Fixture::new(false);
    fixture.kill_at("install", "before_commit", 1);
    fixture.kill_at("recover", "cleanup_entry", 1);
    fixture.fresh();
    assert!(!fixture.final_dir().exists());
    assert!(!fixture.slot().exists());
}

#[test]
fn prepared_receipt_validation_and_post_commit_errors() {
    let fixture = Fixture::new(false);
    let extracted = fixture.root.join("prepared");
    payload(&extracted, b"NEW");
    assert!(activate(
        &fixture.descriptor,
        &extracted,
        &fixture.runtime,
        policy(&fixture.descriptor)
    )
    .is_err());
    assert!(!fixture.final_dir().exists());
    prepare(&fixture.descriptor, &extracted).unwrap();
    assert!(matches!(
        parse_stored_receipt(&extracted.join(MARKER_FILE)).unwrap(),
        Some(ParsedStoredReceipt::V2(_))
    ));
    assert_eq!(
        artifact_cache_state_at(&fixture.descriptor, &extracted),
        ArtifactCacheState::Ready
    );
    hook("parent_sync", 1, HookAction::Fail);
    assert!(activate(
        &fixture.descriptor,
        &extracted,
        &fixture.runtime,
        policy(&fixture.descriptor)
    )
    .unwrap_err()
    .contains("injected"));
    fixture.assert_final(b"NEW");
    let fixture = Fixture::new(true);
    // Test the policy-approved repair through the public shared activation entry point.
    std::fs::remove_file(fixture.final_dir().join("x32/d3d9.dll")).unwrap();
    let extracted = fixture.root.join("prepared");
    payload(&extracted, b"NEW");
    prepare(&fixture.descriptor, &extracted).unwrap();
    hook("parent_sync", 1, HookAction::Fail);
    assert!(activate(
        &fixture.descriptor,
        &extracted,
        &fixture.runtime,
        policy(&fixture.descriptor)
    )
    .is_err());
    fixture.assert_final(b"NEW");
    fixture.fresh();
    fixture.assert_final(b"NEW");
    assert!(!fixture.slot().exists());
}

#[test]
fn ordinary_precommit_failure_uses_the_same_recovery() {
    for name in ["before_receipt", "prepared_ready", "before_commit"] {
        let fixture = Fixture::new(false);
        hook(name, 1, HookAction::Fail);
        assert!(super::super::import::import_archive_locked(
            &fixture.descriptor,
            &fixture.archive,
            &fixture.runtime
        )
        .is_err());
        fixture.fresh();
        assert!(!fixture.final_dir().exists());
        assert_eq!(std::fs::read_dir(&fixture.runtime).unwrap().count(), 0);
    }
}

#[test]
fn sigkill_production_supporting_repair_and_recovery_preserve_the_canonical_winner() {
    for checkpoint in ["before_commit", "committed"] {
        let fixture = Fixture::new(true);
        std::fs::remove_file(fixture.final_dir().join("x32/d3d9.dll")).unwrap();
        fixture.kill_at("install", checkpoint, 1);
        if checkpoint == "before_commit" {
            assert_eq!(
                classify(&fixture.descriptor, &fixture.final_dir()).unwrap(),
                Tree::OwnedIncomplete
            );
            assert_eq!(
                std::fs::read(fixture.final_dir().join("sentinel")).unwrap(),
                b"OLD"
            );
            fixture.kill_at("recover", "cleanup_entry", 1);
        } else {
            fixture.assert_final(b"NEW");
        }
        fixture.fresh();
        if checkpoint == "before_commit" {
            super::super::import::import_archive_locked(
                &fixture.descriptor,
                &fixture.archive,
                &fixture.runtime,
            )
            .unwrap();
        }
        fixture.assert_final(b"NEW");
    }
}

#[test]
fn cleanup_failure_and_invalid_preparation_cannot_mutate_the_committed_final() {
    let fixture = Fixture::new(true);
    let extracted = fixture.root.join("prepared");
    payload(&extracted, b"NEW");
    prepare(&fixture.descriptor, &extracted).unwrap();
    std::fs::remove_file(extracted.join("x32/d3d9.dll")).unwrap();
    assert!(activate(
        &fixture.descriptor,
        &extracted,
        &fixture.runtime,
        policy(&fixture.descriptor)
    )
    .is_err());
    fixture.assert_final(b"OLD");
    std::fs::write(extracted.join("x32/d3d9.dll"), b"NEW").unwrap();
    std::fs::remove_file(fixture.final_dir().join("x32/d3d9.dll")).unwrap();
    hook("cleanup_tombstone", 1, HookAction::Fail);
    assert!(activate(
        &fixture.descriptor,
        &extracted,
        &fixture.runtime,
        policy(&fixture.descriptor)
    )
    .is_err());
    fixture.assert_final(b"NEW");
    assert_eq!(
        classify(&fixture.descriptor, &fixture.slot()).unwrap(),
        Tree::Cleanup
    );
    fixture.fresh();
    fixture.assert_final(b"NEW");
    assert!(!fixture.slot().exists());
}

#[test]
fn foreign_slot_beside_ready_final_is_preserved_and_does_not_disable_reuse() {
    let fixture = Fixture::new(true);
    std::fs::create_dir(fixture.slot()).unwrap();
    std::fs::write(fixture.slot().join("foreign"), b"KEEP").unwrap();
    let before = snapshot(&fixture.runtime);
    assert!(!needs_install(
        &fixture.descriptor,
        &fixture.runtime,
        policy(&fixture.descriptor)
    )
    .unwrap());
    assert_eq!(snapshot(&fixture.runtime), before);
    fixture.assert_final(b"OLD");
}

#[test]
fn replacing_only_receipted_incomplete_supporting_artifacts_and_refusing_runners() {
    let fixture = Fixture::new(true);
    let inode = std::os::unix::fs::MetadataExt::ino(&fixture.final_dir().metadata().unwrap());
    assert!(!needs_install(
        &fixture.descriptor,
        &fixture.runtime,
        policy(&fixture.descriptor)
    )
    .unwrap());
    assert_eq!(
        std::os::unix::fs::MetadataExt::ino(&fixture.final_dir().metadata().unwrap()),
        inode
    );
    std::fs::remove_file(fixture.final_dir().join("x32/d3d9.dll")).unwrap();
    assert!(needs_install(
        &fixture.descriptor,
        &fixture.runtime,
        ExistingTargetPolicy::Refuse
    )
    .is_err());
    assert!(needs_install(
        &fixture.descriptor,
        &fixture.runtime,
        ExistingTargetPolicy::ReplaceNotReady
    )
    .unwrap());
    super::super::import::import_archive_locked(
        &fixture.descriptor,
        &fixture.archive,
        &fixture.runtime,
    )
    .unwrap();
    fixture.assert_final(b"NEW");
    for id in [
        MANAGED_RUNNER_ID,
        MANAGED_WINE716_ID,
        LEGACY_MANAGED_RUNNER_ID,
    ] {
        assert_eq!(
            policy(catalog_descriptor(id).unwrap()),
            ExistingTargetPolicy::Refuse
        );
    }
}

#[test]
fn first_install_noreplace_preserves_a_racing_empty_destination() {
    let fixture = Fixture::new(false);
    let extracted = fixture.root.join("prepared");
    payload(&extracted, b"NEW");
    prepare(&fixture.descriptor, &extracted).unwrap();
    hook("before_commit", 1, HookAction::Race(fixture.final_dir()));
    assert!(activate(
        &fixture.descriptor,
        &extracted,
        &fixture.runtime,
        policy(&fixture.descriptor)
    )
    .is_err());
    assert!(fixture.final_dir().is_dir());
    assert_eq!(std::fs::read_dir(fixture.final_dir()).unwrap().count(), 0);
    assert_eq!(
        artifact_cache_state_at(&fixture.descriptor, &fixture.slot()),
        ArtifactCacheState::Ready
    );
}

fn fifo(path: &Path) {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
}

fn foreign(path: &Path, kind: &str, fixture: &Fixture) {
    match kind {
        "symlink" => std::os::unix::fs::symlink(fixture.root.join("source"), path).unwrap(),
        "file" => std::fs::write(path, b"FOREIGN").unwrap(),
        "fifo" => fifo(path),
        _ => {
            payload(path, b"FOREIGN");
            let receipt = path.join(MARKER_FILE);
            match kind {
                "directory" => {}
                "receipt-symlink" => {
                    std::os::unix::fs::symlink(fixture.root.join("fixture.tar"), receipt).unwrap()
                }
                "receipt-directory" => std::fs::create_dir(receipt).unwrap(),
                "receipt-fifo" => fifo(&receipt),
                "receipt-corrupt" => std::fs::write(receipt, b"{").unwrap(),
                "receipt-future" => {
                    let mut r = receipt_v2_from_descriptor(&fixture.descriptor);
                    r.schema_version = 3;
                    replace_json(&receipt, &r).unwrap();
                }
                "receipt-wraparound" => {
                    let mut r =
                        serde_json::to_value(receipt_v2_from_descriptor(&fixture.descriptor))
                            .unwrap();
                    r["schemaVersion"] = (4_294_967_298_u64).into();
                    replace_json(&receipt, &r).unwrap();
                }
                "receipt-identity" => {
                    let mut r = receipt_v2_from_descriptor(&fixture.descriptor);
                    r.artifact_id = "other".into();
                    replace_json(&receipt, &r).unwrap();
                }
                _ => panic!("unknown foreign kind"),
            }
        }
    }
}

fn snapshot(root: &Path) -> Vec<(PathBuf, String, Vec<u8>)> {
    fn visit(path: &Path, base: &Path, out: &mut Vec<(PathBuf, String, Vec<u8>)>) {
        let m = path.symlink_metadata().unwrap();
        let (kind, bytes) = if m.file_type().is_symlink() {
            (
                "symlink",
                std::fs::read_link(path)
                    .unwrap()
                    .as_os_str()
                    .as_encoded_bytes()
                    .to_vec(),
            )
        } else if m.is_dir() {
            ("directory", vec![])
        } else if m.is_file() {
            ("file", std::fs::read(path).unwrap())
        } else {
            ("special", vec![])
        };
        out.push((
            path.strip_prefix(base).unwrap().to_path_buf(),
            kind.into(),
            bytes,
        ));
        if m.is_dir() {
            for entry in std::fs::read_dir(path).unwrap() {
                visit(&entry.unwrap().path(), base, out);
            }
        }
    }
    let mut out = vec![];
    visit(root, root, &mut out);
    out.sort();
    out
}

#[test]
fn foreign_final_slot_and_receipts_are_preserved_byte_for_byte() {
    for location in ["final", "slot"] {
        for kind in [
            "symlink",
            "file",
            "fifo",
            "directory",
            "receipt-symlink",
            "receipt-directory",
            "receipt-fifo",
            "receipt-corrupt",
            "receipt-future",
            "receipt-wraparound",
            "receipt-identity",
        ] {
            let fixture = Fixture::new(false);
            let path = if location == "final" {
                fixture.final_dir()
            } else {
                fixture.slot()
            };
            foreign(&path, kind, &fixture);
            let before = snapshot(&fixture.runtime);
            let extracted = fixture.root.join("prepared");
            payload(&extracted, b"NEW");
            prepare(&fixture.descriptor, &extracted).unwrap();
            assert!(
                activate(
                    &fixture.descriptor,
                    &extracted,
                    &fixture.runtime,
                    policy(&fixture.descriptor)
                )
                .is_err(),
                "{location}/{kind}"
            );
            assert_eq!(snapshot(&fixture.runtime), before, "{location}/{kind}");
            assert!(recover(&fixture.descriptor, &fixture.runtime).is_err());
            assert_eq!(snapshot(&fixture.runtime), before);
        }
    }
}

#[test]
fn residue_sweep_requires_exact_names_and_matching_identity() {
    let fixture = Fixture::new(true);
    for kind in ["staging", "import", "download"] {
        let path = fixture
            .runtime
            .join(format!(".{}.{kind}-123-456", fixture.descriptor.id));
        create_work(&fixture.descriptor, &path).unwrap();
        std::fs::write(path.join("partial"), b"ours").unwrap();
    }
    let previous = fixture
        .runtime
        .join(format!(".{}.previous-123", fixture.descriptor.id));
    fixture.ready(&previous, b"OLD");
    let foreign = fixture
        .runtime
        .join(format!(".{}.staging-234-567", fixture.descriptor.id));
    std::fs::create_dir(&foreign).unwrap();
    std::fs::write(foreign.join("foreign"), b"KEEP").unwrap();
    let unrelated = fixture
        .runtime
        .join(format!(".{}.staging-123-456-extra", fixture.descriptor.id));
    fixture.ready(&unrelated, b"KEEP");
    let before_foreign = snapshot(&foreign);
    let before_unrelated = snapshot(&unrelated);
    assert!(!needs_install(
        &fixture.descriptor,
        &fixture.runtime,
        policy(&fixture.descriptor)
    )
    .unwrap());
    assert_eq!(snapshot(&foreign), before_foreign);
    assert_eq!(snapshot(&unrelated), before_unrelated);
    assert_eq!(std::fs::read_dir(&fixture.runtime).unwrap().count(), 3);
    fixture.assert_final(b"OLD");
}

#[test]
fn v1_read_and_atomic_elevation_preserve_identity_and_unknown_schemas() {
    let fixture = Fixture::new(true);
    let path = fixture.final_dir().join(MARKER_FILE);
    let v1 = RuntimeMarkerV1 {
        schema_version: 1,
        artifact_id: fixture.descriptor.id.into(),
        digest: expected_digest_hex(fixture.descriptor.digest).into(),
    };
    replace_json(&path, &v1).unwrap();
    fixture.assert_final(b"OLD");
    hook("receipt_temp_synced", 1, HookAction::Fail);
    assert!(elevate_v1(&fixture.descriptor, &fixture.final_dir()).is_err());
    assert!(matches!(
        parse_stored_receipt(&path).unwrap(),
        Some(ParsedStoredReceipt::V1(_))
    ));
    fixture.assert_final(b"OLD");
    elevate_v1(&fixture.descriptor, &fixture.final_dir()).unwrap();
    assert!(matches!(
        parse_stored_receipt(&path).unwrap(),
        Some(ParsedStoredReceipt::V2(_))
    ));
    let mut r = receipt_v2_from_descriptor(&fixture.descriptor);
    r.schema_version = 100;
    replace_json(&path, &r).unwrap();
    let before = std::fs::read(&path).unwrap();
    elevate_v1(&fixture.descriptor, &fixture.final_dir()).unwrap();
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn lock_free_reader_retries_old_inode_after_exchange_and_cleanup() {
    let fixture = Fixture::new(true);
    fixture.ready(&fixture.slot(), b"NEW");
    let barrier = Arc::new(Barrier::new(2));
    let other = barrier.clone();
    let d = fixture.descriptor;
    let root = fixture.final_dir();
    let reader = std::thread::spawn(move || {
        hook("reader_pinned", 1, HookAction::Barrier(other));
        artifact_cache_state_at(&d, &root)
    });
    barrier.wait();
    commit(&d, &fixture.slot(), &fixture.final_dir(), true).unwrap();
    recover(&d, &fixture.runtime).unwrap();
    barrier.wait();
    assert_eq!(reader.join().unwrap(), ArtifactCacheState::Ready);
    fixture.assert_final(b"NEW");
    // Stress independent lock-free readers through repeated complete-generation exchanges.
    let running = std::sync::atomic::AtomicBool::new(true);
    std::thread::scope(|scope| {
        let reader = scope.spawn(|| {
            while running.load(std::sync::atomic::Ordering::Acquire) {
                assert_eq!(
                    artifact_cache_state_at(&d, &fixture.final_dir()),
                    ArtifactCacheState::Ready
                );
            }
        });
        for _ in 0..30 {
            fixture.ready(&fixture.slot(), b"NEXT");
            commit(&d, &fixture.slot(), &fixture.final_dir(), true).unwrap();
            recover(&d, &fixture.runtime).unwrap();
        }
        running.store(false, std::sync::atomic::Ordering::Release);
        reader.join().unwrap();
    });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_real_import_waiter_keeps_extraction_paths_and_runtime_guard() {
    let fixture = Fixture::new(false);
    let operation = OperationGuard::acquire("runtime", &fixture.runtime).unwrap();
    let d = fixture.descriptor;
    let archive = fixture.archive.clone();
    let runtime = fixture.runtime.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    let waiter = tokio::spawn(async move {
        tokio::task::spawn_blocking(move || {
            let _operation = operation;
            hook(
                "extracted",
                1,
                HookAction::Rendezvous(started_tx, release_rx),
            );
            let result = super::super::import::import_archive_locked(&d, &archive, &runtime);
            drop(_operation);
            done_tx.send(result).unwrap();
        })
        .await
        .unwrap();
    });
    started_rx.await.unwrap();
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());
    assert!(OperationGuard::acquire("runtime", &fixture.runtime).is_err());
    assert!(!fixture.final_dir().exists());
    assert_eq!(std::fs::read_dir(&fixture.runtime).unwrap().count(), 2);
    release_tx.send(()).unwrap();
    done_rx.await.unwrap().unwrap();
    fixture.assert_final(b"NEW");
    assert_eq!(std::fs::read_dir(&fixture.runtime).unwrap().count(), 1);
    assert!(OperationGuard::acquire("runtime", &fixture.runtime).is_ok());
}

#[test]
fn sigkill_v1_elevation_never_exposes_an_invalid_canonical_receipt() {
    for name in ["receipt_temp_synced", "receipt_renamed"] {
        let fixture = Fixture::new(true);
        let path = fixture.final_dir().join(MARKER_FILE);
        let marker = RuntimeMarkerV1 {
            schema_version: 1,
            artifact_id: fixture.descriptor.id.into(),
            digest: expected_digest_hex(fixture.descriptor.digest).into(),
        };
        replace_json(&path, &marker).unwrap();
        fixture.kill_at("elevate", name, 1);
        fixture.assert_final(b"OLD");
        fixture.fresh();
        fixture.assert_final(b"OLD");
    }
}

#[test]
fn prepared_payload_cannot_depend_on_a_symlinked_directory_outside_its_tree() {
    let fixture = Fixture::new(false);
    let prepared = fixture.root.join("prepared");
    std::fs::create_dir(&prepared).unwrap();
    std::os::unix::fs::symlink(fixture.root.join("source/x32"), prepared.join("x32")).unwrap();
    std::os::unix::fs::symlink(fixture.root.join("source/x64"), prepared.join("x64")).unwrap();
    assert!(prepare(&fixture.descriptor, &prepared).is_err());
    assert!(!fixture.final_dir().exists());
}

#[test]
fn representative_descriptor_identity_uses_the_shared_synthetic_activation_protocol() {
    // Runner module hashes stay production-pinned. These synthetic descriptors substitute only
    // the payload fixture, explicitly not real Wine/Proton qualification.
    for id in [
        MANAGED_RUNNER_ID,
        MANAGED_WINE716_ID,
        UMU_ID,
        MANAGED_DXVK_ID,
    ] {
        let fixture = Fixture::new(false);
        let mut d = *catalog_descriptor(id).unwrap();
        d.payload = PayloadValidatorId::UmuZipapp;
        let extracted = fixture.root.join("prepared");
        std::fs::create_dir(&extracted).unwrap();
        let exe = extracted.join("umu-run");
        std::fs::write(&exe, b"synthetic").unwrap();
        std::fs::set_permissions(exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        prepare(&d, &extracted).unwrap();
        activate(&d, &extracted, &fixture.runtime, policy(&d)).unwrap();
        assert_eq!(
            artifact_cache_state_at(&d, &fixture.runtime.join(d.id)),
            ArtifactCacheState::Ready
        );
    }
}
