//! Desktop ownership surrounds all product initialization and exit cleanup. The explicit
//! headless importer never enters this boundary and keeps its resource-level guards.

use std::path::Path;
use std::time::Duration;

use super::{OperationGuard, OperationLockError};

// Tauri 2.11.2 app.rs:1422-1428 completes RunEvent::Exit and Tauri cleanup BEFORE
// process.rs:74-91 spawns the successor and immediately exits. One second allows
// scheduling that final handoff, without waiting for service cleanup in a duplicate launch.
const HANDOFF_TIMEOUT: Duration = Duration::from_secs(1);

pub fn with_desktop_instance(data_root: &Path, initialize: impl FnOnce()) -> Result<(), String> {
    with_instance(data_root, HANDOFF_TIMEOUT, initialize).map_err(|error| match error {
        OperationLockError::Busy(_) => {
            format!("RO-Launcher ya está abierto para {}", data_root.display())
        }
        error => format!("No se pudo obtener la instancia de RO-Launcher: {error}"),
    })
}

fn with_instance(
    data_root: &Path,
    timeout: Duration,
    initialize: impl FnOnce(),
) -> Result<(), OperationLockError> {
    let _instance = OperationGuard::acquire_for("instance", data_root, timeout)?;
    // The lexical guard includes Builder construction, setup, the complete .run(),
    // RunEvent::Exit cleanup and restart's spawn/exit. process::exit/SIGKILL release
    // the kernel lock even when Rust destructors do not run.
    initialize();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("ro-desktop-instance-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn instance_refusal_never_enters_desktop_initialization() {
        let root = root();
        let _owner = OperationGuard::acquire("instance", &root).unwrap();
        let entered = std::cell::Cell::new(false);
        let result = with_instance(&root, Duration::from_millis(50), || entered.set(true));
        assert!(matches!(result, Err(OperationLockError::Busy(_))));
        assert!(!entered.get());
        let message = with_desktop_instance(&root, || panic!("duplicate initialized")).unwrap_err();
        assert!(message.starts_with("RO-Launcher ya está abierto para "));
    }

    #[test]
    fn instance_guard_survives_the_entire_desktop_callback() {
        let root = root();
        with_desktop_instance(&root, || {
            assert!(OperationGuard::acquire("instance", &root).is_err());
            // The importer uses a separate namespace, even while the desktop owns the root.
            assert!(OperationGuard::acquire("runtime", &root).is_ok());
        })
        .unwrap();
        assert!(OperationGuard::acquire("instance", &root).is_ok());
    }
}
