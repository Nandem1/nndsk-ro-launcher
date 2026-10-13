use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

use crate::models::update::UpdateSnapshot;
use crate::state::GameState;
use crate::tools::updater::{
    check_for_packaged_update, OfficialUpdaterBackend, ShutdownForUpdate, UpdateLease,
};

/// Retain access to managed GameState for the host task without borrowing the IPC waiter.
struct UpdateShutdown(AppHandle);

#[async_trait::async_trait]
impl ShutdownForUpdate for UpdateShutdown {
    async fn shutdown_for_update(&self) -> Result<(), String> {
        self.0.state::<GameState>().shutdown_for_update().await
    }
}

#[tauri::command]
pub async fn get_update_snapshot(lease: State<'_, UpdateLease>) -> Result<UpdateSnapshot, String> {
    Ok(lease.snapshot().await)
}

#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    lease: State<'_, UpdateLease>,
    backend: State<'_, OfficialUpdaterBackend>,
) -> Result<UpdateSnapshot, String> {
    Ok(check_for_packaged_update(&lease, &app, Arc::new(backend.inner().clone())).await)
}

#[tauri::command]
pub async fn install_checked_update(
    app: AppHandle,
    lease: State<'_, UpdateLease>,
    backend: State<'_, OfficialUpdaterBackend>,
) -> Result<UpdateSnapshot, String> {
    let census_app = app.clone();
    let census = move || census_app.state::<GameState>().game.active_count();
    let shutdown = Arc::new(UpdateShutdown(app.clone()));
    Ok(lease
        .install(
            Some(app),
            Arc::new(backend.inner().clone()),
            census,
            shutdown,
        )
        .await)
}

#[tauri::command]
pub async fn relaunch_updated_app(
    app: AppHandle,
    lease: State<'_, UpdateLease>,
    state: State<'_, GameState>,
) -> Result<(), String> {
    let census = || state.game.active_count();
    lease.relaunch(&app, &census).await
}

#[cfg(test)]
mod tests {
    /// Commands accept only AppHandle / State — no endpoint, pubkey, URL, or signature args.
    #[test]
    fn production_commands_have_no_transport_inputs() {
        let source = include_str!("updater.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production command source");
        for command in [
            "get_update_snapshot",
            "check_for_update",
            "install_checked_update",
            "relaunch_updated_app",
        ] {
            assert!(production.contains(command), "missing command {command}");
        }
        assert!(!production.contains("endpoint"));
        assert!(!production.contains("pubkey"));
        assert!(!production.contains("signature"));
        assert!(!production.contains("CheckOptions"));
        assert!(!production.contains("proxy"));
        assert!(!production.contains("headers"));
    }
}
