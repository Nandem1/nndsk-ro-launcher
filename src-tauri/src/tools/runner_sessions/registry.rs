use crate::state::GameProcessHandle;
use ro_session_protocol::{clamp_grace_ms, ProcessSpec};
use ro_tools_linux::{capture_process_identity, verify_process_identity, ProcessIdentity};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use tauri::AppHandle;
use tokio::process::Child;
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

use crate::utils::{RunnerInvocation, RunnerKind, WineContext};

use super::bootstrap::bootstrap_prefix_for_supervisor;
use super::client::{kill_child_by_identity, spawn_supervisor, SessionRedactions};
use super::diagnostics::{emit_session_line, path_log_token};
use super::protocol::{canonicalize_prefix_path, invocation_to_spec, SessionProtocol};

const RUNNER_CONFLICT_MSG: &str =
    "El prefix ya está activo con otro runner; ciérralo antes de cambiarlo.";

const IDLE_SHUTDOWN_SECS: u64 = 2;
const SHUTDOWN_DEFAULT_GRACE_MS: u64 = 5_000;
const SHUTDOWN_ALL_SESSION_TIMEOUT: Duration = Duration::from_secs(15);
const SHUTDOWN_ALL_GLOBAL_TIMEOUT: Duration = Duration::from_secs(30);
const TERMINATE_KILL_DELAY: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct SessionError {
    pub message: String,
}

impl SessionError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn handshake(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn protocol(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn remote(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<String> for SessionError {
    fn from(message: String) -> Self {
        Self { message }
    }
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessExit {
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
}

#[allow(dead_code)]
pub enum SessionOwnership {
    Direct,
    Supervised(ClientLease),
}

pub struct MemoryLease;

pub struct ClientRuntimeGuard {
    #[allow(dead_code)]
    pub session: SessionOwnership,
    pub memory: Option<MemoryLease>,
}

impl std::fmt::Debug for ClientRuntimeGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientRuntimeGuard")
            .field("memory", &self.memory.is_some())
            .finish_non_exhaustive()
    }
}

pub struct ClientLease {
    #[allow(dead_code)]
    session: Arc<RunnerSessionInner>,
}

impl Drop for ClientLease {
    fn drop(&mut self) {
        self.session.release_client_lease();
    }
}

pub struct OperationLease {
    session: Arc<RunnerSessionInner>,
}

impl Drop for OperationLease {
    fn drop(&mut self) {
        self.session.release_operation_lease();
    }
}

pub struct SupervisedProcess {
    session: Arc<RunnerSessionInner>,
    request_id: String,
    controller_identity: ProcessIdentity,
    exit: Mutex<Option<ProcessExit>>,
}

impl SupervisedProcess {
    pub fn controller_pid(&self) -> u32 {
        self.controller_identity.pid
    }

    pub fn try_exit(&self) -> Option<ProcessExit> {
        if let Ok(guard) = self.exit.lock() {
            if let Some(exit) = guard.clone() {
                return Some(exit);
            }
        }
        if let Some(controller) = self.session.protocol.try_controller_exit(&self.request_id) {
            let process_exit = ProcessExit {
                exit_code: controller.exit_code,
                signal: controller.signal,
            };
            if let Ok(mut slot) = self.exit.lock() {
                *slot = Some(process_exit.clone());
            }
            return Some(process_exit);
        }
        None
    }

    pub async fn wait(&mut self) -> Result<ProcessExit, SessionError> {
        if let Some(exit) = self.try_exit() {
            return Ok(exit);
        }
        let exit = self
            .session
            .protocol
            .wait_controller_exit(self.request_id.clone())
            .await?;
        let process_exit = ProcessExit {
            exit_code: exit.exit_code,
            signal: exit.signal,
        };
        if let Ok(mut slot) = self.exit.lock() {
            *slot = Some(process_exit.clone());
        }
        Ok(process_exit)
    }

    pub async fn terminate(&mut self) -> Result<(), SessionError> {
        if self.try_exit().is_some() {
            return Ok(());
        }
        if !verify_process_identity(&self.controller_identity) {
            let _ = self.wait().await;
            return Ok(());
        }
        send_signal(&self.controller_identity, libc::SIGTERM)?;
        if tokio::time::timeout(TERMINATE_KILL_DELAY, self.wait())
            .await
            .is_ok()
        {
            return Ok(());
        }
        if verify_process_identity(&self.controller_identity) {
            send_signal(&self.controller_identity, libc::SIGKILL)?;
        }
        let _ = self.wait().await;
        Ok(())
    }
}

fn send_signal(identity: &ProcessIdentity, signal: i32) -> Result<(), SessionError> {
    if !verify_process_identity(identity) {
        return Ok(());
    }
    let rc = unsafe { libc::kill(identity.pid as i32, signal) };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        return Err(SessionError::internal(format!(
            "kill pid {}: {}",
            identity.pid, err
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionState {
    #[allow(dead_code)]
    Starting,
    Ready,
    Stopping,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, Copy)]
enum ShutdownCause {
    Idle { generation: u64 },
    Forced,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunnerAnchor {
    kind: RunnerKind,
    path: PathBuf,
}

struct RunnerSessionInner {
    prefix: PathBuf,
    runner: RunnerAnchor,
    shutdown_spec: ProcessSpec,
    state: Mutex<SessionState>,
    protocol: Arc<SessionProtocol>,
    supervisor_identity: ProcessIdentity,
    child: Mutex<Option<Child>>,
    redactions: SessionRedactions,
    active_requests: AtomicU32,
    operation_leases: AtomicU32,
    client_leases: AtomicU32,
    idle_generation: AtomicU64,
    registry: Weak<RunnerSessionRegistryInner>,
}

impl RunnerSessionInner {
    fn runner_kind_label(&self) -> &'static str {
        match self.runner.kind {
            RunnerKind::Wine => "wine",
            RunnerKind::Proton => "proton",
        }
    }
}

impl RunnerSessionInner {
    fn release_operation_lease(self: &Arc<Self>) {
        self.operation_leases.fetch_sub(1, Ordering::SeqCst);
        self.schedule_idle_shutdown();
    }

    fn release_client_lease(self: &Arc<Self>) {
        self.client_leases.fetch_sub(1, Ordering::SeqCst);
        self.schedule_idle_shutdown();
    }

    fn bump_idle_generation(&self) {
        self.idle_generation.fetch_add(1, Ordering::SeqCst);
    }

    fn schedule_idle_shutdown(self: &Arc<Self>) {
        if self.operation_leases.load(Ordering::SeqCst) > 0
            || self.client_leases.load(Ordering::SeqCst) > 0
            || self.active_requests.load(Ordering::SeqCst) > 0
        {
            return;
        }
        let generation = self.idle_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let weak_registry = self.registry.clone();
        let prefix_key = self.prefix.to_string_lossy().to_string();
        let weak_session = Arc::downgrade(self);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(IDLE_SHUTDOWN_SECS)).await;
            if let Some(inner) = weak_registry.upgrade() {
                let registry = RunnerSessionRegistry { inner };
                registry.idle_shutdown_if_quiet(&prefix_key, generation, weak_session);
            }
        });
    }

    fn on_launch_accepted(self: &Arc<Self>) {
        self.active_requests.fetch_add(1, Ordering::SeqCst);
    }

    fn on_controller_exited(self: &Arc<Self>) {
        let _ = self
            .active_requests
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                Some(count.saturating_sub(1))
            });
        self.schedule_idle_shutdown();
    }
}

struct RunnerSessionRegistryInner {
    sessions: Mutex<HashMap<String, Arc<RunnerSessionInner>>>,
    startup_locks: Mutex<HashMap<String, Arc<AsyncMutex<()>>>>,
    sidecar_override: Mutex<Option<PathBuf>>,
}

#[derive(Clone)]
pub struct RunnerSessionRegistry {
    inner: Arc<RunnerSessionRegistryInner>,
}

impl Default for RunnerSessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RunnerSessionRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RunnerSessionRegistryInner {
                sessions: Mutex::new(HashMap::new()),
                startup_locks: Mutex::new(HashMap::new()),
                sidecar_override: Mutex::new(None),
            }),
        }
    }

    #[allow(dead_code)]
    pub fn with_sidecar_for_test(path: PathBuf) -> Self {
        let registry = Self::new();
        *registry.inner.sidecar_override.lock().unwrap() = Some(path);
        registry
    }

    fn sidecar_override(&self) -> Option<PathBuf> {
        self.inner.sidecar_override.lock().unwrap().clone()
    }

    fn startup_lock(&self, prefix_key: &str) -> Arc<AsyncMutex<()>> {
        let mut locks = self.inner.startup_locks.lock().unwrap();
        locks
            .entry(prefix_key.to_string())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone()
    }

    pub async fn begin_operation(
        &self,
        app: &AppHandle,
        ctx: &WineContext,
        game: &GameProcessHandle,
    ) -> Result<OperationLease, SessionError> {
        self.begin_operation_opt(Some(app), ctx, game).await
    }

    pub async fn begin_operation_opt(
        &self,
        app: Option<&AppHandle>,
        ctx: &WineContext,
        game: &GameProcessHandle,
    ) -> Result<OperationLease, SessionError> {
        let prefix = canonicalize_prefix_path(Path::new(&ctx.prefix))
            .ok_or_else(|| SessionError::validation("prefix path could not be canonicalized"))?;
        let prefix_key = prefix.to_string_lossy().to_string();
        let runner = runner_anchor(ctx)?;

        let lock = self.startup_lock(&prefix_key);
        let _guard = lock.lock().await;

        if let Some(existing) = self.get_session(&prefix_key) {
            let state = *existing.state.lock().unwrap();
            match state {
                SessionState::Ready => {
                    if existing.runner != runner {
                        return Err(SessionError::validation(RUNNER_CONFLICT_MSG));
                    }
                    existing.bump_idle_generation();
                    existing.operation_leases.fetch_add(1, Ordering::SeqCst);
                    return Ok(OperationLease { session: existing });
                }
                SessionState::Starting | SessionState::Stopping => {
                    return Err(SessionError::internal("session is not ready"));
                }
                SessionState::Stopped | SessionState::Failed => {
                    self.remove_session(&prefix_key);
                }
            }
        }

        bootstrap_prefix_for_supervisor(ctx, game).await?;

        let shutdown_invocation = ctx
            .resolved
            .shutdown_invocation(&ctx.prefix)
            .map_err(SessionError::validation)?;
        let shutdown_spec = invocation_to_spec(&shutdown_invocation, &prefix)?;

        let override_path = self.sidecar_override();
        let spawned = spawn_supervisor(app, &prefix, override_path.as_deref()).await?;

        let supervisor_identity = spawned.supervisor_identity;

        let session = Arc::new(RunnerSessionInner {
            prefix: prefix.clone(),
            runner,
            shutdown_spec,
            state: Mutex::new(SessionState::Ready),
            protocol: Arc::clone(&spawned.protocol),
            supervisor_identity,
            child: Mutex::new(Some(spawned.child)),
            redactions: spawned.redactions,
            active_requests: AtomicU32::new(0),
            operation_leases: AtomicU32::new(1),
            client_leases: AtomicU32::new(0),
            idle_generation: AtomicU64::new(0),
            registry: Arc::downgrade(&self.inner),
        });

        let session_for_hook = Arc::clone(&session);
        spawned.protocol.set_launch_accepted_hook(Arc::new({
            let session_for_hook = Arc::clone(&session_for_hook);
            move || session_for_hook.on_launch_accepted()
        }));
        spawned
            .protocol
            .set_controller_exited_hook(Arc::new(move |_| {
                session_for_hook.on_controller_exited();
            }));

        self.insert_session(prefix_key, Arc::clone(&session));

        Ok(OperationLease { session })
    }

    pub async fn launch(
        &self,
        app: Option<&AppHandle>,
        operation: &OperationLease,
        invocation: RunnerInvocation,
        redactions: &[String],
    ) -> Result<SupervisedProcess, SessionError> {
        let session = &operation.session;
        if *session.state.lock().unwrap() != SessionState::Ready {
            return Err(SessionError::internal("session is not ready for launch"));
        }

        let spec = invocation_to_spec(&invocation, &session.prefix)?;
        let request_id = Uuid::new_v4().to_string();

        {
            let mut guard = session.redactions.lock().unwrap();
            guard.clear();
            guard.extend(redactions.iter().cloned());
        }

        let runner_token = path_log_token(&session.runner.path);
        emit_session_line(
            app,
            format!(
                "runner={}/{} controller=pending request={}",
                session.runner_kind_label(),
                runner_token,
                request_id
            ),
        );

        let controller_pid = session.protocol.launch(request_id.clone(), spec).await?;

        let controller_identity = capture_process_identity(controller_pid)
            .ok_or_else(|| SessionError::internal("failed to capture controller identity"))?;

        emit_session_line(
            app,
            format!("controller={} request={}", controller_pid, request_id),
        );

        Ok(SupervisedProcess {
            session: Arc::clone(session),
            request_id,
            controller_identity,
            exit: Mutex::new(None),
        })
    }

    pub fn attach_client(
        &self,
        operation: &OperationLease,
        _client_id: &str,
    ) -> Result<ClientLease, SessionError> {
        let session = &operation.session;
        session.client_leases.fetch_add(1, Ordering::SeqCst);
        session.bump_idle_generation();
        Ok(ClientLease {
            session: Arc::clone(session),
        })
    }

    #[allow(dead_code)] // fase 4 idle prefix shutdown.
    pub async fn shutdown_prefix(&self, ctx: &WineContext) -> Result<(), SessionError> {
        let prefix = canonicalize_prefix_path(Path::new(&ctx.prefix))
            .ok_or_else(|| SessionError::validation("prefix path could not be canonicalized"))?;
        let prefix_key = prefix.to_string_lossy().to_string();
        let session = self
            .get_session(&prefix_key)
            .ok_or_else(|| SessionError::validation("no active session for prefix"))?;

        if session.operation_leases.load(Ordering::SeqCst) > 0
            || session.client_leases.load(Ordering::SeqCst) > 0
            || session.active_requests.load(Ordering::SeqCst) > 0
        {
            return Err(SessionError::validation(
                "cannot shutdown prefix while leases or requests are active",
            ));
        }

        self.shutdown_session(&prefix_key, &session, None, ShutdownCause::Forced)
            .await
    }

    pub async fn shutdown_all(&self) -> Vec<SessionError> {
        let keys: Vec<String> = {
            let map = self.inner.sessions.lock().unwrap();
            map.keys().cloned().collect()
        };
        if keys.is_empty() {
            return vec![];
        }

        let result = tokio::time::timeout(SHUTDOWN_ALL_GLOBAL_TIMEOUT, async {
            let mut errors = Vec::new();
            for key in keys {
                if let Some(session) = self.get_session(&key) {
                    match tokio::time::timeout(
                        SHUTDOWN_ALL_SESSION_TIMEOUT,
                        self.shutdown_session(&key, &session, None, ShutdownCause::Forced),
                    )
                    .await
                    {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => errors.push(e),
                        Err(_) => {
                            errors.push(SessionError::internal("session shutdown timed out"));
                            self.force_kill_session(&key, &session).await;
                        }
                    }
                }
            }
            errors
        })
        .await;

        match result {
            Ok(errors) => errors,
            Err(_) => {
                let errors = vec![SessionError::internal("shutdown_all global timeout")];
                let remaining: Vec<String> = {
                    let map = self.inner.sessions.lock().unwrap();
                    map.keys().cloned().collect()
                };
                for key in remaining {
                    if let Some(session) = self.get_session(&key) {
                        self.force_kill_session(&key, &session).await;
                    }
                }
                errors
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn has_session(&self, prefix_key: &str) -> bool {
        self.get_session(prefix_key).is_some()
    }

    fn get_session(&self, prefix_key: &str) -> Option<Arc<RunnerSessionInner>> {
        self.inner.sessions.lock().unwrap().get(prefix_key).cloned()
    }

    fn insert_session(&self, prefix_key: String, session: Arc<RunnerSessionInner>) {
        self.inner
            .sessions
            .lock()
            .unwrap()
            .insert(prefix_key, session);
    }

    fn remove_session(&self, prefix_key: &str) {
        self.inner.sessions.lock().unwrap().remove(prefix_key);
    }

    async fn shutdown_session(
        &self,
        prefix_key: &str,
        session: &Arc<RunnerSessionInner>,
        grace_ms: Option<u64>,
        cause: ShutdownCause,
    ) -> Result<(), SessionError> {
        let skip_shutdown_request = {
            let mut state = session.state.lock().unwrap();
            match *state {
                SessionState::Stopped | SessionState::Failed => return Ok(()),
                SessionState::Stopping => true,
                SessionState::Ready | SessionState::Starting => {
                    if let ShutdownCause::Idle { generation } = cause {
                        if session.idle_generation.load(Ordering::SeqCst) != generation
                            || session.operation_leases.load(Ordering::SeqCst) > 0
                            || session.client_leases.load(Ordering::SeqCst) > 0
                            || session.active_requests.load(Ordering::SeqCst) > 0
                            || *state != SessionState::Ready
                        {
                            return Ok(());
                        }
                    }
                    *state = SessionState::Stopping;
                    false
                }
            }
        };

        if !skip_shutdown_request {
            let spec = session.shutdown_spec.clone();
            let request_id = Uuid::new_v4().to_string();
            let grace = clamp_grace_ms(grace_ms.unwrap_or(SHUTDOWN_DEFAULT_GRACE_MS));

            if let Err(e) = session.protocol.shutdown(request_id, spec, grace).await {
                return self.fail_session(prefix_key, session, e).await;
            }
        }

        let stopped =
            tokio::time::timeout(Duration::from_secs(20), session.protocol.wait_stopped()).await;

        match stopped {
            Ok(Ok(())) => {
                *session.state.lock().unwrap() = SessionState::Stopped;
                let taken = session.child.lock().unwrap().take();
                if let Some(mut child) = taken {
                    let _ = tokio::time::timeout(Duration::from_secs(3), child.wait()).await;
                }
                self.remove_session(prefix_key);
                emit_session_line(None, "prefix idle; supervisor stopped cleanly; zombies=0");
                Ok(())
            }
            Ok(Err(e)) => self.fail_session(prefix_key, session, e).await,
            Err(_) => {
                self.fail_session(
                    prefix_key,
                    session,
                    SessionError::internal("wait Stopped timed out"),
                )
                .await
            }
        }
    }

    async fn force_kill_session(&self, prefix_key: &str, session: &Arc<RunnerSessionInner>) {
        let _ = self
            .fail_session(
                prefix_key,
                session,
                SessionError::internal("forced supervisor shutdown"),
            )
            .await;
    }

    async fn fail_session(
        &self,
        prefix_key: &str,
        session: &Arc<RunnerSessionInner>,
        err: SessionError,
    ) -> Result<(), SessionError> {
        *session.state.lock().unwrap() = SessionState::Failed;
        kill_supervisor_identity(&session.supervisor_identity);
        let taken = session.child.lock().unwrap().take();
        if let Some(mut child) = taken {
            let _ = kill_child_by_identity(&mut child, &session.supervisor_identity).await;
        }
        self.remove_session(prefix_key);
        Err(err)
    }

    fn idle_shutdown_if_quiet(
        &self,
        prefix_key: &str,
        generation: u64,
        weak_session: Weak<RunnerSessionInner>,
    ) {
        let session = match weak_session.upgrade() {
            Some(session) => session,
            None => return,
        };
        if !self
            .get_session(prefix_key)
            .is_some_and(|active| Arc::ptr_eq(&active, &session))
        {
            return;
        }
        if session.idle_generation.load(Ordering::SeqCst) != generation {
            return;
        }
        if session.operation_leases.load(Ordering::SeqCst) > 0
            || session.client_leases.load(Ordering::SeqCst) > 0
            || session.active_requests.load(Ordering::SeqCst) > 0
        {
            return;
        }
        if *session.state.lock().unwrap() != SessionState::Ready {
            return;
        }
        let registry = self.clone();
        let key = prefix_key.to_string();
        tokio::spawn(async move {
            if let Err(e) = registry
                .shutdown_session(&key, &session, None, ShutdownCause::Idle { generation })
                .await
            {
                emit_session_line(None, format!("idle shutdown failed: {e}"));
            }
        });
    }
}

fn runner_anchor(ctx: &WineContext) -> Result<RunnerAnchor, SessionError> {
    let path = std::fs::canonicalize(ctx.resolved.runner_path())
        .unwrap_or_else(|_| ctx.resolved.runner_path().to_path_buf());
    Ok(RunnerAnchor {
        kind: ctx.resolved.kind(),
        path,
    })
}

fn kill_supervisor_identity(identity: &ProcessIdentity) {
    if verify_process_identity(identity) {
        let _ = unsafe { libc::kill(identity.pid as i32, libc::SIGKILL) };
    }
}

#[cfg(test)]
mod integration {
    use super::*;
    use crate::state::GameProcessHandle;
    use crate::tools::runner_sessions::client::{spawn_supervisor, workspace_debug_sessiond};
    use crate::utils::{resolve_runner, PrefixLocation, PrefixScope, WineContext};
    use std::ffi::OsString;

    fn test_prefix() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ro-session-registry-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        canonicalize_prefix_path(&dir).unwrap()
    }

    fn test_invocation(prefix: &Path, program: &str, args: &[&str]) -> RunnerInvocation {
        RunnerInvocation {
            program: PathBuf::from(program),
            args: args.iter().map(|a| OsString::from(*a)).collect(),
            cwd: prefix.to_path_buf(),
            env: vec![(
                OsString::from("WINEPREFIX"),
                Some(OsString::from(prefix.to_string_lossy().as_ref())),
            )],
        }
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn handshake_ready_subreaper() {
        let sessiond = workspace_debug_sessiond().expect("build ro-sessiond first");
        let prefix = test_prefix();
        let spawned = spawn_supervisor(None, &prefix, Some(&sessiond))
            .await
            .expect("spawn");
        assert!(spawned.ready.subreaper);
        assert_eq!(
            spawned.ready.protocol_version,
            ro_session_protocol::PROTOCOL_VERSION
        );
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn launch_sleep_and_idle_shutdown() {
        let sessiond = workspace_debug_sessiond().expect("build ro-sessiond first");
        let prefix = test_prefix();
        let registry = RunnerSessionRegistry::with_sidecar_for_test(sessiond.clone());

        let ctx = test_wine_context(&prefix);
        let lease = registry
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new())
            .await
            .expect("begin");
        let mut child = registry
            .launch(
                None,
                &lease,
                test_invocation(&prefix, "/usr/bin/sleep", &["0.05"]),
                &[],
            )
            .await
            .expect("launch");
        let _ = child.wait().await.expect("wait exit");
        drop(child);
        drop(lease);

        let prefix_key = prefix.to_string_lossy().to_string();
        for _ in 0..40 {
            if !registry.has_session(&prefix_key) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        panic!("supervisor session did not shut down after idle grace");
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn runner_conflict_message() {
        let sessiond = workspace_debug_sessiond().expect("build ro-sessiond first");
        let prefix = test_prefix();
        let registry = RunnerSessionRegistry::with_sidecar_for_test(sessiond.clone());
        let ctx = test_wine_context(&prefix);
        let lease = registry
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new())
            .await
            .unwrap();
        let _client = registry.attach_client(&lease, "client-1").unwrap();

        let other_ctx = WineContext {
            prefix: ctx.prefix.clone(),
            location: ctx.location.clone(),
            resolved: other_runner(&ctx.resolved),
        };

        match registry
            .begin_operation_opt(None, &other_ctx, &GameProcessHandle::new())
            .await
        {
            Err(SessionError { message }) => {
                assert_eq!(message, RUNNER_CONFLICT_MSG);
            }
            Ok(_) => panic!("expected runner conflict"),
        }

        registry.shutdown_all().await;
    }

    fn install_fake_wine_pair(dir: &Path, wine_name: &str) {
        std::fs::create_dir_all(dir).unwrap();
        for name in [wine_name, "wineserver"] {
            let path = dir.join(name);
            std::fs::write(&path, b"#!/bin/sh\nexit 0\n").unwrap();
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).unwrap();
        }
    }

    fn test_wine_context(prefix: &Path) -> WineContext {
        let runner_dir = std::env::temp_dir().join(format!(
            "ro-fake-wine-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        install_fake_wine_pair(&runner_dir, "wine");
        let wine_path = runner_dir.join("wine");
        let resolved =
            resolve_runner(wine_path.to_string_lossy().as_ref()).expect("fake wine runner");
        WineContext {
            prefix: prefix.to_string_lossy().into_owned(),
            location: PrefixLocation {
                path: prefix.to_string_lossy().into_owned(),
                scope: PrefixScope::Shared,
                managed: true,
                server_id: None,
            },
            resolved,
        }
    }

    fn other_runner(primary: &crate::utils::ResolvedRunner) -> crate::utils::ResolvedRunner {
        let runner_dir = std::env::temp_dir().join(format!(
            "ro-fake-wine64-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        install_fake_wine_pair(&runner_dir, "wine64");
        let wine_path = runner_dir.join("wine64");
        let resolved =
            resolve_runner(wine_path.to_string_lossy().as_ref()).expect("fake wine64 runner");
        assert_ne!(
            primary.runner_path(),
            resolved.runner_path(),
            "need distinct runner paths for conflict test"
        );
        resolved
    }

    fn broken_sidecar() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "ro-broken-sessiond-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        std::fs::write(&path, b"#!/bin/sh\nsleep 120\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
        path
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn begin_operation_rejects_missing_sidecar() {
        let prefix = test_prefix();
        let registry = RunnerSessionRegistry::with_sidecar_for_test(PathBuf::from(
            "/tmp/ro-launcher-no-such-sessiond-sidecar",
        ));
        let ctx = test_wine_context(&prefix);
        let prefix_key = prefix.to_string_lossy().to_string();
        assert!(registry
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new())
            .await
            .is_err());
        assert!(!registry.has_session(&prefix_key));
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn begin_operation_rejects_invalid_handshake() {
        let prefix = test_prefix();
        let broken = broken_sidecar();
        let registry = RunnerSessionRegistry::with_sidecar_for_test(broken);
        let ctx = test_wine_context(&prefix);
        let prefix_key = prefix.to_string_lossy().to_string();
        assert!(registry
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new())
            .await
            .is_err());
        assert!(!registry.has_session(&prefix_key));
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn shutdown_all_with_ready_lease() {
        let sessiond = workspace_debug_sessiond().expect("build ro-sessiond first");
        let prefix = test_prefix();
        let registry = RunnerSessionRegistry::with_sidecar_for_test(sessiond);
        let ctx = test_wine_context(&prefix);
        let prefix_key = prefix.to_string_lossy().to_string();
        let _lease = registry
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new())
            .await
            .expect("begin");
        let errors = registry.shutdown_all().await;
        assert!(errors.is_empty());
        assert!(!registry.has_session(&prefix_key));
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn dropping_supervised_process_keeps_session_until_controller_exits() {
        let sessiond = workspace_debug_sessiond().expect("build ro-sessiond first");
        let prefix = test_prefix();
        let registry = RunnerSessionRegistry::with_sidecar_for_test(sessiond);
        let ctx = test_wine_context(&prefix);
        let prefix_key = prefix.to_string_lossy().to_string();
        let lease = registry
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new())
            .await
            .expect("begin");
        let child = registry
            .launch(
                None,
                &lease,
                test_invocation(&prefix, "/usr/bin/sleep", &["2"]),
                &[],
            )
            .await
            .expect("launch");
        drop(child);
        assert!(registry.has_session(&prefix_key));
        tokio::time::sleep(Duration::from_secs(3)).await;
        drop(lease);
        for _ in 0..20 {
            if !registry.has_session(&prefix_key) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        panic!("session still active after controller exit and lease drop");
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn idle_shutdown_aborts_with_active_lease() {
        let sessiond = workspace_debug_sessiond().expect("build ro-sessiond first");
        let prefix = test_prefix();
        let registry = RunnerSessionRegistry::with_sidecar_for_test(sessiond);
        let ctx = test_wine_context(&prefix);
        let prefix_key = prefix.to_string_lossy().to_string();
        let _lease = registry
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new())
            .await
            .expect("begin");
        let session = registry.get_session(&prefix_key).expect("session");
        let generation = session.idle_generation.load(Ordering::SeqCst);
        registry
            .shutdown_session(
                &prefix_key,
                &session,
                None,
                ShutdownCause::Idle { generation },
            )
            .await
            .expect("idle shutdown should noop");
        assert!(registry.has_session(&prefix_key));
        assert_eq!(*session.state.lock().unwrap(), SessionState::Ready);
        registry.shutdown_all().await;
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn concurrent_shutdown_all_is_idempotent() {
        let sessiond = workspace_debug_sessiond().expect("build ro-sessiond first");
        let prefix = test_prefix();
        let registry = RunnerSessionRegistry::with_sidecar_for_test(sessiond);
        let ctx = test_wine_context(&prefix);
        let prefix_key = prefix.to_string_lossy().to_string();
        let _lease = registry
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new())
            .await
            .expect("begin");
        let reg_a = registry.clone();
        let reg_check = registry.clone();
        let (a, b) = tokio::join!(
            tokio::spawn(async move { reg_a.shutdown_all().await }),
            tokio::spawn(async move { registry.shutdown_all().await })
        );
        let _ = a.expect("task a");
        let _ = b.expect("task b");
        assert!(!reg_check.has_session(&prefix_key));
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn launch_fails_when_supervisor_killed_before_accept() {
        use ro_tools_linux::verify_process_identity;

        let sessiond = workspace_debug_sessiond().expect("build ro-sessiond first");
        let prefix = test_prefix();
        let registry = RunnerSessionRegistry::with_sidecar_for_test(sessiond);
        let ctx = test_wine_context(&prefix);
        let lease = registry
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new())
            .await
            .expect("begin");
        let prefix_key = prefix.to_string_lossy().to_string();
        let session = registry.get_session(&prefix_key).expect("session");
        kill_supervisor_identity(&session.supervisor_identity);
        let taken = session.child.lock().unwrap().take();
        if let Some(mut child) = taken {
            let _ = kill_child_by_identity(&mut child, &session.supervisor_identity).await;
        }
        assert!(!verify_process_identity(&session.supervisor_identity));

        let result = tokio::time::timeout(
            Duration::from_secs(5),
            registry.launch(
                None,
                &lease,
                test_invocation(&prefix, "/usr/bin/sleep", &["30"]),
                &[],
            ),
        )
        .await
        .expect("launch should not hang");
        assert!(result.is_err());
        registry.shutdown_all().await;
    }
}
