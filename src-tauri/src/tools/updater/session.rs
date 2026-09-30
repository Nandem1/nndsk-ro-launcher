use async_trait::async_trait;
use tauri::{AppHandle, Emitter};

#[cfg(test)]
use crate::models::update::UpdateUnavailableReason;
use crate::models::update::{UpdateFailureClass, UpdatePhase, UpdateSnapshot};
#[cfg(test)]
use crate::tools::updater::backend::BackendError;
use crate::tools::updater::backend::{CheckOutcome, UpdateBackend};
use crate::tools::updater::policy::{classify_backend_error, packaged_appimage, IdleClients};
use crate::utils::EVENT_UPDATE;

/// Host-only pending candidate. Artifact bytes live in the backend slot keyed by generation.
struct HostPending {
    generation: u64,
    release: crate::models::update::ReleaseSummary,
}

struct LeaseInner {
    current_version: String,
    phase: UpdatePhase,
    generation: u64,
    pending: Option<HostPending>,
}

pub struct UpdateLease {
    inner: tokio::sync::Mutex<LeaseInner>,
}

#[async_trait]
pub trait ShutdownForUpdate: Send + Sync {
    async fn shutdown_for_update(&self) -> Result<(), String>;
}

impl UpdateLease {
    pub fn new(current_version: impl Into<String>) -> Self {
        Self {
            inner: tokio::sync::Mutex::new(LeaseInner {
                current_version: current_version.into(),
                phase: UpdatePhase::Idle,
                generation: 0,
                pending: None,
            }),
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
            let _ = app.emit(EVENT_UPDATE, snapshot.clone());
        }
        snapshot
    }

    pub async fn check(
        &self,
        app: Option<&AppHandle>,
        backend: &dyn UpdateBackend,
    ) -> UpdateSnapshot {
        {
            let inner = self.inner.lock().await;
            match &inner.phase {
                UpdatePhase::Checking | UpdatePhase::Downloading { .. } => {
                    return UpdateSnapshot {
                        current_version: inner.current_version.clone(),
                        phase: inner.phase.clone(),
                    };
                }
                _ => {}
            }
        }

        {
            let mut inner = self.inner.lock().await;
            inner.phase = UpdatePhase::Checking;
            let snapshot = UpdateSnapshot {
                current_version: inner.current_version.clone(),
                phase: inner.phase.clone(),
            };
            drop(inner);
            let _ = self.publish(app, snapshot).await;
        }

        let generation = {
            let mut inner = self.inner.lock().await;
            inner.generation = inner.generation.saturating_add(1);
            inner.generation
        };

        let outcome = backend.check(generation).await;
        let snapshot = {
            let mut inner = self.inner.lock().await;
            match outcome {
                Ok(CheckOutcome::Current) => {
                    inner.pending = None;
                    backend.clear_pending();
                    inner.phase = UpdatePhase::Current;
                }
                Ok(CheckOutcome::Available { release }) => {
                    inner.pending = Some(HostPending {
                        generation,
                        release: release.clone(),
                    });
                    inner.phase = UpdatePhase::Available { release };
                }
                Err(error) => {
                    inner.pending = None;
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

    /// Test helper: force NotPackaged without constructing an AppHandle.
    #[cfg(test)]
    pub async fn mark_unpackaged(&self, app: Option<&AppHandle>) -> UpdateSnapshot {
        let snapshot = {
            let mut inner = self.inner.lock().await;
            inner.pending = None;
            inner.phase = UpdatePhase::Unavailable {
                reason: UpdateUnavailableReason::NotPackaged,
            };
            UpdateSnapshot {
                current_version: inner.current_version.clone(),
                phase: inner.phase.clone(),
            }
        };
        self.publish(app, snapshot).await
    }

    pub async fn check_packaged(
        &self,
        app: &AppHandle,
        backend: &dyn UpdateBackend,
    ) -> UpdateSnapshot {
        if let Err(reason) = packaged_appimage(app) {
            let snapshot = {
                let mut inner = self.inner.lock().await;
                inner.pending = None;
                backend.clear_pending();
                inner.phase = UpdatePhase::Unavailable { reason };
                UpdateSnapshot {
                    current_version: inner.current_version.clone(),
                    phase: inner.phase.clone(),
                }
            };
            return self.publish(Some(app), snapshot).await;
        }
        self.check(Some(app), backend).await
    }

    pub async fn install(
        &self,
        app: Option<&AppHandle>,
        backend: &dyn UpdateBackend,
        census: &(dyn Fn() -> Result<usize, String> + Send + Sync),
        shutdown: &dyn ShutdownForUpdate,
    ) -> UpdateSnapshot {
        {
            let inner = self.inner.lock().await;
            match &inner.phase {
                UpdatePhase::Downloading { .. } => {
                    return UpdateSnapshot {
                        current_version: inner.current_version.clone(),
                        phase: inner.phase.clone(),
                    };
                }
                UpdatePhase::ReadyToRestart { .. } => {
                    return UpdateSnapshot {
                        current_version: inner.current_version.clone(),
                        phase: inner.phase.clone(),
                    };
                }
                _ => {}
            }
        }

        let pending = {
            let inner = self.inner.lock().await;
            match &inner.pending {
                Some(pending) => pending.clone_fields(),
                None => {
                    let snapshot = UpdateSnapshot {
                        current_version: inner.current_version.clone(),
                        phase: UpdatePhase::Failed {
                            class: UpdateFailureClass::NoPending,
                            message: "No hay una actualización comprobada para instalar"
                                .to_string(),
                        },
                    };
                    drop(inner);
                    {
                        let mut inner = self.inner.lock().await;
                        inner.phase = snapshot.phase.clone();
                    }
                    return self.publish(app, snapshot).await;
                }
            }
        };

        let active = match census() {
            Ok(count) => count,
            Err(message) => {
                let snapshot = {
                    let mut inner = self.inner.lock().await;
                    inner.phase = UpdatePhase::Failed {
                        class: UpdateFailureClass::Install,
                        message,
                    };
                    UpdateSnapshot {
                        current_version: inner.current_version.clone(),
                        phase: inner.phase.clone(),
                    }
                };
                return self.publish(app, snapshot).await;
            }
        };
        if IdleClients::attest(active).is_err() {
            let snapshot = {
                let mut inner = self.inner.lock().await;
                inner.phase = UpdatePhase::Failed {
                    class: UpdateFailureClass::ClientsActive,
                    message: "Cierra los clientes del juego antes de instalar".to_string(),
                };
                UpdateSnapshot {
                    current_version: inner.current_version.clone(),
                    phase: inner.phase.clone(),
                }
            };
            return self.publish(app, snapshot).await;
        }

        {
            let mut inner = self.inner.lock().await;
            inner.phase = UpdatePhase::Downloading {
                release: pending.release.clone(),
                downloaded_bytes: 0,
                content_length: None,
            };
            let snapshot = UpdateSnapshot {
                current_version: inner.current_version.clone(),
                phase: inner.phase.clone(),
            };
            drop(inner);
            let _ = self.publish(app, snapshot).await;
        }

        let current_version = {
            let inner = self.inner.lock().await;
            inner.current_version.clone()
        };
        let download_result = {
            let lease = self;
            let mut last_emit = 0u64;
            backend
                .download(pending.generation, &mut |downloaded, total| {
                    let should_emit = downloaded == 0
                        || total.is_some_and(|t| downloaded >= t)
                        || downloaded.saturating_sub(last_emit) >= 256 * 1024;
                    if !should_emit {
                        return;
                    }
                    last_emit = downloaded;
                    let phase = UpdatePhase::Downloading {
                        release: pending.release.clone(),
                        downloaded_bytes: downloaded,
                        content_length: total,
                    };
                    if let Ok(mut inner) = lease.inner.try_lock() {
                        inner.phase = phase.clone();
                    }
                    if let Some(app_handle) = app {
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
                if matches!(class, UpdateFailureClass::Unauthentic) {
                    backend.clear_pending();
                }
                let snapshot = {
                    let mut inner = self.inner.lock().await;
                    if matches!(class, UpdateFailureClass::Unauthentic) {
                        inner.pending = None;
                    }
                    inner.phase = UpdatePhase::Failed { class, message };
                    UpdateSnapshot {
                        current_version: inner.current_version.clone(),
                        phase: inner.phase.clone(),
                    }
                };
                return self.publish(app, snapshot).await;
            }
        };

        let active_after = match census() {
            Ok(count) => count,
            Err(message) => {
                let snapshot = {
                    let mut inner = self.inner.lock().await;
                    inner.phase = UpdatePhase::Failed {
                        class: UpdateFailureClass::Install,
                        message,
                    };
                    UpdateSnapshot {
                        current_version: inner.current_version.clone(),
                        phase: inner.phase.clone(),
                    }
                };
                return self.publish(app, snapshot).await;
            }
        };
        if IdleClients::attest(active_after).is_err() {
            let snapshot = {
                let mut inner = self.inner.lock().await;
                inner.phase = UpdatePhase::Failed {
                    class: UpdateFailureClass::ClientsActive,
                    message: "Cierra los clientes del juego antes de instalar".to_string(),
                };
                UpdateSnapshot {
                    current_version: inner.current_version.clone(),
                    phase: inner.phase.clone(),
                }
            };
            return self.publish(app, snapshot).await;
        }

        if let Err(message) = shutdown.shutdown_for_update().await {
            let snapshot = {
                let mut inner = self.inner.lock().await;
                inner.phase = UpdatePhase::Failed {
                    class: UpdateFailureClass::Install,
                    message,
                };
                UpdateSnapshot {
                    current_version: inner.current_version.clone(),
                    phase: inner.phase.clone(),
                }
            };
            return self.publish(app, snapshot).await;
        }

        if let Err(error) = backend.install(pending.generation, &bytes) {
            let (class, message) = classify_backend_error(error);
            let snapshot = {
                let mut inner = self.inner.lock().await;
                inner.phase = UpdatePhase::Failed { class, message };
                UpdateSnapshot {
                    current_version: inner.current_version.clone(),
                    phase: inner.phase.clone(),
                }
            };
            return self.publish(app, snapshot).await;
        }

        let installed_version = pending.release.version.clone();
        let snapshot = {
            let mut inner = self.inner.lock().await;
            inner.pending = None;
            backend.clear_pending();
            inner.phase = UpdatePhase::ReadyToRestart { installed_version };
            UpdateSnapshot {
                current_version: inner.current_version.clone(),
                phase: inner.phase.clone(),
            }
        };
        self.publish(app, snapshot).await
    }

    pub async fn relaunch(
        &self,
        app: &AppHandle,
        census: &(dyn Fn() -> Result<usize, String> + Send + Sync),
    ) -> Result<(), String> {
        {
            let inner = self.inner.lock().await;
            match &inner.phase {
                UpdatePhase::ReadyToRestart { .. } => {}
                _ => return Err("No hay una actualización instalada para reiniciar".to_string()),
            }
        }

        let active = census()?;
        IdleClients::attest(active)
            .map_err(|_| "Cierra los clientes del juego antes de reiniciar".to_string())?;

        app.restart();
    }
}

impl HostPending {
    fn clone_fields(&self) -> HostPending {
        HostPending {
            generation: self.generation,
            release: self.release.clone(),
        }
    }
}

/// Production packaging gate used by IPC: unpackaged never hits the backend.
pub async fn check_for_packaged_update(
    lease: &UpdateLease,
    app: &AppHandle,
    backend: &dyn UpdateBackend,
) -> UpdateSnapshot {
    lease.check_packaged(app, backend).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::updater::backend::mock::MockBackend;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    struct RecordingShutdown {
        called: AtomicBool,
    }

    #[async_trait]
    impl ShutdownForUpdate for RecordingShutdown {
        async fn shutdown_for_update(&self) -> Result<(), String> {
            self.called.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    fn census(count: usize) -> impl Fn() -> Result<usize, String> {
        move || Ok(count)
    }

    #[tokio::test]
    async fn install_without_pending_is_no_pending() {
        let lease = UpdateLease::new("0.1.0");
        let backend = MockBackend::available("0.2.0");
        let shutdown = RecordingShutdown {
            called: AtomicBool::new(false),
        };
        let snapshot = lease.install(None, &backend, &census(0), &shutdown).await;
        assert!(matches!(
            snapshot.phase,
            UpdatePhase::Failed {
                class: UpdateFailureClass::NoPending,
                ..
            }
        ));
        assert!(!shutdown.called.load(Ordering::SeqCst));
        assert_eq!(backend.install_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn unauthentic_check_drops_pending_and_never_installs() {
        let lease = UpdateLease::new("0.1.0");
        let backend = MockBackend::unauthentic_check();
        let snapshot = lease.check(None, &backend).await;
        assert!(matches!(
            snapshot.phase,
            UpdatePhase::Failed {
                class: UpdateFailureClass::Unauthentic,
                ..
            }
        ));
        let shutdown = RecordingShutdown {
            called: AtomicBool::new(false),
        };
        let install = lease.install(None, &backend, &census(0), &shutdown).await;
        assert!(matches!(
            install.phase,
            UpdatePhase::Failed {
                class: UpdateFailureClass::NoPending,
                ..
            }
        ));
        assert_eq!(backend.install_calls.load(Ordering::SeqCst), 0);
        assert!(!shutdown.called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn second_check_invalidates_previous_generation() {
        let lease = UpdateLease::new("0.1.0");
        let backend = MockBackend::available("0.2.0");
        let first = lease.check(None, &backend).await;
        assert!(matches!(first.phase, UpdatePhase::Available { .. }));
        let first_generation = {
            let inner = lease.inner.lock().await;
            inner.pending.as_ref().unwrap().generation
        };

        *backend.check.lock().unwrap() =
            crate::tools::updater::backend::mock::MockCheck::Available {
                version: "0.3.0".to_string(),
                notes: None,
            };
        let second = lease.check(None, &backend).await;
        assert!(matches!(
            second.phase,
            UpdatePhase::Available { release } if release.version == "0.3.0"
        ));
        let second_generation = {
            let inner = lease.inner.lock().await;
            inner.pending.as_ref().unwrap().generation
        };
        assert!(second_generation > first_generation);

        // Old generation is no longer in the backend slot.
        let err = backend
            .download(first_generation, &mut |_, _| {})
            .await
            .unwrap_err();
        assert!(matches!(err, BackendError::Other(_)));
    }

    #[tokio::test]
    async fn install_refused_when_clients_active_before_and_after_download() {
        let lease = UpdateLease::new("0.1.0");
        let backend = MockBackend::available("0.2.0");
        let _ = lease.check(None, &backend).await;
        let shutdown = RecordingShutdown {
            called: AtomicBool::new(false),
        };

        let before = lease.install(None, &backend, &census(2), &shutdown).await;
        assert!(matches!(
            before.phase,
            UpdatePhase::Failed {
                class: UpdateFailureClass::ClientsActive,
                ..
            }
        ));
        assert_eq!(backend.download_calls.load(Ordering::SeqCst), 0);
        assert!(!shutdown.called.load(Ordering::SeqCst));

        let _ = lease.check(None, &backend).await;
        let calls = Arc::new(AtomicUsize::new(0));
        let flipping = {
            let calls = Arc::clone(&calls);
            move || {
                let n = calls.fetch_add(1, Ordering::SeqCst);
                Ok(if n == 0 { 0 } else { 1 })
            }
        };
        let after = lease.install(None, &backend, &flipping, &shutdown).await;
        assert!(matches!(
            after.phase,
            UpdatePhase::Failed {
                class: UpdateFailureClass::ClientsActive,
                ..
            }
        ));
        assert!(backend.download_calls.load(Ordering::SeqCst) >= 1);
        assert!(!shutdown.called.load(Ordering::SeqCst));
        assert_eq!(backend.install_calls.load(Ordering::SeqCst), 0);

        let retry = lease.install(None, &backend, &census(0), &shutdown).await;
        assert!(matches!(
            retry.phase,
            UpdatePhase::ReadyToRestart {
                installed_version: ref v
            } if v == "0.2.0"
        ));
        assert_eq!(backend.install_calls.load(Ordering::SeqCst), 1);
        assert!(shutdown.called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn unauthentic_download_does_not_shutdown_tools() {
        let lease = UpdateLease::new("0.1.0");
        let backend = MockBackend::unauthentic_download("0.2.0");
        let _ = lease.check(None, &backend).await;
        let shutdown = RecordingShutdown {
            called: AtomicBool::new(false),
        };
        let snapshot = lease.install(None, &backend, &census(0), &shutdown).await;
        assert!(matches!(
            snapshot.phase,
            UpdatePhase::Failed {
                class: UpdateFailureClass::Unauthentic,
                ..
            }
        ));
        assert!(!shutdown.called.load(Ordering::SeqCst));
        assert_eq!(backend.install_calls.load(Ordering::SeqCst), 0);
        let inner = lease.inner.lock().await;
        assert!(inner.pending.is_none());
    }

    #[tokio::test]
    async fn ready_to_relaunch_install_is_idempotent() {
        let lease = UpdateLease::new("0.1.0");
        let backend = MockBackend::available("0.2.0");
        let _ = lease.check(None, &backend).await;
        let shutdown = RecordingShutdown {
            called: AtomicBool::new(false),
        };
        let first = lease.install(None, &backend, &census(0), &shutdown).await;
        assert!(matches!(
            first.phase,
            UpdatePhase::ReadyToRestart {
                installed_version: ref v
            } if v == "0.2.0"
        ));
        let installs = backend.install_calls.load(Ordering::SeqCst);
        let second = lease.install(None, &backend, &census(0), &shutdown).await;
        assert_eq!(first, second);
        assert_eq!(backend.install_calls.load(Ordering::SeqCst), installs);
    }

    #[tokio::test]
    async fn unpackaged_app_does_not_hit_backend() {
        let lease = UpdateLease::new("0.1.0");
        let backend = MockBackend::available("0.2.0");
        let snapshot = lease.mark_unpackaged(None).await;
        assert!(matches!(
            snapshot.phase,
            UpdatePhase::Unavailable {
                reason: UpdateUnavailableReason::NotPackaged
            }
        ));
        assert_eq!(backend.check_calls.load(Ordering::SeqCst), 0);
    }
}
