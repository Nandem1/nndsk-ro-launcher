//! Cliente Tokio del sidecar `ro-sessiond`.
//!
//! El supervisor está activo por defecto; `RO_LAUNCHER_SESSION_SUPERVISOR=0` es rollback de una release.

mod bootstrap;
mod client;
mod diagnostics;
mod exec;
mod protocol;
mod registry;

pub fn session_supervisor_enabled() -> bool {
    session_supervisor_enabled_from(
        std::env::var("RO_LAUNCHER_SESSION_SUPERVISOR")
            .ok()
            .as_deref(),
    )
}

pub(crate) fn session_supervisor_enabled_from(value: Option<&str>) -> bool {
    value != Some("0")
}

#[allow(unused_imports)]
pub use crate::tools::memory_sessions::MemoryLease;
#[allow(unused_imports)]
pub use crate::utils::RunnerInvocation;
#[allow(unused_imports)]
pub use client::find_ro_sessiond;
pub(crate) use diagnostics::{path_log_token, prefix_log_token};
#[allow(unused_imports)]
pub use exec::{RunnerOperation, SpawnedRunner};
pub use registry::{
    ClientRuntimeGuard, OperationLease, ProcessExit, RunnerSessionRegistry, SessionError,
    SessionOwnership, SupervisedProcess,
};

#[cfg(test)]
mod flag_tests {
    use super::session_supervisor_enabled_from;

    #[test]
    fn supervisor_flag_defaults_on_zero_disables() {
        assert!(session_supervisor_enabled_from(None));
        assert!(session_supervisor_enabled_from(Some("1")));
        assert!(!session_supervisor_enabled_from(Some("0")));
        assert!(session_supervisor_enabled_from(Some("")));
        assert!(session_supervisor_enabled_from(Some("false")));
    }
}
