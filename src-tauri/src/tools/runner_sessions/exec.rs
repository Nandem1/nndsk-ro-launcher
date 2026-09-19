use ro_tools_linux::find_prefix_processes;
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

pub struct RunnerOperation {
    app: Option<AppHandle>,
    sessions: RunnerSessionRegistry,
    ctx: WineContext,
    lease: Option<OperationLease>,
}

impl RunnerOperation {
    pub async fn begin(
        app: Option<&AppHandle>,
        sessions: &RunnerSessionRegistry,
        game: &GameProcessHandle,
        ctx: &WineContext,
        anchor: &SessionAnchorV2,
    ) -> Result<Self, String> {
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
        Ok(Self {
            app: app.cloned(),
            sessions: sessions.clone(),
            ctx: ctx.clone(),
            lease,
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
        let active = find_prefix_processes(&self.ctx.prefix).len();
        if !shutdown_is_complete(code == 0, active) {
            return Err(format!(
                "{} no pudo detener el entorno (código {code})",
                self.ctx.resolved.kind_label()
            ));
        }
        Ok(())
    }

    /// Quiesces every process owned by this prefix before transactional filesystem rollback.
    ///
    /// In supervised mode this consumes the operation lease and closes the whole sidecar, whose
    /// `Stopped` event is only emitted after `waitpid` reaches `ECHILD`. Direct rollback retains the
    /// legacy wineserver shutdown plus an explicit prefix-process check.
    pub async fn quiesce_for_restore(&mut self) -> Result<(), String> {
        if self.lease.take().is_some() {
            self.sessions
                .shutdown_prefix(&self.ctx)
                .await
                .map_err(|error| error.message)?;
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
            Err(format!(
                "quedan {remaining} proceso(s) usando el prefix reconstruido"
            ))
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
                    let _ = child.kill().await;
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

fn shutdown_is_complete(status_success: bool, active_processes: usize) -> bool {
    status_success || active_processes == 0
}
