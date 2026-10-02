use crate::state::GameProcessHandle;
use ro_session_protocol::{clamp_grace_ms, ProcessSpec};
use ro_tools_linux::{
    capture_process_identity, is_descendant_of, signal_process_identity, verify_process_identity,
    ProcessIdentity,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use tauri::AppHandle;
use tokio::process::Child;
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

use crate::models::memory::{MemoryAccess, ProfileMemory};
use crate::tools::memory_sessions::MemoryLease;
use crate::utils::{RunnerInvocation, RunnerKind, WineContext};

use super::bootstrap::bootstrap_prefix_for_supervisor;
use super::client::{kill_child_by_identity, spawn_supervisor, SessionRedactions};
use super::diagnostics::{emit_session_line, path_log_token};
use super::protocol::{canonicalize_prefix_path, invocation_to_spec, SessionProtocol};

const RUNNER_CONFLICT_MSG: &str =
    "El prefix ya está activo con otro runner; ciérralo antes de cambiarlo.";
const PLAN_CONFLICT_MSG: &str = "El entorno todavía está en uso con otra configuración. Espera a que termine la preparación o cierra el juego y las herramientas de ese servidor.";

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

pub struct ClientRuntimeGuard {
    #[allow(dead_code)]
    pub session: SessionOwnership,
    pub memory: Option<MemoryLease>,
    pub memory_access: Option<MemoryAccess>,
    pub profile_memory: Option<ProfileMemory>,
}

impl std::fmt::Debug for ClientRuntimeGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientRuntimeGuard")
            .field("memory", &self.memory.is_some())
            .field("memory_access", &self.memory_access)
            .field("profile_memory", &self.profile_memory)
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

impl OperationLease {
    pub fn supervisor_identity(&self) -> ProcessIdentity {
        self.session.supervisor_identity
    }
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

    pub fn controller_identity(&self) -> ProcessIdentity {
        self.controller_identity
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
    signal_process_identity(identity, signal)
        .map(|_| ())
        .map_err(|error| SessionError::internal(format!("kill pid {}: {error}", identity.pid)))
}

fn merge_redactions(current: &mut Vec<String>, incoming: &[String]) {
    for value in incoming.iter().filter(|value| !value.is_empty()) {
        if !current.contains(value) {
            current.push(value.clone());
        }
    }
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
    Quiescent,
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
    plan_id: String,
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
    fn is_quiet(&self) -> bool {
        self.operation_leases.load(Ordering::SeqCst) == 0
            && self.client_leases.load(Ordering::SeqCst) == 0
            && self.active_requests.load(Ordering::SeqCst) == 0
    }

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
        saturating_decrement(&self.active_requests);
        self.schedule_idle_shutdown();
    }
}

fn saturating_decrement(counter: &AtomicU32) {
    // Keep the saturating transition atomic without depending on fetch_update/try_update,
    // whose availability and deprecation differ between supported Rust toolchains.
    let mut count = counter.load(Ordering::SeqCst);
    while count != 0 {
        match counter.compare_exchange_weak(count, count - 1, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return,
            Err(current) => count = current,
        }
    }
}

struct RunnerSessionRegistryInner {
    sessions: Mutex<HashMap<String, Arc<RunnerSessionInner>>>,
    startup_locks: Mutex<HashMap<String, Arc<AsyncMutex<()>>>>,
    sidecar_override: Mutex<Option<PathBuf>>,
    closing: AtomicBool,
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
                closing: AtomicBool::new(false),
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
        anchor: &crate::tools::runtime::SessionAnchorV2,
    ) -> Result<OperationLease, SessionError> {
        self.begin_operation_opt(Some(app), ctx, game, anchor).await
    }

    pub async fn begin_operation_opt(
        &self,
        app: Option<&AppHandle>,
        ctx: &WineContext,
        game: &GameProcessHandle,
        anchor: &crate::tools::runtime::SessionAnchorV2,
    ) -> Result<OperationLease, SessionError> {
        let prefix = canonicalize_prefix_path(Path::new(&ctx.prefix))
            .ok_or_else(|| SessionError::validation("prefix path could not be canonicalized"))?;
        let prefix_key = prefix.to_string_lossy().to_string();
        let runner = runner_anchor(ctx)?;

        loop {
            let lock = self.startup_lock(&prefix_key);
            let guard = lock.lock().await;
            if self.inner.closing.load(Ordering::SeqCst) {
                return Err(SessionError::validation("El launcher se está cerrando"));
            }

            if let Some(existing) = self.get_session(&prefix_key) {
                let shutdown = {
                    let state = existing.state.lock().unwrap();
                    match *state {
                        SessionState::Ready => {
                            if existing.runner != runner {
                                return Err(SessionError::validation(RUNNER_CONFLICT_MSG));
                            }
                            if existing.plan_id != anchor.plan_id {
                                if !existing.is_quiet() {
                                    return Err(SessionError::validation(PLAN_CONFLICT_MSG));
                                }
                                Some(ShutdownCause::Idle {
                                    generation: existing.idle_generation.load(Ordering::SeqCst),
                                })
                            } else {
                                existing.bump_idle_generation();
                                existing.operation_leases.fetch_add(1, Ordering::SeqCst);
                                return Ok(OperationLease {
                                    session: Arc::clone(&existing),
                                });
                            }
                        }
                        SessionState::Stopping => Some(ShutdownCause::Forced),
                        SessionState::Starting => {
                            return Err(SessionError::internal(
                                "El entorno todavía se está preparando",
                            ));
                        }
                        SessionState::Stopped | SessionState::Failed => {
                            self.remove_session_if_same(&prefix_key, &existing);
                            None
                        }
                    }
                };
                if let Some(cause) = shutdown {
                    drop(guard);
                    emit_session_line(
                        app,
                        "Esperando el cierre de la sesión anterior del entorno...",
                    );
                    self.shutdown_session(&prefix_key, &existing, None, cause)
                        .await?;
                    continue;
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
                plan_id: anchor.plan_id.clone(),
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

            let session_for_hook = Arc::downgrade(&session);
            spawned.protocol.set_launch_accepted_hook(Arc::new({
                let session_for_hook = session_for_hook.clone();
                move || {
                    if let Some(session) = session_for_hook.upgrade() {
                        session.on_launch_accepted();
                    }
                }
            }));
            spawned
                .protocol
                .set_controller_exited_hook(Arc::new(move |_| {
                    if let Some(session) = session_for_hook.upgrade() {
                        session.on_controller_exited();
                    }
                }));

            if self.inner.closing.load(Ordering::SeqCst) {
                self.shutdown_session_locked(&prefix_key, &session, None, ShutdownCause::Forced)
                    .await?;
                return Err(SessionError::validation("El launcher se está cerrando"));
            }
            self.insert_session(prefix_key, Arc::clone(&session));

            return Ok(OperationLease { session });
        }
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
            merge_redactions(&mut guard, redactions);
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

        let controller_identity = session.protocol.launch(request_id.clone(), spec).await?;
        let controller_pid = controller_identity.pid;

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

        self.shutdown_session(&prefix_key, &session, None, ShutdownCause::Quiescent)
            .await
    }

    pub async fn shutdown_all(&self) -> Vec<SessionError> {
        let registry = self.clone();
        // Admission closes synchronously; cleanup has an owner even if its caller disappears.
        self.inner.closing.store(true, Ordering::SeqCst);
        tokio::spawn(async move { registry.shutdown_all_owned().await })
            .await
            .unwrap_or_else(|error| {
                vec![SessionError::internal(format!(
                    "shutdown_all task failed: {error}"
                ))]
            })
    }

    async fn shutdown_all_owned(&self) -> Vec<SessionError> {
        // Close admission before taking the census. Include startups still holding their lock:
        // they may not have inserted a session yet.
        self.inner.closing.store(true, Ordering::SeqCst);
        let keys: Vec<String> = {
            let map = self.inner.startup_locks.lock().unwrap();
            map.keys().cloned().collect()
        };
        if keys.is_empty() {
            return vec![];
        }

        let result = tokio::time::timeout(SHUTDOWN_ALL_GLOBAL_TIMEOUT, async {
            let mut shutdowns = tokio::task::JoinSet::new();
            for key in keys {
                let registry = self.clone();
                shutdowns.spawn(async move {
                    let shutdown = async {
                        let lock = registry.startup_lock(&key);
                        let _guard = lock.lock().await;
                        if let Some(session) = registry.get_session(&key) {
                            registry
                                .shutdown_session_locked(
                                    &key,
                                    &session,
                                    None,
                                    ShutdownCause::Forced,
                                )
                                .await
                        } else {
                            Ok(())
                        }
                    };
                    match tokio::time::timeout(SHUTDOWN_ALL_SESSION_TIMEOUT, shutdown).await {
                        Ok(Ok(())) => None,
                        Ok(Err(error)) => Some(error),
                        Err(_) => {
                            if let Some(session) = registry.get_session(&key) {
                                registry.force_kill_session(&key, &session).await;
                            }
                            Some(SessionError::internal("session shutdown timed out"))
                        }
                    }
                });
            }

            let mut errors = Vec::new();
            while let Some(result) = shutdowns.join_next().await {
                match result {
                    Ok(Some(error)) => errors.push(error),
                    Ok(None) => {}
                    Err(error) => errors.push(SessionError::internal(format!(
                        "session shutdown task failed: {error}"
                    ))),
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
                let mut cleanup = tokio::task::JoinSet::new();
                for key in remaining {
                    if let Some(session) = self.get_session(&key) {
                        let registry = self.clone();
                        cleanup.spawn(async move {
                            registry.force_kill_session(&key, &session).await;
                        });
                    }
                }
                while cleanup.join_next().await.is_some() {}
                errors
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn has_session(&self, prefix_key: &str) -> bool {
        self.get_session(prefix_key).is_some()
    }

    pub fn plan_id_for_prefix(&self, prefix: &std::path::Path) -> Option<String> {
        let prefix_key = super::protocol::canonicalize_prefix_path(prefix)
            .map(|path| path.to_string_lossy().to_string())?;
        self.get_session(&prefix_key)
            .map(|session| session.plan_id.clone())
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

    fn remove_session_if_same(&self, prefix_key: &str, session: &Arc<RunnerSessionInner>) {
        let mut sessions = self.inner.sessions.lock().unwrap();
        if sessions
            .get(prefix_key)
            .is_some_and(|active| Arc::ptr_eq(active, session))
        {
            sessions.remove(prefix_key);
        }
    }

    async fn shutdown_session(
        &self,
        prefix_key: &str,
        session: &Arc<RunnerSessionInner>,
        grace_ms: Option<u64>,
        cause: ShutdownCause,
    ) -> Result<(), SessionError> {
        // Own shutdown independently of its caller: cancellation must not leave a half-closed
        // supervisor. The same per-prefix lock serializes shutdown, lease acquisition and startup.
        let registry = self.clone();
        let key = prefix_key.to_string();
        let session = Arc::clone(session);
        tokio::spawn(async move {
            let lock = registry.startup_lock(&key);
            let _guard = lock.lock().await;
            if !registry
                .get_session(&key)
                .is_some_and(|active| Arc::ptr_eq(&active, &session))
            {
                return Ok(());
            }
            registry
                .shutdown_session_locked(&key, &session, grace_ms, cause)
                .await
        })
        .await
        .map_err(|error| SessionError::internal(format!("session shutdown task failed: {error}")))?
    }

    async fn shutdown_session_locked(
        &self,
        prefix_key: &str,
        session: &Arc<RunnerSessionInner>,
        grace_ms: Option<u64>,
        cause: ShutdownCause,
    ) -> Result<(), SessionError> {
        let skip_shutdown_request = {
            let mut state = session.state.lock().unwrap();
            if matches!(cause, ShutdownCause::Quiescent) && !session.is_quiet() {
                return Err(SessionError::validation(
                    "cannot shutdown prefix while leases or requests are active",
                ));
            }
            match *state {
                SessionState::Stopped | SessionState::Failed => return Ok(()),
                SessionState::Stopping => true,
                SessionState::Ready | SessionState::Starting => {
                    if let ShutdownCause::Idle { generation } = cause {
                        if session.idle_generation.load(Ordering::SeqCst) != generation
                            || !session.is_quiet()
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

        let stopped = tokio::time::timeout(Duration::from_secs(20), async {
            if !skip_shutdown_request {
                let spec = session.shutdown_spec.clone();
                let request_id = Uuid::new_v4().to_string();
                let grace = clamp_grace_ms(grace_ms.unwrap_or(SHUTDOWN_DEFAULT_GRACE_MS));

                session.protocol.shutdown(request_id, spec, grace).await?;
            }
            session.protocol.wait_stopped().await
        })
        .await;

        match stopped {
            Ok(Ok(())) => {
                let taken = session.child.lock().unwrap().take();
                if let Some(mut child) = taken {
                    if !matches!(
                        tokio::time::timeout(Duration::from_secs(3), child.wait()).await,
                        Ok(Ok(_))
                    ) {
                        let _ =
                            kill_child_by_identity(&mut child, &session.supervisor_identity).await;
                        return self
                            .fail_session(
                                prefix_key,
                                session,
                                SessionError::internal("supervisor did not exit after Stopped"),
                            )
                            .await;
                    }
                }
                *session.state.lock().unwrap() = SessionState::Stopped;
                self.remove_session_if_same(prefix_key, session);
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
        // A broken protocol or timeout must not orphan a live runner tree. Keep its subreaper
        // alive long enough to reap the killed descendants before terminating the supervisor.
        kill_owned_descendants(&session.supervisor_identity);
        let taken = session.child.lock().unwrap().take();
        if let Some(mut child) = taken {
            let _ = signal_process_identity(&session.supervisor_identity, libc::SIGTERM);
            let _ = signal_process_identity(&session.supervisor_identity, libc::SIGCONT);
            if tokio::time::timeout(Duration::from_secs(3), child.wait())
                .await
                .is_err()
            {
                kill_supervisor_identity(&session.supervisor_identity);
            }
            let _ = kill_child_by_identity(&mut child, &session.supervisor_identity).await;
        }
        *session.state.lock().unwrap() = SessionState::Failed;
        self.remove_session_if_same(prefix_key, session);
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
    let _ = signal_process_identity(identity, libc::SIGKILL);
}

fn kill_owned_descendants(supervisor: &ProcessIdentity) {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let Some(identity) = capture_process_identity(pid) else {
            continue;
        };
        if pid != supervisor.pid
            && verify_process_identity(supervisor)
            && is_descendant_of(pid, supervisor.pid)
            && verify_process_identity(&identity)
        {
            let _ = signal_process_identity(&identity, libc::SIGKILL);
        }
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

    #[test]
    fn redactions_accumulate_for_overlapping_clients() {
        let mut current = vec!["first-secret".to_string()];
        merge_redactions(
            &mut current,
            &["second-secret".to_string(), "first-secret".to_string()],
        );
        assert_eq!(current, ["first-secret", "second-secret"]);
    }

    #[test]
    fn active_requests_decrement_saturates_under_concurrent_exits() {
        let counter = AtomicU32::new(10_000);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    for _ in 0..2_000 {
                        saturating_decrement(&counter);
                    }
                });
            }
        });
        assert_eq!(counter.load(Ordering::SeqCst), 0);
        counter.store(1, Ordering::SeqCst);
        saturating_decrement(&counter);
        saturating_decrement(&counter);
        assert_eq!(counter.load(Ordering::SeqCst), 0);
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
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new(), &placeholder_anchor())
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
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new(), &placeholder_anchor())
            .await
            .unwrap();
        let _client = registry.attach_client(&lease, "client-1").unwrap();

        let other_resolved = other_runner(&ctx.resolved);
        let other_probe = crate::tools::runtime::probe_runner(&other_resolved);
        let other_identity = crate::tools::runtime::resolve_prefix_binding(
            None,
            &other_resolved,
            &other_probe,
            other_resolved.is_wine_7_16(),
        )
        .expect("other prefix binding");
        let other_ctx = WineContext {
            prefix: ctx.prefix.clone(),
            location: ctx.location.clone(),
            resolved: other_resolved,
            identity: other_identity,
            probe: other_probe,
        };

        match registry
            .begin_operation_opt(
                None,
                &other_ctx,
                &GameProcessHandle::new(),
                &placeholder_anchor(),
            )
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
        let probe = crate::tools::runtime::probe_runner(&resolved);
        let identity = crate::tools::runtime::resolve_prefix_binding(
            None,
            &resolved,
            &probe,
            resolved.is_wine_7_16(),
        )
        .expect("test prefix binding");
        WineContext {
            prefix: prefix.to_string_lossy().into_owned(),
            location: PrefixLocation {
                path: prefix.to_string_lossy().into_owned(),
                scope: PrefixScope::Shared,
                managed: true,
                server_id: None,
            },
            resolved,
            identity,
            probe,
        }
    }

    fn placeholder_anchor() -> crate::tools::runtime::SessionAnchorV2 {
        crate::tools::runtime::session_anchor_from_context(
            &test_wine_context(&std::env::temp_dir()),
        )
    }

    #[tokio::test]
    async fn shutdown_closes_admission_and_waits_for_startup_lock() {
        let prefix = test_prefix();
        let registry =
            RunnerSessionRegistry::with_sidecar_for_test(workspace_debug_sessiond().unwrap());
        let ctx = test_wine_context(&prefix);
        let key = prefix.to_string_lossy().to_string();
        let lock = registry.startup_lock(&key);
        let guard = lock.lock().await;
        let shutdown = tokio::spawn({
            let registry = registry.clone();
            async move { registry.shutdown_all().await }
        });
        tokio::time::timeout(Duration::from_secs(1), async {
            while !registry.inner.closing.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(
            !shutdown.is_finished(),
            "shutdown must include pending startup"
        );
        drop(guard);
        assert!(shutdown.await.unwrap().is_empty());
        assert!(registry
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new(), &placeholder_anchor())
            .await
            .err()
            .unwrap()
            .message
            .contains("cerrando"));
    }

    #[tokio::test]
    async fn tool_descendant_keeps_its_prefix_guard_after_controller_exit() {
        let prefix = test_prefix();
        let registry =
            RunnerSessionRegistry::with_sidecar_for_test(workspace_debug_sessiond().unwrap());
        let ctx = test_wine_context(&prefix);
        let game = GameProcessHandle::new();
        let op = crate::tools::runner_sessions::RunnerOperation::begin(
            None,
            &registry,
            &game,
            &ctx,
            &placeholder_anchor(),
        )
        .await
        .unwrap();
        let guard = crate::utils::OperationGuard::acquire("prefix", &prefix).unwrap();
        let mut process = op
            .spawn(
                test_invocation(
                    &prefix,
                    "/bin/sh",
                    &["-c", "env -u WINEPREFIX sleep 1 & exit 0"],
                ),
                &[],
            )
            .await
            .unwrap();
        process.wait().await.unwrap();
        let supervisor = op.lease().unwrap().supervisor_identity();
        let observed: Vec<_> = std::fs::read_dir("/proc")
            .unwrap()
            .flatten()
            .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
            .filter(|pid| *pid != supervisor.pid && is_descendant_of(*pid, supervisor.pid))
            .filter_map(capture_process_identity)
            .collect();
        assert!(!observed.is_empty(), "descendant outlives its controller");
        assert!(tokio::time::timeout(
            Duration::from_millis(50),
            crate::tools::runner_sessions::wait_for_prefix_programs(&ctx.prefix, Some(supervisor))
        )
        .await
        .is_err());
        assert!(crate::utils::OperationGuard::acquire("prefix", &prefix).is_err());
        tokio::time::timeout(
            Duration::from_secs(3),
            crate::tools::runner_sessions::wait_for_prefix_programs(&ctx.prefix, Some(supervisor)),
        )
        .await
        .unwrap();
        drop(guard);
        drop(op);
        assert!(registry.shutdown_all().await.is_empty());
        for identity in observed {
            assert!(!verify_process_identity(&identity));
        }
    }

    #[tokio::test]
    async fn repeated_quiesce_cannot_kill_another_operation() {
        let prefix = test_prefix();
        let registry =
            RunnerSessionRegistry::with_sidecar_for_test(workspace_debug_sessiond().unwrap());
        let ctx = test_wine_context(&prefix);
        let game = GameProcessHandle::new();
        let anchor = placeholder_anchor();
        let mut first = crate::tools::runner_sessions::RunnerOperation::begin(
            None, &registry, &game, &ctx, &anchor,
        )
        .await
        .unwrap();
        let other = registry
            .begin_operation_opt(None, &ctx, &game, &anchor)
            .await
            .unwrap();
        let mut process = registry
            .launch(
                None,
                &other,
                test_invocation(&prefix, "/bin/sleep", &["1"]),
                &[],
            )
            .await
            .unwrap();
        assert!(first.quiesce().await.is_err());
        assert!(first.quiesce().await.is_err());
        assert!(verify_process_identity(&process.controller_identity()));
        assert!(verify_process_identity(&other.supervisor_identity()));
        process.wait().await.unwrap();
        drop(other);
        assert!(registry.shutdown_all().await.is_empty());
    }

    #[tokio::test]
    async fn successful_shutdown_controller_is_not_proof_that_prefix_is_clear() {
        let prefix = test_prefix();
        let registry =
            RunnerSessionRegistry::with_sidecar_for_test(workspace_debug_sessiond().unwrap());
        let ctx = test_wine_context(&prefix);
        let op = crate::tools::runner_sessions::RunnerOperation::begin(
            None,
            &registry,
            &GameProcessHandle::new(),
            &ctx,
            &placeholder_anchor(),
        )
        .await
        .unwrap();
        op.run_shutdown_ok(
            test_invocation(&prefix, "/bin/sh", &["-c", "sleep 1 & exit 0"]),
            "shutdown fixture",
        )
        .await
        .unwrap();
        assert!(ro_tools_linux::find_prefix_processes(&ctx.prefix).is_empty());
        drop(op);
        assert!(registry.shutdown_all().await.is_empty());
    }

    #[tokio::test]
    async fn failed_session_cleans_its_descendants_and_preserves_another_prefix() {
        let registry =
            RunnerSessionRegistry::with_sidecar_for_test(workspace_debug_sessiond().unwrap());
        let first_prefix = test_prefix();
        let second_prefix = test_prefix();
        let first = registry
            .begin_operation_opt(
                None,
                &test_wine_context(&first_prefix),
                &GameProcessHandle::new(),
                &placeholder_anchor(),
            )
            .await
            .unwrap();
        let second = registry
            .begin_operation_opt(
                None,
                &test_wine_context(&second_prefix),
                &GameProcessHandle::new(),
                &placeholder_anchor(),
            )
            .await
            .unwrap();
        let mut process = registry
            .launch(
                None,
                &first,
                test_invocation(&first_prefix, "/bin/sh", &["-c", "sleep 60 & wait"]),
                &[],
            )
            .await
            .unwrap();
        let key = first_prefix.to_string_lossy().to_string();
        let session = registry.get_session(&key).unwrap();
        let identity = process.controller_identity();
        registry.force_kill_session(&key, &session).await;
        assert!(!verify_process_identity(&identity));
        assert!(!verify_process_identity(&first.supervisor_identity()));
        assert!(verify_process_identity(&second.supervisor_identity()));
        let _ = process.wait().await;
        drop(first);
        drop(second);
        assert!(registry.shutdown_all().await.is_empty());
    }

    fn golden_plan_anchor(case: &str) -> crate::tools::runtime::SessionAnchorV2 {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../contract-fixtures/runtime-fingerprint-v1.json");
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).unwrap();
        let hex = json[case]["digestSha256Hex"]
            .as_str()
            .expect("digest")
            .to_string();
        let mut anchor = crate::tools::runtime::session_anchor_unavailable();
        anchor.plan_id = hex;
        anchor
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
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new(), &placeholder_anchor())
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
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new(), &placeholder_anchor())
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
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new(), &placeholder_anchor())
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
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new(), &placeholder_anchor())
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
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new(), &placeholder_anchor())
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
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new(), &placeholder_anchor())
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
    async fn shutdown_prefix_rejected_while_lease_active() {
        let sessiond = workspace_debug_sessiond().expect("build ro-sessiond first");
        let prefix = test_prefix();
        let registry = RunnerSessionRegistry::with_sidecar_for_test(sessiond);
        let ctx = test_wine_context(&prefix);
        let lease = registry
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new(), &placeholder_anchor())
            .await
            .expect("begin");
        let prefix_key = prefix.to_string_lossy().to_string();
        assert!(registry.has_session(&prefix_key));
        let err = registry.shutdown_prefix(&ctx).await.unwrap_err();
        assert!(err.message.contains("cannot shutdown prefix while leases"));
        assert!(registry.has_session(&prefix_key));
        drop(lease);
        registry.shutdown_all().await;
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn two_prefixes_create_independent_sessions() {
        let sessiond = workspace_debug_sessiond().expect("build ro-sessiond first");
        let prefix_a = test_prefix();
        let prefix_b = test_prefix();
        let registry = RunnerSessionRegistry::with_sidecar_for_test(sessiond);
        let ctx_a = test_wine_context(&prefix_a);
        let ctx_b = test_wine_context(&prefix_b);
        let key_a = prefix_a.to_string_lossy().to_string();
        let key_b = prefix_b.to_string_lossy().to_string();
        let lease_a = registry
            .begin_operation_opt(
                None,
                &ctx_a,
                &GameProcessHandle::new(),
                &placeholder_anchor(),
            )
            .await
            .expect("begin a");
        let lease_b = registry
            .begin_operation_opt(
                None,
                &ctx_b,
                &GameProcessHandle::new(),
                &placeholder_anchor(),
            )
            .await
            .expect("begin b");
        assert!(registry.has_session(&key_a));
        assert!(registry.has_session(&key_b));
        drop(lease_a);
        drop(lease_b);
        registry.shutdown_all().await;
        assert!(!registry.has_session(&key_a));
        assert!(!registry.has_session(&key_b));
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn begin_operation_rejects_plan_id_mismatch_on_active_lease() {
        let sessiond = workspace_debug_sessiond().expect("build ro-sessiond first");
        let prefix = test_prefix();
        let registry = RunnerSessionRegistry::with_sidecar_for_test(sessiond);
        let ctx = test_wine_context(&prefix);
        let first = golden_plan_anchor("wine716ManagedDxvk");
        let second = golden_plan_anchor("wine716DgVoodooDxvk");
        assert_ne!(first.plan_id, second.plan_id);
        let lease = registry
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new(), &first)
            .await
            .expect("begin");
        let mismatch = registry
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new(), &second)
            .await;
        let err = match mismatch {
            Ok(_) => panic!("plan mismatch should fail"),
            Err(error) => error,
        };
        assert_eq!(err.message, PLAN_CONFLICT_MSG);
        drop(lease);
        registry.shutdown_all().await;
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn quiet_plan_change_reaps_old_supervisor_before_starting_new_session() {
        let prefix = test_prefix();
        let registry = RunnerSessionRegistry::with_sidecar_for_test(
            workspace_debug_sessiond().expect("build ro-sessiond first"),
        );
        let ctx = test_wine_context(&prefix);
        let game = GameProcessHandle::new();
        let first = golden_plan_anchor("wine716ManagedDxvk");
        let second = golden_plan_anchor("wine716DgVoodooDxvk");
        let lease = registry
            .begin_operation_opt(None, &ctx, &game, &first)
            .await
            .unwrap();
        let old_identity = lease.supervisor_identity();
        let old_session = Arc::downgrade(
            &registry
                .get_session(prefix.to_string_lossy().as_ref())
                .unwrap(),
        );
        let mut child = registry
            .launch(
                None,
                &lease,
                test_invocation(&prefix, "/usr/bin/sleep", &["0.02"]),
                &[],
            )
            .await
            .unwrap();
        child.wait().await.unwrap();
        drop(child);
        drop(lease);

        let next = registry
            .begin_operation_opt(None, &ctx, &game, &second)
            .await
            .unwrap();
        assert_ne!(next.supervisor_identity(), old_identity);
        assert!(
            !verify_process_identity(&old_identity),
            "old supervisor must be reaped"
        );
        assert_eq!(registry.plan_id_for_prefix(&prefix), Some(second.plan_id));
        assert!(
            old_session.upgrade().is_none(),
            "closed sessions must not retain an ownership cycle"
        );
        drop(next);
        assert!(registry.shutdown_all().await.is_empty());
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn preparation_quiesces_before_next_operation_with_another_graphics_plan() {
        let prefix = test_prefix();
        let registry = RunnerSessionRegistry::with_sidecar_for_test(
            workspace_debug_sessiond().expect("build ro-sessiond first"),
        );
        let ctx = test_wine_context(&prefix);
        let game = GameProcessHandle::new();
        let first = golden_plan_anchor("wine716ManagedDxvk");
        let second = golden_plan_anchor("wine716DgVoodooDxvk");
        let mut op = crate::tools::runner_sessions::RunnerOperation::begin(
            None, &registry, &game, &ctx, &first,
        )
        .await
        .unwrap();
        let identity = op.lease().unwrap().supervisor_identity();
        op.run_ok(
            test_invocation(&prefix, "/usr/bin/sleep", &["0.1"]),
            "preparation",
        )
        .await
        .unwrap();
        op.quiesce().await.unwrap();
        assert!(op.lease().is_none());
        assert!(!verify_process_identity(&identity));
        assert!(!registry.has_session(prefix.to_string_lossy().as_ref()));
        let next = registry
            .begin_operation_opt(None, &ctx, &game, &second)
            .await
            .unwrap();
        drop(next);
        assert!(registry.shutdown_all().await.is_empty());
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn plan_change_preserves_requests_and_each_client_lease() {
        let prefix = test_prefix();
        let registry = RunnerSessionRegistry::with_sidecar_for_test(
            workspace_debug_sessiond().expect("build ro-sessiond first"),
        );
        let ctx = test_wine_context(&prefix);
        let game = GameProcessHandle::new();
        let first = golden_plan_anchor("wine716ManagedDxvk");
        let second = golden_plan_anchor("wine716DgVoodooDxvk");
        let lease = registry
            .begin_operation_opt(None, &ctx, &game, &first)
            .await
            .unwrap();
        let old_identity = lease.supervisor_identity();
        let mut child = registry
            .launch(
                None,
                &lease,
                test_invocation(&prefix, "/usr/bin/sleep", &["0.3"]),
                &[],
            )
            .await
            .unwrap();
        drop(lease);
        assert!(registry
            .begin_operation_opt(None, &ctx, &game, &second)
            .await
            .is_err());
        assert!(verify_process_identity(&old_identity));
        child.wait().await.unwrap();

        let lease = registry
            .begin_operation_opt(None, &ctx, &game, &first)
            .await
            .unwrap();
        let client_a = registry.attach_client(&lease, "client-a").unwrap();
        let client_b = registry.attach_client(&lease, "client-b").unwrap();
        drop(lease);
        drop(client_a);
        assert!(registry
            .begin_operation_opt(None, &ctx, &game, &second)
            .await
            .is_err());
        let same = registry
            .begin_operation_opt(None, &ctx, &game, &first)
            .await
            .unwrap();
        assert_eq!(same.supervisor_identity(), old_identity);
        drop(same);
        drop(client_b);

        let next = registry
            .begin_operation_opt(None, &ctx, &game, &second)
            .await
            .unwrap();
        assert!(!verify_process_identity(&old_identity));
        drop(next);
        assert!(registry.shutdown_all().await.is_empty());
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn immediate_retry_waits_for_shutdown_even_if_caller_is_cancelled() {
        let prefix = test_prefix();
        let registry = RunnerSessionRegistry::with_sidecar_for_test(
            workspace_debug_sessiond().expect("build ro-sessiond first"),
        );
        let ctx = test_wine_context(&prefix);
        std::fs::write(
            ctx.resolved
                .runner_path()
                .parent()
                .unwrap()
                .join("wineserver"),
            b"#!/bin/sh\nsleep 0.3\nexit 0\n",
        )
        .unwrap();
        let game = GameProcessHandle::new();
        let anchor = placeholder_anchor();
        let key = prefix.to_string_lossy().to_string();
        for _ in 0..3 {
            let lease = registry
                .begin_operation_opt(None, &ctx, &game, &anchor)
                .await
                .unwrap();
            let old = registry.get_session(&key).unwrap();
            let old_identity = lease.supervisor_identity();
            drop(lease);
            let shutdown = tokio::spawn({
                let registry = registry.clone();
                let ctx = ctx.clone();
                async move { registry.shutdown_prefix(&ctx).await }
            });
            tokio::time::timeout(Duration::from_secs(2), async {
                while *old.state.lock().unwrap() != SessionState::Stopping {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .expect("shutdown must start");
            shutdown.abort();
            let _ = shutdown.await;

            let next = tokio::time::timeout(
                Duration::from_secs(5),
                registry.begin_operation_opt(None, &ctx, &game, &anchor),
            )
            .await
            .expect("retry must complete")
            .expect("retry must wait instead of failing");
            assert!(!verify_process_identity(&old_identity));
            let new_identity = next.supervisor_identity();
            assert_ne!(new_identity, old_identity);
            registry.remove_session_if_same(&key, &old);
            registry
                .shutdown_session(&key, &old, None, ShutdownCause::Forced)
                .await
                .unwrap();
            assert!(
                registry.has_session(&key),
                "stale cleanup must preserve the new session"
            );
            assert!(verify_process_identity(&new_identity));
            drop(next);
        }
        assert!(registry.shutdown_all().await.is_empty());
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
            .begin_operation_opt(None, &ctx, &GameProcessHandle::new(), &placeholder_anchor())
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
