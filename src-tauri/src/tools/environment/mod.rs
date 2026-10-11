//! Read-only assessment and preparation preconditions. Runtime identity and mutation
//! remain owned by runtime, artifacts, prefix and runner_sessions.
mod assessment;
mod guidance;
pub(crate) mod host;

pub(crate) use assessment::{assess, preparation_only};

pub(crate) async fn preflight_preparation(
    server_runner: Option<&str>,
    default_runner: Option<&str>,
) -> Result<(), String> {
    let mut checks = host::host_checks(
        host::selection_uses_proton(server_runner, default_runner),
        true,
    )
    .await;
    checks.extend(host::selected_wine716_loader_check(
        host::selection_uses_managed_wine716(server_runner, default_runner),
    ));
    match host::host_blocker(&checks) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
