use std::collections::HashMap;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use sha2::{Digest, Sha256};

static ACTIVE_OPERATIONS: OnceLock<Mutex<HashMap<String, LockState>>> = OnceLock::new();

#[derive(Debug, Default)]
struct LockState {
    readers: usize,
    writer: bool,
}

/// Exclusión mutua no bloqueante para operaciones destructivas sobre una misma ruta.
///
/// Los comandos Tauri pueden invocarse en paralelo aunque la UI tenga botones deshabilitados. El
/// guard evita que dos setup/reset o dos operaciones dgVoodoo compitan por backups y manifiestos.
///
/// El registro en memoria solo ve este proceso. Otra instancia del launcher o
/// `ro-launcher --import-runtime` son procesos distintos, así que cada guard también toma un
/// `flock(2)` no bloqueante (compartido o exclusivo) sobre un archivo por clave. El descriptor es
/// `O_CLOEXEC`: Wine y los sidecars no lo heredan, y el lock muere con el guard o con el proceso.
pub struct OperationGuard {
    key: String,
    shared: bool,
    _cross_process: File,
}

impl OperationGuard {
    pub fn acquire(namespace: &str, path: &Path) -> Result<Self, String> {
        Self::acquire_with_mode(namespace, path, false)
    }

    /// Registra un uso no destructivo de la ruta. Varios procesos pueden compartirlo, pero una
    /// operación exclusiva como setup/reset/install debe esperar a que todos terminen.
    pub fn acquire_shared(namespace: &str, path: &Path) -> Result<Self, String> {
        Self::acquire_with_mode(namespace, path, true)
    }

    fn acquire_with_mode(namespace: &str, path: &Path, shared: bool) -> Result<Self, String> {
        let normalized = normalize_path(path);
        let key = format!("{namespace}:{}", normalized.display());
        let operations = ACTIVE_OPERATIONS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut operations = operations
            .lock()
            .map_err(|_| "El registro de operaciones está bloqueado".to_string())?;
        let active = operations.entry(key.clone()).or_default();
        let unavailable = if shared {
            active.writer
        } else {
            active.writer || active.readers > 0
        };
        if unavailable {
            return Err(format!(
                "Ya hay una operación {namespace} en curso para {}",
                normalized.display()
            ));
        }
        let cross_process = lock_across_processes(&key, shared).map_err(|error| match error {
            CrossProcessLockError::Busy => format!(
                "Ya hay una operación {namespace} en curso para {} en otro proceso de RO-Launcher",
                normalized.display()
            ),
            CrossProcessLockError::Io(error) => format!(
                "No se pudo bloquear la operación {namespace} para {}: {error}",
                normalized.display()
            ),
        })?;
        if shared {
            active.readers += 1;
        } else {
            active.writer = true;
        }
        Ok(Self {
            key,
            shared,
            _cross_process: cross_process,
        })
    }
}

enum CrossProcessLockError {
    Busy,
    Io(std::io::Error),
}

fn lock_directory() -> PathBuf {
    #[cfg(test)]
    {
        std::env::temp_dir().join(format!("ro-launcher-test-locks-{}", unsafe {
            libc::getuid()
        }))
    }
    #[cfg(not(test))]
    {
        super::app_data_dir().join("locks")
    }
}

fn lock_file_path(key: &str) -> PathBuf {
    let digest = Sha256::digest(key.as_bytes());
    let name: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    lock_directory().join(format!("{name}.lock"))
}

fn lock_across_processes(key: &str, shared: bool) -> Result<File, CrossProcessLockError> {
    let path = lock_file_path(key);
    if let Some(parent) = path.parent() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)
            .map_err(CrossProcessLockError::Io)?;
    }
    // std opens with O_CLOEXEC; the file is never truncated or removed, so every process always
    // locks the same inode.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&path)
        .map_err(CrossProcessLockError::Io)?;
    let operation = if shared { libc::LOCK_SH } else { libc::LOCK_EX };
    loop {
        if unsafe { libc::flock(file.as_raw_fd(), operation | libc::LOCK_NB) } == 0 {
            return Ok(file);
        }
        let error = std::io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::EINTR) => continue,
            Some(libc::EWOULDBLOCK) => return Err(CrossProcessLockError::Busy),
            _ => return Err(CrossProcessLockError::Io(error)),
        }
    }
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        if let Some(operations) = ACTIVE_OPERATIONS.get() {
            if let Ok(mut operations) = operations.lock() {
                let remove = if let Some(active) = operations.get_mut(&self.key) {
                    if self.shared {
                        active.readers = active.readers.saturating_sub(1);
                    } else {
                        active.writer = false;
                    }
                    active.readers == 0 && !active.writer
                } else {
                    false
                };
                if remove {
                    operations.remove(&self.key);
                }
            }
        }
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    ro_session_protocol::canonicalize_prefix_path(path).unwrap_or_else(|| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn missing_parents_under_a_home_alias_keep_the_same_lock_after_creation() {
        let root = std::env::temp_dir().join(format!("ro-lock-alias-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(root.join("home")).unwrap();
        std::os::unix::fs::symlink(root.join("home"), root.join("alias")).unwrap();
        let prefix = root.join("alias/.local/share/ro-launcher/prefixes/new");
        let guard = OperationGuard::acquire("prefix", &prefix).unwrap();
        std::fs::create_dir_all(&prefix).unwrap();
        assert!(OperationGuard::acquire(
            "prefix",
            &root.join("home/.local/share/ro-launcher/prefixes/new")
        )
        .is_err());
        drop(guard);
        assert!(OperationGuard::acquire("prefix", &prefix).is_ok());
        std::fs::remove_dir_all(root).unwrap();
    }

    /// Another process (a second launcher or the import CLI) holding the key's flock must block
    /// both exclusive and shared admission here. A separate open file description is exactly what
    /// another process would hold.
    #[test]
    fn another_process_holding_the_lock_blocks_admission() {
        let path = std::env::temp_dir().join(format!(
            "ro-launcher-operation-cross-process-{}",
            std::process::id()
        ));
        let key = format!("runtime:{}", normalize_path(&path).display());
        let foreign = lock_across_processes(&key, false)
            .ok()
            .expect("foreign exclusive lock");
        let error = OperationGuard::acquire("runtime", &path).err().unwrap();
        assert!(error.contains("otro proceso"), "{error}");
        assert!(OperationGuard::acquire_shared("runtime", &path).is_err());
        drop(foreign);

        let foreign_reader = lock_across_processes(&key, true)
            .ok()
            .expect("foreign shared lock");
        let reader = OperationGuard::acquire_shared("runtime", &path).unwrap();
        assert!(OperationGuard::acquire("runtime", &path).is_err());
        drop(reader);
        assert!(OperationGuard::acquire("runtime", &path).is_err());
        drop(foreign_reader);
        let writer = OperationGuard::acquire("runtime", &path).unwrap();
        assert!(lock_across_processes(&key, true).is_err());
        drop(writer);
        assert!(lock_across_processes(&key, false).is_ok());
    }

    #[test]
    fn lock_descriptor_is_not_inherited_by_children() {
        let path = std::env::temp_dir().join(format!(
            "ro-launcher-operation-cloexec-{}",
            std::process::id()
        ));
        let guard = OperationGuard::acquire("prefix", &path).unwrap();
        let flags = unsafe { libc::fcntl(guard._cross_process.as_raw_fd(), libc::F_GETFD) };
        assert!(flags & libc::FD_CLOEXEC != 0);
    }

    #[test]
    fn serializes_the_same_namespace_and_path() {
        let path =
            std::env::temp_dir().join(format!("ro-launcher-operation-lock-{}", std::process::id()));
        let first = OperationGuard::acquire("prefix", &path).unwrap();
        assert!(OperationGuard::acquire("prefix", &path).is_err());
        assert!(OperationGuard::acquire("dgvoodoo", &path).is_ok());
        drop(first);
        assert!(OperationGuard::acquire("prefix", &path).is_ok());
    }

    #[test]
    fn permits_shared_users_and_blocks_exclusive_mutation() {
        let path = std::env::temp_dir().join(format!(
            "ro-launcher-operation-shared-{}",
            std::process::id()
        ));
        let first = OperationGuard::acquire_shared("prefix", &path).unwrap();
        let second = OperationGuard::acquire_shared("prefix", &path).unwrap();
        assert!(OperationGuard::acquire("prefix", &path).is_err());
        drop(first);
        assert!(OperationGuard::acquire("prefix", &path).is_err());
        drop(second);
        assert!(OperationGuard::acquire("prefix", &path).is_ok());
    }

    #[test]
    fn shared_dgvoodoo_blocks_exclusive_install() {
        let path = std::env::temp_dir().join(format!(
            "ro-launcher-operation-dgvoodoo-{}",
            std::process::id()
        ));
        let launch = OperationGuard::acquire_shared("dgvoodoo", &path).unwrap();
        assert!(OperationGuard::acquire("dgvoodoo", &path).is_err());
        drop(launch);
        assert!(OperationGuard::acquire("dgvoodoo", &path).is_ok());
    }
}
