use async_trait::async_trait;
use std::sync::Arc;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

use crate::models::update::{truncate_release_notes, ReleaseSummary};
use crate::tools::updater::activate::activate_appimage;
use crate::tools::updater::policy::{classify_plugin_error, packaged_appimage};

#[derive(Debug, Clone)]
pub enum BackendError {
    Unauthentic(String),
    Unreachable(String),
    Install(String),
    Other(String),
}

#[derive(Debug, Clone)]
pub enum CheckOutcome {
    Current,
    Available { release: ReleaseSummary },
}

/// A candidate moves out of the backend slot into the sole admitted installer.
/// Consuming it on install prevents a second installation attempt with the same candidate.
#[async_trait]
pub trait UpdateCandidate: Send + Sync {
    fn release(&self) -> ReleaseSummary;
    /// Move the uninstalled candidate back to its original empty slot under the lease gate.
    fn return_to_slot(self: Box<Self>) -> Result<(), BackendError>;
    async fn download(
        &self,
        on_chunk: &mut (dyn FnMut(u64, Option<u64>) + Send),
    ) -> Result<Vec<u8>, BackendError>;
    fn install(self: Box<Self>, bytes: &[u8]) -> Result<(), BackendError>;
}

/// Official plugin is the only verifier. UpdateLease serializes all slot access.
#[async_trait]
pub trait UpdateBackend: Send + Sync {
    async fn check(&self) -> Result<CheckOutcome, BackendError>;
    fn take_pending(&self) -> Result<Box<dyn UpdateCandidate>, BackendError>;
    fn clear_pending(&self);
}

#[derive(Clone)]
pub struct OfficialUpdaterBackend {
    app: AppHandle,
    slot: Arc<std::sync::Mutex<Option<tauri_plugin_updater::Update>>>,
}

impl OfficialUpdaterBackend {
    /// Production constructor. Reads endpoint + pubkey from tauri.conf via the plugin.
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            slot: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    fn builder(&self) -> tauri_plugin_updater::UpdaterBuilder {
        self.app.updater_builder()
    }
}

#[async_trait]
impl UpdateBackend for OfficialUpdaterBackend {
    async fn check(&self) -> Result<CheckOutcome, BackendError> {
        let updater = self.builder().build().map_err(classify_plugin_error)?;
        let found = updater.check().await.map_err(classify_plugin_error)?;
        let mut slot = self
            .slot
            .lock()
            .map_err(|_| BackendError::Other("Updater slot poisoned".to_string()))?;
        match found {
            None => {
                *slot = None;
                Ok(CheckOutcome::Current)
            }
            Some(update) => {
                let release = release_summary(&update);
                *slot = Some(update);
                Ok(CheckOutcome::Available { release })
            }
        }
    }

    fn take_pending(&self) -> Result<Box<dyn UpdateCandidate>, BackendError> {
        let update = self
            .slot
            .lock()
            .map_err(|_| BackendError::Other("Updater slot poisoned".to_string()))?
            .take()
            .ok_or_else(|| BackendError::Other("No hay un artefacto pendiente".to_string()))?;
        Ok(Box::new(OfficialCandidate {
            update,
            slot: self.slot.clone(),
            app: self.app.clone(),
        }))
    }

    fn clear_pending(&self) {
        if let Ok(mut slot) = self.slot.lock() {
            *slot = None;
        }
    }
}

fn release_summary(update: &tauri_plugin_updater::Update) -> ReleaseSummary {
    ReleaseSummary {
        version: update.version.clone(),
        published_at: update.date.map(|d| d.to_string()),
        notes: truncate_release_notes(update.body.as_deref()),
    }
}

struct OfficialCandidate {
    update: tauri_plugin_updater::Update,
    slot: Arc<std::sync::Mutex<Option<tauri_plugin_updater::Update>>>,
    app: AppHandle,
}

#[async_trait]
impl UpdateCandidate for OfficialCandidate {
    fn release(&self) -> ReleaseSummary {
        release_summary(&self.update)
    }

    fn return_to_slot(self: Box<Self>) -> Result<(), BackendError> {
        let mut slot = self
            .slot
            .lock()
            .map_err(|_| BackendError::Other("Updater slot poisoned".to_string()))?;
        if slot.is_some() {
            return Err(BackendError::Other(
                "Updater slot already occupied".to_string(),
            ));
        }
        *slot = Some(self.update);
        Ok(())
    }

    async fn download(
        &self,
        on_chunk: &mut (dyn FnMut(u64, Option<u64>) + Send),
    ) -> Result<Vec<u8>, BackendError> {
        // Plugin on_chunk is chunk length, not cumulative downloaded bytes.
        let mut downloaded = 0u64;
        self.update
            .download(
                |chunk, total| {
                    downloaded = downloaded.saturating_add(chunk as u64);
                    on_chunk(downloaded, total);
                },
                || {},
            )
            .await
            .map_err(classify_plugin_error)
    }

    fn install(self: Box<Self>, bytes: &[u8]) -> Result<(), BackendError> {
        // Same host-captured APPIMAGE that the plugin uses for extract_path.
        // Neither the WebView nor the remote manifest supplies a destination.
        let target = packaged_appimage(&self.app)
            .map_err(|e| BackendError::Install(format!("AppImage no disponible: {e:?}")))?;
        activate_appimage(&target, bytes)
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Mutex;

    #[derive(Debug, Clone)]
    pub enum MockCheck {
        Current,
        Available {
            version: String,
            notes: Option<String>,
        },
        Error(BackendError),
    }

    #[derive(Debug, Clone)]
    pub enum MockDownload {
        Bytes(Vec<u8>),
        Error(BackendError),
    }

    pub struct MockBackend {
        pub check_calls: AtomicU64,
        pub take_calls: AtomicU64,
        pub download_calls: Arc<AtomicU64>,
        pub install_calls: Arc<AtomicU64>,
        pub clear_calls: AtomicU64,
        pub check: Mutex<MockCheck>,
        pub download: Arc<Mutex<MockDownload>>,
        pub install_ok: Arc<AtomicBool>,
        pub installed_versions: Arc<Mutex<Vec<String>>>,
        slot: Arc<Mutex<Option<ReleaseSummary>>>,
    }

    impl MockBackend {
        pub fn available(version: &str) -> Arc<Self> {
            Arc::new(Self {
                check_calls: AtomicU64::new(0),
                take_calls: AtomicU64::new(0),
                download_calls: Arc::new(AtomicU64::new(0)),
                install_calls: Arc::new(AtomicU64::new(0)),
                clear_calls: AtomicU64::new(0),
                check: Mutex::new(MockCheck::Available {
                    version: version.to_string(),
                    notes: Some("notas".to_string()),
                }),
                download: Arc::new(Mutex::new(MockDownload::Bytes(b"appimage".to_vec()))),
                install_ok: Arc::new(AtomicBool::new(true)),
                installed_versions: Arc::new(Mutex::new(Vec::new())),
                slot: Arc::new(Mutex::new(None)),
            })
        }

        pub fn unauthentic_download(version: &str) -> Arc<Self> {
            let backend = Self::available(version);
            *backend.download.lock().unwrap() =
                MockDownload::Error(BackendError::Unauthentic("bad sig".to_string()));
            backend
        }

        pub fn unauthentic_check() -> Arc<Self> {
            let backend = Self::available("9.9.9");
            *backend.check.lock().unwrap() =
                MockCheck::Error(BackendError::Unauthentic("bad manifest".to_string()));
            backend
        }

        pub fn pending_version(&self) -> Option<String> {
            self.slot
                .lock()
                .unwrap()
                .as_ref()
                .map(|release| release.version.clone())
        }
    }

    #[async_trait]
    impl UpdateBackend for MockBackend {
        async fn check(&self) -> Result<CheckOutcome, BackendError> {
            self.check_calls.fetch_add(1, Ordering::SeqCst);
            let outcome = self.check.lock().unwrap().clone();
            match outcome {
                MockCheck::Current => {
                    *self.slot.lock().unwrap() = None;
                    Ok(CheckOutcome::Current)
                }
                MockCheck::Available { version, notes } => {
                    let release = ReleaseSummary {
                        version,
                        published_at: None,
                        notes,
                    };
                    *self.slot.lock().unwrap() = Some(release.clone());
                    Ok(CheckOutcome::Available { release })
                }
                MockCheck::Error(error) => {
                    // Match production early failures: the lease must clear the old slot.
                    Err(error)
                }
            }
        }

        fn take_pending(&self) -> Result<Box<dyn UpdateCandidate>, BackendError> {
            let release =
                self.slot.lock().unwrap().take().ok_or_else(|| {
                    BackendError::Other("No hay un artefacto pendiente".to_string())
                })?;
            self.take_calls.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(MockCandidate {
                release,
                slot: self.slot.clone(),
                download_calls: self.download_calls.clone(),
                install_calls: self.install_calls.clone(),
                download: self.download.clone(),
                install_ok: self.install_ok.clone(),
                installed_versions: self.installed_versions.clone(),
            }))
        }

        fn clear_pending(&self) {
            self.clear_calls.fetch_add(1, Ordering::SeqCst);
            *self.slot.lock().unwrap() = None;
        }
    }

    struct MockCandidate {
        release: ReleaseSummary,
        slot: Arc<Mutex<Option<ReleaseSummary>>>,
        download_calls: Arc<AtomicU64>,
        install_calls: Arc<AtomicU64>,
        download: Arc<Mutex<MockDownload>>,
        install_ok: Arc<AtomicBool>,
        installed_versions: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl UpdateCandidate for MockCandidate {
        fn release(&self) -> ReleaseSummary {
            self.release.clone()
        }

        fn return_to_slot(self: Box<Self>) -> Result<(), BackendError> {
            let mut slot = self.slot.lock().unwrap();
            if slot.is_some() {
                return Err(BackendError::Other(
                    "Updater slot already occupied".to_string(),
                ));
            }
            *slot = Some(self.release);
            Ok(())
        }

        async fn download(
            &self,
            on_chunk: &mut (dyn FnMut(u64, Option<u64>) + Send),
        ) -> Result<Vec<u8>, BackendError> {
            self.download_calls.fetch_add(1, Ordering::SeqCst);
            match self.download.lock().unwrap().clone() {
                MockDownload::Bytes(bytes) => {
                    on_chunk(bytes.len() as u64, Some(bytes.len() as u64));
                    Ok(bytes)
                }
                MockDownload::Error(error) => Err(error),
            }
        }

        fn install(self: Box<Self>, _bytes: &[u8]) -> Result<(), BackendError> {
            self.install_calls.fetch_add(1, Ordering::SeqCst);
            if self.install_ok.load(Ordering::SeqCst) {
                self.installed_versions
                    .lock()
                    .unwrap()
                    .push(self.release.version);
                Ok(())
            } else {
                Err(BackendError::Install("write failed".to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{mock::MockBackend, mock::MockCheck, UpdateBackend};
    use std::sync::atomic::Ordering;

    #[tokio::test]
    async fn pending_candidate_moves_to_one_installer() {
        let backend = MockBackend::available("0.2.0");
        backend.check().await.unwrap();
        let candidate = backend.take_pending().unwrap();
        assert!(backend.take_pending().is_err());
        assert!(backend.pending_version().is_none());
        let bytes = candidate.download(&mut |_, _| {}).await.unwrap();
        candidate.install(&bytes).unwrap();
        assert_eq!(backend.take_calls.load(Ordering::SeqCst), 1);
        assert_eq!(backend.download_calls.load(Ordering::SeqCst), 1);
        assert_eq!(backend.install_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn downloaded_candidate_returns_to_slot_and_moves_out_once() {
        let backend = MockBackend::available("0.2.0");
        backend.check().await.unwrap();
        let candidate = backend.take_pending().unwrap();
        let release = candidate.release();
        candidate.download(&mut |_, _| {}).await.unwrap();
        candidate.return_to_slot().unwrap();
        assert_eq!(backend.pending_version().as_deref(), Some("0.2.0"));
        let retry = backend.take_pending().unwrap();
        assert_eq!(retry.release(), release);
        assert!(backend.take_pending().is_err());
        let bytes = retry.download(&mut |_, _| {}).await.unwrap();
        retry.install(&bytes).unwrap();
        assert_eq!(backend.download_calls.load(Ordering::SeqCst), 2);
        assert_eq!(backend.install_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn returning_candidate_cannot_overwrite_an_occupied_slot() {
        let backend = MockBackend::available("0.2.0");
        backend.check().await.unwrap();
        let candidate = backend.take_pending().unwrap();
        // Deliberately bypass the lease to exercise the backend's defensive check.
        *backend.check.lock().unwrap() = MockCheck::Available {
            version: "0.3.0".into(),
            notes: None,
        };
        backend.check().await.unwrap();
        assert!(candidate.return_to_slot().is_err());
        assert_eq!(backend.pending_version().as_deref(), Some("0.3.0"));
        assert_eq!(backend.install_calls.load(Ordering::SeqCst), 0);
    }
}
