use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

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
pub struct OperationGuard {
    key: String,
    shared: bool,
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
        if shared {
            active.readers += 1;
        } else {
            active.writer = true;
        }
        Ok(Self { key, shared })
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
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return canonical;
    }
    let Some(parent) = path.parent() else {
        return path.to_path_buf();
    };
    let canonical_parent = std::fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
    path.file_name()
        .map(|name| canonical_parent.join(name))
        .unwrap_or(canonical_parent)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
