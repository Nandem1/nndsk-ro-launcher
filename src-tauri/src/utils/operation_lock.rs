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
    /// The process-wide record lock for this key, held while any guard of the key is alive.
    cross_process: Option<File>,
}

/// Exclusión mutua no bloqueante para operaciones destructivas sobre una misma ruta.
///
/// Los comandos Tauri pueden invocarse en paralelo aunque la UI tenga botones deshabilitados. El
/// guard evita que dos setup/reset o dos operaciones dgVoodoo compitan por backups y manifiestos.
///
/// El registro en memoria solo ve este proceso. Otra instancia del launcher o
/// `ro-launcher --import-runtime` son procesos distintos, así que cada clave también toma un lock
/// de registro POSIX (`fcntl(F_SETLK)`, lectura o escritura) sobre un archivo por clave.
///
/// No se usa `flock(2)`: pertenece a la descripción de archivo abierta y un `fork` de cualquier
/// hilo (Wine, sidecars) la hereda hasta su `exec`, así que un guard ya liberado podía seguir
/// bloqueando. Los locks POSIX pertenecen al proceso, no se heredan con `fork` y mueren con el
/// proceso. Un único descriptor por clave vive en el registro y se cierra bajo el mismo mutex.
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
        // Shared readers in this process reuse the read lock that the first one took.
        if active.cross_process.is_none() {
            match lock_across_processes(&key, shared) {
                Ok(file) => active.cross_process = Some(file),
                Err(error) => {
                    operations.remove(&key);
                    return Err(match error {
                        CrossProcessLockError::Busy => format!(
                            "Ya hay una operación {namespace} en curso para {} en otro proceso de RO-Launcher",
                            normalized.display()
                        ),
                        CrossProcessLockError::Io(error) => format!(
                            "No se pudo bloquear la operación {namespace} para {}: {error}",
                            normalized.display()
                        ),
                    });
                }
            }
        }
        if shared {
            active.readers += 1;
        } else {
            active.writer = true;
        }
        Ok(Self { key, shared })
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
    // locks the same inode. Write access is required for a write lock.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&path)
        .map_err(CrossProcessLockError::Io)?;
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = if shared { libc::F_RDLCK } else { libc::F_WRLCK } as libc::c_short;
    lock.l_whence = libc::SEEK_SET as libc::c_short;
    loop {
        if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &lock) } == 0 {
            return Ok(file);
        }
        let error = std::io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::EINTR) => continue,
            Some(libc::EACCES | libc::EAGAIN) => return Err(CrossProcessLockError::Busy),
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

    /// Forks a child that takes the key's record lock itself, i.e. a genuinely different process
    /// such as a second launcher or the import CLI. Returns (pid, write end kept by the parent).
    fn foreign_process_lock(key: &str, shared: bool) -> libc::pid_t {
        use std::os::unix::ffi::OsStrExt;
        let path = lock_file_path(key);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        let mut ready = [0 as libc::c_int; 2];
        assert_eq!(unsafe { libc::pipe(ready.as_mut_ptr()) }, 0);
        let mut lock: libc::flock = unsafe { std::mem::zeroed() };
        lock.l_type = if shared { libc::F_RDLCK } else { libc::F_WRLCK } as libc::c_short;
        lock.l_whence = libc::SEEK_SET as libc::c_short;
        let pid = unsafe { libc::fork() };
        assert!(pid >= 0);
        if pid == 0 {
            // Child: async-signal-safe calls only, then wait to be killed.
            unsafe {
                close_inherited_except(ready[1]);
                let fd = libc::open(c_path.as_ptr(), libc::O_RDWR | libc::O_CREAT, 0o600);
                let status: u8 = if fd >= 0 && libc::fcntl(fd, libc::F_SETLK, &lock) == 0 {
                    1
                } else {
                    0
                };
                libc::write(ready[1], (&status as *const u8).cast(), 1);
                loop {
                    libc::pause();
                }
            }
        }
        unsafe { libc::close(ready[1]) };
        let mut status = 0u8;
        assert_eq!(
            unsafe { libc::read(ready[0], (&mut status as *mut u8).cast(), 1) },
            1
        );
        unsafe { libc::close(ready[0]) };
        assert_eq!(status, 1, "child could not take the lock");
        pid
    }

    /// Closes every descriptor in a forked test child except `keep`, so it does not hold pipes or
    /// freshly written executables of tests running on other threads (ETXTBSY, missing EOF).
    unsafe fn close_inherited_except(keep: libc::c_int) {
        if keep > 3 {
            libc::syscall(libc::SYS_close_range, 3u32, (keep - 1) as u32, 0u32);
        }
        libc::syscall(
            libc::SYS_close_range,
            (keep.max(2) + 1) as u32,
            u32::MAX,
            0u32,
        );
    }

    fn reap(pid: libc::pid_t) {
        unsafe {
            libc::kill(pid, libc::SIGKILL);
            libc::waitpid(pid, std::ptr::null_mut(), 0);
        }
    }

    #[test]
    fn another_process_holding_the_lock_blocks_admission() {
        let path = std::env::temp_dir().join(format!(
            "ro-launcher-operation-cross-process-{}",
            std::process::id()
        ));
        let key = format!("runtime:{}", normalize_path(&path).display());

        let writer = foreign_process_lock(&key, false);
        let error = OperationGuard::acquire("runtime", &path).err().unwrap();
        assert!(error.contains("otro proceso"), "{error}");
        assert!(OperationGuard::acquire_shared("runtime", &path).is_err());
        reap(writer);

        let reader = foreign_process_lock(&key, true);
        let shared = OperationGuard::acquire_shared("runtime", &path).unwrap();
        drop(shared);
        assert!(OperationGuard::acquire("runtime", &path).is_err());
        reap(reader);
        assert!(OperationGuard::acquire("runtime", &path).is_ok());
    }

    /// Regression: with flock(2) a fork inherited the lock's open file description, so a released
    /// guard stayed locked until that child exec'd or exited. The launcher forks Wine, UMU and
    /// sidecars from other threads at any time.
    #[test]
    fn a_forked_child_does_not_keep_a_released_lock() {
        let path =
            std::env::temp_dir().join(format!("ro-launcher-operation-fork-{}", std::process::id()));
        let guard = OperationGuard::acquire("prefix", &path).unwrap();
        let lock_fd = ACTIVE_OPERATIONS.get().unwrap().lock().unwrap()[&guard.key]
            .cross_process
            .as_ref()
            .unwrap()
            .as_raw_fd();
        let child = unsafe { libc::fork() };
        assert!(child >= 0);
        if child == 0 {
            // Keep exactly what a forked Wine/sidecar would inherit from this guard.
            unsafe {
                close_inherited_except(lock_fd);
                libc::sleep(2);
                libc::_exit(0);
            }
        }
        drop(guard);
        let again = OperationGuard::acquire("prefix", &path);
        reap(child);
        assert!(again.is_ok(), "{:?}", again.err());
    }

    #[test]
    fn lock_descriptor_is_not_inherited_by_children() {
        let path = std::env::temp_dir().join(format!(
            "ro-launcher-operation-cloexec-{}",
            std::process::id()
        ));
        let guard = OperationGuard::acquire("prefix", &path).unwrap();
        let operations = ACTIVE_OPERATIONS.get().unwrap().lock().unwrap();
        let fd = operations[&guard.key]
            .cross_process
            .as_ref()
            .unwrap()
            .as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
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
