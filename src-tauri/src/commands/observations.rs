use std::path::PathBuf;

use crate::models::observation::{RuntimeObservationExportResultIpc, RuntimeObservationSummaryIpc};
use crate::tools::runtime::{delete_observations, export_observations, list_observations};

#[tauri::command]
pub fn list_runtime_observations() -> Result<Vec<RuntimeObservationSummaryIpc>, String> {
    list_observations()
}

#[tauri::command]
pub fn export_runtime_observations(
    dest_path: String,
) -> Result<RuntimeObservationExportResultIpc, String> {
    let count = export_observations(PathBuf::from(dest_path).as_path())?;
    Ok(RuntimeObservationExportResultIpc {
        exported_count: count,
    })
}

#[tauri::command]
pub fn delete_runtime_observations() -> Result<(), String> {
    delete_observations()
}
