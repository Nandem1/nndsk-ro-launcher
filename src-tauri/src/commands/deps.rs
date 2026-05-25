use crate::models::dependency::DependencyStatus;
use crate::tools::deps;

#[tauri::command]
pub async fn check_dependencies(runner: Option<String>) -> Result<DependencyStatus, String> {
    deps::check_dependencies(runner).await
}
