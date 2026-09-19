use super::fingerprint::RuntimeFingerprint;
use super::identity::PrefixBinding;
use super::model::{FingerprintDigest, RuntimePlan, FINGERPRINT_SCHEMA_VERSION};
use super::runtime_fingerprint::{
    compute_runtime_fingerprint_from_input, runtime_fingerprint_input_from_plan,
};

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
    binding: &PrefixBinding,
    _facts: &ExecutionFacts,
) -> RuntimeFingerprint {
    let input = runtime_fingerprint_input_from_plan(plan, binding);
    compute_runtime_fingerprint_from_input(&input)
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

pub(crate) fn session_anchor_unavailable() -> SessionAnchorV2 {
    SessionAnchorV2 {
        plan_id: "unavailable".to_string(),
        runtime_fingerprint: RuntimeFingerprint {
            digest: FingerprintDigest {
                schema_version: FINGERPRINT_SCHEMA_VERSION,
                algorithm: "sha256".to_string(),
                digest: [0u8; 32],
            },
        },
    }
}

#[cfg(test)]
pub(crate) fn session_anchor_from_context(_ctx: &crate::utils::WineContext) -> SessionAnchorV2 {
    use super::fingerprint::runtime_fingerprint_placeholder;
    let runtime_fingerprint = runtime_fingerprint_placeholder();
    SessionAnchorV2 {
        plan_id: runtime_fingerprint.digest.hex_digest(),
        runtime_fingerprint,
    }
}
