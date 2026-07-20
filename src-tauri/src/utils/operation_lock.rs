use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

static ACTIVE_OPERATIONS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

/// Exclusión mutua no bloqueante para operaciones destructivas sobre una misma ruta.
///
/// Los comandos Tauri pueden invocarse en paralelo aunque la UI tenga botones deshabilitados. El
/// guard evita que dos setup/reset o dos operaciones dgVoodoo compitan por backups y manifiestos.
pub struct OperationGuard {
    key: String,
}

impl OperationGuard {
    pub fn acquire(namespace: &str, path: &Path) -> Result<Self, String> {
        let normalized = normalize_path(path);
        let key = format!("{namespace}:{}", normalized.display());
        let operations = ACTIVE_OPERATIONS.get_or_init(|| Mutex::new(HashSet::new()));
        let mut operations = operations
            .lock()
            .map_err(|_| "El registro de operaciones está bloqueado".to_string())?;
        if !operations.insert(key.clone()) {
            return Err(format!(
                "Ya hay una operación {namespace} en curso para {}",
                normalized.display()
            ));
        }
        Ok(Self { key })
    }
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        if let Some(operations) = ACTIVE_OPERATIONS.get() {
            if let Ok(mut operations) = operations.lock() {
                operations.remove(&self.key);
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
}
