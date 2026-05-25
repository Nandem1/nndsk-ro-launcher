use tauri::AppHandle;

use crate::tools::prefix;

#[tauri::command]
pub async fn setup_prefix(app: AppHandle) -> Result<(), String> {
    prefix::setup_prefix(app).await
}

#[tauri::command]
pub async fn reset_prefix(app: AppHandle) -> Result<(), String> {
    prefix::reset_prefix(app).await
}
