use ro_tools_linux::{
    capture_process_identity, find_prefix_processes, is_descendant_of, is_prefix_leftover_process,
    verify_process_identity, ProcessIdentity,
};
use std::sync::Once;
use tauri::AppHandle;
use tokio::process::Child;

use crate::state::GameProcessHandle;
use crate::utils::{pipe_output, RunnerInvocation, WineContext};

use super::diagnostics::emit_session_line;
use crate::tools::runtime::SessionAnchorV2;

use super::{
    session_supervisor_enabled, OperationLease, ProcessExit, RunnerSessionRegistry,
    SupervisedProcess,
};

static ROLLBACK_LOG_ONCE: Once = Once::new();

/// Called while an exclusive prefix guard is held: no other launcher program can enter.
/// Wine services may persist, but a tool or its handoff child keeps ownership until it exits.
pub(crate) async fn wait_for_prefix_programs(prefix: &str, supervisor: Option<ProcessIdentity>) {
    loop {
        let mut identities = find_prefix_processes(prefix);
        // A handoff child may clear WINEPREFIX. The subreaper remains its stable ancestor.
        if let Some(supervisor) = supervisor.filter(verify_process_identity) {
            if let Ok(entries) = std::fs::read_dir("/proc") {
                for entry in entries.flatten() {
                    let Some(pid) = entry
                        .file_name()
                        .to_str()
                        .and_then(|name| name.parse().ok())
                    else {
                        continue;
                    };
                    if pid == supervisor.pid {
                        continue;
                    }
                    let Some(identity) = capture_process_identity(pid) else {
                        continue;
                    };
                    if is_descendant_of(pid, supervisor.pid)
                        && verify_process_identity(&supervisor)
                        && verify_process_identity(&identity)
                    {
                        identities.push(identity);
                    }
                }
            }
        }
        let active = identities.into_iter().any(|identity| {
            identity.pid != std::process::id()
                && verify_process_identity(&identity)
                && !is_prefix_leftover_process(identity.pid)
                && verify_process_identity(&identity)
        });
        if !active {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

pub struct RunnerOperation {
    app: Option<AppHandle>,
    sessions: RunnerSessionRegistry,
    ctx: WineContext,
    lease: Option<OperationLease>,
    supervised: bool,
}

impl RunnerOperation {
    pub async fn begin(
        app: Option<&AppHandle>,
        sessions: &RunnerSessionRegistry,
        game: &GameProcessHandle,
        ctx: &WineContext,
        anchor: &SessionAnchorV2,
    ) -> Result<Self, String> {
        if let Some(reasons) = ctx.identity.ineligible_reasons() {
            return Err(format!(
                "La identidad del entorno no es compatible: {}",
                reasons.join(" · ")
            ));
        }
        let lease = if session_supervisor_enabled() {
            let result = match app {
                Some(app) => sessions.begin_operation(app, ctx, game, anchor).await,
                None => sessions.begin_operation_opt(None, ctx, game, anchor).await,
            };
            Some(result.map_err(|error| error.message)?)
        } else {
            if let Some(app) = app {
                ROLLBACK_LOG_ONCE.call_once(|| {
                    emit_session_line(
                        Some(app),
                        "supervisor=disabled rollback=RO_LAUNCHER_SESSION_SUPERVISOR=0",
                    );
                });
            }
            None
        };
        let supervised = lease.is_some();
        Ok(Self {
            app: app.cloned(),
            sessions: sessions.clone(),
            ctx: ctx.clone(),
            lease,
            supervised,
        })
    }

    pub fn ctx(&self) -> &WineContext {
        &self.ctx
    }

    pub fn lease(&self) -> Option<&OperationLease> {
        self.lease.as_ref()
    }

    pub async fn run(
        &self,
        invocation: RunnerInvocation,
        error_context: &str,
    ) -> Result<i32, String> {
        if let Some(lease) = &self.lease {
            let mut process = self
                .sessions
                .launch(self.app.as_ref(), lease, invocation, &[])
                .await
                .map_err(|error| error.message)?;
            let exit = process.wait().await.map_err(|error| error.message)?;
            return Ok(exit_code_from_process_exit(&exit, error_context));
        }

        // Path directo: rollback de una release (`RO_LAUNCHER_SESSION_SUPERVISOR=0`); se elimina cuando desaparezca la variable.
        let mut cmd = invocation.into_command();
        pipe_output(&mut cmd);
        let mut child = cmd
            .spawn()
            .map_err(|error| format!("Error al ejecutar {error_context}: {error}"))?;
        if let Some(app) = self.app.as_ref() {
            crate::utils::drain_and_log(app, &mut child).await;
        }
        let status = child.wait().await.map_err(|e| e.to_string())?;
        Ok(status.code().unwrap_or(-1))
    }

    pub async fn run_ok(
        &self,
        invocation: RunnerInvocation,
        error_context: &str,
    ) -> Result<(), String> {
        let code = self.run(invocation, error_context).await?;
        if code != 0 {
            return Err(format!("{error_context} falló con código: {code}"));
        }
        Ok(())
    }

    pub async fn run_shutdown_ok(
        &self,
        invocation: RunnerInvocation,
        error_context: &str,
    ) -> Result<(), String> {
        let code = self.run(invocation, error_context).await?;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let active = find_prefix_processes(&self.ctx.prefix).len();
            if active == 0 {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(format!(
                    "{} no pudo detener el entorno (código {code}; {active} procesos activos)",
                    self.ctx.resolved.kind_label()
                ));
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    /// Quiesces every process owned by this prefix before completing setup or restoring files.
    ///
    /// In supervised mode this consumes the operation lease and closes the whole sidecar, whose
    /// `Stopped` event is only emitted after `waitpid` reaches `ECHILD`. Direct rollback retains the
    /// legacy wineserver shutdown plus an explicit prefix-process check.
    pub async fn quiesce(&mut self) -> Result<(), String> {
        if self.supervised {
            // Repeated cleanup must never switch to a prefix-wide direct shutdown.
            drop(self.lease.take());
            if self
                .sessions
                .plan_id_for_prefix(std::path::Path::new(&self.ctx.prefix))
                .is_some()
            {
                self.sessions
                    .shutdown_prefix(&self.ctx)
                    .await
                    .map_err(|error| error.message)?;
            }
        } else if !find_prefix_processes(&self.ctx.prefix).is_empty() {
            self.run_shutdown_ok(
                self.ctx.resolved.shutdown_invocation(&self.ctx.prefix)?,
                "apagado del entorno incompleto",
            )
            .await?;
        }

        let remaining = find_prefix_processes(&self.ctx.prefix).len();
        if remaining == 0 {
            Ok(())
        } else {
            Err(format!("quedan {remaining} proceso(s) usando el entorno"))
        }
    }

    pub async fn spawn(
        &self,
        invocation: RunnerInvocation,
        redactions: &[String],
    ) -> Result<SpawnedRunner, String> {
        if let Some(lease) = &self.lease {
            let process = self
                .sessions
                .launch(self.app.as_ref(), lease, invocation, redactions)
                .await
                .map_err(|error| error.message)?;
            return Ok(SpawnedRunner::Supervised(process));
        }

        // Path directo: rollback de una release (`RO_LAUNCHER_SESSION_SUPERVISOR=0`); se elimina cuando desaparezca la variable.
        let mut cmd = invocation.into_command();
        pipe_output(&mut cmd);
        let child = cmd
            .spawn()
            .map_err(|error| format!("Error al iniciar el runner: {error}"))?;
        Ok(SpawnedRunner::Direct(child))
    }
}

pub enum SpawnedRunner {
    Direct(Child),
    Supervised(SupervisedProcess),
}

impl SpawnedRunner {
    pub fn controller_identity(&self) -> Option<ro_tools_linux::ProcessIdentity> {
        match self {
            Self::Direct(child) => child
                .id()
                .and_then(ro_tools_linux::capture_process_identity),
            Self::Supervised(process) => Some(process.controller_identity()),
        }
    }
    pub fn controller_pid(&self) -> Option<u32> {
        match self {
            Self::Direct(child) => child.id(),
            Self::Supervised(process) => Some(process.controller_pid()),
        }
    }

    pub async fn wait(&mut self) -> Result<i32, String> {
        match self {
            Self::Direct(child) => {
                let status = child.wait().await.map_err(|error| error.to_string())?;
                Ok(status.code().unwrap_or(-1))
            }
            Self::Supervised(process) => {
                let exit = process.wait().await.map_err(|error| error.message)?;
                Ok(exit.exit_code.unwrap_or(-1))
            }
        }
    }

    pub async fn terminate(&mut self) -> Result<(), String> {
        match self {
            Self::Direct(child) => {
                if child.try_wait().ok().flatten().is_none() {
                    if let Some(identity) = child
                        .id()
                        .and_then(ro_tools_linux::capture_process_identity)
                    {
                        let _ = ro_tools_linux::signal_process_identity(&identity, libc::SIGKILL);
                    }
                }
                let _ = child.wait().await;
                Ok(())
            }
            Self::Supervised(process) => process.terminate().await.map_err(|e| e.message),
        }
    }

    pub fn try_exit_code(&mut self) -> Option<i32> {
        self.try_exit_status()
            .map(|exit| exit.exit_code.unwrap_or(-1))
    }

    pub fn try_exit_status(&mut self) -> Option<ProcessExit> {
        match self {
            Self::Direct(child) => child.try_wait().ok().flatten().map(|status| {
                #[cfg(unix)]
                {
                    use std::os::unix::process::ExitStatusExt;
                    ProcessExit {
                        exit_code: status.code(),
                        signal: status.signal(),
                    }
                }
                #[cfg(not(unix))]
                {
                    ProcessExit {
                        exit_code: status.code(),
                        signal: None,
                    }
                }
            }),
            Self::Supervised(process) => process.try_exit(),
        }
    }

    pub fn take_direct_stdout_stderr(
        &mut self,
    ) -> (
        Option<tokio::process::ChildStdout>,
        Option<tokio::process::ChildStderr>,
    ) {
        match self {
            Self::Direct(child) => (child.stdout.take(), child.stderr.take()),
            Self::Supervised(_) => (None, None),
        }
    }
}

fn exit_code_from_process_exit(exit: &ProcessExit, error_context: &str) -> i32 {
    if let Some(code) = exit.exit_code {
        return code;
    }
    if let Some(signal) = exit.signal {
        return 128 + signal;
    }
    let _ = error_context;
    -1
}
