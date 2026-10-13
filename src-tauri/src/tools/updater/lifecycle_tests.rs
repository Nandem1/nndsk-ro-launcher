use super::{backend::mock::MockBackend, shutdown_launcher_services, UpdateLease};
use crate::models::{
    installation::CreateInstallation,
    update::{UpdateFailureClass, UpdatePhase},
};
use crate::state::{GameProcessHandle, GameState};
use crate::tools::{
    autobuff::AutobuffHandle, autopot::AutopotHandle, input::InputGateway,
    installations::InstallationManager, memory_sessions::MemorySessionRegistry,
    presence::PresenceHandle, runner_sessions::RunnerSessionRegistry, spammer::SpammerHandle,
};
use std::{
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
};

struct Fixture {
    root: PathBuf,
    state: GameState,
}

impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("ro-updater-lifecycle-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("Installer.exe"), b"fixture").unwrap();
        let memory = MemorySessionRegistry::new();
        Self {
            state: GameState {
                game: GameProcessHandle::new(),
                tool_lifecycle: Arc::new(tokio::sync::Mutex::new(())),
                autopot: AutopotHandle::new(memory.clone()),
                autobuff: AutobuffHandle::new(memory.clone()),
                spammer: SpammerHandle::new(),
                input: InputGateway::new(),
                presence: PresenceHandle::new(memory.clone()),
                sessions: RunnerSessionRegistry::with_sidecar_for_test(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/debug/ro-sessiond"),
                ),
                memory,
                installations: InstallationManager::new(root.join("installations.json")),
            },
            root,
        }
    }

    fn input(&self) -> CreateInstallation {
        CreateInstallation {
            name: "Test RO".into(),
            installer_path: self.root.join("Installer.exe").to_string_lossy().into(),
            destination_path: None,
            runner: crate::tools::runners::managed_proton_path()
                .to_string_lossy()
                .into(),
            require_webview2: false,
        }
    }

    fn launch(&self) -> Result<crate::state::LaunchReservation, String> {
        self.state
            .game
            .begin_launch("test".into(), "server".into(), "Server".into())
    }

    // Real registry/supervisor admission, with an inert Wine pair in a disposable root.
    // This exercises registry closing without executing Wine or replacing any AppImage.
    fn context(&self) -> crate::utils::WineContext {
        use crate::utils::{resolve_runner, PrefixLocation, PrefixScope, WineContext};
        use std::os::unix::fs::PermissionsExt;
        let runner = self.root.join("runner");
        let prefix = self.root.join("prefix");
        std::fs::create_dir_all(&runner).unwrap();
        std::fs::create_dir_all(&prefix).unwrap();
        for name in ["wine", "wineserver"] {
            let path = runner.join(name);
            std::fs::write(&path, b"#!/bin/sh\nexit 0\n").unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let resolved = resolve_runner(runner.join("wine").to_str().unwrap()).unwrap();
        let probe = crate::tools::runtime::probe_runner(&resolved);
        let identity =
            crate::tools::runtime::resolve_prefix_binding(None, &resolved, &probe, false).unwrap();
        WineContext {
            prefix: prefix.to_string_lossy().into(),
            location: PrefixLocation {
                path: prefix.to_string_lossy().into(),
                scope: PrefixScope::Shared,
                managed: true,
                server_id: None,
            },
            resolved,
            probe,
            identity,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.state.presence.shutdown();
        self.state.input.shutdown();
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}

async fn ready(lease: &UpdateLease) {
    let backend = MockBackend::available("0.2.0");
    lease.check(None, backend.clone()).await;
    assert!(matches!(
        lease.install(None, backend, || Ok(0)).await.phase,
        UpdatePhase::ReadyToRestart { .. }
    ));
}

#[tokio::test]
async fn install_failure_and_success_keep_launcher_usable_without_shutdown() {
    for succeeds in [false, true] {
        let fixture = Fixture::new();
        let lease = UpdateLease::new("0.1.0");
        let backend = MockBackend::available("0.2.0");
        backend.install_ok.store(succeeds, Ordering::SeqCst);
        let draft = fixture.state.installations.create(fixture.input()).unwrap();
        let context = fixture.context();
        let anchor = crate::tools::runtime::session_anchor_from_context(&context);
        let operation = fixture
            .state
            .sessions
            .begin_operation_opt(None, &context, &fixture.state.game, &anchor)
            .await
            .unwrap();
        let supervisor = operation.supervisor_identity();
        lease.check(None, backend.clone()).await;
        let game = fixture.state.game.clone();
        let census_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let calls = census_calls.clone();
        let result = lease
            .install(None, backend.clone(), move || {
                calls.fetch_add(1, Ordering::SeqCst);
                game.active_count()
            })
            .await;
        if succeeds {
            assert!(matches!(result.phase, UpdatePhase::ReadyToRestart { .. }));
        } else {
            assert!(matches!(
                result.phase,
                UpdatePhase::Failed {
                    class: UpdateFailureClass::Install,
                    ..
                }
            ));
        }
        assert_eq!(backend.install_calls.load(Ordering::SeqCst), 1);
        assert_eq!(census_calls.load(Ordering::SeqCst), 2);
        assert!(ro_tools_linux::verify_process_identity(&supervisor));
        let next = fixture
            .state
            .sessions
            .begin_operation_opt(None, &context, &fixture.state.game, &anchor)
            .await
            .unwrap();
        assert_eq!(next.supervisor_identity(), supervisor);
        let launch = fixture.launch().unwrap();
        fixture.state.game.cancel_launch(launch);
        fixture.state.installations.create(fixture.input()).unwrap();
        let (_, cancel, active) = fixture.state.installations.reserve(&draft.id).unwrap();
        assert!(!*cancel.borrow());
        drop(active);
        drop(next);
        drop(operation);
        assert!(fixture.state.sessions.shutdown_all().await.is_empty());
    }
}

#[tokio::test]
async fn relaunch_requires_ready_without_closing_admissions() {
    let fixture = Fixture::new();
    assert!(UpdateLease::new("0.1.0")
        .reserve_relaunch(&fixture.state.game, &fixture.state.installations)
        .await
        .is_err());
    fixture.launch().unwrap();
    fixture.state.installations.create(fixture.input()).unwrap();
}

#[tokio::test]
async fn active_game_refuses_relaunch_and_keeps_installations_open() {
    let fixture = Fixture::new();
    let lease = UpdateLease::new("0.1.0");
    ready(&lease).await;
    let launch = fixture.launch().unwrap();
    assert!(lease
        .reserve_relaunch(&fixture.state.game, &fixture.state.installations)
        .await
        .is_err());
    fixture.state.installations.create(fixture.input()).unwrap();
    fixture.state.game.cancel_launch(launch);
    fixture.launch().unwrap();
}

#[tokio::test]
async fn installation_refusal_rolls_back_game_admission_without_cancelling() {
    let fixture = Fixture::new();
    let lease = UpdateLease::new("0.1.0");
    ready(&lease).await;
    let draft = fixture.state.installations.create(fixture.input()).unwrap();
    let (_, cancel, active) = fixture.state.installations.reserve(&draft.id).unwrap();
    assert!(lease
        .reserve_relaunch(&fixture.state.game, &fixture.state.installations)
        .await
        .is_err());
    assert!(!*cancel.borrow());
    let launch = fixture.launch().unwrap();
    fixture.state.game.cancel_launch(launch);
    drop(active);
    drop(
        lease
            .reserve_relaunch(&fixture.state.game, &fixture.state.installations)
            .await
            .unwrap(),
    );
}

#[tokio::test]
async fn relaunch_reservation_blocks_both_admissions_and_return_rolls_back() {
    let fixture = Fixture::new();
    let lease = UpdateLease::new("0.1.0");
    ready(&lease).await;
    let draft = fixture.state.installations.create(fixture.input()).unwrap();
    let admission = lease
        .reserve_relaunch(&fixture.state.game, &fixture.state.installations)
        .await
        .unwrap();
    assert!(fixture.launch().is_err());
    assert!(fixture.state.installations.create(fixture.input()).is_err());
    assert!(fixture.state.installations.reserve(&draft.id).is_err());
    assert!(lease
        .reserve_relaunch(&fixture.state.game, &fixture.state.installations)
        .await
        .is_err());
    assert!(matches!(
        lease.snapshot().await.phase,
        UpdatePhase::ReadyToRestart { .. }
    ));
    // A substitute restart that returns/unwinds releases both guards.
    drop(admission);
    fixture.launch().unwrap();
    fixture.state.installations.create(fixture.input()).unwrap();
    let (_, _, active) = fixture.state.installations.reserve(&draft.id).unwrap();
    drop(active);
}

#[tokio::test]
async fn normal_exit_cancels_installations_closes_sessions_and_waits_for_workers() {
    let fixture = Fixture::new();
    let draft = fixture.state.installations.create(fixture.input()).unwrap();
    let (_, mut cancel, active) = fixture.state.installations.reserve(&draft.id).unwrap();
    let worker = tokio::spawn(async move {
        cancel.changed().await.unwrap();
        assert!(*cancel.borrow());
        drop(active);
    });
    let context = fixture.context();
    let anchor = crate::tools::runtime::session_anchor_from_context(&context);
    let operation = fixture
        .state
        .sessions
        .begin_operation_opt(None, &context, &fixture.state.game, &anchor)
        .await
        .unwrap();
    let supervisor = operation.supervisor_identity();
    shutdown_launcher_services(&fixture.state).await;
    worker.await.unwrap();
    assert!(!ro_tools_linux::verify_process_identity(&supervisor));
    assert!(fixture
        .state
        .sessions
        .begin_operation_opt(None, &context, &fixture.state.game, &anchor)
        .await
        .is_err());
    assert!(fixture.state.installations.create(fixture.input()).is_err());
    assert!(fixture.state.installations.reserve(&draft.id).is_err());
    drop(operation);
}
