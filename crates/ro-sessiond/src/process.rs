use ro_session_protocol::{validate_process_spec_structure, ProcessSpec};
use ro_tools_linux::{
    capture_process_identity, signal_process_identity, verify_process_identity, ProcessIdentity,
};
use std::collections::HashMap;
use std::ffi::CString;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::io::{FromRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

const RUNNER_LINE_MAX: usize = 64 * 1024;

pub fn canonicalize_prefix(path: &Path) -> Option<PathBuf> {
    if let Ok(canonical) = fs::canonicalize(path) {
        return Some(canonical);
    }
    let parent = path.parent()?;
    let canonical_parent = fs::canonicalize(parent).ok()?;
    path.file_name().map(|name| canonical_parent.join(name))
}

pub fn validate_spec_for_launch(
    spec: &ProcessSpec,
    owned_prefix: &Path,
    base_env: &HashMap<String, String>,
) -> Result<HashMap<String, String>, String> {
    validate_process_spec_structure(spec).map_err(|e| e.message().to_string())?;

    let program = Path::new(&spec.program);
    if !program.is_absolute() {
        return Err("program must be an absolute path".into());
    }
    if !is_executable(program) {
        return Err(format!("program is not executable: {}", spec.program));
    }

    let cwd = Path::new(&spec.cwd);
    if !cwd.is_absolute() {
        return Err("cwd must be an absolute path".into());
    }
    let meta = fs::metadata(cwd).map_err(|e| format!("cwd inaccessible: {e}"))?;
    if !meta.is_dir() {
        return Err("cwd must be a directory".into());
    }

    let mut env = base_env.clone();
    // Prefix ownership must come from the validated request, not from a Steam/wrapper environment
    // inherited by the launcher.
    env.remove("WINEPREFIX");
    env.remove("STEAM_COMPAT_DATA_PATH");
    for change in &spec.env {
        match &change.value {
            Some(value) => {
                env.insert(change.key.clone(), value.clone());
            }
            None => {
                env.remove(&change.key);
            }
        }
    }

    let wineprefix = env
        .get("WINEPREFIX")
        .ok_or_else(|| "WINEPREFIX missing after merge".to_string())?;
    let canon_spec_prefix = canonicalize_prefix(Path::new(wineprefix))
        .ok_or_else(|| "WINEPREFIX path could not be canonicalized".to_string())?;
    if canon_spec_prefix != owned_prefix {
        return Err("WINEPREFIX does not match owned prefix".into());
    }

    if let Some(steam_data) = env.get("STEAM_COMPAT_DATA_PATH") {
        let canon_steam = canonicalize_prefix(Path::new(steam_data))
            .ok_or_else(|| "STEAM_COMPAT_DATA_PATH could not be canonicalized".to_string())?;
        if canon_steam != owned_prefix {
            return Err("STEAM_COMPAT_DATA_PATH does not match owned prefix".into());
        }
    }

    Ok(env)
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .ok()
        .filter(|m| m.is_file())
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[derive(Debug, Clone)]
pub struct LaunchControllerError {
    pub message: String,
    pub errno: Option<i32>,
}

impl LaunchControllerError {
    fn syscall(prefix: &str) -> Self {
        let err = std::io::Error::last_os_error();
        Self {
            message: format!("{prefix}: {err}"),
            errno: err.raw_os_error(),
        }
    }
}

pub struct LaunchOutcome {
    pub controller_pid: u32,
    stream_threads: Vec<thread::JoinHandle<()>>,
}

impl LaunchOutcome {
    pub fn take_stream_handles(&mut self) -> Vec<thread::JoinHandle<()>> {
        std::mem::take(&mut self.stream_threads)
    }
}

pub fn join_finished_stream_handles(handles: &mut Vec<thread::JoinHandle<()>>) {
    let mut pending = Vec::new();
    for handle in handles.drain(..) {
        if handle.is_finished() {
            let _ = handle.join();
        } else {
            pending.push(handle);
        }
    }
    *handles = pending;
}

pub fn join_all_stream_handles(handles: &mut Vec<thread::JoinHandle<()>>) {
    for handle in handles.drain(..) {
        let _ = handle.join();
    }
}

pub fn launch_controller(
    spec: &ProcessSpec,
    merged_env: &HashMap<String, String>,
    stderr_writer: Arc<Mutex<File>>,
    expected_parent: u32,
    mut on_spawn: impl FnMut(u32),
) -> Result<LaunchOutcome, LaunchControllerError> {
    let mut argv: Vec<CString> = Vec::with_capacity(spec.args.len() + 1);
    argv.push(
        CString::new(spec.program.as_bytes()).map_err(|_| LaunchControllerError {
            message: "program contains NUL".into(),
            errno: None,
        })?,
    );
    for arg in &spec.args {
        argv.push(
            CString::new(arg.as_bytes()).map_err(|_| LaunchControllerError {
                message: "arg contains NUL".into(),
                errno: None,
            })?,
        );
    }

    let cwd = CString::new(spec.cwd.as_bytes()).map_err(|_| LaunchControllerError {
        message: "cwd contains NUL".into(),
        errno: None,
    })?;

    let envp: Vec<CString> = merged_env
        .iter()
        .map(|(k, v)| {
            CString::new(format!("{k}={v}").into_bytes()).map_err(|_| LaunchControllerError {
                message: "env contains NUL".into(),
                errno: None,
            })
        })
        .collect::<Result<_, _>>()?;

    let mut argv_ptrs: Vec<*const libc::c_char> = argv.iter().map(|s| s.as_ptr()).collect();
    argv_ptrs.push(std::ptr::null());
    let mut envp_ptrs: Vec<*const libc::c_char> = envp.iter().map(|s| s.as_ptr()).collect();
    envp_ptrs.push(std::ptr::null());
    let argv0 = argv_ptrs[0];
    let argv_ptr = argv_ptrs.as_ptr();
    let envp_ptr = envp_ptrs.as_ptr();
    let cwd_ptr = cwd.as_ptr();

    let (stdout_r, stdout_w) = pipe_cloexec()?;
    let (stderr_r, stderr_w) = pipe_cloexec()?;
    let null_fd = open_dev_null()?;

    let pid = unsafe { libc::fork() };
    if pid < 0 {
        close_fd(stdout_w);
        close_fd(stderr_w);
        close_fd(null_fd);
        close_fd(stdout_r);
        close_fd(stderr_r);
        return Err(LaunchControllerError::syscall("fork failed"));
    }

    if pid == 0 {
        unsafe {
            child_exec(
                argv0,
                argv_ptr,
                envp_ptr,
                cwd_ptr,
                null_fd,
                stdout_w,
                stderr_w,
                stdout_r,
                stderr_r,
                expected_parent,
            );
        }
    }

    on_spawn(pid as u32);

    close_fd(stdout_w);
    close_fd(stderr_w);
    close_fd(null_fd);

    let stdout_thread = spawn_stream_forwarder(stdout_r, "[runner:stdout]", stderr_writer.clone());
    let stderr_thread = spawn_stream_forwarder(stderr_r, "[runner:stderr]", stderr_writer);

    Ok(LaunchOutcome {
        controller_pid: pid as u32,
        stream_threads: vec![stdout_thread, stderr_thread],
    })
}

#[allow(clippy::too_many_arguments)]
unsafe fn child_exec(
    argv0: *const libc::c_char,
    argv: *const *const libc::c_char,
    envp: *const *const libc::c_char,
    cwd: *const libc::c_char,
    null_fd: RawFd,
    stdout_w: RawFd,
    stderr_w: RawFd,
    stdout_r: RawFd,
    stderr_r: RawFd,
    expected_parent: u32,
) -> ! {
    libc::dup2(null_fd, libc::STDIN_FILENO);
    libc::dup2(stdout_w, libc::STDOUT_FILENO);
    libc::dup2(stderr_w, libc::STDERR_FILENO);
    libc::close(null_fd);
    libc::close(stdout_w);
    libc::close(stderr_w);
    libc::close(stdout_r);
    libc::close(stderr_r);

    if libc::chdir(cwd) != 0 {
        libc::write(
            libc::STDERR_FILENO,
            b"chdir failed\n".as_ptr() as *const libc::c_void,
            12,
        );
        libc::_exit(127);
    }
    libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
    if libc::getppid() as u32 != expected_parent {
        libc::write(
            libc::STDERR_FILENO,
            b"parent changed before exec\n".as_ptr() as *const libc::c_void,
            28,
        );
        libc::_exit(127);
    }

    libc::execve(argv0, argv, envp);
    libc::write(
        libc::STDERR_FILENO,
        b"execve failed\n".as_ptr() as *const libc::c_void,
        13,
    );
    libc::_exit(127);
}

fn spawn_stream_forwarder(
    read_fd: RawFd,
    prefix: &'static str,
    writer: Arc<Mutex<File>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut file = unsafe { File::from_raw_fd(read_fd) };
        let mut buf = [0u8; 8192];
        let mut carry = Vec::new();
        let mut continuation = false;
        loop {
            let n = match file.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            carry.extend_from_slice(&buf[..n]);
            continuation = flush_runner_carry(&writer, prefix, &mut carry, continuation);
        }
        if !carry.is_empty() {
            emit_runner_line(&writer, prefix, &carry, continuation);
        }
    })
}

#[cfg(test)]
pub(crate) fn flush_runner_carry_for_test(
    writer: &Arc<Mutex<File>>,
    prefix: &'static str,
    carry: &mut Vec<u8>,
    continuation: bool,
) -> bool {
    flush_runner_carry(writer, prefix, carry, continuation)
}

fn flush_runner_carry(
    writer: &Arc<Mutex<File>>,
    prefix: &'static str,
    carry: &mut Vec<u8>,
    mut continuation: bool,
) -> bool {
    while let Some(pos) = carry.iter().position(|&b| b == b'\n') {
        let line: Vec<u8> = carry.drain(..=pos).collect();
        let line = &line[..line.len().saturating_sub(1)];
        emit_runner_line(writer, prefix, line, continuation);
        continuation = false;
    }
    while carry.len() > RUNNER_LINE_MAX {
        let chunk: Vec<u8> = carry.drain(..RUNNER_LINE_MAX).collect();
        emit_runner_line(writer, prefix, &chunk, continuation);
        continuation = true;
    }
    continuation
}

fn emit_runner_line(writer: &Arc<Mutex<File>>, prefix: &str, line: &[u8], continuation: bool) {
    let text = String::from_utf8_lossy(line);
    let label = if continuation {
        format!("{prefix}[cont] ")
    } else {
        format!("{prefix} ")
    };
    let _ = writeln!(writer.lock().unwrap(), "{label}{text}");
}

fn pipe_cloexec() -> Result<(RawFd, RawFd), LaunchControllerError> {
    let mut fds = [0i32; 2];
    let rc = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
    if rc != 0 {
        return Err(LaunchControllerError::syscall("pipe2 failed"));
    }
    Ok((fds[0], fds[1]))
}

fn open_dev_null() -> Result<RawFd, LaunchControllerError> {
    let path = CString::new("/dev/null").map_err(|_| LaunchControllerError {
        message: "NUL in /dev/null path".into(),
        errno: None,
    })?;
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(LaunchControllerError::syscall("open /dev/null failed"));
    }
    Ok(fd)
}

fn close_fd(fd: RawFd) {
    unsafe {
        libc::close(fd);
    }
}

pub fn read_ppid(pid: u32) -> Option<u32> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let close_paren = stat.rfind(')')?;
    let fields: Vec<&str> = stat.get(close_paren + 1..)?.split_whitespace().collect();
    fields.get(1)?.parse().ok()
}

pub fn descendants_of(root: u32) -> Vec<ProcessIdentity> {
    let mut all_pids = Vec::new();
    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                if let Ok(pid) = name.parse::<u32>() {
                    all_pids.push(pid);
                }
            }
        }
    }
    all_pids
        .into_iter()
        .filter(|pid| *pid != root && is_descendant_of(*pid, root))
        .filter_map(capture_process_identity)
        .collect()
}

fn is_descendant_of(pid: u32, ancestor: u32) -> bool {
    if pid == ancestor {
        return true;
    }
    let mut current = pid;
    for _ in 0..64 {
        let Some(ppid) = read_ppid(current) else {
            return false;
        };
        if ppid == ancestor {
            return true;
        }
        if ppid <= 1 || ppid == current {
            return false;
        }
        current = ppid;
    }
    false
}

pub fn signal_descendants(root: u32, sig: i32) {
    for identity in descendants_of(root) {
        // Recheck ancestry and stable identity immediately before signalling. A process that was
        // reparented, exited, or reused is no longer a valid target.
        if is_descendant_of(identity.pid, root) && verify_process_identity(&identity) {
            let _ = signal_process_identity(&identity, sig);
        }
    }
}

pub fn capture_base_env() -> HashMap<String, String> {
    std::env::vars().collect()
}

pub type ReapResult = Result<Option<(u32, Option<i32>, Option<i32>)>, ()>;

pub fn waitpid_reap_with_signal() -> ReapResult {
    loop {
        let mut status: i32 = 0;
        let pid = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };
        if pid == 0 {
            return Ok(None);
        }
        if pid < 0 {
            let err = std::io::Error::last_os_error();
            match err.raw_os_error() {
                Some(libc::ECHILD) => return Err(()),
                Some(libc::EINTR) => continue,
                _ => return Ok(None),
            }
        }
        let exit_code = if libc::WIFEXITED(status) {
            Some(libc::WEXITSTATUS(status))
        } else {
            None
        };
        let signal = if libc::WIFSIGNALED(status) {
            Some(libc::WTERMSIG(status))
        } else {
            None
        };
        return Ok(Some((pid as u32, exit_code, signal)));
    }
}

#[cfg(test)]
mod runner_line_tests {
    use super::*;
    use ro_session_protocol::EnvironmentChange;

    #[test]
    fn runner_carry_sets_continuation_across_reads() {
        let file = fs::File::create("/dev/null").expect("/dev/null");
        let writer = Arc::new(Mutex::new(file));
        let mut carry = vec![b'a'; RUNNER_LINE_MAX + 1];
        let cont = flush_runner_carry_for_test(&writer, "[runner:stdout]", &mut carry, false);
        assert!(cont);
        assert_eq!(carry.len(), 1);
    }

    #[test]
    fn validated_request_replaces_inherited_prefix_ownership_variables() {
        let owned = std::env::temp_dir().join(format!("ro-sessiond-env-{}", std::process::id()));
        fs::create_dir_all(&owned).unwrap();
        let owned = fs::canonicalize(&owned).unwrap();
        let mut base = HashMap::new();
        base.insert("WINEPREFIX".into(), "/tmp/inherited-prefix".into());
        base.insert(
            "STEAM_COMPAT_DATA_PATH".into(),
            "/tmp/inherited-steam-prefix".into(),
        );
        let spec = ProcessSpec {
            program: "/usr/bin/true".into(),
            args: vec![],
            cwd: owned.to_string_lossy().into_owned(),
            env: vec![EnvironmentChange {
                key: "WINEPREFIX".into(),
                value: Some(owned.to_string_lossy().into_owned()),
            }],
        };

        let merged = validate_spec_for_launch(&spec, &owned, &base).unwrap();
        assert_eq!(merged.get("WINEPREFIX"), spec.env[0].value.as_ref());
        assert!(!merged.contains_key("STEAM_COMPAT_DATA_PATH"));

        let mut explicit_steam = spec.clone();
        explicit_steam.env.push(EnvironmentChange {
            key: "STEAM_COMPAT_DATA_PATH".into(),
            value: Some(owned.to_string_lossy().into_owned()),
        });
        let merged = validate_spec_for_launch(&explicit_steam, &owned, &base).unwrap();
        assert_eq!(
            merged.get("STEAM_COMPAT_DATA_PATH"),
            explicit_steam.env[1].value.as_ref()
        );

        explicit_steam.env[1].value = Some("/tmp".into());
        assert!(validate_spec_for_launch(&explicit_steam, &owned, &base)
            .unwrap_err()
            .contains("STEAM_COMPAT_DATA_PATH"));
        fs::remove_dir_all(owned).unwrap();
    }
}
