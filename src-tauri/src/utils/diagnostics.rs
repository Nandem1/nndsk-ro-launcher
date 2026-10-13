//! Durable sink for failures that have no caller to return them to.
//!
//! Background tasks, rollbacks and shutdown paths used to drop these errors or print them only to
//! stderr, which nobody sees when the AppImage is started from a desktop entry. `report` keeps
//! stderr, appends to a bounded `diagnostics.log` in the data directory and, once the app has
//! started, mirrors the line to the UI log panel.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use tauri::AppHandle;

const LOG_FILE: &str = "diagnostics.log";
const ROTATED_FILE: &str = "diagnostics.log.1";
const MAX_LOG_BYTES: u64 = 256 * 1024;

static APP: OnceLock<AppHandle> = OnceLock::new();
static FILE_LOCK: Mutex<()> = Mutex::new(());

/// Registers the handle so later reports also reach the UI log panel.
pub fn init_diagnostics(app: &AppHandle) {
    let _ = APP.set(app.clone());
}

/// Records a failure that would otherwise be discarded. `scope` names the subsystem.
pub fn report(scope: &str, message: impl std::fmt::Display) {
    let line = format!("[{scope}] {message}");
    eprintln!("[ro-launcher] {line}");
    let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    if let Err(error) = append_bounded(
        &diagnostics_dir(),
        &format!("{timestamp} {line}\n"),
        MAX_LOG_BYTES,
    ) {
        eprintln!("[ro-launcher] [diagnostics] {LOG_FILE} not writable: {error}");
    }
    super::emit_log_opt(APP.get(), format!("[diagnóstico] {line}"));
}

fn diagnostics_dir() -> PathBuf {
    #[cfg(test)]
    {
        std::env::temp_dir().join(format!(
            "ro-launcher-test-diagnostics-{}",
            std::process::id()
        ))
    }
    #[cfg(not(test))]
    {
        super::app_data_dir()
    }
}

/// Appends `line`, rotating once to `diagnostics.log.1` when the file would exceed `max_bytes`.
fn append_bounded(dir: &Path, line: &str, max_bytes: u64) -> std::io::Result<()> {
    let _guard = FILE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::fs::create_dir_all(dir)?;
    let path = dir.join(LOG_FILE);
    let current = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
    if current > 0 && current + line.len() as u64 > max_bytes {
        std::fs::rename(&path, dir.join(ROTATED_FILE))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    file.write_all(line.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_and_rotates_once_past_the_limit() {
        let dir = std::env::temp_dir().join(format!(
            "ro-diagnostics-rotate-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        append_bounded(&dir, "first line\n", 32).unwrap();
        append_bounded(&dir, "second line\n", 32).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join(LOG_FILE)).unwrap(),
            "first line\nsecond line\n"
        );
        append_bounded(&dir, "third line that overflows\n", 32).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join(ROTATED_FILE)).unwrap(),
            "first line\nsecond line\n"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join(LOG_FILE)).unwrap(),
            "third line that overflows\n"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn report_without_an_app_still_writes_the_log() {
        report("test", "sin app registrada");
        let log = std::fs::read_to_string(diagnostics_dir().join(LOG_FILE)).unwrap();
        assert!(log.contains("[test] sin app registrada"));
    }
}
