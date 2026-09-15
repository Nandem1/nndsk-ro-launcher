//! Cliente Tokio del sidecar `ro-sessiond` (fase 2).

mod bootstrap;
mod client;
mod diagnostics;
mod exec;
mod protocol;
mod registry;

pub fn session_supervisor_enabled() -> bool {
    std::env::var("RO_LAUNCHER_SESSION_SUPERVISOR").as_deref() == Ok("1")
}

#[allow(unused_imports)]
pub use crate::utils::RunnerInvocation;
#[allow(unused_imports)]
pub use client::find_ro_sessiond;
#[allow(unused_imports)]
pub use exec::{RunnerOperation, SpawnedRunner};
#[allow(unused_imports)]
pub use registry::{
    ClientLease, ClientRuntimeGuard, OperationLease, ProcessExit, RunnerSessionRegistry,
    SessionError, SessionOwnership, SupervisedProcess,
};

#[cfg(test)]
mod flag_tests {
    use super::session_supervisor_enabled;

    #[test]
    fn supervisor_flag_defaults_off() {
        std::env::remove_var("RO_LAUNCHER_SESSION_SUPERVISOR");
        assert!(!session_supervisor_enabled());
        std::env::set_var("RO_LAUNCHER_SESSION_SUPERVISOR", "1");
        assert!(session_supervisor_enabled());
        std::env::set_var("RO_LAUNCHER_SESSION_SUPERVISOR", "0");
        assert!(!session_supervisor_enabled());
        std::env::remove_var("RO_LAUNCHER_SESSION_SUPERVISOR");
    }
}
