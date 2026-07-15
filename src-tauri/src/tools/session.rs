use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{watch, Mutex};
use tokio::task::JoinHandle;
use tokio::time::timeout;

const DEFAULT_STOP_TIMEOUT: Duration = Duration::from_secs(3);

struct SessionTask {
    stop_tx: watch::Sender<bool>,
    join: JoinHandle<()>,
}

#[derive(Clone)]
pub struct SessionController {
    name: &'static str,
    task: Arc<Mutex<Option<SessionTask>>>,
    stop_timeout: Duration,
}

impl SessionController {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            task: Arc::new(Mutex::new(None)),
            stop_timeout: DEFAULT_STOP_TIMEOUT,
        }
    }

    pub async fn replace<F, Fut>(&self, spawn: F) -> Result<(), String>
    where
        F: FnOnce(watch::Receiver<bool>) -> Fut,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let mut slot = self.task.lock().await;
        self.stop_locked(&mut slot).await?;
        let (stop_tx, stop_rx) = watch::channel(false);
        *slot = Some(SessionTask {
            stop_tx,
            join: tokio::spawn(spawn(stop_rx)),
        });
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), String> {
        let mut slot = self.task.lock().await;
        self.stop_locked(&mut slot).await
    }

    async fn stop_locked(&self, slot: &mut Option<SessionTask>) -> Result<(), String> {
        let Some(SessionTask { stop_tx, mut join }) = slot.take() else {
            return Ok(());
        };
        let _ = stop_tx.send(true);
        match timeout(self.stop_timeout, &mut join).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(format!("{} terminó con error: {error}", self.name)),
            Err(_) => {
                join.abort();
                let _ = join.await;
                Err(format!(
                    "{} no se detuvo en {} ms",
                    self.name,
                    self.stop_timeout.as_millis()
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn controller(timeout: Duration) -> SessionController {
        SessionController {
            name: "Test",
            task: Arc::new(Mutex::new(None)),
            stop_timeout: timeout,
        }
    }

    #[tokio::test]
    async fn stop_waits_for_cleanup() {
        let controller = controller(Duration::from_secs(1));
        let cleaned = Arc::new(AtomicUsize::new(0));
        let task_cleaned = Arc::clone(&cleaned);
        controller
            .replace(move |mut stop_rx| async move {
                let _ = stop_rx.changed().await;
                task_cleaned.store(1, Ordering::SeqCst);
            })
            .await
            .unwrap();

        controller.stop().await.unwrap();
        assert_eq!(cleaned.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn replace_does_not_overlap_sessions() {
        let controller = controller(Duration::from_secs(1));
        let active = Arc::new(AtomicUsize::new(0));
        let old_active = Arc::clone(&active);
        controller
            .replace(move |mut stop_rx| async move {
                old_active.fetch_add(1, Ordering::SeqCst);
                let _ = stop_rx.changed().await;
                tokio::time::sleep(Duration::from_millis(20)).await;
                old_active.fetch_sub(1, Ordering::SeqCst);
            })
            .await
            .unwrap();

        let new_active = Arc::clone(&active);
        controller
            .replace(move |mut stop_rx| async move {
                assert_eq!(new_active.fetch_add(1, Ordering::SeqCst), 0);
                let _ = stop_rx.changed().await;
                new_active.fetch_sub(1, Ordering::SeqCst);
            })
            .await
            .unwrap();

        controller.stop().await.unwrap();
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn aborts_a_session_after_timeout() {
        let controller = controller(Duration::from_millis(20));
        controller
            .replace(|_stop_rx| async move {
                std::future::pending::<()>().await;
            })
            .await
            .unwrap();

        let error = controller.stop().await.unwrap_err();
        assert!(error.contains("no se detuvo"));
    }
}
