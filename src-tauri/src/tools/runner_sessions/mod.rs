//! Cliente Tokio del sidecar `ro-sessiond`.
//!
//! El supervisor es obligatorio para todas las operaciones de runners.

mod bootstrap;
mod client;
mod diagnostics;
mod exec;
mod protocol;
mod registry;
pub(crate) use exec::{prefix_has_active_programs, wait_for_prefix_programs};

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
    SupervisedProcess,
};
