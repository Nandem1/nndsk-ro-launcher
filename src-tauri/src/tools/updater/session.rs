#[cfg(test)]
use async_trait::async_trait;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

use crate::models::update::{
    UpdateFailureClass, UpdatePhase, UpdateSnapshot, UpdateUnavailableReason,
};
#[cfg(test)]
use crate::tools::updater::backend::BackendError;
use crate::tools::updater::backend::{CheckOutcome, UpdateBackend};
use crate::tools::updater::policy::{classify_backend_error, packaged_appimage, IdleClients};
use crate::utils::EVENT_UPDATE;

struct LeaseInner {
    current_version: String,
    phase: UpdatePhase,
}

#[derive(Clone)]
pub struct UpdateLease {
    inner: Arc<tokio::sync::Mutex<LeaseInner>>,
    op: Arc<tokio::sync::Mutex<()>>,
}

pub(crate) struct RelaunchAdmission {
    _game: crate::state::GameLaunchAdmission,
    _installations: crate::tools::installations::InstallationAdmission,
}

impl UpdateLease {
    pub fn new(current_version: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(LeaseInner {
                current_version: current_version.into(),
                phase: UpdatePhase::Idle,
            })),
            op: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub async fn snapshot(&self) -> UpdateSnapshot {
        let inner = self.inner.lock().await;
        UpdateSnapshot {
            current_version: inner.current_version.clone(),
            phase: inner.phase.clone(),
        }
    }

    async fn publish(&self, app: Option<&AppHandle>, snapshot: UpdateSnapshot) -> UpdateSnapshot {
        if let Some(app) = app {
            // Best effort: host state remains authoritative if no webview is listening.
            let _ = app.emit(EVENT_UPDATE, snapshot.clone());
        }
        snapshot
    }

    /// Admission and spawn have no intervening await. The task owns the guard through
    /// final publication; dropping its IPC waiter only detaches the JoinHandle.
    async fn run_owned<F, Fut>(&self, body: F) -> UpdateSnapshot
    where
        F: FnOnce(Self) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = UpdateSnapshot> + Send + 'static,
    {
        let Ok(guard) = self.op.clone().try_lock_owned() else {
            return self.snapshot().await;
        };
        let lease = self.clone();
        tauri::async_runtime::spawn(async move {
            let _guard = guard;
            body(lease).await
        })
        .await
        .expect("host updater operation panicked")
    }

    pub async fn check(
        &self,
        app: Option<AppHandle>,
        backend: Arc<dyn UpdateBackend>,
    ) -> UpdateSnapshot {
        self.run_owned(move |lease| async move {
            let packaging = app
                .as_ref()
                .map_or(Ok(()), |app| packaged_appimage(app).map(|_| ()));
            lease
                .check_owned(app.as_ref(), backend.as_ref(), packaging)
                .await
        })
        .await
    }

    async fn check_owned(
        &self,
        app: Option<&AppHandle>,
        backend: &dyn UpdateBackend,
        packaging: Result<(), UpdateUnavailableReason>,
    ) -> UpdateSnapshot {
        {
            let mut inner = self.inner.lock().await;
            if matches!(inner.phase, UpdatePhase::ReadyToRestart { .. }) {
                return UpdateSnapshot {
                    current_version: inner.current_version.clone(),
                    phase: inner.phase.clone(),
                };
            }
            // Invalidate before beginning another check, including early packaging/errors.
            backend.clear_pending();
            inner.phase = match packaging {
                Ok(()) => UpdatePhase::Checking,
                Err(reason) => UpdatePhase::Unavailable { reason },
            };
            let snapshot = UpdateSnapshot {
                current_version: inner.current_version.clone(),
                phase: inner.phase.clone(),
            };
            drop(inner);
            let snapshot = self.publish(app, snapshot).await;
            if packaging.is_err() {
                return snapshot;
            }
        }

        let outcome = backend.check().await;
        let snapshot = {
            let mut inner = self.inner.lock().await;
            match outcome {
                Ok(CheckOutcome::Current) => {
                    backend.clear_pending();
                    inner.phase = UpdatePhase::Current;
                }
                Ok(CheckOutcome::Available { release }) => {
                    inner.phase = UpdatePhase::Available { release };
                }
                Err(error) => {
                    backend.clear_pending();
                    let (class, message) = classify_backend_error(error);
                    inner.phase = UpdatePhase::Failed { class, message };
                }
            }
            UpdateSnapshot {
                current_version: inner.current_version.clone(),
                phase: inner.phase.clone(),
            }
        };
        self.publish(app, snapshot).await
    }

    /// Exercise the same packaging/error path without constructing an AppHandle.
    #[cfg(test)]
    async fn mark_unpackaged(&self, backend: Arc<dyn UpdateBackend>) -> UpdateSnapshot {
        self.run_owned(move |lease| async move {
            lease
                .check_owned(
                    None,
                    backend.as_ref(),
                    Err(UpdateUnavailableReason::NotPackaged),
                )
                .await
        })
        .await
    }

    pub async fn check_packaged(
        &self,
        app: &AppHandle,
        backend: Arc<dyn UpdateBackend>,
    ) -> UpdateSnapshot {
        self.check(Some(app.clone()), backend).await
    }

    pub async fn install(
        &self,
        app: Option<AppHandle>,
        backend: Arc<dyn UpdateBackend>,
        census: impl Fn() -> Result<usize, String> + Send + Sync + 'static,
    ) -> UpdateSnapshot {
        self.run_owned(move |lease| async move {
            lease
                .install_owned(app.as_ref(), backend.as_ref(), &census)
                .await
        })
        .await
    }

    async fn install_owned(
        &self,
        app: Option<&AppHandle>,
        backend: &dyn UpdateBackend,
        census: &(dyn Fn() -> Result<usize, String> + Send + Sync),
    ) -> UpdateSnapshot {
        {
            let mut inner = self.inner.lock().await;
            match &inner.phase {
                UpdatePhase::ReadyToRestart { .. } => {
                    return UpdateSnapshot {
                        current_version: inner.current_version.clone(),
                        phase: inner.phase.clone(),
                    }
                }
                UpdatePhase::Available { .. }
                | UpdatePhase::Failed {
                    class: UpdateFailureClass::ClientsActive,
                    ..
                } => {}
                _ => {
                    backend.clear_pending();
                    inner.phase = UpdatePhase::Failed {
                        class: UpdateFailureClass::NoPending,
                        message: "No hay una actualización comprobada para instalar".to_string(),
                    };
                    let snapshot = UpdateSnapshot {
                        current_version: inner.current_version.clone(),
                        phase: inner.phase.clone(),
                    };
                    drop(inner);
                    return self.publish(app, snapshot).await;
                }
            }
        }

        // ClientsActive is a precondition refusal: leave the checked candidate in its slot.
        let active = match census() {
            Ok(count) => count,
            Err(message) => {
                {
                    let _inner = self.inner.lock().await;
                    backend.clear_pending();
                }
                return self.fail(app, UpdateFailureClass::Install, message).await;
            }
        };
        if IdleClients::attest(active).is_err() {
            return self
                .fail(
                    app,
                    UpdateFailureClass::ClientsActive,
                    "Cierra los clientes del juego antes de instalar".to_string(),
                )
                .await;
        }

        // The admitted task alone owns the moved candidate through download and install.
        let candidate = match backend.take_pending() {
            Ok(candidate) => candidate,
            Err(error) => {
                let (class, message) = classify_backend_error(error);
                return self.fail(app, class, message).await;
            }
        };
        let release = candidate.release();

        {
            let mut inner = self.inner.lock().await;
            inner.phase = UpdatePhase::Downloading {
                release: release.clone(),
                downloaded_bytes: 0,
                content_length: None,
            };
            let snapshot = UpdateSnapshot {
                current_version: inner.current_version.clone(),
                phase: inner.phase.clone(),
            };
            drop(inner);
            self.publish(app, snapshot).await;
        }

        let current_version = {
            let inner = self.inner.lock().await;
            inner.current_version.clone()
        };
        let download_result = {
            let lease = self;
            let mut last_emit = 0u64;
            candidate
                .download(&mut |downloaded, total| {
                    let should_emit = downloaded == 0
                        || total.is_some_and(|t| downloaded >= t)
                        || downloaded.saturating_sub(last_emit) >= 256 * 1024;
                    if !should_emit {
                        return;
                    }
                    last_emit = downloaded;
                    let phase = UpdatePhase::Downloading {
                        release: release.clone(),
                        downloaded_bytes: downloaded,
                        content_length: total,
                    };
                    if let Ok(mut inner) = lease.inner.try_lock() {
                        inner.phase = phase.clone();
                    }
                    if let Some(app_handle) = app {
                        // Best effort: progress events do not own updater state.
                        let _ = app_handle.emit(
                            EVENT_UPDATE,
                            UpdateSnapshot {
                                current_version: current_version.clone(),
                                phase,
                            },
                        );
                    }
                })
                .await
        };

        let bytes = match download_result {
            Ok(bytes) => bytes,
            Err(error) => {
                let (class, message) = classify_backend_error(error);
                return self.fail(app, class, message).await;
            }
        };

        let active_after = match census() {
            Ok(count) => count,
            Err(message) => {
                return self.fail(app, UpdateFailureClass::Install, message).await;
            }
        };
        if IdleClients::attest(active_after).is_err() {
            let snapshot = {
                let mut inner = self.inner.lock().await;
                // Consumes the sole owner before publishing refusal, with the op gate held.
                let (class, message) = match candidate.return_to_slot() {
                    Ok(()) => (
                        UpdateFailureClass::ClientsActive,
                        "Cierra los clientes del juego antes de instalar".to_string(),
                    ),
                    Err(error) => classify_backend_error(error),
                };
                inner.phase = UpdatePhase::Failed { class, message };
                UpdateSnapshot {
                    current_version: inner.current_version.clone(),
                    phase: inner.phase.clone(),
                }
            };
            return self.publish(app, snapshot).await;
        }

        if let Err(error) = candidate.install(&bytes) {
            let (class, message) = classify_backend_error(error);
            return self.fail(app, class, message).await;
        }

        let installed_version = release.version;
        let snapshot = {
            let mut inner = self.inner.lock().await;
            backend.clear_pending();
            inner.phase = UpdatePhase::ReadyToRestart { installed_version };
            UpdateSnapshot {
                current_version: inner.current_version.clone(),
                phase: inner.phase.clone(),
            }
        };
        self.publish(app, snapshot).await
    }

    async fn fail(
        &self,
        app: Option<&AppHandle>,
        class: UpdateFailureClass,
        message: String,
    ) -> UpdateSnapshot {
        let mut inner = self.inner.lock().await;
        inner.phase = UpdatePhase::Failed { class, message };
        let snapshot = UpdateSnapshot {
            current_version: inner.current_version.clone(),
            phase: inner.phase.clone(),
        };
        drop(inner);
        self.publish(app, snapshot).await
    }

    pub(crate) async fn reserve_relaunch(
        &self,
        game: &crate::state::GameProcessHandle,
        installations: &crate::tools::installations::InstallationManager,
    ) -> Result<RelaunchAdmission, String> {
        if !matches!(
            self.inner.lock().await.phase,
            UpdatePhase::ReadyToRestart { .. }
        ) {
            return Err("No hay una actualización instalada para reiniciar".into());
        }
        // No await between closures: each owner checks idle and excludes admission
        // under its own mutex. If installations refuse, Drop reopens game admission.
        let game = game.close_admission_if_idle()?;
        let installations = installations.close_admission_if_idle()?;
        Ok(RelaunchAdmission {
            _game: game,
            _installations: installations,
        })
    }

    pub async fn relaunch(
        &self,
        app: &AppHandle,
        game: &crate::state::GameProcessHandle,
        installations: &crate::tools::installations::InstallationManager,
    ) -> Result<(), String> {
        let _admission = self.reserve_relaunch(game, installations).await?;
        // Tauri 2.11.2 AppHandle::restart -> !. This async command runs off the
        // event-loop thread: request_exit delivers RunEvent::Exit for actual cleanup.
        // Keep both guards alive until exit; unwinding drops them and rolls back.
        app.restart();
    }
}

/// Production packaging gate used by IPC: unpackaged never hits the backend.
pub async fn check_for_packaged_update(
    lease: &UpdateLease,
    app: &AppHandle,
    backend: Arc<dyn UpdateBackend>,
) -> UpdateSnapshot {
    lease.check_packaged(app, backend).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::updater::backend::mock::MockBackend;
    use crate::tools::updater::backend::mock::{MockCheck, MockDownload};
    use crate::tools::updater::backend::UpdateCandidate;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    #[derive(Default)]
    struct Pause {
        enabled: AtomicBool,
        entered: tokio::sync::Notify,
        resume: tokio::sync::Notify,
    }

    impl Pause {
        fn arm(&self) {
            self.enabled.store(true, Ordering::SeqCst);
        }

        async fn wait(&self) {
            if self.enabled.swap(false, Ordering::SeqCst) {
                self.entered.notify_one();
                self.resume.notified().await;
            }
        }
    }

    struct PausedBackend {
        mock: Arc<MockBackend>,
        check_calls: AtomicUsize,
        checking: Arc<Pause>,
        downloading: Arc<Pause>,
    }

    impl PausedBackend {
        fn available() -> Arc<Self> {
            Arc::new(Self {
                mock: MockBackend::available("0.2.0"),
                check_calls: AtomicUsize::new(0),
                checking: Arc::new(Pause::default()),
                downloading: Arc::new(Pause::default()),
            })
        }
    }

    #[async_trait]
    impl UpdateBackend for PausedBackend {
        async fn check(&self) -> Result<CheckOutcome, BackendError> {
            self.check_calls.fetch_add(1, Ordering::SeqCst);
            self.checking.wait().await;
            self.mock.check().await
        }

        fn take_pending(&self) -> Result<Box<dyn UpdateCandidate>, BackendError> {
            Ok(Box::new(PausedCandidate {
                candidate: self.mock.take_pending()?,
                downloading: self.downloading.clone(),
            }))
        }

        fn clear_pending(&self) {
            self.mock.clear_pending();
        }
    }

    struct PausedCandidate {
        candidate: Box<dyn UpdateCandidate>,
        downloading: Arc<Pause>,
    }

    #[async_trait]
    impl UpdateCandidate for PausedCandidate {
        fn release(&self) -> crate::models::update::ReleaseSummary {
            self.candidate.release()
        }

        fn return_to_slot(self: Box<Self>) -> Result<(), BackendError> {
            self.candidate.return_to_slot()
        }

        async fn download(
            &self,
            on_chunk: &mut (dyn FnMut(u64, Option<u64>) + Send),
        ) -> Result<Vec<u8>, BackendError> {
            let result = self.candidate.download(on_chunk).await;
            self.downloading.wait().await;
            result
        }

        fn install(self: Box<Self>, bytes: &[u8]) -> Result<(), BackendError> {
            self.candidate.install(bytes)
        }
    }

    fn poll_pending(future: std::pin::Pin<&mut impl std::future::Future>) {
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(future.poll(&mut context).is_pending());
    }

    async fn terminal_snapshot(lease: &UpdateLease) -> UpdateSnapshot {
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                let snapshot = lease.snapshot().await;
                if !matches!(
                    snapshot.phase,
                    UpdatePhase::Checking | UpdatePhase::Downloading { .. }
                ) {
                    return snapshot;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("dropped waiter stranded updater operation")
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn regression_check_check_admission_is_atomic() {
        let lease = Arc::new(UpdateLease::new("0.1.0"));
        let backend = PausedBackend::available();
        backend.checking.arm();
        let mut first = Box::pin({
            let lease = lease.clone();
            let backend = backend.clone();
            async move { lease.check(None, backend).await }
        });
        let mut second = Box::pin({
            let lease = lease.clone();
            let backend = backend.clone();
            async move { lease.check(None, backend).await }
        });
        // FIFO state-lock contention forces both old admission reads ahead of either write.
        let state = lease.inner.lock().await;
        poll_pending(first.as_mut());
        poll_pending(second.as_mut());
        drop(state);
        let pair = tokio::spawn(async move { tokio::join!(first, second) });
        backend.checking.entered.notified().await;
        backend.checking.resume.notify_one();
        let _ = pair.await.unwrap();
        assert_eq!(backend.mock.check_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn regression_check_check_while_backend_paused_coalesces() {
        let lease = Arc::new(UpdateLease::new("0.1.0"));
        let backend = PausedBackend::available();
        backend.checking.arm();
        let first = tokio::spawn({
            let lease = lease.clone();
            let backend = backend.clone();
            async move { lease.check(None, backend).await }
        });
        backend.checking.entered.notified().await;
        let second = lease.check(None, backend.clone()).await;
        assert_eq!(backend.check_calls.load(Ordering::SeqCst), 1);
        assert_eq!(second.phase, UpdatePhase::Checking);
        assert_eq!(lease.snapshot().await, second);
        backend.checking.resume.notify_one();
        let _ = first.await.unwrap();
        assert_eq!(backend.mock.check_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn regression_install_during_recheck_keeps_check_owner() {
        let lease = Arc::new(UpdateLease::new("0.1.0"));
        let backend = PausedBackend::available();
        lease.check(None, backend.clone()).await;
        backend.checking.arm();
        let first = tokio::spawn({
            let lease = lease.clone();
            let backend = backend.clone();
            async move { lease.check(None, backend).await }
        });
        backend.checking.entered.notified().await;
        assert!(backend.mock.pending_version().is_none());

        let second = lease.install(None, backend.clone(), census(0)).await;
        assert_eq!(lease.snapshot().await.phase, UpdatePhase::Checking);
        backend.checking.resume.notify_one();
        let _ = first.await.unwrap();
        assert_eq!(second.phase, UpdatePhase::Checking);
        assert_eq!(backend.mock.download_calls.load(Ordering::SeqCst), 0);
        assert_eq!(backend.mock.install_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn regression_no_pending_during_check_cannot_publish_failed() {
        let lease = UpdateLease::new("0.1.0");
        let backend = PausedBackend::available();
        backend.checking.arm();
        let first = tokio::spawn({
            let lease = lease.clone();
            let backend = backend.clone();
            async move { lease.check(None, backend).await }
        });
        backend.checking.entered.notified().await;

        let overlap = lease.install(None, backend.clone(), census(0)).await;
        assert_eq!(overlap.phase, UpdatePhase::Checking);
        assert_eq!(lease.snapshot().await, overlap);
        backend.checking.resume.notify_one();
        assert!(matches!(
            first.await.unwrap().phase,
            UpdatePhase::Available { .. }
        ));
        assert_eq!(backend.mock.take_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn regression_current_and_error_rechecks_release_owner_and_clear_slot() {
        for outcome in [
            MockCheck::Current,
            MockCheck::Error(BackendError::Unreachable("offline".into())),
        ] {
            let lease = UpdateLease::new("0.1.0");
            let backend = PausedBackend::available();
            lease.check(None, backend.clone()).await;
            *backend.mock.check.lock().unwrap() = outcome.clone();
            backend.checking.arm();
            let first = tokio::spawn({
                let lease = lease.clone();
                let backend = backend.clone();
                async move { lease.check(None, backend).await }
            });
            backend.checking.entered.notified().await;
            assert!(backend.mock.pending_version().is_none());
            let overlap = lease.check(None, backend.clone()).await;
            assert_eq!(overlap.phase, UpdatePhase::Checking);
            assert_eq!(backend.check_calls.load(Ordering::SeqCst), 2);
            backend.checking.resume.notify_one();
            let finished = first.await.unwrap();
            match outcome {
                MockCheck::Current => assert_eq!(finished.phase, UpdatePhase::Current),
                _ => assert!(matches!(
                    finished.phase,
                    UpdatePhase::Failed {
                        class: UpdateFailureClass::Unreachable,
                        ..
                    }
                )),
            }
            assert!(backend.mock.pending_version().is_none());
            *backend.mock.check.lock().unwrap() = MockCheck::Available {
                version: "0.3.0".into(),
                notes: None,
            };
            assert!(
                matches!(lease.check(None, backend.clone()).await.phase, UpdatePhase::Available { release } if release.version == "0.3.0")
            );
            assert_eq!(backend.mock.pending_version().as_deref(), Some("0.3.0"));
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn regression_install_install_admission_is_atomic() {
        let lease = Arc::new(UpdateLease::new("0.1.0"));
        let backend = PausedBackend::available();
        lease.check(None, backend.clone()).await;
        backend.downloading.arm();
        let mut first = Box::pin({
            let lease = lease.clone();
            let backend = backend.clone();
            async move { lease.install(None, backend, census(0)).await }
        });
        let mut second = Box::pin({
            let lease = lease.clone();
            let backend = backend.clone();
            async move { lease.install(None, backend, census(0)).await }
        });
        let state = lease.inner.lock().await;
        poll_pending(first.as_mut());
        poll_pending(second.as_mut());
        drop(state);
        let pair = tokio::spawn(async move { tokio::join!(first, second) });
        backend.downloading.entered.notified().await;
        backend.downloading.resume.notify_one();
        let _ = pair.await.unwrap();
        assert_eq!(backend.mock.download_calls.load(Ordering::SeqCst), 1);
        assert_eq!(backend.mock.install_calls.load(Ordering::SeqCst), 1);
        assert_eq!(backend.mock.take_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn regression_check_and_install_during_download_coalesce() {
        let lease = UpdateLease::new("0.1.0");
        let backend = PausedBackend::available();
        lease.check(None, backend.clone()).await;
        backend.downloading.arm();

        let first = tokio::spawn({
            let lease = lease.clone();
            let backend = backend.clone();
            async move { lease.install(None, backend, census(0)).await }
        });
        backend.downloading.entered.notified().await;
        let downloading = lease.snapshot().await;
        assert!(matches!(
            downloading.phase,
            UpdatePhase::Downloading {
                downloaded_bytes: 8,
                content_length: Some(8),
                ..
            }
        ));
        assert_eq!(lease.check(None, backend.clone()).await, downloading);
        assert_eq!(
            lease.install(None, backend.clone(), census(0)).await,
            downloading
        );
        assert_eq!(backend.check_calls.load(Ordering::SeqCst), 1);
        assert_eq!(backend.mock.take_calls.load(Ordering::SeqCst), 1);
        assert_eq!(backend.mock.download_calls.load(Ordering::SeqCst), 1);
        assert_eq!(backend.mock.install_calls.load(Ordering::SeqCst), 0);
        assert!(backend.mock.pending_version().is_none());
        backend.downloading.resume.notify_one();
        assert!(matches!(
            first.await.unwrap().phase,
            UpdatePhase::ReadyToRestart { .. }
        ));
        assert_eq!(backend.mock.install_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn regression_ready_to_restart_check_is_terminal() {
        let lease = UpdateLease::new("0.1.0");
        let backend = MockBackend::available("0.2.0");
        lease.check(None, backend.clone()).await;

        let installed = lease.install(None, backend.clone(), census(0)).await;
        let checked = lease.check(None, backend.clone()).await;
        assert_eq!(checked, installed);
        assert_eq!(lease.mark_unpackaged(backend.clone()).await, installed);
        assert_eq!(backend.check_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn regression_dropped_check_waiter_completes() {
        let lease = Arc::new(UpdateLease::new("0.1.0"));
        let backend = PausedBackend::available();
        backend.checking.arm();
        let waiter = tokio::spawn({
            let lease = lease.clone();
            let backend = backend.clone();
            async move { lease.check(None, backend).await }
        });
        backend.checking.entered.notified().await;
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        backend.checking.resume.notify_one();
        assert!(matches!(
            terminal_snapshot(&lease).await.phase,
            UpdatePhase::Available { .. }
        ));
        let gate = lease.op.lock().await;
        drop(gate);
        lease.check(None, backend.clone()).await;
        assert_eq!(backend.check_calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn regression_dropped_install_waiter_completes() {
        let lease = Arc::new(UpdateLease::new("0.1.0"));
        let backend = PausedBackend::available();
        lease.check(None, backend.clone()).await;
        backend.downloading.arm();
        let waiter = tokio::spawn({
            let lease = lease.clone();
            let backend = backend.clone();
            async move { lease.install(None, backend, census(0)).await }
        });
        backend.downloading.entered.notified().await;
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        backend.downloading.resume.notify_one();
        assert!(matches!(
            terminal_snapshot(&lease).await.phase,
            UpdatePhase::ReadyToRestart { .. }
        ));
        let gate = lease.op.lock().await;
        drop(gate);
        assert_eq!(backend.mock.download_calls.load(Ordering::SeqCst), 1);
        assert_eq!(backend.mock.install_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn regression_dropped_download_error_waiter_releases_gate() {
        let lease = UpdateLease::new("0.1.0");
        let backend = PausedBackend::available();
        lease.check(None, backend.clone()).await;
        *backend.mock.download.lock().unwrap() =
            MockDownload::Error(BackendError::Unreachable("offline".into()));
        backend.downloading.arm();
        let waiter = tokio::spawn({
            let lease = lease.clone();
            let backend = backend.clone();
            async move { lease.install(None, backend, census(0)).await }
        });
        backend.downloading.entered.notified().await;
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        backend.downloading.resume.notify_one();
        assert!(matches!(
            terminal_snapshot(&lease).await.phase,
            UpdatePhase::Failed {
                class: UpdateFailureClass::Unreachable,
                ..
            }
        ));
        let gate = lease.op.lock().await;
        drop(gate);
        assert!(backend.mock.pending_version().is_none());
        assert_eq!(backend.mock.install_calls.load(Ordering::SeqCst), 0);
        assert!(matches!(
            lease.check(None, backend.clone()).await.phase,
            UpdatePhase::Available { .. }
        ));
    }

    fn census(count: usize) -> impl Fn() -> Result<usize, String> {
        move || Ok(count)
    }

    #[tokio::test]
    async fn install_without_pending_is_no_pending() {
        let lease = UpdateLease::new("0.1.0");
        let backend = MockBackend::available("0.2.0");

        let snapshot = lease.install(None, backend.clone(), census(0)).await;
        assert!(matches!(
            snapshot.phase,
            UpdatePhase::Failed {
                class: UpdateFailureClass::NoPending,
                ..
            }
        ));
        assert_eq!(backend.install_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn unauthentic_check_drops_pending_and_never_installs() {
        let lease = UpdateLease::new("0.1.0");
        let backend = MockBackend::unauthentic_check();
        let snapshot = lease.check(None, backend.clone()).await;
        assert!(matches!(
            snapshot.phase,
            UpdatePhase::Failed {
                class: UpdateFailureClass::Unauthentic,
                ..
            }
        ));

        let install = lease.install(None, backend.clone(), census(0)).await;
        assert!(matches!(
            install.phase,
            UpdatePhase::Failed {
                class: UpdateFailureClass::NoPending,
                ..
            }
        ));
        assert_eq!(backend.install_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn second_check_replaces_previous_candidate() {
        let lease = UpdateLease::new("0.1.0");
        let backend = MockBackend::available("0.2.0");
        let first = lease.check(None, backend.clone()).await;
        assert!(matches!(first.phase, UpdatePhase::Available { .. }));

        *backend.check.lock().unwrap() =
            crate::tools::updater::backend::mock::MockCheck::Available {
                version: "0.3.0".to_string(),
                notes: None,
            };
        let second = lease.check(None, backend.clone()).await;
        assert!(matches!(
            second.phase,
            UpdatePhase::Available { release } if release.version == "0.3.0"
        ));

        lease.install(None, backend.clone(), census(0)).await;
        assert_eq!(*backend.installed_versions.lock().unwrap(), ["0.3.0"]);
        assert!(backend.take_pending().is_err());
    }

    #[tokio::test]
    async fn clients_active_before_download_retains_candidate_for_direct_retry() {
        let lease = UpdateLease::new("0.1.0");
        let backend = MockBackend::available("0.2.0");
        let _ = lease.check(None, backend.clone()).await;

        let before = lease.install(None, backend.clone(), census(2)).await;
        assert!(matches!(
            before.phase,
            UpdatePhase::Failed {
                class: UpdateFailureClass::ClientsActive,
                ..
            }
        ));
        assert_eq!(backend.pending_version().as_deref(), Some("0.2.0"));
        assert_eq!(backend.take_calls.load(Ordering::SeqCst), 0);
        assert_eq!(backend.download_calls.load(Ordering::SeqCst), 0);
        assert_eq!(backend.install_calls.load(Ordering::SeqCst), 0);

        // Repeated refusals still leave the same checked candidate in its slot.
        let refused_again = lease.install(None, backend.clone(), census(1)).await;
        assert_eq!(refused_again, before);
        assert_eq!(backend.take_calls.load(Ordering::SeqCst), 0);

        let retry = lease.install(None, backend.clone(), census(0)).await;
        assert!(matches!(
            retry.phase,
            UpdatePhase::ReadyToRestart { ref installed_version } if installed_version == "0.2.0"
        ));
        assert!(backend.pending_version().is_none());
        assert_eq!(backend.check_calls.load(Ordering::SeqCst), 1);
        assert_eq!(backend.take_calls.load(Ordering::SeqCst), 1);
        assert_eq!(backend.download_calls.load(Ordering::SeqCst), 1);
        assert_eq!(*backend.installed_versions.lock().unwrap(), ["0.2.0"]);
        assert_eq!(lease.install(None, backend.clone(), census(0)).await, retry);
        assert_eq!(backend.install_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn clients_active_after_download_restores_candidate_for_direct_retry() {
        let lease = UpdateLease::new("0.1.0");
        let backend = PausedBackend::available();
        lease.check(None, backend.clone()).await;
        backend.downloading.arm();

        let active = Arc::new(AtomicUsize::new(0));
        let waiter = tokio::spawn({
            let lease = lease.clone();
            let backend = backend.clone();
            let active = active.clone();
            async move {
                lease
                    .install(None, backend, move || Ok(active.load(Ordering::SeqCst)))
                    .await
            }
        });
        backend.downloading.entered.notified().await;
        assert!(backend.mock.pending_version().is_none());
        assert_eq!(backend.mock.download_calls.load(Ordering::SeqCst), 1);
        active.store(1, Ordering::SeqCst);
        backend.downloading.resume.notify_one();
        let after = waiter.await.unwrap();
        assert!(matches!(
            after.phase,
            UpdatePhase::Failed {
                class: UpdateFailureClass::ClientsActive,
                ..
            }
        ));
        assert_eq!(backend.mock.pending_version().as_deref(), Some("0.2.0"));
        assert_eq!(backend.mock.install_calls.load(Ordering::SeqCst), 0);

        active.store(0, Ordering::SeqCst);
        backend.downloading.arm();
        let retrying = tokio::spawn({
            let lease = lease.clone();
            let backend = backend.clone();
            async move { lease.install(None, backend, census(0)).await }
        });
        backend.downloading.entered.notified().await;
        let downloading = lease.snapshot().await;
        assert!(
            matches!(&downloading.phase, UpdatePhase::Downloading { release, .. }
            if release.version == "0.2.0" && release.notes.as_deref() == Some("notas"))
        );
        assert_eq!(lease.check(None, backend.clone()).await, downloading);
        assert_eq!(
            lease.install(None, backend.clone(), census(0)).await,
            downloading
        );
        assert!(backend.mock.pending_version().is_none());
        assert_eq!(backend.mock.take_calls.load(Ordering::SeqCst), 2);
        assert_eq!(backend.mock.install_calls.load(Ordering::SeqCst), 0);
        backend.downloading.resume.notify_one();
        let retry = retrying.await.unwrap();
        assert!(matches!(
            retry.phase,
            UpdatePhase::ReadyToRestart {
                installed_version: ref v
            } if v == "0.2.0"
        ));
        assert!(backend.mock.pending_version().is_none());
        assert_eq!(backend.mock.check_calls.load(Ordering::SeqCst), 1);
        assert_eq!(backend.mock.take_calls.load(Ordering::SeqCst), 2);
        // The returned candidate downloads/verifies again; no filesystem install preceded retry.
        assert_eq!(backend.mock.download_calls.load(Ordering::SeqCst), 2);
        assert_eq!(*backend.mock.installed_versions.lock().unwrap(), ["0.2.0"]);
        assert_eq!(lease.install(None, backend.clone(), census(0)).await, retry);
        assert_eq!(backend.mock.install_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn check_after_clients_active_invalidates_retained_candidate() {
        let lease = UpdateLease::new("0.1.0");
        let backend = PausedBackend::available();
        lease.check(None, backend.clone()).await;

        lease.install(None, backend.clone(), census(1)).await;
        assert_eq!(backend.mock.pending_version().as_deref(), Some("0.2.0"));
        *backend.mock.check.lock().unwrap() = MockCheck::Available {
            version: "0.3.0".into(),
            notes: None,
        };
        backend.checking.arm();
        let checking = tokio::spawn({
            let lease = lease.clone();
            let backend = backend.clone();
            async move { lease.check(None, backend).await }
        });
        backend.checking.entered.notified().await;
        assert!(backend.mock.pending_version().is_none());
        assert_eq!(
            lease.install(None, backend.clone(), census(0)).await.phase,
            UpdatePhase::Checking
        );
        assert_eq!(backend.mock.download_calls.load(Ordering::SeqCst), 0);
        backend.checking.resume.notify_one();
        assert!(matches!(
            checking.await.unwrap().phase,
            UpdatePhase::Available { release } if release.version == "0.3.0"
        ));
        assert_eq!(backend.mock.pending_version().as_deref(), Some("0.3.0"));
        lease.install(None, backend.clone(), census(0)).await;
        assert_eq!(*backend.mock.installed_versions.lock().unwrap(), ["0.3.0"]);
    }

    #[tokio::test]
    async fn other_failed_classes_require_new_check_without_backend_work() {
        for class in [
            UpdateFailureClass::NoPending,
            UpdateFailureClass::Install,
            UpdateFailureClass::Unauthentic,
            UpdateFailureClass::Unreachable,
        ] {
            let lease = UpdateLease::new("0.1.0");
            let backend = MockBackend::available("0.2.0");
            lease.check(None, backend.clone()).await;
            // Even a stale occupied slot cannot authorize these phases.
            lease.inner.lock().await.phase = UpdatePhase::Failed {
                class,
                message: "previous failure".into(),
            };

            let result = lease.install(None, backend.clone(), census(0)).await;
            assert!(
                matches!(
                    result.phase,
                    UpdatePhase::Failed {
                        class: UpdateFailureClass::NoPending,
                        ..
                    }
                ),
                "unexpected admission from {class:?}"
            );
            assert!(backend.pending_version().is_none());
            assert_eq!(backend.take_calls.load(Ordering::SeqCst), 0);
            assert_eq!(backend.download_calls.load(Ordering::SeqCst), 0);
            assert_eq!(backend.install_calls.load(Ordering::SeqCst), 0);
        }
    }

    #[tokio::test]
    async fn unauthentic_download_never_installs() {
        let lease = UpdateLease::new("0.1.0");
        let backend = MockBackend::unauthentic_download("0.2.0");
        let _ = lease.check(None, backend.clone()).await;

        let snapshot = lease.install(None, backend.clone(), census(0)).await;
        assert!(matches!(
            snapshot.phase,
            UpdatePhase::Failed {
                class: UpdateFailureClass::Unauthentic,
                ..
            }
        ));
        assert_eq!(backend.install_calls.load(Ordering::SeqCst), 0);
        assert!(backend.take_pending().is_err());
    }

    #[tokio::test]
    async fn ready_to_relaunch_install_is_idempotent() {
        let lease = UpdateLease::new("0.1.0");
        let backend = MockBackend::available("0.2.0");
        let _ = lease.check(None, backend.clone()).await;

        let first = lease.install(None, backend.clone(), census(0)).await;
        assert!(matches!(
            first.phase,
            UpdatePhase::ReadyToRestart {
                installed_version: ref v
            } if v == "0.2.0"
        ));
        let installs = backend.install_calls.load(Ordering::SeqCst);
        let second = lease.install(None, backend.clone(), census(0)).await;
        assert_eq!(first, second);
        assert_eq!(backend.install_calls.load(Ordering::SeqCst), installs);
    }

    #[tokio::test]
    async fn unpackaged_app_does_not_hit_backend() {
        let lease = UpdateLease::new("0.1.0");
        let backend = MockBackend::available("0.2.0");
        let snapshot = lease.mark_unpackaged(backend.clone()).await;
        assert!(matches!(
            snapshot.phase,
            UpdatePhase::Unavailable {
                reason: UpdateUnavailableReason::NotPackaged
            }
        ));
        assert_eq!(backend.check_calls.load(Ordering::SeqCst), 0);
    }
}
