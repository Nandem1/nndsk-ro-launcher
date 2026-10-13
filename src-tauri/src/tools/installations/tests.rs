use super::{
    repository::InstallationManager,
    scan::candidates,
    session::{dispatch, map_destination, record_installer_exit, wait_installer, write_protection},
};
use crate::models::installation::{CreateInstallation, InstallationBinding, InstallationStatus};
use crate::tools::{
    runner_sessions::{RunnerOperation, RunnerSessionRegistry},
    runtime::{probe_runner, resolve_prefix_binding, session_anchor_unavailable},
};
use crate::utils::{
    ensure_installation_rebuild_allowed, resolve_runner, validate_installation_context,
    OperationGuard, PrefixLocation, PrefixScope, RunnerInvocation, WineContext,
    INSTALLATION_MARKER,
};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

struct Fixture {
    root: PathBuf,
    manager: InstallationManager,
}
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("ro-installation-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("Installer.exe"), b"fixture").unwrap();
        Self {
            manager: InstallationManager::new(root.join("installations.json")),
            root,
        }
    }
    fn input(&self) -> CreateInstallation {
        CreateInstallation {
            name: "Test RO".into(),
            installer_path: self.root.join("Installer.exe").to_string_lossy().into(),
            destination_path: Some(self.root.join("game").to_string_lossy().into()),
            runner: crate::tools::runners::managed_proton_path()
                .to_string_lossy()
                .into(),
            require_webview2: false,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn wait_for_installation_idle(manager: &InstallationManager, id: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while manager.is_active(id).unwrap() {
        assert!(
            Instant::now() < deadline,
            "Background installation did not release admission"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn regression_relaunch_census_reserves_installation_admission() {
    let fixture = Fixture::new();
    let draft = fixture.manager.create(fixture.input()).unwrap();
    let _admission = fixture.manager.close_admission_if_idle().unwrap();
    let admitted = fixture.manager.reserve(&draft.id);
    assert!(
        admitted.is_err(),
        "idle census did not reserve installation admission"
    );
}

#[test]
fn active_installation_refuses_relaunch_without_cancellation() {
    let fixture = Fixture::new();
    let draft = fixture.manager.create(fixture.input()).unwrap();
    let (_, cancel, active) = fixture.manager.reserve(&draft.id).unwrap();
    assert!(fixture.manager.close_admission_if_idle().is_err());
    assert!(!*cancel.borrow());
    assert_eq!(
        fixture.manager.get(&draft.id).unwrap().status,
        InstallationStatus::Preparing
    );
    // A failed relaunch must leave ordinary create admission open too.
    fixture.manager.create(fixture.input()).unwrap();
    drop(active);
    drop(fixture.manager.close_admission_if_idle().unwrap());
    let (_, _, retry) = fixture.manager.reserve(&draft.id).unwrap();
    drop(retry);
}

#[test]
fn relaunch_installation_guard_cannot_reopen_real_shutdown() {
    let fixture = Fixture::new();
    let draft = fixture.manager.create(fixture.input()).unwrap();
    let admission = fixture.manager.close_admission_if_idle().unwrap();
    assert!(fixture.manager.close_admission_if_idle().is_err());
    assert!(fixture.manager.create(fixture.input()).is_err());
    assert!(fixture.manager.reserve(&draft.id).is_err());
    fixture.manager.request_shutdown();
    drop(admission);
    assert!(fixture.manager.create(fixture.input()).is_err());
    assert!(fixture.manager.reserve(&draft.id).is_err());
    assert!(fixture.manager.close_admission_if_idle().is_err());
}

#[test]
fn installation_and_relaunch_race_have_exactly_one_winner() {
    let fixture = Fixture::new();
    let draft = fixture.manager.create(fixture.input()).unwrap();
    for _ in 0..1_000 {
        let barrier = std::sync::Barrier::new(2);
        std::thread::scope(|scope| {
            let installer = scope.spawn(|| {
                barrier.wait();
                fixture.manager.reserve(&draft.id)
            });
            barrier.wait();
            let restart = fixture.manager.close_admission_if_idle();
            let installer = installer.join().unwrap();
            assert_ne!(installer.is_ok(), restart.is_ok());
        });
    }
}

#[test]
fn create_and_relaunch_share_admission_mutex() {
    let fixture = Fixture::new();
    fixture.manager.create(fixture.input()).unwrap();
    let admission = fixture.manager.close_admission_if_idle().unwrap();
    assert!(fixture.manager.create(fixture.input()).is_err());
    drop(admission);
    fixture.manager.create(fixture.input()).unwrap();
}

#[test]
fn synchronous_dispatch_survives_ipc_return_and_delivers_cancellation() {
    let fixture = Fixture::new();
    let draft = fixture.manager.create(fixture.input()).unwrap();
    let manager = fixture.manager.clone();
    let id = draft.id.clone();
    let (ready, ready_receive) = std::sync::mpsc::channel();
    // A Tauri synchronous command runs on the native UI thread, not a Tokio worker.
    let preparing = std::thread::spawn(move || {
        assert!(tokio::runtime::Handle::try_current().is_err());
        dispatch(manager, &id, move |manager, draft, mut cancel| async move {
            assert!(tokio::runtime::Handle::try_current().is_ok());
            // Exercise the reactor, not just an immediately-ready future.
            tokio::time::sleep(Duration::from_millis(1)).await;
            ready.send(()).unwrap();
            if !*cancel.borrow_and_update() {
                cancel.changed().await.map_err(|error| error.to_string())?;
            }
            manager.update(&draft.id, |draft| {
                draft.status = InstallationStatus::Interrupted;
                Ok(())
            })?;
            Ok(())
        })
    })
    .join()
    .expect("Synchronous installation entry panicked")
    .unwrap();
    assert_eq!(preparing.status, InstallationStatus::Preparing);
    ready_receive.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(fixture.manager.is_active(&draft.id).unwrap());
    assert!(fixture.manager.reserve(&draft.id).is_err());
    fixture.manager.cancel(&draft.id).unwrap();
    wait_for_installation_idle(&fixture.manager, &draft.id);
    let result = fixture.manager.get(&draft.id).unwrap();
    assert_eq!(result.status, InstallationStatus::Interrupted);
    assert_eq!(result.server_id, draft.server_id);
    assert_eq!(result.runner, draft.runner);
}

#[test]
fn synchronous_dispatch_persists_failure_and_releases_admission() {
    let fixture = Fixture::new();
    let draft = fixture.manager.create(fixture.input()).unwrap();
    let manager = fixture.manager.clone();
    let id = draft.id.clone();
    std::thread::spawn(move || {
        assert!(tokio::runtime::Handle::try_current().is_err());
        dispatch(manager, &id, |_, _, _| async {
            tokio::time::sleep(Duration::from_millis(1)).await;
            Err("synthetic preparation failure".into())
        })
    })
    .join()
    .expect("Synchronous installation entry panicked")
    .unwrap();
    wait_for_installation_idle(&fixture.manager, &draft.id);
    let result = fixture.manager.get(&draft.id).unwrap();
    assert_eq!(result.status, InstallationStatus::Failed);
    assert_eq!(
        result.message.as_deref(),
        Some("synthetic preparation failure")
    );
    let (_, _, active) = fixture.manager.reserve(&draft.id).unwrap();
    drop(active);
}

#[test]
fn synchronous_dispatch_shutdown_cancels_owned_task_and_closes_admission() {
    let fixture = Fixture::new();
    let draft = fixture.manager.create(fixture.input()).unwrap();
    let manager = fixture.manager.clone();
    let id = draft.id.clone();
    let (ready, ready_receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        assert!(tokio::runtime::Handle::try_current().is_err());
        dispatch(manager, &id, move |_, _, mut cancel| async move {
            ready.send(()).unwrap();
            if !*cancel.borrow_and_update() {
                cancel.changed().await.map_err(|error| error.to_string())?;
            }
            Ok(())
        })
    })
    .join()
    .expect("Synchronous installation entry panicked")
    .unwrap();
    ready_receive.recv_timeout(Duration::from_secs(5)).unwrap();
    fixture.manager.request_shutdown();
    wait_for_installation_idle(&fixture.manager, &draft.id);
    assert_eq!(
        fixture.manager.get(&draft.id).unwrap().status,
        InstallationStatus::Interrupted
    );
    assert!(
        dispatch(fixture.manager.clone(), &draft.id, |_, _, _| async {
            Ok(())
        })
        .is_err()
    );
    assert!(fixture.manager.create(fixture.input()).is_err());
}

#[test]
fn durable_recovery_preserves_identity_and_refuses_duplicate_execution() {
    let fixture = Fixture::new();
    let draft = fixture.manager.create(fixture.input()).unwrap();
    let (preparing, mut cancel, guard) = fixture.manager.reserve(&draft.id).unwrap();
    assert_eq!(preparing.status, InstallationStatus::Preparing);
    assert!(fixture.manager.reserve(&draft.id).is_err());
    // Simulate a fresh application reading the last durable state after a crash.
    let recovered = InstallationManager::new(fixture.root.join("installations.json"))
        .get(&draft.id)
        .unwrap();
    assert_eq!(recovered.status, InstallationStatus::Interrupted);
    assert_eq!(recovered.server_id, draft.server_id);
    assert_eq!(recovered.runner, draft.runner);
    fixture.manager.cancel(&draft.id).unwrap();
    assert!(*cancel.borrow_and_update());
    drop(guard);
    assert_eq!(
        fixture.manager.get(&draft.id).unwrap().status,
        InstallationStatus::Interrupted
    );
    let (_, _, guard) = fixture.manager.reserve(&draft.id).unwrap();
    drop(guard);
    fixture.manager.request_shutdown();
    assert!(fixture.manager.reserve(&draft.id).is_err());
    assert!(fixture.root.join("Installer.exe").is_file());
}

#[test]
fn closed_installer_can_restart_in_the_same_environment_until_client_confirmation() {
    let fixture = Fixture::new();
    let draft = fixture.manager.create(fixture.input()).unwrap();
    let binding = InstallationBinding {
        installation_id: draft.id.clone(),
        prefix_path: fixture.root.join("prefix").to_string_lossy().into(),
        prefix_fingerprint: "a".repeat(64),
    };
    fixture
        .manager
        .update(&draft.id, |draft| {
            draft.binding = Some(binding.clone());
            Ok(())
        })
        .unwrap();
    record_installer_exit(&fixture.manager, &draft.id, 0).unwrap();
    // The persisted state is the same whether the installer installed or just closed.
    let reloaded = InstallationManager::new(fixture.root.join("installations.json"));
    let (preparing, _, guard) = reloaded.reserve(&draft.id).unwrap();
    assert_eq!(preparing.status, InstallationStatus::Preparing);
    assert_eq!(preparing.binding, Some(binding));
    assert_eq!(preparing.server_id, draft.server_id);
    assert_eq!(preparing.runner, draft.runner);
    assert_eq!(preparing.exit_code, None);
    assert!(reloaded.reserve(&draft.id).is_err());
    drop(guard);
    reloaded
        .update(&draft.id, |draft| {
            draft.status = InstallationStatus::Completed;
            draft.exit_code = Some(0);
            Ok(())
        })
        .unwrap();
    assert!(reloaded.reserve(&draft.id).is_err());
}

#[test]
fn installer_restart_cannot_race_client_confirmation() {
    let fixture = Fixture::new();
    let draft = fixture.manager.create(fixture.input()).unwrap();
    let admission = OperationGuard::acquire("installation-confirm", Path::new(&draft.id)).unwrap();
    assert!(fixture.manager.reserve(&draft.id).is_err());
    drop(admission);
    let (_, _, guard) = fixture.manager.reserve(&draft.id).unwrap();
    drop(guard);
}

#[test]
fn default_destination_and_legacy_external_drafts_roundtrip_without_migration() {
    let fixture = Fixture::new();
    let mut wire = serde_json::to_value(fixture.manager.create(fixture.input()).unwrap()).unwrap();
    let legacy: crate::models::installation::InstallationDraft =
        serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(legacy.destination_path, fixture.input().destination_path);
    legacy.validate().unwrap();
    wire.as_object_mut().unwrap().remove("destinationPath");
    let default: crate::models::installation::InstallationDraft =
        serde_json::from_value(wire).unwrap();
    assert!(default.destination_path.is_none());
    default.validate().unwrap();
    let mut input = serde_json::to_value(&legacy).unwrap();
    input
        .as_object_mut()
        .unwrap()
        .retain(|key, _| ["name", "installerPath", "runner"].contains(&key.as_str()));
    let input: CreateInstallation = serde_json::from_value(input).unwrap();
    let created = fixture.manager.create(input).unwrap();
    assert!(created.destination_path.is_none());
    assert!(serde_json::to_value(&created).unwrap()["destinationPath"].is_null());
    assert!(!fixture.root.join("game").exists());
    let recovered = InstallationManager::new(fixture.root.join("installations.json"));
    assert_eq!(recovered.list().unwrap().len(), 2);
    assert_eq!(recovered.get(&created.id).unwrap().destination_path, None);
    assert_eq!(
        recovered.get(&legacy.id).unwrap().destination_path,
        legacy.destination_path
    );
}

#[test]
fn default_candidate_search_finds_client_in_c_without_adopting_other_directories() {
    let fixture = Fixture::new();
    let mut input = fixture.input();
    input.destination_path = None;
    let draft = fixture.manager.create(input).unwrap();
    let prefix = fixture.root.join("prefix");
    let game = prefix.join("drive_c/HoneyRO");
    fs::create_dir_all(&game).unwrap();
    fs::write(game.join("HoneyRO.exe"), b"game").unwrap();
    fs::write(fixture.root.join("unrelated.exe"), b"unrelated").unwrap();
    std::os::unix::fs::symlink(&fixture.root, game.join("escape")).unwrap();
    fixture
        .manager
        .update(&draft.id, |draft| {
            draft.binding = Some(InstallationBinding {
                installation_id: draft.id.clone(),
                prefix_path: prefix.to_string_lossy().into(),
                prefix_fingerprint: "a".repeat(64),
            });
            Ok(())
        })
        .unwrap();
    record_installer_exit(&fixture.manager, &draft.id, 0).unwrap();
    assert_eq!(
        super::scan_candidates(&fixture.manager, &draft.id).unwrap(),
        vec![game.join("HoneyRO.exe").to_string_lossy().to_string()]
    );
    record_installer_exit(&fixture.manager, &draft.id, 7).unwrap();
    assert!(super::scan_candidates(&fixture.manager, &draft.id).is_err());
    assert_eq!(fixture.manager.get(&draft.id).unwrap().exit_code, Some(7));
    assert!(game.join("HoneyRO.exe").is_file());
}

#[test]
fn failed_installer_cannot_be_promoted_by_files_or_zero_code_alone() {
    let fixture = Fixture::new();
    let draft = fixture.manager.create(fixture.input()).unwrap();
    assert!(fixture
        .manager
        .update(&draft.id, |draft| {
            draft.status = InstallationStatus::AwaitingClient;
            draft.exit_code = Some(0);
            Ok(())
        })
        .is_err());
    fixture
        .manager
        .update(&draft.id, |draft| {
            draft.status = InstallationStatus::Failed;
            draft.exit_code = Some(7);
            Ok(())
        })
        .unwrap();
    assert!(fixture
        .manager
        .update(&draft.id, |draft| {
            draft.binding = Some(InstallationBinding {
                installation_id: draft.id.clone(),
                prefix_path: fixture.root.join("prefix").to_string_lossy().into(),
                prefix_fingerprint: "a".repeat(64),
            });
            draft.status = InstallationStatus::AwaitingClient;
            Ok(())
        })
        .is_err());
    assert!(super::scan_candidates(&fixture.manager, &draft.id).is_err());
    let wire = serde_json::to_value(&draft).unwrap();
    assert_eq!(wire["serverId"], draft.server_id);
    assert!(wire.get("installerPath").is_some());
    assert!(wire.get("executablePath").is_none());
}

#[test]
fn mappings_and_rebuild_protection_preserve_existing_data() {
    let fixture = Fixture::new();
    let prefix = fixture.root.join("prefix");
    fs::create_dir_all(prefix.join("dosdevices")).unwrap();
    let game = fixture.root.join("game");
    map_destination(&prefix, &game).unwrap();
    map_destination(&prefix, &game).unwrap();
    fs::write(game.join("client.exe"), b"game data").unwrap();
    assert!(map_destination(&prefix, &fixture.root.join("other")).is_err());
    assert_eq!(
        fs::canonicalize(prefix.join("dosdevices/r:")).unwrap(),
        game
    );
    assert!(ensure_installation_rebuild_allowed(&prefix).is_ok());
    fs::write(
        prefix.join(INSTALLATION_MARKER),
        b"corrupt protection marker",
    )
    .unwrap();
    assert!(ensure_installation_rebuild_allowed(&prefix).is_err());
    // A missing main marker cannot disable protection if its durable backup remains.
    fs::rename(
        prefix.join(INSTALLATION_MARKER),
        prefix.join(format!("{INSTALLATION_MARKER}.bak")),
    )
    .unwrap();
    assert!(ensure_installation_rebuild_allowed(&prefix).is_err());
    assert_eq!(fs::read(game.join("client.exe")).unwrap(), b"game data");
}

#[test]
fn protection_receipt_is_immutable_and_does_not_follow_symlinks() {
    let fixture = Fixture::new();
    let binding = InstallationBinding {
        installation_id: uuid::Uuid::new_v4().to_string(),
        prefix_path: fixture.root.to_string_lossy().into(),
        prefix_fingerprint: "a".repeat(64),
    };
    write_protection(&fixture.root, &binding).unwrap();
    write_protection(&fixture.root, &binding).unwrap();
    let changed = InstallationBinding {
        installation_id: uuid::Uuid::new_v4().to_string(),
        ..binding.clone()
    };
    assert!(write_protection(&fixture.root, &changed).is_err());
    let target = fixture.root.join("personal-data");
    fs::write(&target, b"preserve me").unwrap();
    let backup = fixture.root.join(format!("{INSTALLATION_MARKER}.bak"));
    std::os::unix::fs::symlink(&target, backup).unwrap();
    assert!(write_protection(&fixture.root, &binding).is_err());
    assert_eq!(fs::read(target).unwrap(), b"preserve me");
}

#[test]
fn unreadable_protection_is_not_treated_as_absent() {
    let fixture = Fixture::new();
    let binding = InstallationBinding {
        installation_id: uuid::Uuid::new_v4().to_string(),
        prefix_path: fixture.root.to_string_lossy().into(),
        prefix_fingerprint: "a".repeat(64),
    };
    let marker = fixture.root.join(INSTALLATION_MARKER);
    fs::write(&marker, vec![b'x'; 8193]).unwrap();
    assert!(write_protection(&fixture.root, &binding).is_err());
    assert!(ensure_installation_rebuild_allowed(&fixture.root).is_err());
    assert_eq!(fs::metadata(&marker).unwrap().len(), 8193);
    // A non-directory ancestor yields ENOTDIR, not absence of protection.
    let invalid_prefix = marker.join("prefix");
    assert!(write_protection(&invalid_prefix, &binding).is_err());
    assert!(ensure_installation_rebuild_allowed(&invalid_prefix).is_err());
}

#[test]
fn candidate_search_is_scoped_and_does_not_follow_directory_symlinks() {
    let fixture = Fixture::new();
    let draft = fixture.manager.create(fixture.input()).unwrap();
    let game = Path::new(draft.destination_path.as_deref().unwrap());
    fs::create_dir(game).unwrap();
    fs::write(game.join("Ragexe.exe"), b"game").unwrap();
    fs::write(game.join("unins000.exe"), b"uninstaller").unwrap();
    fs::write(fixture.root.join("personal.exe"), b"unrelated").unwrap();
    std::os::unix::fs::symlink(&fixture.root, game.join("escape")).unwrap();
    assert_eq!(
        candidates(&draft),
        vec![game.join("Ragexe.exe").to_string_lossy().to_string()]
    );
}

fn context(root: &Path) -> WineContext {
    use std::os::unix::fs::PermissionsExt;
    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    for name in ["wine", "wineserver"] {
        let path = bin.join(name);
        fs::write(&path, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let resolved = resolve_runner(bin.join("wine").to_str().unwrap()).unwrap();
    let probe = probe_runner(&resolved);
    let mut identity = resolve_prefix_binding(None, &resolved, &probe, false).unwrap();
    let prefix = root.join("prefix");
    fs::create_dir_all(&prefix).unwrap();
    let location = PrefixLocation {
        path: prefix.to_string_lossy().into(),
        scope: PrefixScope::Shared,
        managed: false,
        server_id: None,
    };
    identity.location = location.clone();
    WineContext {
        prefix: location.path.clone(),
        location,
        resolved,
        identity,
        probe,
    }
}

#[test]
fn installed_client_rejects_another_prefix_or_material_identity() {
    let fixture = Fixture::new();
    let ctx = context(&fixture.root);
    let binding = InstallationBinding {
        installation_id: uuid::Uuid::new_v4().to_string(),
        prefix_path: ctx.prefix.clone(),
        prefix_fingerprint: ctx.identity.desired_fingerprint.digest.hex_digest(),
    };
    assert!(validate_installation_context(&binding, &ctx.identity).is_ok());
    let wrong = InstallationBinding {
        prefix_path: fixture.root.join("other").to_string_lossy().into(),
        ..binding.clone()
    };
    assert!(validate_installation_context(&wrong, &ctx.identity).is_err());
    let wrong = InstallationBinding {
        prefix_fingerprint: "0".repeat(64),
        ..binding
    };
    assert!(validate_installation_context(&wrong, &ctx.identity).is_err());
}

#[test]
fn draft_and_final_server_resolve_the_same_isolated_environment() {
    let fixture = Fixture::new();
    let ctx = context(&fixture.root);
    let server_id = format!("installed-{}", uuid::Uuid::new_v4());
    let draft = crate::tools::runtime::resolve_installation_prefix_binding(
        &server_id,
        &ctx.resolved,
        &ctx.probe,
    )
    .unwrap();
    let server: crate::models::server::ServerConfig = serde_json::from_value(serde_json::json!({
        "id": server_id,
        "name": "Installed client",
        "executablePath": fixture.root.join("game/Ragexe.exe"),
        "prefixMode": "isolated",
        "runner": ctx.resolved.runner_path(),
    }))
    .unwrap();
    let finished = resolve_prefix_binding(
        Some(&server),
        &ctx.resolved,
        &ctx.probe,
        ctx.resolved.is_wine_7_16(),
    )
    .unwrap();
    assert_eq!(draft.location.path, finished.location.path);
    assert_eq!(
        draft.desired_fingerprint.digest.hex_digest(),
        finished.desired_fingerprint.digest.hex_digest(),
    );
}

fn python_invocation(ctx: &WineContext, code: &str) -> RunnerInvocation {
    RunnerInvocation {
        program: PathBuf::from("/usr/bin/python3"),
        args: vec!["-c".into(), code.into()],
        cwd: PathBuf::from(&ctx.prefix),
        env: vec![(
            OsString::from("WINEPREFIX"),
            Some(ctx.prefix.clone().into()),
        )],
    }
}

#[tokio::test]
async fn handoff_waits_for_children_with_cleared_environment_and_cancel_preserves_other_prefix() {
    let fixture = Fixture::new();
    let ctx = context(&fixture.root.join("a"));
    let other = context(&fixture.root.join("b"));
    let sidecar = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("target/debug/ro-sessiond");
    assert!(
        sidecar.is_file(),
        "Build ro-sessiond before runtime integration tests"
    );
    let sessions = RunnerSessionRegistry::with_sidecar_for_test(sidecar);
    let game = crate::state::GameProcessHandle::new();
    let anchor = session_anchor_unavailable();
    for expected in [0, 7, 0] {
        let mut op = RunnerOperation::begin(None, &sessions, &game, &ctx, &anchor)
            .await
            .unwrap();
        let prefix_guard = OperationGuard::acquire("prefix", Path::new(&ctx.prefix)).unwrap();
        assert!(OperationGuard::acquire_shared("prefix", Path::new(&ctx.prefix)).is_err());
        let code = format!("import subprocess,sys,os; e=dict(os.environ); e.pop('WINEPREFIX',None); subprocess.Popen([sys.executable,'-c','import time; time.sleep(0.35)'],env=e); sys.exit({expected})");
        let mut child = op.spawn(python_invocation(&ctx, &code), &[]).await.unwrap();
        let (_send, mut cancel) = tokio::sync::watch::channel(false);
        let started = tokio::time::Instant::now();
        let code = tokio::time::timeout(
            Duration::from_secs(5),
            wait_installer(
                &mut child,
                &ctx.prefix,
                op.lease().map(|lease| lease.supervisor_identity()),
                &mut cancel,
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(code, Some(expected));
        assert!(started.elapsed() >= Duration::from_millis(300));
        op.quiesce().await.unwrap();
        drop(prefix_guard);
        assert!(ro_tools_linux::find_prefix_processes(&ctx.prefix).is_empty());
    }
    let mut op = RunnerOperation::begin(None, &sessions, &game, &ctx, &anchor)
        .await
        .unwrap();
    let mut other_op = RunnerOperation::begin(None, &sessions, &game, &other, &anchor)
        .await
        .unwrap();
    let mut child = op
        .spawn(python_invocation(&ctx, "import subprocess,sys,os; e=dict(os.environ); e.pop('WINEPREFIX',None); subprocess.Popen([sys.executable,'-c','import time; time.sleep(60)'],env=e)"), &[])
        .await
        .unwrap();
    let mut other_child = other_op
        .spawn(
            python_invocation(&other, "import time; time.sleep(60)"),
            &[],
        )
        .await
        .unwrap();
    let identity = other_child.controller_identity().unwrap();
    assert_eq!(child.wait().await.unwrap(), 0);
    assert!(crate::tools::runner_sessions::prefix_has_active_programs(
        &ctx.prefix,
        op.lease().map(|lease| lease.supervisor_identity())
    ));
    let (send, mut cancel) = tokio::sync::watch::channel(true);
    assert_eq!(
        wait_installer(
            &mut child,
            &ctx.prefix,
            op.lease().map(|lease| lease.supervisor_identity()),
            &mut cancel
        )
        .await
        .unwrap(),
        None
    );
    child.terminate().await.unwrap();
    op.quiesce().await.unwrap();
    assert!(ro_tools_linux::verify_process_identity(&identity));
    assert!(other_child.try_exit_code().is_none());
    other_child.terminate().await.unwrap();
    other_op.quiesce().await.unwrap();
    drop(send);
    assert!(sessions.shutdown_all().await.is_empty());
}
