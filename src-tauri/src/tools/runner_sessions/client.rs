use ro_session_protocol::PROTOCOL_VERSION;
use ro_tools_linux::{capture_process_identity, verify_process_identity, ProcessIdentity};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio::process::{Child, Command};
use tokio::time::timeout;

use crate::utils::sanitize_appimage_env;

use super::diagnostics::{emit_session_line, handle_stderr_line, prefix_log_token};
use super::protocol::SessionProtocol;
use super::protocol::{canonicalize_prefix_path, ReadyInfo};
use super::SessionError;
use tauri::AppHandle;

pub fn find_ro_sessiond(override_path: Option<&Path>) -> PathBuf {
    if let Some(path) = override_path {
        return path.to_path_buf();
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let plain = dir.join("ro-sessiond");
            if plain.exists() {
                return plain;
            }
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    if name == "ro-sessiond" || name.starts_with("ro-sessiond-") {
                        return entry.path();
                    }
                }
            }
        }
    }
    PathBuf::from("ro-sessiond")
}

#[cfg(test)]
pub fn workspace_debug_sessiond() -> Option<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest.parent()?;
    let target_root = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace.join("target"));
    [
        target_root.join("debug/ro-sessiond"),
        workspace.join("target/debug/ro-sessiond"),
    ]
    .into_iter()
    .find(|path| path.exists())
}

pub type SessionRedactions = Arc<StdMutex<Vec<String>>>;

pub struct SpawnedSupervisor {
    pub protocol: Arc<SessionProtocol>,
    #[allow(dead_code)]
    pub ready: ReadyInfo,
    #[allow(dead_code)]
    pub prefix: PathBuf,
    pub child: Child,
    pub supervisor_identity: ProcessIdentity,
    pub redactions: SessionRedactions,
}

pub async fn kill_child_by_identity(
    child: &mut Child,
    identity: &ProcessIdentity,
) -> Result<(), SessionError> {
    if verify_process_identity(identity) {
        let _ = unsafe { libc::kill(identity.pid as i32, libc::SIGTERM) };
        let _ = timeout(Duration::from_secs(2), child.wait()).await;
        if verify_process_identity(identity) {
            let _ = unsafe { libc::kill(identity.pid as i32, libc::SIGKILL) };
        }
    }
    let _ = timeout(Duration::from_secs(3), child.wait()).await;
    Ok(())
}

fn child_identity(child: &Child) -> Option<ProcessIdentity> {
    child.id().and_then(capture_process_identity)
}

pub async fn spawn_supervisor(
    app: Option<&AppHandle>,
    prefix: &Path,
    sidecar_override: Option<&Path>,
) -> Result<SpawnedSupervisor, SessionError> {
    let canonical = canonicalize_prefix_path(prefix)
        .ok_or_else(|| SessionError::validation("prefix path could not be canonicalized"))?;

    let sessiond_path = find_ro_sessiond(sidecar_override);
    let parent_pid = std::process::id().to_string();

    let mut cmd = Command::new(&sessiond_path);
    cmd.args([
        "--prefix",
        canonical.to_string_lossy().as_ref(),
        "--parent-pid",
        &parent_pid,
    ]);
    sanitize_appimage_env(&mut cmd);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|e| {
        SessionError::internal(format!("spawn ro-sessiond ({sessiond_path:?}): {e}"))
    })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| SessionError::internal("missing supervisor stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| SessionError::internal("missing supervisor stderr"))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| SessionError::internal("missing supervisor stdin"))?;

    let redactions: SessionRedactions = Arc::new(StdMutex::new(Vec::new()));
    let protocol = SessionProtocol::new(stdin);
    SessionProtocol::spawn_reader(stdout, Arc::clone(&protocol));

    let app_stderr = app.cloned();
    let redactions_stderr = Arc::clone(&redactions);
    tokio::spawn(async move {
        let mut lines = tokio::io::BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let snapshot = redactions_stderr.lock().unwrap().clone();
            handle_stderr_line(app_stderr.as_ref(), &line, &snapshot);
        }
    });

    let identity = child_identity(&child)
        .ok_or_else(|| SessionError::internal("failed to capture supervisor identity at spawn"))?;

    let ready_result = timeout(Duration::from_secs(5), protocol.handshake()).await;

    let ready = match ready_result {
        Ok(Ok(info)) => info,
        Ok(Err(e)) => {
            let _ = kill_child_by_identity(&mut child, &identity).await;
            return Err(e);
        }
        Err(_) => {
            let _ = kill_child_by_identity(&mut child, &identity).await;
            return Err(SessionError::handshake("timeout waiting for Ready"));
        }
    };

    if ready.protocol_version != PROTOCOL_VERSION {
        let _ = kill_child_by_identity(&mut child, &identity).await;
        return Err(SessionError::handshake(format!(
            "protocol version mismatch: {}",
            ready.protocol_version
        )));
    }
    if !ready.subreaper {
        let _ = kill_child_by_identity(&mut child, &identity).await;
        return Err(SessionError::handshake("supervisor is not a subreaper"));
    }
    if ready.prefix != canonical.to_string_lossy() {
        let _ = kill_child_by_identity(&mut child, &identity).await;
        return Err(SessionError::handshake("prefix mismatch in Ready"));
    }

    let supervisor_identity = child_identity(&child).unwrap_or(identity);

    let token = prefix_log_token(&canonical);
    emit_session_line(
        app,
        format!(
            "prefix={token} supervisor={} subreaper=true protocol={}",
            ready.supervisor_pid, PROTOCOL_VERSION
        ),
    );

    Ok(SpawnedSupervisor {
        protocol,
        ready,
        prefix: canonical,
        child,
        supervisor_identity,
        redactions,
    })
}
