//! Cliente Tokio del sidecar `ro-sessiond` (fase 2).

#![allow(dead_code)] // API completa para fase 3; `shutdown_all` ya se usa al salir.

mod client;
mod diagnostics;
mod protocol;
mod registry;

#[allow(unused_imports)]
pub use client::find_ro_sessiond;
pub use registry::{RunnerInvocation, RunnerSessionRegistry, SessionError};
