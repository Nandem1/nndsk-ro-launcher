use super::*;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::os::unix::fs::symlink;
use std::os::unix::process::CommandExt;
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const CHILD_TEST: &str = "tools::updater::activate::tests::activation_child";

struct Fixture {
    root: PathBuf,
    target: PathBuf,
    old: Vec<u8>,
    new: Vec<u8>,
    mode: u32,
}

// Synthetic header fixture matching the shipped ELF64 LE AppImage type-2 runtime.
// Not executable game data and not a second signature verifier.
fn appimage(fill: u8) -> Vec<u8> {
    let mut bytes = vec![fill; 4096];
    bytes[..64].fill(0);
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    bytes[8..11].copy_from_slice(b"AI\x02");
    bytes[16..18].copy_from_slice(&3u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&62u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1u32.to_le_bytes());
    bytes[52..54].copy_from_slice(&64u16.to_le_bytes());
    bytes
}

impl Fixture {
    fn new(mode: u32) -> Self {
        let root = std::env::temp_dir().join(format!("ro-f03-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let target = root.join("RO Launcher.AppImage");
        let old = appimage(0x31);
        let new = appimage(0x72);
        fs::write(&target, &old).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(mode)).unwrap();
        Self {
            root,
            target,
            old,
            new,
            mode,
        }
    }

    fn temp(&self) -> PathBuf {
        temporary_path(&self.target).unwrap()
    }

    fn assert_target(&self, expected: &[u8]) {
        assert_eq!(fs::read(&self.target).unwrap(), expected);
        assert_eq!(
            fs::metadata(&self.target).unwrap().permissions().mode() & 0o7777,
            self.mode
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Also clean up after a failed assertion in the read-only-parent test.
        fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700)).unwrap();
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn accepts_shipped_appimage_header_identity() {
    validate_payload(&appimage(1)).unwrap();
}

#[test]
fn replaces_exact_bytes_preserves_modes_and_cleans_temp() {
    for mode in [0o755, 0o751, 0o700] {
        let f = Fixture::new(mode);
        // The running image/mount can continue holding the old inode.
        let mut old_inode = File::open(&f.target).unwrap();
        activate_appimage(&f.target, &f.new).unwrap();
        f.assert_target(&f.new);
        assert!(!f.temp().exists());
        let mut still_old = Vec::new();
        old_inode.read_to_end(&mut still_old).unwrap();
        assert_eq!(still_old, f.old);
    }
}

#[test]
fn stale_regular_temp_is_recreated_without_touching_target_before_commit() {
    let f = Fixture::new(0o755);
    fs::write(f.temp(), b"incomplete residue").unwrap();
    activate_inner(&f.target, &f.new, |point| {
        if point != Checkpoint::AfterRename {
            f.assert_target(&f.old);
        }
        Ok(())
    })
    .unwrap();
    f.assert_target(&f.new);
    assert!(!f.temp().exists());
}

fn rejects_without_mutation(bytes: &[u8]) {
    let f = Fixture::new(0o751);
    fs::write(f.temp(), b"existing residue").unwrap();
    assert!(matches!(
        activate_appimage(&f.target, bytes),
        Err(BackendError::Install(_))
    ));
    f.assert_target(&f.old);
    assert_eq!(fs::read(f.temp()).unwrap(), b"existing residue");
}

#[test]
fn gzip_is_rejected_without_mutation() {
    rejects_without_mutation(&[0x1f, 0x8b, 8, 0]);
}

#[test]
fn tar_is_rejected_without_mutation() {
    let mut tar = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(4096);
    header.set_mode(0o755);
    header.set_cksum();
    tar.append_data(&mut header, "new.AppImage", appimage(2).as_slice())
        .unwrap();
    rejects_without_mutation(&tar.into_inner().unwrap());
    // A legacy tar need not have ustar magic: the positive AppImage check still rejects it.
    rejects_without_mutation(&vec![0; 1024]);
}

#[test]
fn random_bytes_are_rejected_without_mutation() {
    rejects_without_mutation(b"not an appimage");
    rejects_without_mutation(&vec![0x73; 4096]);
}

#[test]
fn every_truncated_header_is_rejected_without_mutation() {
    let bytes = appimage(2);
    for length in 0..=64 {
        rejects_without_mutation(&bytes[..length]);
    }
}

#[test]
fn malformed_and_plain_elf_headers_are_rejected_without_mutation() {
    for offset in [0, 4, 5, 6, 8, 10, 16, 20, 52] {
        let mut bytes = appimage(2);
        bytes[offset] = 0;
        rejects_without_mutation(&bytes);
    }
}

#[test]
fn symlink_temp_is_refused_and_referent_is_untouched() {
    let f = Fixture::new(0o755);
    let unrelated = f.root.join("unrelated");
    fs::write(&unrelated, b"preserve me").unwrap();
    symlink(&unrelated, f.temp()).unwrap();
    assert!(activate_appimage(&f.target, &f.new).is_err());
    f.assert_target(&f.old);
    assert_eq!(fs::read(unrelated).unwrap(), b"preserve me");
    assert!(fs::symlink_metadata(f.temp())
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn directory_temp_is_refused_and_preserved() {
    let f = Fixture::new(0o755);
    fs::create_dir(f.temp()).unwrap();
    fs::write(f.temp().join("unrelated"), b"preserve me").unwrap();
    assert!(activate_appimage(&f.target, &f.new).is_err());
    f.assert_target(&f.old);
    assert_eq!(
        fs::read(f.temp().join("unrelated")).unwrap(),
        b"preserve me"
    );
}

#[test]
fn hardlinked_temp_is_refused_and_preserved() {
    let f = Fixture::new(0o755);
    fs::hard_link(&f.target, f.temp()).unwrap();
    assert!(activate_appimage(&f.target, &f.new).is_err());
    f.assert_target(&f.old);
    assert_eq!(fs::read(f.temp()).unwrap(), f.old);
}

#[test]
fn fifo_temp_is_refused_without_blocking() {
    use std::os::unix::ffi::OsStrExt;
    let f = Fixture::new(0o755);
    let path = std::ffi::CString::new(f.temp().as_os_str().as_bytes()).unwrap();
    // SAFETY: path is a live NUL-terminated string; mkfifo has no other pointer inputs.
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
    assert!(activate_appimage(&f.target, &f.new).is_err());
    f.assert_target(&f.old);
}

#[test]
fn symlink_and_nonregular_targets_are_refused() {
    let f = Fixture::new(0o755);
    let link = f.root.join("link.AppImage");
    symlink(&f.target, &link).unwrap();
    assert!(activate_appimage(&link, &f.new).is_err());
    assert!(activate_appimage(&f.root, &f.new).is_err());
    f.assert_target(&f.old);
}

#[test]
fn nonexecutable_target_is_refused_without_mutation() {
    let f = Fixture::new(0o644);
    assert!(activate_appimage(&f.target, &f.new).is_err());
    f.assert_target(&f.old);
    assert!(!f.temp().exists());
}

#[test]
fn missing_parent_and_relative_target_fail_safely() {
    let f = Fixture::new(0o755);
    assert!(activate_appimage(&f.root.join("missing/new.AppImage"), &f.new).is_err());
    assert!(activate_appimage(Path::new("relative.AppImage"), &f.new).is_err());
    f.assert_target(&f.old);
    assert!(!f.root.join("missing").exists());
}

#[test]
fn symlink_parent_is_refused_before_residue_cleanup() {
    let f = Fixture::new(0o755);
    fs::write(f.temp(), b"existing residue").unwrap();
    let link = f.root.join("parent-link");
    symlink(&f.root, &link).unwrap();
    assert!(activate_appimage(&link.join(f.target.file_name().unwrap()), &f.new).is_err());
    f.assert_target(&f.old);
    assert_eq!(fs::read(f.temp()).unwrap(), b"existing residue");
}

#[test]
fn precommit_error_cleans_temp_and_preserves_old_target() {
    for fail_at in [Checkpoint::TempCreated, Checkpoint::BeforeRename] {
        let f = Fixture::new(0o755);
        let result = activate_inner(&f.target, &f.new, |point| {
            if point == fail_at {
                Err(io::Error::other("injected pre-commit failure"))
            } else {
                Ok(())
            }
        });
        assert!(result.is_err());
        f.assert_target(&f.old);
        assert!(!f.temp().exists());
    }
}

#[test]
fn cleanup_failure_is_reported_and_preserves_old_target() {
    let f = Fixture::new(0o755);
    let result = activate_inner(&f.target, &f.new, |point| {
        if point == Checkpoint::BeforeRename {
            fs::remove_file(f.temp())?;
            fs::create_dir(f.temp())?;
            return Err(io::Error::other("injected pre-commit failure"));
        }
        Ok(())
    });
    assert!(
        matches!(result, Err(BackendError::Install(message)) if message.contains("no se pudo retirar"))
    );
    f.assert_target(&f.old);
    assert!(f.temp().is_dir());
}

#[test]
fn postcommit_error_reports_activation_and_does_not_roll_back() {
    let f = Fixture::new(0o751);
    let result = activate_inner(&f.target, &f.new, |point| {
        if point == Checkpoint::AfterRename {
            Err(io::Error::other("injected post-commit failure"))
        } else {
            Ok(())
        }
    });
    assert!(matches!(result, Err(BackendError::Install(message)) if message.contains("activado")));
    f.assert_target(&f.new);
    assert!(!f.temp().exists());
}

// All environment hooks, checkpoints and rendezvous code compile ONLY in the test binary.
#[test]
#[ignore = "subprocess entrypoint, driven by parent fault-injection tests"]
fn activation_child() {
    let target = PathBuf::from(std::env::var_os("RO_F03_TEST_TARGET").unwrap());
    let action = std::env::var("RO_F03_TEST_ACTION").unwrap();
    let bytes = appimage(0x72);
    if action == "read-only-parent" {
        assert!(
            matches!(activate_appimage(&target, &bytes), Err(BackendError::Install(message)) if message.contains("crear temporal"))
        );
        return;
    }
    if action == "write-failure" {
        let limit = libc::rlimit {
            rlim_cur: 128,
            rlim_max: 128,
        };
        // SAFETY: this disposable subprocess changes only its own signal disposition
        // and file-size limit. limit points to an initialized live rlimit.
        unsafe {
            assert_ne!(libc::signal(libc::SIGXFSZ, libc::SIG_IGN), libc::SIG_ERR);
            assert_eq!(libc::setrlimit(libc::RLIMIT_FSIZE, &limit), 0);
        }
        assert!(activate_appimage(&target, &bytes).is_err());
        return;
    }
    activate_inner(&target, &bytes, |point| {
        let pause = (action == "before-rename" && point == Checkpoint::BeforeRename)
            || (action == "after-rename" && point == Checkpoint::AfterRename);
        if pause {
            println!("F03-CHECKPOINT:{action}");
            io::stdout().flush()?;
            // Rendezvous, no sleeps. Parent keeps this pipe open until SIGKILL.
            io::stdin().read_exact(&mut [0u8])?;
        }
        Ok(())
    })
    .unwrap();
}

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.0.try_wait().unwrap().is_none() {
            // Child is owned and unreaped, so its PID cannot be reused before wait.
            self.0.kill().unwrap();
            self.0.wait().unwrap();
        }
    }
}

fn spawn_child(f: &Fixture, action: &str) -> ChildGuard {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", CHILD_TEST, "--ignored", "--nocapture"])
        .env("RO_F03_TEST_TARGET", &f.target)
        .env("RO_F03_TEST_ACTION", action)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    // SAFETY: geteuid takes no pointers. If run as root, use an unprivileged
    // child so DAC refusal is tested instead of silently bypassed by root.
    if action == "read-only-parent" && unsafe { libc::geteuid() } == 0 {
        command.uid(65534).gid(65534);
    }
    ChildGuard(command.spawn().unwrap())
}

fn kill_at_checkpoint(f: &Fixture, action: &str) {
    let mut child = spawn_child(f, action);
    let stdout = child.0.stdout.take().unwrap();
    let expected = format!("F03-CHECKPOINT:{action}");
    let (sender, receiver) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if line.unwrap() == expected {
                sender.send(()).unwrap();
                return;
            }
        }
    });
    receiver
        .recv_timeout(Duration::from_secs(15))
        .expect("child did not reach activation checkpoint");
    // On Unix Child::kill sends SIGKILL. wait proves real process death and reaps it.
    child.0.kill().unwrap();
    assert_eq!(child.0.wait().unwrap().signal(), Some(libc::SIGKILL));
    reader.join().unwrap();
}

#[test]
fn sigkill_before_rename_preserves_old_and_retry_recovers_residue() {
    for _ in 0..5 {
        let f = Fixture::new(0o755);
        kill_at_checkpoint(&f, "before-rename");
        f.assert_target(&f.old);
        assert_eq!(fs::read(f.temp()).unwrap(), f.new);
        assert_eq!(
            fs::metadata(f.temp()).unwrap().permissions().mode() & 0o7777,
            f.mode
        );
        activate_appimage(&f.target, &f.new).unwrap();
        f.assert_target(&f.new);
        assert!(!f.temp().exists());
    }
}

#[test]
fn sigkill_after_rename_preserves_new_before_directory_fsync() {
    for _ in 0..5 {
        let f = Fixture::new(0o751);
        kill_at_checkpoint(&f, "after-rename");
        f.assert_target(&f.new);
        assert!(!f.temp().exists());
    }
}

#[test]
fn short_write_failure_preserves_target_and_cleans_temp() {
    let f = Fixture::new(0o755);
    let mut child = spawn_child(&f, "write-failure");
    wait_for_success(&mut child);
    f.assert_target(&f.old);
    assert!(!f.temp().exists());
}

fn wait_for_success(child: &mut ChildGuard) {
    // Bound completion using pipe EOF, not timing sleeps.
    let stdout = child.0.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        io::copy(&mut BufReader::new(stdout), &mut io::sink()).unwrap();
        sender.send(()).unwrap();
    });
    receiver.recv_timeout(Duration::from_secs(15)).unwrap();
    assert!(child.0.wait().unwrap().success());
    reader.join().unwrap();
}

#[test]
fn read_only_parent_preserves_target_without_creating_temp() {
    let f = Fixture::new(0o755);
    fs::set_permissions(&f.root, fs::Permissions::from_mode(0o555)).unwrap();
    let mut child = spawn_child(&f, "read-only-parent");
    wait_for_success(&mut child);
    f.assert_target(&f.old);
    assert!(!f.temp().exists());
    fs::set_permissions(&f.root, fs::Permissions::from_mode(0o755)).unwrap();
}
