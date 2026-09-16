use ro_session_protocol::{ProcessSpec, SessionEvent, SessionRequest, PROTOCOL_VERSION};
use ro_tools_linux::{capture_process_identity, verify_process_identity};
use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const MAGIC: u32 = 0x015F_F908;

fn read_ptrace_scope() -> Option<u32> {
    fs::read_to_string("/proc/sys/kernel/yama/ptrace_scope")
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn read_ppid(pid: u32) -> Option<u32> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let close_paren = stat.rfind(')')?;
    let fields: Vec<&str> = stat.get(close_paren + 1..)?.split_whitespace().collect();
    fields.get(1)?.parse().ok()
}

fn canonicalize_prefix(path: &Path) -> PathBuf {
    if let Ok(c) = fs::canonicalize(path) {
        return c;
    }
    let parent = path.parent().unwrap_or(path);
    let canon_parent = fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
    path.file_name()
        .map(|n| canon_parent.join(n))
        .unwrap_or(canon_parent)
}

fn process_vm_read_u32(pid: u32, address: u32) -> Result<u32, i32> {
    let mut buf = [0u8; 4];
    let local = libc::iovec {
        iov_base: buf.as_mut_ptr() as *mut libc::c_void,
        iov_len: 4,
    };
    let remote = libc::iovec {
        iov_base: address as *mut libc::c_void,
        iov_len: 4,
    };
    let n = unsafe { libc::process_vm_readv(pid as libc::pid_t, &local, 1, &remote, 1, 0) };
    if n < 0 {
        return Err(std::io::Error::last_os_error().raw_os_error().unwrap_or(-1));
    }
    Ok(u32::from_le_bytes(buf))
}

fn zombie_children_of(root: u32) -> Vec<u32> {
    let mut zombies = Vec::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return zombies;
    };
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else {
            continue;
        };
        if !stat.contains(") Z ") {
            continue;
        }
        let mut current = pid;
        for _ in 0..64 {
            let Some(ppid) = read_ppid(current) else {
                break;
            };
            if ppid == root {
                zombies.push(pid);
                break;
            }
            if ppid <= 1 {
                break;
            }
            current = ppid;
        }
    }
    zombies
}

struct SessionClient {
    events: Arc<Mutex<VecDeque<SessionEvent>>>,
    stderr_lines: Arc<Mutex<VecDeque<String>>>,
    child: std::process::Child,
}

impl SessionClient {
    fn spawn(prefix: &Path, parent_pid: u32) -> Self {
        let sessiond = env!("CARGO_BIN_EXE_ro-sessiond");
        let mut child = Command::new(sessiond)
            .arg("--prefix")
            .arg(prefix)
            .arg("--parent-pid")
            .arg(parent_pid.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn ro-sessiond");
        let stdout = child.stdout.take().expect("stdout");
        let stderr = child.stderr.take().expect("stderr");
        let events = Arc::new(Mutex::new(VecDeque::new()));
        let events_reader = events.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if let Ok(event) = serde_json::from_str::<SessionEvent>(&line) {
                    events_reader.lock().unwrap().push_back(event);
                }
            }
        });
        let stderr_lines = Arc::new(Mutex::new(VecDeque::new()));
        let stderr_reader = stderr_lines.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                stderr_reader.lock().unwrap().push_back(line);
            }
        });
        Self {
            events,
            stderr_lines,
            child,
        }
    }

    fn send(&mut self, request: &SessionRequest) {
        let line = serde_json::to_string(request).unwrap();
        let stdin = self.child.stdin.as_mut().expect("stdin");
        writeln!(stdin, "{line}").unwrap();
        stdin.flush().unwrap();
    }

    fn send_batch(&mut self, requests: &[SessionRequest]) {
        let mut payload = String::new();
        for request in requests {
            payload.push_str(&serde_json::to_string(request).unwrap());
            payload.push('\n');
        }
        let stdin = self.child.stdin.as_mut().expect("stdin");
        stdin.write_all(payload.as_bytes()).unwrap();
        stdin.flush().unwrap();
    }

    fn wait_event(
        &self,
        timeout: Duration,
        predicate: impl Fn(&SessionEvent) -> bool,
    ) -> Option<SessionEvent> {
        let deadline = Instant::now() + timeout;
        let mut skipped = VecDeque::new();
        loop {
            let mut events = self.events.lock().unwrap();
            while let Some(ev) = events.pop_front() {
                if predicate(&ev) {
                    events.extend(skipped.drain(..));
                    return Some(ev);
                }
                skipped.push_back(ev);
            }
            drop(events);
            if Instant::now() >= deadline {
                self.events.lock().unwrap().extend(skipped.drain(..));
                return None;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn shutdown(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn wine_spec(prefix: &Path, program: &Path, role: Option<&str>) -> ProcessSpec {
    let mut env = vec![ro_session_protocol::EnvironmentChange {
        key: "WINEPREFIX".into(),
        value: Some(prefix.display().to_string()),
    }];
    if let Some(role) = role {
        env.push(ro_session_protocol::EnvironmentChange {
            key: "RO_YAMA_ROLE".into(),
            value: Some(role.into()),
        });
    }
    ProcessSpec {
        program: program.display().to_string(),
        args: Vec::new(),
        cwd: prefix.display().to_string(),
        env,
    }
}

fn test_handshake_and_orphan() {
    let dir = std::env::temp_dir().join(format!("ro-sessiond-test-{}", std::process::id()));
    let _ = fs::create_dir_all(&dir);
    let prefix = canonicalize_prefix(&dir);
    let test_pid = std::process::id();
    let mut client = SessionClient::spawn(&prefix, test_pid);

    client.send(&SessionRequest::Hello {
        protocol_version: PROTOCOL_VERSION,
    });
    let ready = client
        .wait_event(Duration::from_secs(5), |ev| {
            matches!(ev, SessionEvent::Ready { .. })
        })
        .expect("Ready");
    let SessionEvent::Ready {
        supervisor_pid,
        subreaper,
        prefix: ready_prefix,
        ..
    } = ready
    else {
        unreachable!()
    };
    assert!(subreaper);
    assert_eq!(ready_prefix, prefix.display().to_string());

    let worker_path = PathBuf::from(env!("CARGO_BIN_EXE_orphan-worker"));
    let request_id = "550e8400-e29b-41d4-a716-446655440001".to_string();
    client.send(&SessionRequest::Launch {
        request_id: request_id.clone(),
        spec: wine_spec(&prefix, &worker_path, Some("intermediate")),
    });

    client
        .wait_event(Duration::from_secs(5), |ev| {
            matches!(ev, SessionEvent::LaunchAccepted { .. })
        })
        .expect("LaunchAccepted");

    client
        .wait_event(Duration::from_secs(10), |ev| {
            matches!(ev, SessionEvent::ControllerExited { .. })
        })
        .unwrap_or_else(|| {
            let pending: Vec<_> = client.events.lock().unwrap().iter().cloned().collect();
            let stderr: Vec<_> = client.stderr_lines.lock().unwrap().iter().cloned().collect();
            let status = client.child.try_wait().ok().flatten();
            panic!(
                "ControllerExited after intermediate; pending: {pending:?} stderr: {stderr:?} sidecar: {status:?}"
            );
        });

    let mut target_pid = None;
    let mut target_addr = None;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Some(line) = client.stderr_lines.lock().unwrap().pop_front() {
            assert!(
                line.starts_with("[runner:stdout] "),
                "runner stderr prefix must be '[runner:stdout] ': {line:?}"
            );
            if let Some(rest) = line.strip_prefix("[runner:stdout] ") {
                let mut parts = rest.split_whitespace();
                if let (Some(pid), Some(addr)) = (parts.next(), parts.next()) {
                    target_pid = pid.parse().ok();
                    target_addr = addr.parse().ok();
                    break;
                }
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    let target_pid = target_pid.expect("target pid from runner stdout");
    let target_addr = target_addr.expect("target addr");

    thread::sleep(Duration::from_millis(100));
    assert_eq!(
        read_ppid(target_pid),
        Some(supervisor_pid),
        "target should be reparented to ro-sessiond"
    );

    if read_ptrace_scope() == Some(1) {
        let value = process_vm_read_u32(target_pid, target_addr).expect("read under Yama");
        assert_eq!(value, MAGIC);
    }

    unsafe {
        let identity = capture_process_identity(target_pid).expect("target identity");
        assert!(verify_process_identity(&identity));
        libc::kill(target_pid as libc::pid_t, libc::SIGKILL);
    }

    client.shutdown();
    let _ = fs::remove_dir_all(&dir);
}

fn test_hundred_true_cycles() {
    let dir = std::env::temp_dir().join(format!("ro-sessiond-cycles-{}", std::process::id()));
    let _ = fs::create_dir_all(&dir);
    let prefix = canonicalize_prefix(&dir);
    let test_pid = std::process::id();
    let mut client = SessionClient::spawn(&prefix, test_pid);
    client.send(&SessionRequest::Hello {
        protocol_version: PROTOCOL_VERSION,
    });
    let ready = client
        .wait_event(Duration::from_secs(5), |ev| {
            matches!(ev, SessionEvent::Ready { .. })
        })
        .expect("Ready");
    let supervisor_pid = match ready {
        SessionEvent::Ready { supervisor_pid, .. } => supervisor_pid,
        _ => unreachable!(),
    };

    let true_path = PathBuf::from("/usr/bin/true");
    assert!(true_path.is_file());

    for i in 0..100 {
        let request_id = format!("00000000-0000-4000-8000-{i:012x}");
        client.send(&SessionRequest::Launch {
            request_id,
            spec: wine_spec(&prefix, &true_path, None),
        });
        client
            .wait_event(Duration::from_secs(5), |ev| {
                matches!(ev, SessionEvent::LaunchAccepted { .. })
            })
            .expect("LaunchAccepted");
        client
            .wait_event(Duration::from_secs(5), |ev| {
                matches!(ev, SessionEvent::ControllerExited { .. })
            })
            .expect("ControllerExited");
    }

    assert!(
        zombie_children_of(supervisor_pid).is_empty(),
        "expected zero zombies after 100 cycles"
    );

    client.shutdown();
    let _ = fs::remove_dir_all(&dir);
}

fn test_incompatible_hello() {
    let dir = std::env::temp_dir().join(format!("ro-sessiond-bad-hello-{}", std::process::id()));
    let _ = fs::create_dir_all(&dir);
    let prefix = canonicalize_prefix(&dir);
    let mut client = SessionClient::spawn(&prefix, std::process::id());
    client.send(&SessionRequest::Hello {
        protocol_version: 999,
    });
    let err = client
        .wait_event(Duration::from_secs(5), |ev| {
            matches!(ev, SessionEvent::Error { .. })
        })
        .expect("Error event");
    assert!(matches!(err, SessionEvent::Error { .. }));
    thread::sleep(Duration::from_millis(200));
    assert!(client.child.try_wait().expect("wait").is_some());
    let _ = fs::remove_dir_all(&dir);
}

fn test_wrong_parent_pid() {
    let dir = std::env::temp_dir().join(format!("ro-sessiond-bad-ppid-{}", std::process::id()));
    let _ = fs::create_dir_all(&dir);
    let prefix = canonicalize_prefix(&dir);
    let sessiond = env!("CARGO_BIN_EXE_ro-sessiond");
    let mut child = Command::new(sessiond)
        .arg("--prefix")
        .arg(&prefix)
        .arg("--parent-pid")
        .arg("1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn");
    thread::sleep(Duration::from_millis(300));
    assert!(child.try_wait().expect("wait").is_some());
    let _ = fs::remove_dir_all(&dir);
}

fn test_controller_exited_before_inherited_pipe_closes() {
    let dir =
        std::env::temp_dir().join(format!("ro-sessiond-inherited-pipe-{}", std::process::id()));
    let _ = fs::create_dir_all(&dir);
    let prefix = canonicalize_prefix(&dir);
    let test_pid = std::process::id();
    let mut client = SessionClient::spawn(&prefix, test_pid);
    client.send(&SessionRequest::Hello {
        protocol_version: PROTOCOL_VERSION,
    });
    client
        .wait_event(Duration::from_secs(5), |ev| {
            matches!(ev, SessionEvent::Ready { .. })
        })
        .expect("Ready");

    let mut spec = wine_spec(&prefix, Path::new("/usr/bin/sh"), None);
    spec.args = vec!["-c".into(), "/usr/bin/sleep 30 & exit 0".into()];
    let request_id = "550e8400-e29b-41d4-a716-446655440002".to_string();
    client.send(&SessionRequest::Launch { request_id, spec });
    client
        .wait_event(Duration::from_secs(5), |ev| {
            matches!(ev, SessionEvent::LaunchAccepted { .. })
        })
        .expect("LaunchAccepted");

    let start = Instant::now();
    client
        .wait_event(Duration::from_secs(3), |ev| {
            matches!(ev, SessionEvent::ControllerExited { .. })
        })
        .expect("ControllerExited while sleep still holds inherited pipe");
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "ControllerExited must not wait for inherited pipe EOF"
    );

    client.shutdown();
    let _ = fs::remove_dir_all(&dir);
}

fn test_batched_launch_lines_are_drained_without_another_write() {
    let dir = std::env::temp_dir().join(format!("ro-sessiond-batch-{}", std::process::id()));
    let _ = fs::create_dir_all(&dir);
    let prefix = canonicalize_prefix(&dir);
    let mut client = SessionClient::spawn(&prefix, std::process::id());
    client.send(&SessionRequest::Hello {
        protocol_version: PROTOCOL_VERSION,
    });
    client
        .wait_event(Duration::from_secs(5), |ev| {
            matches!(ev, SessionEvent::Ready { .. })
        })
        .expect("Ready");

    let first = "550e8400-e29b-41d4-a716-446655440010".to_string();
    let second = "550e8400-e29b-41d4-a716-446655440011".to_string();
    client.send_batch(&[
        SessionRequest::Launch {
            request_id: first.clone(),
            spec: wine_spec(&prefix, Path::new("/usr/bin/true"), None),
        },
        SessionRequest::Launch {
            request_id: second.clone(),
            spec: wine_spec(&prefix, Path::new("/usr/bin/true"), None),
        },
    ]);

    for expected in [first, second] {
        client
            .wait_event(Duration::from_secs(3), |event| {
                matches!(
                    event,
                    SessionEvent::LaunchAccepted { request_id, .. } if request_id == &expected
                )
            })
            .unwrap_or_else(|| panic!("LaunchAccepted missing for {expected}"));
    }
    client.shutdown();
    let _ = fs::remove_dir_all(&dir);
}

fn main() {
    test_wrong_parent_pid();
    test_incompatible_hello();
    test_controller_exited_before_inherited_pipe_closes();
    test_batched_launch_lines_are_drained_without_another_write();
    test_handshake_and_orphan();
    test_hundred_true_cycles();
    eprintln!("orphan_reparent integration tests passed");
}
