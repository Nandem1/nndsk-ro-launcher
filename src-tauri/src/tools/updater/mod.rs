mod backend;
mod policy;
mod session;

pub use backend::OfficialUpdaterBackend;
pub use session::{check_for_packaged_update, UpdateLease};

use crate::state::GameState;

/// Actual service shutdown belongs only to `RunEvent::Exit`.
pub async fn shutdown_launcher_services(state: &GameState) {
    state.installations.request_shutdown();
    state.presence.shutdown();
    let (autopot, autobuff, spammer) = tokio::join!(
        state.autopot.stop(),
        state.autobuff.stop(),
        state.spammer.stop()
    );
    for (scope, result) in [
        ("exit/autopot", autopot),
        ("exit/autobuff", autobuff),
        ("exit/spammer", spammer),
    ] {
        if let Err(error) = result {
            crate::utils::report(scope, error);
        }
    }
    for error in state.sessions.shutdown_all().await {
        crate::utils::report("exit/sessions", error);
    }
    state.installations.wait_for_shutdown().await;
    state.input.shutdown();
}

#[cfg(test)]
mod lifecycle_tests;
