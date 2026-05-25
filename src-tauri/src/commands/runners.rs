use crate::models::runner::RunnerInfo;
use crate::tools::runners;

#[tauri::command]
pub async fn list_runners() -> Result<Vec<RunnerInfo>, String> {
    runners::discover_runners()
}
