//! Exec'd copies of the test binary exercise the real OperationGuard registry and fcntl path.
//! Pipes rendezvous at ownership and at the successor's first failed kernel lock attempt.

use super::*;
use std::io::{BufRead, Read, Write};
use std::os::unix::fs::MetadataExt;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const CHILD_TEST: &str = "utils::operation_lock::instance_tests::instance_child";
const EVENT: &str = "INSTANCE_EVENT ";

struct Process {
    child: Child,
    input: ChildStdin,
    output: ChildStdout,
}

impl Process {
    fn start(root: &Path, mode: &str, timeout: Duration) -> Self {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", CHILD_TEST, "--ignored", "--nocapture"])
            .env("RO_TEST_INSTANCE_ROOT", root)
            .env("RO_TEST_INSTANCE_MODE", mode)
            .env(
                "RO_TEST_INSTANCE_TIMEOUT_MS",
                timeout.as_millis().to_string(),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        Self {
            input: child.stdin.take().unwrap(),
            output: child.stdout.take().unwrap(),
            child,
        }
    }

    fn event(&mut self) -> String {
        let started = Instant::now();
        let mut line = Vec::new();
        loop {
            let remaining = Duration::from_secs(5).saturating_sub(started.elapsed());
            assert!(!remaining.is_zero(), "subprocess rendezvous timed out");
            let mut poll = libc::pollfd {
                fd: self.output.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: poll points to one live pollfd; timeout is bounded to five seconds.
            let ready = unsafe { libc::poll(&mut poll, 1, remaining.as_millis() as i32) };
            assert!(ready > 0, "subprocess did not report an event");
            let mut byte = [0];
            self.output.read_exact(&mut byte).unwrap();
            if byte[0] == b'\n' {
                let text = String::from_utf8(std::mem::take(&mut line)).unwrap();
                if let Some((_, event)) = text.split_once(EVENT) {
                    return event.to_string();
                }
            } else {
                line.push(byte[0]);
            }
        }
    }

    fn exit_normally(&mut self) {
        self.input.write_all(b"exit\n").unwrap();
        self.wait(true);
    }

    fn sigkill(&mut self) {
        // Child::kill sends actual SIGKILL on Unix; wait reaps this exact owned child.
        self.child.kill().unwrap();
        self.wait(false);
    }

    fn wait(&mut self, success: bool) {
        let started = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert_eq!(status.success(), success, "{status}");
                if !success {
                    use std::os::unix::process::ExitStatusExt;
                    assert_eq!(status.signal(), Some(libc::SIGKILL));
                }
                return;
            }
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "child exit hung"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // Failure-path cleanup: kill only the child owned by this handle, then reap it.
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn root() -> PathBuf {
    std::env::temp_dir().join(format!("ro-instance-process-{}", uuid::Uuid::new_v4()))
}

fn event(message: impl std::fmt::Display) {
    println!("{EVENT}{message}");
    std::io::stdout().flush().unwrap();
}

#[test]
#[ignore = "subprocess entrypoint; invoked only by the instance schedules"]
fn instance_child() {
    let root = PathBuf::from(std::env::var_os("RO_TEST_INSTANCE_ROOT").unwrap());
    let mode = std::env::var("RO_TEST_INSTANCE_MODE").unwrap();
    let timeout = Duration::from_millis(
        std::env::var("RO_TEST_INSTANCE_TIMEOUT_MS")
            .unwrap()
            .parse()
            .unwrap(),
    );
    let key = format!("instance:{}", normalize_path(&root).display());
    let lock_path = lock_file_path(&key);
    if mode == "io" {
        std::fs::create_dir_all(&lock_path).unwrap();
    }
    let mut reported_busy = false;
    let started = Instant::now();
    let result = OperationGuard::acquire_for_inner("instance", &root, timeout, || {
        if !reported_busy {
            event("busy");
            reported_busy = true;
        }
    });
    match result {
        Ok(_guard) => {
            assert_ne!(mode, "io");
            event("owned");
            let mut line = String::new();
            std::io::stdin().lock().read_line(&mut line).unwrap();
            assert_eq!(line, "exit\n");
            // Model Tauri's final process::exit: no Drop, only kernel release of ownership.
            std::process::exit(0);
        }
        Err(OperationLockError::Busy(_)) => {
            assert_eq!(mode, "contender");
            event(format!("timeout {}", started.elapsed().as_millis()));
        }
        Err(OperationLockError::Io(message)) => {
            assert_eq!(mode, "io");
            assert!(!reported_busy);
            let startup_error = crate::utils::with_desktop_instance(&root, || {
                panic!("I/O failure entered desktop initialization")
            })
            .unwrap_err();
            assert!(startup_error.starts_with("No se pudo obtener la instancia"));
            assert!(!startup_error.contains("ya está abierto"));
            std::fs::remove_dir(&lock_path).unwrap();
            event(format!("io {} {message}", started.elapsed().as_millis()));
        }
        Err(error) => panic!("unexpected lock failure: {error}"),
    }
}

#[test]
fn instance_duplicate_owner_times_out_with_a_monotonic_bound() {
    let root = root();
    let mut owner = Process::start(&root, "owner", Duration::ZERO);
    assert_eq!(owner.event(), "owned");
    let started = Instant::now();
    let mut contender = Process::start(&root, "contender", Duration::from_millis(150));
    assert_eq!(contender.event(), "busy");
    let result = contender.event();
    let elapsed: u128 = result.strip_prefix("timeout ").unwrap().parse().unwrap();
    assert!((150..2000).contains(&elapsed), "waited {elapsed} ms");
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(owner.child.try_wait().unwrap().is_none());
    contender.wait(true);
    owner.exit_normally();
    let mut next = Process::start(&root, "owner", Duration::ZERO);
    assert_eq!(next.event(), "owned");
    next.exit_normally();
}

#[test]
fn instance_restart_successor_waits_until_predecessor_process_exit() {
    let root = root();
    let mut old = Process::start(&root, "owner", Duration::ZERO);
    assert_eq!(old.event(), "owned");
    let mut successor = Process::start(&root, "owner", Duration::from_secs(1));
    // Emitted from inside acquire_for, after the real fcntl attempt returned Busy.
    assert_eq!(successor.event(), "busy");
    assert!(OperationGuard::acquire("instance", &root).is_err());
    old.exit_normally();
    assert_eq!(successor.event(), "owned");
    assert!(OperationGuard::acquire("instance", &root).is_err());
    successor.exit_normally();
    assert!(OperationGuard::acquire("instance", &root).is_ok());
}

#[test]
fn instance_sigkill_releases_ownership_without_rust_cleanup() {
    let root = root();
    let mut owner = Process::start(&root, "owner", Duration::ZERO);
    assert_eq!(owner.event(), "owned");
    owner.sigkill();
    let mut next = Process::start(&root, "owner", Duration::ZERO);
    assert_eq!(next.event(), "owned");
    next.exit_normally();
}

#[test]
fn instance_different_roots_have_simultaneous_process_owners() {
    let first_root = root();
    let second_root = root();
    let mut first = Process::start(&first_root, "owner", Duration::ZERO);
    assert_eq!(first.event(), "owned");
    let mut second = Process::start(&second_root, "owner", Duration::ZERO);
    assert_eq!(second.event(), "owned");
    assert!(first.child.try_wait().unwrap().is_none());
    assert!(OperationGuard::acquire("instance", &first_root).is_err());
    assert!(OperationGuard::acquire("instance", &second_root).is_err());
    first.exit_normally();
    second.exit_normally();
}

#[test]
fn instance_persistent_lock_file_is_neither_truncated_nor_stale_ownership() {
    let root = root();
    let key = format!("instance:{}", normalize_path(&root).display());
    let path = lock_file_path(&key);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"persistent inode").unwrap();
    let inode = std::fs::metadata(&path).unwrap().ino();
    for _ in 0..2 {
        let mut owner = Process::start(&root, "owner", Duration::ZERO);
        assert_eq!(owner.event(), "owned");
        assert_eq!(std::fs::metadata(&path).unwrap().ino(), inode);
        owner.exit_normally();
        assert_eq!(std::fs::read(&path).unwrap(), b"persistent inode");
    }
    // Intentionally leave the .lock file in place, as production does.
}

#[test]
fn instance_home_alias_before_and_after_data_root_creation_has_one_key() {
    let base = root();
    std::fs::create_dir_all(base.join("home")).unwrap();
    std::os::unix::fs::symlink(base.join("home"), base.join("alias")).unwrap();
    let alias = base.join("alias/.local/share/ro-launcher");
    let canonical = base.join("home/.local/share/ro-launcher");
    let mut owner = Process::start(&alias, "owner", Duration::ZERO);
    assert_eq!(owner.event(), "owned");
    std::fs::create_dir_all(&canonical).unwrap();
    let mut contender = Process::start(&canonical, "contender", Duration::from_millis(50));
    assert_eq!(contender.event(), "busy");
    assert!(contender.event().starts_with("timeout "));
    contender.wait(true);
    owner.exit_normally();
    let mut next = Process::start(&canonical, "owner", Duration::ZERO);
    assert_eq!(next.event(), "owned");
    next.exit_normally();
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn instance_io_failure_is_immediate_and_never_retried_as_busy() {
    let mut child = Process::start(&root(), "io", Duration::from_secs(2));
    let result = child.event();
    let elapsed: u128 = result
        .strip_prefix("io ")
        .unwrap()
        .split(' ')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(elapsed < 500, "I/O failure waited {elapsed} ms");
    assert!(result.contains("No se pudo bloquear la operación instance"));
    child.wait(true);
}
