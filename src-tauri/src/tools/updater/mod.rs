mod backend;
mod policy;
mod session;

pub use backend::OfficialUpdaterBackend;
pub use session::{check_for_packaged_update, ShutdownForUpdate, UpdateLease};

use crate::state::GameState;

/// Same sequence as `RunEvent::Exit`: presence, tools, sessions, input.
pub async fn shutdown_launcher_services(state: &GameState) {
    state.presence.shutdown();
    let _ = tokio::join!(
        state.autopot.stop(),
        state.autobuff.stop(),
        state.spammer.stop()
    );
    let _ = state.sessions.shutdown_all().await;
    state.input.shutdown();
}

#[async_trait::async_trait]
impl ShutdownForUpdate for GameState {
    async fn shutdown_for_update(&self) -> Result<(), String> {
        shutdown_launcher_services(self).await;
        Ok(())
    }
}
