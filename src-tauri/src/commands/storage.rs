use tauri::State;

use crate::models::storage::StorageNotice;
use crate::state::StorageNotices;

#[tauri::command]
pub fn take_storage_notices(
    notices: State<'_, StorageNotices>,
) -> Result<Vec<StorageNotice>, String> {
    notices.take()
}
