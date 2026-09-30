use tauri::{AppHandle, State};

use crate::models::update::UpdateSnapshot;
use crate::state::GameState;
use crate::tools::updater::{check_for_packaged_update, OfficialUpdaterBackend, UpdateLease};

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
    Ok(check_for_packaged_update(&lease, &app, backend.inner()).await)
}

#[tauri::command]
pub async fn install_checked_update(
    app: AppHandle,
    lease: State<'_, UpdateLease>,
    backend: State<'_, OfficialUpdaterBackend>,
    state: State<'_, GameState>,
) -> Result<UpdateSnapshot, String> {
    let census = || state.game.active_count();
    Ok(lease
        .install(Some(&app), backend.inner(), &census, &*state)
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
