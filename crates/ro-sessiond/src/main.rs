mod process;
mod protocol;
mod supervisor;

use protocol::{BoundedLineReader, EventWriter, SharedWriter};
use ro_session_protocol::{SessionEvent, SessionRequest, PROTOCOL_VERSION};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use process::{canonicalize_prefix, capture_base_env};
use supervisor::{SessionPhase, Supervisor};

struct Config {
    prefix: PathBuf,
    parent_pid: u32,
}

fn parse_args() -> Result<Config, String> {
    let mut prefix: Option<String> = None;
    let mut parent_pid: Option<u32> = None;
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--prefix" if i + 1 < args.len() => {
                prefix = Some(args[i + 1].clone());
                i += 2;
            }
            "--parent-pid" if i + 1 < args.len() => {
                parent_pid = Some(args[i + 1].parse().map_err(|_| "invalid parent pid")?);
                i += 2;
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    let prefix = prefix.ok_or("missing --prefix")?;
    let parent_pid = parent_pid.ok_or("missing --parent-pid")?;
    let prefix_path = PathBuf::from(prefix);
    let canonical =
        canonicalize_prefix(&prefix_path).ok_or("prefix path could not be canonicalized")?;
    Ok(Config {
        prefix: canonical,
        parent_pid,
    })
}

fn verify_parent(expected: u32) -> bool {
    unsafe { libc::getppid() as u32 == expected }
}

fn set_subreaper() -> Result<(), String> {
    let rc = unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) };
    if rc != 0 {
        return Err(format!(
            "PR_SET_CHILD_SUBREAPER failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn set_pdeathsig() {
    unsafe {
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM, 0, 0, 0);
    }
}

static SIGTERM_FLAG: AtomicBool = AtomicBool::new(false);
static SIGCHLD_FLAG: AtomicBool = AtomicBool::new(false);

fn install_signal_hooks() {
    thread::spawn(|| {
        let mut signals =
            signal_hook::iterator::Signals::new([libc::SIGCHLD, libc::SIGTERM]).expect("signals");
        for sig in signals.forever() {
            match sig {
                libc::SIGCHLD => SIGCHLD_FLAG.store(true, Ordering::SeqCst),
                libc::SIGTERM => SIGTERM_FLAG.store(true, Ordering::SeqCst),
                _ => {}
            }
        }
    });
}

fn set_stdin_nonblocking() -> Result<(), String> {
    let flags = unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_GETFL) };
    if flags < 0 {
        return Err(format!(
            "fcntl F_GETFL failed: {}",
            io::Error::last_os_error()
        ));
    }
    let rc = unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    if rc < 0 {
        return Err(format!(
            "fcntl F_SETFL failed: {}",
            io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn emit_error(
    writer: &SharedWriter,
    request_id: Option<String>,
    stage: &str,
    message: &str,
    errno: Option<i32>,
) {
    let _ = writer.emit(&SessionEvent::Error {
        request_id,
        stage: stage.into(),
        errno,
        message: message.into(),
    });
}

fn handshake(
    reader: &mut BoundedLineReader<io::Stdin>,
    writer: &SharedWriter,
    prefix: &Path,
) -> Result<(), i32> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if Instant::now() >= deadline {
            emit_error(
                writer,
                None,
                "handshake",
                "timed out waiting for Hello",
                None,
            );
            return Err(1);
        }
        if !poll_stdin(100) {
            continue;
        }
        let line = match reader.read_line() {
            Ok(Some(line)) => line,
            Ok(None) => {
                emit_error(
                    writer,
                    None,
                    "handshake",
                    "unexpected EOF before Hello",
                    None,
                );
                return Err(1);
            }
            Err(e) => {
                emit_error(
                    writer,
                    None,
                    "protocol",
                    &format!("invalid handshake message: {e}"),
                    None,
                );
                return Err(1);
            }
        };
        let request: SessionRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                emit_error(
                    writer,
                    None,
                    "handshake",
                    &format!("invalid JSON: {e}"),
                    None,
                );
                return Err(1);
            }
        };
        return match request {
            SessionRequest::Hello { protocol_version } => {
                if protocol_version != PROTOCOL_VERSION {
                    emit_error(
                        writer,
                        None,
                        "handshake",
                        &format!("unsupported protocol version {protocol_version}"),
                        None,
                    );
                    return Err(1);
                }
                writer
                    .emit(&SessionEvent::Ready {
                        protocol_version: PROTOCOL_VERSION,
                        supervisor_pid: std::process::id(),
                        prefix: prefix.display().to_string(),
                        subreaper: true,
                    })
                    .map_err(|_| 1)?;
                Ok(())
            }
            _ => {
                emit_error(
                    writer,
                    None,
                    "handshake",
                    "first request must be Hello",
                    None,
                );
                Err(1)
            }
        };
    }
}

fn poll_stdin(timeout_ms: i32) -> bool {
    let mut pfd = libc::pollfd {
        fd: libc::STDIN_FILENO,
        events: libc::POLLIN,
        revents: 0,
    };
    let rc = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
    rc > 0 && (pfd.revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR)) != 0
}

fn stdin_poll_hup_or_err() -> bool {
    let mut pfd = libc::pollfd {
        fd: libc::STDIN_FILENO,
        events: libc::POLLIN,
        revents: 0,
    };
    let rc = unsafe { libc::poll(&mut pfd, 1, 0) };
    rc > 0 && (pfd.revents & (libc::POLLHUP | libc::POLLERR)) != 0
}

fn run_session(config: Config) -> i32 {
    if !verify_parent(config.parent_pid) {
        eprintln!("ro-sessiond: parent pid mismatch");
        return 1;
    }
    if set_subreaper().is_err() {
        eprintln!("ro-sessiond: failed to become subreaper");
        return 1;
    }
    set_pdeathsig();
    if !verify_parent(config.parent_pid) {
        eprintln!("ro-sessiond: parent pid changed during startup");
        return 1;
    }

    install_signal_hooks();

    let writer: SharedWriter = Arc::new(EventWriter::new());
    let mut reader = BoundedLineReader::new(io::stdin());
    if handshake(&mut reader, &writer, &config.prefix).is_err() {
        return 1;
    }
    if set_stdin_nonblocking().is_err() {
        eprintln!("ro-sessiond: failed to configure stdin");
        return 1;
    }

    let stderr_file = std::fs::File::options()
        .append(true)
        .open("/dev/stderr")
        .unwrap();
    let stderr_writer = Arc::new(Mutex::new(stderr_file));
    let base_env = capture_base_env();

    let mut sup = Supervisor::new(
        config.prefix.clone(),
        config.parent_pid,
        base_env,
        stderr_writer,
        writer.clone(),
    );

    let mut stdin_eof = false;
    let mut idle_streak = 0u32;
    let mut idle_emitted = false;

    loop {
        if !verify_parent(config.parent_pid) {
            sup.begin_forced_shutdown();
        }
        if SIGTERM_FLAG.swap(false, Ordering::SeqCst) {
            sup.begin_forced_shutdown();
        }

        let _sigchld = SIGCHLD_FLAG.swap(false, Ordering::SeqCst);

        if !stdin_eof && (reader.has_buffered_line() || poll_stdin(0) || stdin_poll_hup_or_err()) {
            match reader.read_line() {
                Ok(Some(line)) => {
                    if let Err(e) = sup.handle_line(&line) {
                        emit_error(
                            &writer,
                            None,
                            "protocol",
                            &format!("invalid request: {e}"),
                            None,
                        );
                    }
                }
                Ok(None) => {
                    stdin_eof = true;
                    sup.begin_forced_shutdown();
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => emit_error(&writer, None, "protocol", &format!("{e}"), None),
            }
        }

        sup.drain_reap_events();
        sup.join_finished_streams();

        if sup.phase == SessionPhase::Stopping {
            sup.advance_shutdown();
        }

        if sup.phase == SessionPhase::Ready {
            if sup.last_wait_echild {
                idle_streak += 1;
                if idle_streak >= 2 && !idle_emitted {
                    let _ = writer.emit(&SessionEvent::Idle);
                    idle_emitted = true;
                }
            } else {
                idle_streak = 0;
                idle_emitted = false;
            }
        }

        if sup.phase == SessionPhase::Stopped {
            sup.join_all_pending_streams();
            let _ = writer.emit(&SessionEvent::Stopped);
            return 0;
        }

        thread::sleep(Duration::from_millis(100));
    }
}

fn main() {
    let config = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ro-sessiond: {e}");
            std::process::exit(1);
        }
    };
    std::process::exit(run_session(config));
}
