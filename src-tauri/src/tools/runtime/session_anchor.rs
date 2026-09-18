use super::fingerprint::{runtime_fingerprint_placeholder, RuntimeFingerprint};
use super::identity::PrefixBinding;
use super::model::RuntimePlan;

#[derive(Debug, Clone)]
pub(crate) struct ExecutionFacts {
    pub(crate) webview2_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionAnchorV2 {
    pub(crate) plan_id: String,
    pub(crate) runtime_fingerprint: RuntimeFingerprint,
}

pub(crate) fn compute_runtime_fingerprint(
    plan: &RuntimePlan,
    _binding: &PrefixBinding,
    _facts: &ExecutionFacts,
) -> RuntimeFingerprint {
    let _ = plan;
    runtime_fingerprint_placeholder()
}

pub(crate) fn runtime_anchor_for_operation(
    plan: &RuntimePlan,
    binding: &PrefixBinding,
    facts: ExecutionFacts,
) -> SessionAnchorV2 {
    let runtime_fingerprint = compute_runtime_fingerprint(plan, binding, &facts);
    SessionAnchorV2 {
        plan_id: runtime_fingerprint.digest.hex_digest(),
        runtime_fingerprint,
    }
}

pub(crate) fn session_anchor_from_context(ctx: &crate::utils::WineContext) -> SessionAnchorV2 {
    let _ = ctx;
    let runtime_fingerprint = runtime_fingerprint_placeholder();
    SessionAnchorV2 {
        plan_id: runtime_fingerprint.digest.hex_digest(),
        runtime_fingerprint,
    }
}
