use async_trait::async_trait;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

use crate::models::update::{truncate_release_notes, ReleaseSummary};
use crate::tools::updater::policy::classify_plugin_error;

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

/// Official plugin is the only verifier. Pending artifact is keyed by lease generation
/// so a second check cannot install a stale candidate.
#[async_trait]
pub trait UpdateBackend: Send + Sync {
    async fn check(&self, generation: u64) -> Result<CheckOutcome, BackendError>;
    async fn download(
        &self,
        generation: u64,
        on_chunk: &mut (dyn FnMut(u64, Option<u64>) + Send),
    ) -> Result<Vec<u8>, BackendError>;
    fn install(&self, generation: u64, bytes: &[u8]) -> Result<(), BackendError>;
    fn clear_pending(&self);
}

pub struct OfficialUpdaterBackend {
    app: AppHandle,
    slot: std::sync::Mutex<Option<(u64, tauri_plugin_updater::Update)>>,
}

impl OfficialUpdaterBackend {
    /// Production constructor. Reads endpoint + pubkey from tauri.conf via the plugin.
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            slot: std::sync::Mutex::new(None),
        }
    }

    fn builder(&self) -> tauri_plugin_updater::UpdaterBuilder {
        self.app.updater_builder()
    }

    fn take_matching(&self, generation: u64) -> Result<tauri_plugin_updater::Update, BackendError> {
        let slot = self
            .slot
            .lock()
            .map_err(|_| BackendError::Other("Updater slot poisoned".to_string()))?;
        match slot.as_ref() {
            Some((gen, update)) if *gen == generation => Ok(update.clone()),
            _ => Err(BackendError::Other(
                "No hay un artefacto pendiente para esta generación".to_string(),
            )),
        }
    }
}

#[async_trait]
impl UpdateBackend for OfficialUpdaterBackend {
    async fn check(&self, generation: u64) -> Result<CheckOutcome, BackendError> {
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
                let release = ReleaseSummary {
                    version: update.version.clone(),
                    published_at: update.date.map(|d| d.to_string()),
                    notes: truncate_release_notes(update.body.as_deref()),
                };
                *slot = Some((generation, update));
                Ok(CheckOutcome::Available { release })
            }
        }
    }

    async fn download(
        &self,
        generation: u64,
        on_chunk: &mut (dyn FnMut(u64, Option<u64>) + Send),
    ) -> Result<Vec<u8>, BackendError> {
        let update = self.take_matching(generation)?;
        // Plugin on_chunk is chunk length, not cumulative downloaded bytes.
        let mut downloaded = 0u64;
        update
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

    fn install(&self, generation: u64, bytes: &[u8]) -> Result<(), BackendError> {
        let update = self.take_matching(generation)?;
        update.install(bytes).map_err(classify_plugin_error)
    }

    fn clear_pending(&self) {
        if let Ok(mut slot) = self.slot.lock() {
            *slot = None;
        }
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Mutex;

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
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
        pub download_calls: AtomicU64,
        pub install_calls: AtomicU64,
        pub clear_calls: AtomicU64,
        pub check: Mutex<MockCheck>,
        pub download: Mutex<MockDownload>,
        pub install_ok: AtomicBool,
        slot: Mutex<Option<(u64, String)>>,
    }

    impl MockBackend {
        pub fn available(version: &str) -> Self {
            Self {
                check_calls: AtomicU64::new(0),
                download_calls: AtomicU64::new(0),
                install_calls: AtomicU64::new(0),
                clear_calls: AtomicU64::new(0),
                check: Mutex::new(MockCheck::Available {
                    version: version.to_string(),
                    notes: Some("notas".to_string()),
                }),
                download: Mutex::new(MockDownload::Bytes(b"appimage".to_vec())),
                install_ok: AtomicBool::new(true),
                slot: Mutex::new(None),
            }
        }

        pub fn unauthentic_download(version: &str) -> Self {
            let backend = Self::available(version);
            *backend.download.lock().unwrap() =
                MockDownload::Error(BackendError::Unauthentic("bad sig".to_string()));
            backend
        }

        pub fn unauthentic_check() -> Self {
            let backend = Self::available("9.9.9");
            *backend.check.lock().unwrap() =
                MockCheck::Error(BackendError::Unauthentic("bad manifest".to_string()));
            backend
        }
    }

    #[async_trait]
    impl UpdateBackend for MockBackend {
        async fn check(&self, generation: u64) -> Result<CheckOutcome, BackendError> {
            self.check_calls.fetch_add(1, Ordering::SeqCst);
            let outcome = self.check.lock().unwrap().clone();
            match outcome {
                MockCheck::Current => {
                    *self.slot.lock().unwrap() = None;
                    Ok(CheckOutcome::Current)
                }
                MockCheck::Available { version, notes } => {
                    *self.slot.lock().unwrap() = Some((generation, version.clone()));
                    Ok(CheckOutcome::Available {
                        release: ReleaseSummary {
                            version,
                            published_at: None,
                            notes,
                        },
                    })
                }
                MockCheck::Error(error) => {
                    *self.slot.lock().unwrap() = None;
                    Err(error)
                }
            }
        }

        async fn download(
            &self,
            generation: u64,
            on_chunk: &mut (dyn FnMut(u64, Option<u64>) + Send),
        ) -> Result<Vec<u8>, BackendError> {
            self.download_calls.fetch_add(1, Ordering::SeqCst);
            let slot = self.slot.lock().unwrap().clone();
            match slot {
                Some((gen, _)) if gen == generation => {}
                _ => {
                    return Err(BackendError::Other(
                        "No hay un artefacto pendiente para esta generación".to_string(),
                    ))
                }
            }
            match self.download.lock().unwrap().clone() {
                MockDownload::Bytes(bytes) => {
                    on_chunk(bytes.len() as u64, Some(bytes.len() as u64));
                    Ok(bytes)
                }
                MockDownload::Error(error) => Err(error),
            }
        }

        fn install(&self, generation: u64, _bytes: &[u8]) -> Result<(), BackendError> {
            self.install_calls.fetch_add(1, Ordering::SeqCst);
            let slot = self.slot.lock().unwrap().clone();
            match slot {
                Some((gen, _)) if gen == generation => {}
                _ => {
                    return Err(BackendError::Other(
                        "No hay un artefacto pendiente para esta generación".to_string(),
                    ))
                }
            }
            if self.install_ok.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(BackendError::Install("write failed".to_string()))
            }
        }

        fn clear_pending(&self) {
            self.clear_calls.fetch_add(1, Ordering::SeqCst);
            *self.slot.lock().unwrap() = None;
        }
    }
}
