mod setup;

pub use setup::{
    reset_runtime_prefix, setup_runtime_prefix, DxvkProvision, RuntimeRequirements,
    MANAGED_DXVK_COMPONENT,
};

/// A prefix transaction must finish cleanup even if its IPC caller is cancelled.
pub async fn run_prefix_operation<F>(
    operation_id: Option<String>,
    operation: F,
) -> Result<(), String>
where
    F: std::future::Future<Output = Result<(), String>> + Send + 'static,
{
    if operation_id
        .as_ref()
        .is_some_and(|id| uuid::Uuid::parse_str(id).is_err())
    {
        return Err("Identificador de operación inválido".into());
    }
    tokio::spawn(crate::utils::PROGRESS_OPERATION_ID.scope(operation_id, operation))
        .await
        .map_err(|error| format!("La tarea del entorno falló: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancelled_caller_does_not_abandon_transaction_or_its_progress_scope() {
        let id = uuid::Uuid::new_v4().to_string();
        let expected = id.clone();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        let caller = tokio::spawn(run_prefix_operation(Some(id), async move {
            let _ = ready_tx.send(());
            release_rx.await.unwrap();
            assert_eq!(
                crate::utils::PROGRESS_OPERATION_ID.with(Clone::clone),
                Some(expected)
            );
            let _ = done_tx.send(());
            Ok(())
        }));
        ready_rx.await.unwrap();
        caller.abort();
        assert!(caller.await.unwrap_err().is_cancelled());
        release_tx.send(()).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(1), done_rx)
            .await
            .unwrap()
            .unwrap();
    }
}
