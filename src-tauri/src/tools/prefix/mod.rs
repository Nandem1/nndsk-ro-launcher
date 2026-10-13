mod recovery;
mod setup;

use std::path::Path;

use crate::utils::{OperationGuard, PrefixLocation, PrefixScope};

pub use recovery::{
    transaction_residue_present, uncommitted_transaction_present, RecoveryAction, RecoveryOutcome,
    RECOVERED_RETRY_MSG,
};
pub use setup::{
    reset_runtime_prefix, setup_runtime_prefix, DxvkProvision, RuntimeRequirements,
    MANAGED_DXVK_COMPONENT,
};

/// Reconcile an interrupted managed provisioning/reset before any runner session can touch the
/// prefix. Takes the exclusive prefix guard; custom prefixes are never touched.
pub fn recover_interrupted_prefix(location: &PrefixLocation) -> Result<RecoveryOutcome, String> {
    if !is_recoverable(location) {
        return Ok(RecoveryOutcome::untouched());
    }
    let _guard = OperationGuard::acquire("prefix", Path::new(&location.path))?;
    recover_with_guard_held(location)
}

/// Same reconciliation for a caller that already holds the exclusive prefix guard.
pub fn recover_with_guard_held(location: &PrefixLocation) -> Result<RecoveryOutcome, String> {
    if !is_recoverable(location) {
        return Ok(RecoveryOutcome::untouched());
    }
    let outcome = recovery::ManagedPrefix::new(location)?.recover()?;
    if outcome.action != RecoveryAction::None || outcome.removed_empty_backup {
        crate::utils::report(
            "prefix-recovery",
            format!("{}: {:?}", location.path, outcome.action),
        );
    }
    for note in &outcome.notes {
        crate::utils::report("prefix-recovery", format!("{}: {note}", location.path));
    }
    Ok(outcome)
}

/// Hot-path variant for supervisor bootstrap: ordinary prefixes without transaction evidence skip
/// classification entirely.
pub fn recover_residue_with_guard_held(
    location: &PrefixLocation,
) -> Result<RecoveryOutcome, String> {
    if !transaction_residue_present(Path::new(&location.path)) {
        return Ok(RecoveryOutcome::untouched());
    }
    recover_with_guard_held(location)
}

fn is_recoverable(location: &PrefixLocation) -> bool {
    location.managed && location.scope != PrefixScope::Custom
}

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
