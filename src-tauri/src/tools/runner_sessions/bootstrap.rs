use std::path::Path;
use std::time::Duration;

use ro_tools_linux::{find_prefix_processes, is_prefix_leftover_process, ProcessIdentity};
use tokio::time::{sleep, timeout};

use crate::state::GameProcessHandle;
use crate::utils::{
    inspect_prefix, pipe_output, resolve_runner, OperationGuard, WineContext, PREFIX_SCHEMA_V3,
    PREFIX_SCHEMA_VERSION,
};

use super::SessionError;

pub(crate) const OUTSIDE_SUPERVISOR_MSG: &str =
    "El prefix ya está activo fuera del supervisor; cierra el juego y reintenta";

const LEFTOVER_SHUTDOWN_WAIT: Duration = Duration::from_secs(5);
const LEFTOVER_POLL: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BootstrapPrefixAction {
    NothingToDo,
    ShutdownLeftoverOnce,
    ShutdownForeignLeftoverOnce,
}

/// Decisión pura para tests: sin I/O ni spawn.
pub(crate) fn plan_bootstrap_prefix_processes(
    identities: &[ProcessIdentity],
    game: &GameProcessHandle,
) -> Result<BootstrapPrefixAction, SessionError> {
    plan_bootstrap_prefix_processes_with(identities, game, |identity| {
        is_prefix_leftover_process(identity.pid)
    })
}

pub(crate) fn plan_bootstrap_prefix_processes_with(
    identities: &[ProcessIdentity],
    game: &GameProcessHandle,
    is_leftover: impl Fn(&ProcessIdentity) -> bool,
) -> Result<BootstrapPrefixAction, SessionError> {
    if identities.is_empty() {
        return Ok(BootstrapPrefixAction::NothingToDo);
    }
    for identity in identities {
        if game.contains_identity(identity) {
            return Err(SessionError::validation(OUTSIDE_SUPERVISOR_MSG));
        }
        if !is_leftover(identity) {
            return Err(SessionError::validation(OUTSIDE_SUPERVISOR_MSG));
        }
    }
    Ok(BootstrapPrefixAction::ShutdownLeftoverOnce)
}

pub(crate) fn plan_after_first_shutdown(
    remaining: &[ProcessIdentity],
    game: &GameProcessHandle,
    is_leftover: impl Fn(&ProcessIdentity) -> bool,
    foreign_runner_available: bool,
) -> Result<BootstrapPrefixAction, SessionError> {
    if remaining.is_empty() {
        return Ok(BootstrapPrefixAction::NothingToDo);
    }
    for identity in remaining {
        if game.contains_identity(identity) {
            return Err(SessionError::validation(OUTSIDE_SUPERVISOR_MSG));
        }
        if !is_leftover(identity) {
            return Err(SessionError::validation(OUTSIDE_SUPERVISOR_MSG));
        }
    }
    if foreign_runner_available {
        return Ok(BootstrapPrefixAction::ShutdownForeignLeftoverOnce);
    }
    Err(SessionError::validation(OUTSIDE_SUPERVISOR_MSG))
}

pub(crate) fn plan_bootstrap_after_first_shutdown(
    prefix: &str,
    ctx: &WineContext,
    game: &GameProcessHandle,
) -> Result<BootstrapPrefixAction, SessionError> {
    let remaining = find_prefix_processes(prefix);
    let foreign = foreign_runner_available(prefix, ctx);
    plan_after_first_shutdown(
        &remaining,
        game,
        |identity| is_prefix_leftover_process(identity.pid),
        foreign,
    )
}

fn foreign_from_resolved(
    manifest_kind: &str,
    recorded_kind: &str,
    recorded_path: &Path,
    current_path: &Path,
) -> bool {
    recorded_kind == manifest_kind && recorded_path != current_path
}

fn foreign_runner_available(prefix: &str, ctx: &WineContext) -> bool {
    let health = inspect_prefix(prefix);
    let Some(manifest) = health.manifest else {
        return false;
    };
    let schema = manifest.schema_version();
    if (schema != PREFIX_SCHEMA_VERSION && schema != PREFIX_SCHEMA_V3)
        || manifest.runner_kind() == "unknown"
    {
        return false;
    }
    let Ok(recorded) = resolve_runner(manifest.runner_path()) else {
        return false;
    };
    foreign_from_resolved(
        manifest.runner_kind(),
        recorded.kind_label(),
        recorded.runner_path(),
        ctx.resolved.runner_path(),
    )
}

pub(crate) async fn bootstrap_prefix_for_supervisor(
    ctx: &WineContext,
    game: &GameProcessHandle,
) -> Result<(), SessionError> {
    let prefix_path = Path::new(&ctx.prefix);
    let _guard =
        OperationGuard::acquire("prefix", prefix_path).map_err(SessionError::validation)?;

    let identities = find_prefix_processes(&ctx.prefix);
    match plan_bootstrap_prefix_processes(&identities, game)? {
        BootstrapPrefixAction::NothingToDo => Ok(()),
        BootstrapPrefixAction::ShutdownLeftoverOnce => {
            shutdown_leftover_once(ctx).await?;
            if prefix_processes_ok(&ctx.prefix, game)? {
                return Ok(());
            }
            match plan_bootstrap_after_first_shutdown(&ctx.prefix, ctx, game)? {
                BootstrapPrefixAction::NothingToDo => Ok(()),
                BootstrapPrefixAction::ShutdownForeignLeftoverOnce => {
                    shutdown_foreign_leftover_once(ctx).await?;
                    wait_for_prefix_clear(&ctx.prefix, game).await
                }
                BootstrapPrefixAction::ShutdownLeftoverOnce => {
                    wait_for_prefix_clear(&ctx.prefix, game).await
                }
            }
        }
        BootstrapPrefixAction::ShutdownForeignLeftoverOnce => {
            shutdown_foreign_leftover_once(ctx).await?;
            wait_for_prefix_clear(&ctx.prefix, game).await
        }
    }
}

async fn shutdown_leftover_once(ctx: &WineContext) -> Result<(), SessionError> {
    let invocation = ctx
        .resolved
        .shutdown_invocation(&ctx.prefix)
        .map_err(SessionError::validation)?;
    spawn_shutdown_command(invocation).await
}

async fn shutdown_foreign_leftover_once(ctx: &WineContext) -> Result<(), SessionError> {
    let health = inspect_prefix(&ctx.prefix);
    let manifest = health
        .manifest
        .ok_or_else(|| SessionError::validation(OUTSIDE_SUPERVISOR_MSG))?;
    let runner = resolve_runner(manifest.runner_path())
        .map_err(|_| SessionError::validation(OUTSIDE_SUPERVISOR_MSG))?;
    if runner.kind_label() != manifest.runner_kind() {
        return Err(SessionError::validation(OUTSIDE_SUPERVISOR_MSG));
    }
    let invocation = runner
        .shutdown_invocation(&ctx.prefix)
        .map_err(SessionError::validation)?;
    spawn_shutdown_command(invocation).await
}

async fn spawn_shutdown_command(
    invocation: crate::utils::RunnerInvocation,
) -> Result<(), SessionError> {
    let mut cmd = invocation.into_command();
    pipe_output(&mut cmd);
    let mut child = cmd
        .spawn()
        .map_err(|error| SessionError::internal(format!("leftover shutdown spawn: {error}")))?;
    let _ = timeout(LEFTOVER_SHUTDOWN_WAIT, child.wait()).await;
    Ok(())
}

async fn wait_for_prefix_clear(prefix: &str, game: &GameProcessHandle) -> Result<(), SessionError> {
    let deadline = tokio::time::Instant::now() + LEFTOVER_SHUTDOWN_WAIT;
    while tokio::time::Instant::now() < deadline {
        if prefix_processes_ok(prefix, game)? {
            return Ok(());
        }
        sleep(LEFTOVER_POLL).await;
    }
    if prefix_processes_ok(prefix, game)? {
        return Ok(());
    }
    Err(SessionError::validation(OUTSIDE_SUPERVISOR_MSG))
}

fn prefix_processes_ok(prefix: &str, game: &GameProcessHandle) -> Result<bool, SessionError> {
    let identities = find_prefix_processes(prefix);
    if identities.is_empty() {
        return Ok(true);
    }
    for identity in &identities {
        if game.contains_identity(identity) {
            return Err(SessionError::validation(OUTSIDE_SUPERVISOR_MSG));
        }
        if !is_prefix_leftover_process(identity.pid) {
            return Err(SessionError::validation(OUTSIDE_SUPERVISOR_MSG));
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GameProcessHandle;
    use ro_tools_linux::ProcessIdentity;

    fn identity(pid: u32) -> ProcessIdentity {
        ProcessIdentity {
            pid,
            start_time: u64::from(pid),
        }
    }

    #[test]
    fn foreign_from_resolved_requires_matching_kind_and_distinct_paths() {
        let old = Path::new("/runners/old/wine");
        let new = Path::new("/runners/new/wine");
        assert!(foreign_from_resolved("wine", "wine", old, new));
        assert!(!foreign_from_resolved("wine", "proton", old, new));
        assert!(!foreign_from_resolved("wine", "wine", old, old));
    }

    #[test]
    fn empty_prefix_needs_no_shutdown() {
        let game = GameProcessHandle::new();
        assert_eq!(
            plan_bootstrap_prefix_processes(&[], &game).unwrap(),
            BootstrapPrefixAction::NothingToDo
        );
    }

    #[test]
    fn unregistered_active_process_rejects_without_shutdown_plan() {
        let game = GameProcessHandle::new();
        let err =
            plan_bootstrap_prefix_processes_with(&[identity(99)], &game, |_| false).unwrap_err();
        assert_eq!(err.message, OUTSIDE_SUPERVISOR_MSG);
    }

    #[test]
    fn only_leftover_processes_plan_single_shutdown() {
        let game = GameProcessHandle::new();
        let leftovers = [identity(1), identity(2)];
        assert_eq!(
            plan_bootstrap_prefix_processes_with(&leftovers, &game, |_| true).unwrap(),
            BootstrapPrefixAction::ShutdownLeftoverOnce
        );
    }

    #[test]
    fn bootstrap_foreign_runner_leftover_uses_manifest_shutdown_once() {
        let game = GameProcessHandle::new();
        let leftovers = [identity(7)];
        assert_eq!(
            plan_bootstrap_prefix_processes_with(&leftovers, &game, |_| true).unwrap(),
            BootstrapPrefixAction::ShutdownLeftoverOnce
        );
        assert_eq!(
            plan_after_first_shutdown(&leftovers, &game, |_| true, true).unwrap(),
            BootstrapPrefixAction::ShutdownForeignLeftoverOnce
        );
        let err = plan_after_first_shutdown(&leftovers, &game, |_| true, false).unwrap_err();
        assert_eq!(err.message, OUTSIDE_SUPERVISOR_MSG);
    }

    #[test]
    fn registered_client_with_leftover_rejects_without_shutdown() {
        use crate::tools::runner_sessions::{ClientRuntimeGuard, SessionOwnership};

        let game = GameProcessHandle::new();
        let reservation = game
            .begin_launch("c1".into(), "srv".into(), "Srv".into())
            .unwrap();
        let registered = identity(42);
        let runtime = ClientRuntimeGuard {
            session: SessionOwnership::Direct,
            memory: None,
            memory_access: None,
            profile_memory: None,
        };
        game.mark_running(reservation, registered, runtime).unwrap();
        let leftover = identity(1);
        let err = plan_bootstrap_prefix_processes_with(&[leftover, registered], &game, |id| {
            id.pid == leftover.pid
        })
        .unwrap_err();
        assert_eq!(err.message, OUTSIDE_SUPERVISOR_MSG);
    }

    #[test]
    fn shared_prefix_lock_blocks_bootstrap_exclusive() {
        use crate::utils::OperationGuard;

        let path =
            std::env::temp_dir().join(format!("ro-bootstrap-prefix-lock-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        let shared = OperationGuard::acquire_shared("prefix", &path).unwrap();
        let err = OperationGuard::acquire("prefix", &path)
            .err()
            .expect("exclusive should fail while shared is held");
        assert!(err.contains("Ya hay una operación prefix"));
        drop(shared);
        let exclusive = OperationGuard::acquire("prefix", &path).unwrap();
        drop(exclusive);
    }

    #[test]
    fn explorer_comm_with_wine_argv0_is_not_leftover() {
        use ro_tools_linux::is_leftover_from_comm_and_argv0;

        assert!(!is_leftover_from_comm_and_argv0(
            "explorer.exe",
            Some("wine")
        ));
    }
}
