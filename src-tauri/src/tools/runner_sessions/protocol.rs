use ro_session_protocol::{
    EnvironmentChange, ProcessSpec, SessionEvent, SessionRequest, MAX_MESSAGE_BYTES,
    PROTOCOL_VERSION,
};
use serde_json;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};
use tokio::sync::{oneshot, Mutex, Notify};

use super::SessionError;
use crate::utils::RunnerInvocation;

pub fn invocation_to_spec(
    invocation: &RunnerInvocation,
    owned_prefix: &Path,
) -> Result<ProcessSpec, SessionError> {
    let program = utf8_path(&invocation.program, "program")?;
    if !Path::new(&program).is_absolute() {
        return Err(SessionError::validation("program must be an absolute path"));
    }
    let cwd = utf8_path(&invocation.cwd, "cwd")?;
    if !Path::new(&cwd).is_absolute() {
        return Err(SessionError::validation("cwd must be an absolute path"));
    }

    let mut env_map: HashMap<String, Option<String>> = HashMap::new();
    for (key, value) in &invocation.env {
        let key = utf8_os(key, "env key")?;
        if env_map.contains_key(&key) {
            return Err(SessionError::validation("duplicate env keys in invocation"));
        }
        let val = match value {
            Some(v) => Some(utf8_os(v, "env value")?),
            None => None,
        };
        env_map.insert(key, val);
    }

    let prefix_str = owned_prefix.to_string_lossy().to_string();
    match env_map.get("WINEPREFIX") {
        None => {
            env_map.insert("WINEPREFIX".into(), Some(prefix_str));
        }
        Some(Some(existing)) => {
            let canon = canonicalize_prefix_path(Path::new(existing))
                .ok_or_else(|| SessionError::validation("WINEPREFIX could not be canonicalized"))?;
            if canon != owned_prefix {
                return Err(SessionError::validation(
                    "WINEPREFIX does not match owned prefix",
                ));
            }
        }
        Some(None) => {
            return Err(SessionError::validation(
                "WINEPREFIX must be set, not unset",
            ));
        }
    }

    if let Some(Some(steam)) = env_map.get("STEAM_COMPAT_DATA_PATH") {
        let canon = canonicalize_prefix_path(Path::new(steam)).ok_or_else(|| {
            SessionError::validation("STEAM_COMPAT_DATA_PATH could not be canonicalized")
        })?;
        if canon != owned_prefix {
            return Err(SessionError::validation(
                "STEAM_COMPAT_DATA_PATH does not match owned prefix",
            ));
        }
    }

    let env: Vec<EnvironmentChange> = env_map
        .into_iter()
        .map(|(key, value)| EnvironmentChange { key, value })
        .collect();

    let args: Vec<String> = invocation
        .args
        .iter()
        .map(|a| utf8_os(a, "arg"))
        .collect::<Result<Vec<_>, SessionError>>()?;

    Ok(ProcessSpec {
        program,
        args,
        cwd,
        env,
    })
}

pub fn canonicalize_prefix_path(path: &Path) -> Option<PathBuf> {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return Some(canonical);
    }
    let parent = path.parent()?;
    let canonical_parent = std::fs::canonicalize(parent).ok()?;
    path.file_name().map(|name| canonical_parent.join(name))
}

fn utf8_path(path: &Path, field: &str) -> Result<String, SessionError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| SessionError::validation(format!("{field} is not valid UTF-8")))
}

fn utf8_os(value: &OsStr, field: &str) -> Result<String, SessionError> {
    value
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| SessionError::validation(format!("{field} is not valid UTF-8")))
}

#[derive(Debug, Clone)]
pub struct ReadyInfo {
    pub protocol_version: u16,
    pub supervisor_pid: u32,
    pub prefix: String,
    pub subreaper: bool,
}

#[allow(clippy::type_complexity)]
pub struct SessionProtocol {
    stdin: Arc<Mutex<ChildStdin>>,
    launch_waiters: Arc<StdMutex<HashMap<String, oneshot::Sender<Result<u32, SessionError>>>>>,
    exit_waiters:
        Arc<StdMutex<HashMap<String, oneshot::Sender<Result<ControllerExit, SessionError>>>>>,
    completed_exits: Arc<StdMutex<HashMap<String, ControllerExit>>>,
    on_controller_exited: Arc<StdMutex<Option<Arc<dyn Fn(&str) + Send + Sync>>>>,
    on_launch_accepted: Arc<StdMutex<Option<Arc<dyn Fn() + Send + Sync>>>>,
    shutdown_waiters: Arc<StdMutex<HashMap<String, oneshot::Sender<Result<(), SessionError>>>>>,
    ready_waiter: Arc<StdMutex<Option<oneshot::Sender<Result<ReadyInfo, SessionError>>>>>,
    stopped_flag: Arc<AtomicBool>,
    stopped_notify: Arc<Notify>,
    fatal: Arc<StdMutex<Option<SessionError>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerExit {
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
}

impl SessionProtocol {
    pub fn spawn_reader(stdout: ChildStdout, protocol: Arc<SessionProtocol>) {
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            loop {
                match read_bounded_line(&mut reader).await {
                    Ok(Some(line)) => {
                        if line.is_empty() {
                            continue;
                        }
                        let event: SessionEvent = match serde_json::from_str(&line) {
                            Ok(event) => event,
                            Err(e) => {
                                protocol.set_fatal(SessionError::protocol(format!(
                                    "invalid event JSON: {e}"
                                )));
                                break;
                            }
                        };
                        protocol.dispatch(event);
                    }
                    Ok(None) => break,
                    Err(e) => {
                        protocol.set_fatal(e);
                        break;
                    }
                }
            }
            if !protocol.stopped_flag.load(Ordering::SeqCst) {
                protocol.set_fatal(SessionError::protocol("supervisor stdout closed"));
            } else {
                protocol.fail_pending_waiters(SessionError::protocol("supervisor stdout closed"));
                protocol.mark_stopped();
            }
        });
    }

    fn mark_stopped(&self) {
        self.stopped_flag.store(true, Ordering::SeqCst);
        self.stopped_notify.notify_waiters();
    }

    #[cfg(test)]
    pub(crate) fn mark_stopped_for_test(self: &Arc<Self>) {
        self.mark_stopped();
    }

    fn fail_pending_waiters(&self, err: SessionError) {
        if let Ok(mut map) = self.launch_waiters.lock() {
            for (_, tx) in map.drain() {
                let _ = tx.send(Err(err.clone()));
            }
        }
        if let Ok(mut map) = self.shutdown_waiters.lock() {
            for (_, tx) in map.drain() {
                let _ = tx.send(Err(err.clone()));
            }
        }
        if let Ok(mut map) = self.exit_waiters.lock() {
            for (_, tx) in map.drain() {
                let _ = tx.send(Err(err.clone()));
            }
        }
        if let Ok(mut slot) = self.ready_waiter.lock() {
            if let Some(tx) = slot.take() {
                let _ = tx.send(Err(err.clone()));
            }
        }
    }

    fn set_fatal(&self, err: SessionError) {
        if let Ok(mut slot) = self.fatal.lock() {
            if slot.is_none() {
                *slot = Some(err.clone());
            }
        }
        self.fail_pending_waiters(err);
        self.mark_stopped();
    }

    #[cfg(test)]
    pub(crate) fn set_fatal_for_test(self: &Arc<Self>, err: SessionError) {
        self.set_fatal(err);
    }

    fn take_completed_exit(&self, request_id: &str) -> Option<ControllerExit> {
        self.completed_exits
            .lock()
            .ok()
            .and_then(|mut map| map.remove(request_id))
    }

    pub fn try_fatal(&self) -> Option<SessionError> {
        self.fatal.lock().ok().and_then(|slot| slot.clone())
    }

    fn dispatch(&self, event: SessionEvent) {
        match event {
            SessionEvent::LaunchAccepted {
                request_id,
                controller_pid,
            } => {
                if let Some(tx) = self
                    .launch_waiters
                    .lock()
                    .ok()
                    .and_then(|mut m| m.remove(&request_id))
                {
                    let _ = tx.send(Ok(controller_pid));
                }
                if let Ok(cb) = self.on_launch_accepted.lock() {
                    if let Some(cb) = cb.as_ref() {
                        cb();
                    }
                }
            }
            SessionEvent::ControllerExited {
                request_id,
                exit_code,
                signal,
                ..
            } => {
                let exit = ControllerExit { exit_code, signal };
                if let Ok(mut completed) = self.completed_exits.lock() {
                    completed.insert(request_id.clone(), exit.clone());
                }
                if let Some(tx) = self
                    .exit_waiters
                    .lock()
                    .ok()
                    .and_then(|mut m| m.remove(&request_id))
                {
                    if let Ok(mut completed) = self.completed_exits.lock() {
                        completed.remove(&request_id);
                    }
                    let _ = tx.send(Ok(exit));
                }
                if let Ok(cb) = self.on_controller_exited.lock() {
                    if let Some(cb) = cb.as_ref() {
                        cb(&request_id);
                    }
                }
            }
            SessionEvent::ShutdownAccepted { request_id } => {
                if let Some(tx) = self
                    .shutdown_waiters
                    .lock()
                    .ok()
                    .and_then(|mut m| m.remove(&request_id))
                {
                    let _ = tx.send(Ok(()));
                }
            }
            SessionEvent::Error {
                request_id,
                message,
                ..
            } => {
                let err = SessionError::remote(message);
                if let Some(id) = request_id {
                    if let Some(tx) = self
                        .launch_waiters
                        .lock()
                        .ok()
                        .and_then(|mut m| m.remove(&id))
                    {
                        let _ = tx.send(Err(err.clone()));
                    }
                    if let Some(tx) = self
                        .shutdown_waiters
                        .lock()
                        .ok()
                        .and_then(|mut m| m.remove(&id))
                    {
                        let _ = tx.send(Err(err.clone()));
                    }
                    if let Some(tx) = self
                        .exit_waiters
                        .lock()
                        .ok()
                        .and_then(|mut m| m.remove(&id))
                    {
                        let _ = tx.send(Err(err.clone()));
                    }
                } else {
                    if let Some(tx) = self.ready_waiter.lock().ok().and_then(|mut m| m.take()) {
                        let _ = tx.send(Err(err.clone()));
                    }
                    self.set_fatal(err);
                }
            }
            SessionEvent::Stopped => {
                self.mark_stopped();
            }
            SessionEvent::Ready {
                protocol_version,
                supervisor_pid,
                prefix,
                subreaper,
            } => {
                if let Some(tx) = self.ready_waiter.lock().ok().and_then(|mut m| m.take()) {
                    let _ = tx.send(Ok(ReadyInfo {
                        protocol_version,
                        supervisor_pid,
                        prefix,
                        subreaper,
                    }));
                }
            }
            SessionEvent::Idle => {}
        }
    }

    pub fn new(stdin: ChildStdin) -> Arc<Self> {
        Arc::new(Self {
            stdin: Arc::new(Mutex::new(stdin)),
            launch_waiters: Arc::new(StdMutex::new(HashMap::new())),
            exit_waiters: Arc::new(StdMutex::new(HashMap::new())),
            shutdown_waiters: Arc::new(StdMutex::new(HashMap::new())),
            ready_waiter: Arc::new(StdMutex::new(None)),
            completed_exits: Arc::new(StdMutex::new(HashMap::new())),
            on_controller_exited: Arc::new(StdMutex::new(None)),
            on_launch_accepted: Arc::new(StdMutex::new(None)),
            stopped_flag: Arc::new(AtomicBool::new(false)),
            stopped_notify: Arc::new(Notify::new()),
            fatal: Arc::new(StdMutex::new(None)),
        })
    }

    pub fn set_controller_exited_hook(self: &Arc<Self>, hook: Arc<dyn Fn(&str) + Send + Sync>) {
        if let Ok(mut slot) = self.on_controller_exited.lock() {
            *slot = Some(hook);
        }
    }

    pub fn set_launch_accepted_hook(self: &Arc<Self>, hook: Arc<dyn Fn() + Send + Sync>) {
        if let Ok(mut slot) = self.on_launch_accepted.lock() {
            *slot = Some(hook);
        }
    }

    pub async fn send_request(&self, request: &SessionRequest) -> Result<(), SessionError> {
        let line = serde_json::to_string(request)
            .map_err(|e| SessionError::internal(format!("serialize request: {e}")))?;
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| SessionError::internal(format!("write stdin: {e}")))?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|e| SessionError::internal(format!("write newline: {e}")))?;
        stdin
            .flush()
            .await
            .map_err(|e| SessionError::internal(format!("flush stdin: {e}")))?;
        Ok(())
    }

    pub async fn handshake(&self) -> Result<ReadyInfo, SessionError> {
        let (tx, rx) = oneshot::channel();
        *self.ready_waiter.lock().unwrap() = Some(tx);
        if let Err(e) = self
            .send_request(&SessionRequest::Hello {
                protocol_version: PROTOCOL_VERSION,
            })
            .await
        {
            *self.ready_waiter.lock().unwrap() = None;
            return Err(e);
        }
        match rx.await {
            Ok(Ok(info)) => Ok(info),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(SessionError::handshake("ready waiter dropped")),
        }
    }

    pub async fn launch(&self, request_id: String, spec: ProcessSpec) -> Result<u32, SessionError> {
        let rid = request_id.clone();
        let (tx, rx) = oneshot::channel();
        self.launch_waiters.lock().unwrap().insert(rid.clone(), tx);
        if let Err(e) = self
            .send_request(&SessionRequest::Launch { request_id, spec })
            .await
        {
            self.launch_waiters.lock().unwrap().remove(&rid);
            return Err(e);
        }
        match rx.await {
            Ok(result) => result,
            Err(_) => Err(SessionError::internal("launch waiter dropped")),
        }
    }

    pub fn try_controller_exit(&self, request_id: &str) -> Option<ControllerExit> {
        if let Some(exit) = self.take_completed_exit(request_id) {
            return Some(exit);
        }
        if self.try_fatal().is_some() {
            return Some(ControllerExit {
                exit_code: None,
                signal: None,
            });
        }
        None
    }

    fn resolve_controller_exit_wait(
        &self,
        request_id: &str,
    ) -> Result<Option<ControllerExit>, SessionError> {
        if let Some(exit) = self.take_completed_exit(request_id) {
            return Ok(Some(exit));
        }
        if let Some(err) = self.try_fatal() {
            return Err(err);
        }
        Ok(None)
    }

    pub async fn wait_controller_exit(
        &self,
        request_id: String,
    ) -> Result<ControllerExit, SessionError> {
        match self.resolve_controller_exit_wait(&request_id) {
            Ok(Some(exit)) => return Ok(exit),
            Err(err) => return Err(err),
            Ok(None) => {}
        }
        let (tx, rx) = oneshot::channel();
        self.exit_waiters
            .lock()
            .unwrap()
            .insert(request_id.clone(), tx);
        match self.resolve_controller_exit_wait(&request_id) {
            Ok(Some(exit)) => {
                self.exit_waiters.lock().unwrap().remove(&request_id);
                return Ok(exit);
            }
            Err(err) => {
                self.exit_waiters.lock().unwrap().remove(&request_id);
                return Err(err);
            }
            Ok(None) => {}
        }
        match rx.await {
            Ok(result) => result,
            Err(_) => Err(SessionError::internal("exit waiter dropped")),
        }
    }

    pub async fn shutdown(
        &self,
        request_id: String,
        shutdown_spec: ProcessSpec,
        grace_ms: u64,
    ) -> Result<(), SessionError> {
        let rid = request_id.clone();
        let (tx, rx) = oneshot::channel();
        self.shutdown_waiters
            .lock()
            .unwrap()
            .insert(rid.clone(), tx);
        if let Err(e) = self
            .send_request(&SessionRequest::Shutdown {
                request_id,
                shutdown_spec,
                grace_ms,
            })
            .await
        {
            self.shutdown_waiters.lock().unwrap().remove(&rid);
            return Err(e);
        }
        match rx.await {
            Ok(result) => result,
            Err(_) => Err(SessionError::internal("shutdown waiter dropped")),
        }
    }

    pub async fn wait_stopped(&self) -> Result<(), SessionError> {
        loop {
            if self.stopped_flag.load(Ordering::SeqCst) {
                return self.stopped_result();
            }
            let notified = self.stopped_notify.notified();
            if self.stopped_flag.load(Ordering::SeqCst) {
                return self.stopped_result();
            }
            notified.await;
        }
    }

    fn stopped_result(&self) -> Result<(), SessionError> {
        if let Some(err) = self.fatal.lock().unwrap().clone() {
            return Err(err);
        }
        Ok(())
    }
}

async fn read_bounded_line(
    reader: &mut BufReader<ChildStdout>,
) -> Result<Option<String>, SessionError> {
    let mut buf = Vec::new();
    let mut one = [0u8; 1];
    while buf.len() <= MAX_MESSAGE_BYTES {
        let n = reader
            .read(&mut one)
            .await
            .map_err(|e| SessionError::protocol(format!("read stdout: {e}")))?;
        if n == 0 {
            return Ok(if buf.is_empty() {
                None
            } else {
                Some(String::from_utf8_lossy(&buf).into_owned())
            });
        }
        if one[0] == b'\n' {
            return Ok(Some(String::from_utf8_lossy(&buf).into_owned()));
        }
        buf.push(one[0]);
    }
    while reader
        .read(&mut one)
        .await
        .map_err(|e| SessionError::protocol(format!("drain stdout: {e}")))?
        > 0
    {
        if one[0] == b'\n' {
            break;
        }
    }
    Err(SessionError::protocol(
        "event line exceeds MAX_MESSAGE_BYTES",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ro_session_protocol::SessionRequest;
    #[cfg(unix)]
    use std::os::unix::ffi::OsStringExt;

    #[test]
    fn canonical_hello_json() {
        let req = SessionRequest::Hello {
            protocol_version: 1,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, r#"{"type":"hello","protocolVersion":1}"#);
    }

    #[test]
    fn rejects_non_utf8_invocation_fields() {
        let dir = std::env::temp_dir().join(format!("ro-inv-utf8-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let owned = canonicalize_prefix_path(&dir).unwrap();
        let inv = RunnerInvocation {
            program: PathBuf::from(std::ffi::OsString::from_vec(vec![0xff, 0xfe])),
            args: vec![],
            cwd: owned.clone(),
            env: vec![],
        };
        let err = invocation_to_spec(&inv, &owned).unwrap_err();
        assert!(err.message.contains("UTF-8"));
    }

    #[test]
    fn rejects_wineprefix_mismatch() {
        let dir = std::env::temp_dir().join(format!("ro-inv-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let owned = canonicalize_prefix_path(&dir).unwrap();
        let inv = RunnerInvocation {
            program: PathBuf::from("/usr/bin/true"),
            args: vec![],
            cwd: owned.clone(),
            env: vec![(
                std::ffi::OsString::from("WINEPREFIX"),
                Some(std::ffi::OsString::from("/other/prefix")),
            )],
        };
        let err = invocation_to_spec(&inv, &owned).unwrap_err();
        assert!(err.message.contains("WINEPREFIX"));
    }

    #[tokio::test]
    async fn try_controller_exit_reads_completed_exits_without_wait() {
        use std::process::Stdio;

        let mut child = tokio::process::Command::new("/bin/cat")
            .stdin(Stdio::piped())
            .spawn()
            .expect("spawn cat");
        let stdin = child.stdin.take().expect("stdin");
        let protocol = SessionProtocol::new(stdin);
        protocol.dispatch(SessionEvent::ControllerExited {
            request_id: "req-1".to_string(),
            exit_code: Some(7),
            signal: None,
            controller_pid: 0,
        });
        let exit = protocol
            .try_controller_exit("req-1")
            .expect("exit should be visible");
        assert_eq!(exit.exit_code, Some(7));
        assert!(protocol.try_controller_exit("req-1").is_none());
        assert!(protocol.try_controller_exit("other").is_none());
        let _ = child.kill().await;
    }

    #[tokio::test]
    async fn try_controller_exit_reflects_fatal_without_controller_exited() {
        use std::process::Stdio;

        let mut child = tokio::process::Command::new("/bin/cat")
            .stdin(Stdio::piped())
            .spawn()
            .expect("spawn cat");
        let stdin = child.stdin.take().expect("stdin");
        let protocol = SessionProtocol::new(stdin);
        protocol.set_fatal_for_test(SessionError::protocol("supervisor stdout closed"));
        let exit = protocol
            .try_controller_exit("any-request")
            .expect("fatal should surface as synthetic exit");
        assert_eq!(exit.exit_code, None);
        assert_eq!(exit.signal, None);
        let _ = child.kill().await;
    }

    #[tokio::test]
    async fn controller_exited_wins_over_fatal_for_request_id() {
        use std::process::Stdio;

        let mut child = tokio::process::Command::new("/bin/cat")
            .stdin(Stdio::piped())
            .spawn()
            .expect("spawn cat");
        let stdin = child.stdin.take().expect("stdin");
        let protocol = SessionProtocol::new(stdin);
        protocol.set_fatal_for_test(SessionError::protocol("sidecar died"));
        protocol.dispatch(SessionEvent::ControllerExited {
            request_id: "req-1".to_string(),
            exit_code: Some(3),
            signal: None,
            controller_pid: 0,
        });
        let exit = protocol
            .try_controller_exit("req-1")
            .expect("completed exit");
        assert_eq!(exit.exit_code, Some(3));
        let _ = child.kill().await;
    }

    #[tokio::test]
    async fn wait_controller_exit_returns_fatal_without_hanging() {
        use std::process::Stdio;
        use std::time::Duration;

        let mut child = tokio::process::Command::new("/bin/cat")
            .stdin(Stdio::piped())
            .spawn()
            .expect("spawn cat");
        let stdin = child.stdin.take().expect("stdin");
        let protocol = SessionProtocol::new(stdin);
        let fatal = SessionError::protocol("supervisor stdout closed");
        protocol.set_fatal_for_test(fatal.clone());
        let result = tokio::time::timeout(
            Duration::from_millis(200),
            protocol.wait_controller_exit("req-late".to_string()),
        )
        .await
        .expect("wait_controller_exit should not hang");
        let err = result.expect_err("fatal should fail the waiter");
        assert_eq!(err.message, fatal.message);
        let _ = child.kill().await;
    }

    #[tokio::test]
    async fn wait_stopped_returns_immediately_when_already_stopped() {
        use std::process::Stdio;
        use std::time::Duration;

        let mut child = tokio::process::Command::new("/bin/cat")
            .stdin(Stdio::piped())
            .spawn()
            .expect("spawn cat");
        let stdin = child.stdin.take().expect("stdin");
        let protocol = SessionProtocol::new(stdin);
        SessionProtocol::mark_stopped_for_test(&protocol);
        let result = tokio::time::timeout(Duration::from_millis(200), protocol.wait_stopped())
            .await
            .expect("wait_stopped should not hang");
        assert!(result.is_ok());
        let _ = child.kill().await;
    }
}
