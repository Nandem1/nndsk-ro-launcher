use ro_tools_linux::{
    capture_process_identity, find_prefix_processes, is_descendant_of, is_prefix_leftover_process,
    verify_process_identity, ProcessIdentity,
};
use tauri::AppHandle;

use crate::state::GameProcessHandle;
use crate::utils::{RunnerInvocation, WineContext};

use super::diagnostics::{FailureOutput, RunnerOutputHub};
use crate::tools::runtime::SessionAnchorV2;
use std::sync::{Arc, Mutex};

use super::{OperationLease, ProcessExit, RunnerSessionRegistry, SupervisedProcess};

/// Called while an exclusive prefix guard is held: no other launcher program can enter.
/// Wine services may persist, but a tool or its handoff child keeps ownership until it exits.
pub(crate) async fn wait_for_prefix_programs(prefix: &str, supervisor: Option<ProcessIdentity>) {
    while prefix_has_active_programs(prefix, supervisor) {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

pub(crate) fn prefix_has_active_programs(
    prefix: &str,
    supervisor: Option<ProcessIdentity>,
) -> bool {
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
    identities.into_iter().any(|identity| {
        identity.pid != std::process::id()
            && verify_process_identity(&identity)
            && !is_prefix_leftover_process(identity.pid)
            && verify_process_identity(&identity)
    })
}

pub struct RunnerOperation {
    app: Option<AppHandle>,
    sessions: RunnerSessionRegistry,
    ctx: WineContext,
    lease: OperationLease,
    output: Arc<RunnerOutputHub>,
    failure_output: Arc<Mutex<FailureOutput>>,
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
        let lease = match app {
            Some(app) => sessions.begin_operation(app, ctx, game, anchor).await,
            None => sessions.begin_operation_opt(None, ctx, game, anchor).await,
        }
        .map_err(|error| error.message)?;
        let output = lease.output();
        let failure_output = output.subscribe();
        Ok(Self {
            app: app.cloned(),
            sessions: sessions.clone(),
            ctx: ctx.clone(),
            lease,
            output,
            failure_output,
        })
    }

    pub fn ctx(&self) -> &WineContext {
        &self.ctx
    }

    pub fn lease(&self) -> &OperationLease {
        &self.lease
    }

    pub async fn run(
        &self,
        invocation: RunnerInvocation,
        error_context: &str,
    ) -> Result<i32, String> {
        let mut process = self
            .sessions
            .launch(self.app.as_ref(), &self.lease, invocation, &[])
            .await
            .map_err(|error| error.message)?;
        let exit = process.wait().await.map_err(|error| error.message)?;
        Ok(exit_code_from_process_exit(&exit, error_context))
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

    /// End execution ownership before shutdown; cleanup cannot launch more commands.
    pub(crate) fn into_cleanup(self) -> RunnerCleanup {
        let Self {
            sessions,
            ctx,
            lease,
            output,
            failure_output,
            ..
        } = self;
        drop(lease);
        RunnerCleanup {
            sessions,
            ctx,
            output,
            failure_output,
        }
    }

    pub async fn quiesce(self) -> Result<(), String> {
        self.into_cleanup().quiesce().await
    }

    pub async fn spawn(
        &self,
        invocation: RunnerInvocation,
        redactions: &[String],
    ) -> Result<SpawnedRunner, String> {
        let process = self
            .sessions
            .launch(self.app.as_ref(), &self.lease, invocation, redactions)
            .await
            .map_err(|error| error.message)?;
        Ok(SpawnedRunner(process))
    }
}

/// Shutdown and diagnostics retained after the mandatory execution lease is released.
/// This phase supports repeated cleanup without representing a runnable operation.
pub(crate) struct RunnerCleanup {
    sessions: RunnerSessionRegistry,
    ctx: WineContext,
    output: Arc<RunnerOutputHub>,
    failure_output: Arc<Mutex<FailureOutput>>,
}

impl RunnerCleanup {
    pub(crate) fn annotate_failure(&self, primary: String) -> String {
        self.failure_output.lock().unwrap().annotate(primary)
    }

    pub(crate) async fn quiesce(&self) -> Result<(), String> {
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
        // ControllerExited and stderr are separate streams. EOF retains late failure causes.
        // Best effort and bounded: a diagnostic pipe must not prevent filesystem cleanup.
        let _ = tokio::time::timeout(std::time::Duration::from_secs(1), self.output.wait_closed())
            .await;
        let remaining = find_prefix_processes(&self.ctx.prefix).len();
        if remaining == 0 {
            Ok(())
        } else {
            Err(format!("quedan {remaining} proceso(s) usando el entorno"))
        }
    }
}

pub struct SpawnedRunner(SupervisedProcess);

impl SpawnedRunner {
    pub fn controller_identity(&self) -> ProcessIdentity {
        self.0.controller_identity()
    }

    pub fn controller_pid(&self) -> u32 {
        self.0.controller_pid()
    }

    pub async fn wait(&mut self) -> Result<i32, String> {
        let exit = self.0.wait().await.map_err(|error| error.message)?;
        Ok(exit.exit_code.unwrap_or(-1))
    }

    pub async fn terminate(&mut self) -> Result<(), String> {
        self.0.terminate().await.map_err(|e| e.message)
    }

    pub fn try_exit_code(&mut self) -> Option<i32> {
        self.try_exit_status()
            .map(|exit| exit.exit_code.unwrap_or(-1))
    }

    pub fn try_exit_status(&mut self) -> Option<ProcessExit> {
        self.0.try_exit()
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
