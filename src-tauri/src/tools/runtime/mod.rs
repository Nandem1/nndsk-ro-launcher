//! Dominio tipado del runtime. Identidad de prefix y gráficos operacionales bajo flag.
#![allow(dead_code)]

mod apply;
pub(crate) mod benchmark;
pub(crate) mod benchmark_store;
mod compatibility;
mod encode;
mod environment;
mod fingerprint;
mod host_gpu;
mod identity;
mod inspection;
mod managed_identity;
mod material;
mod model;
mod observation;
mod observation_store;
mod probe;
mod resolver;
mod runtime_fingerprint;
mod session_anchor;
mod shadow;

#[cfg(test)]
mod d7vk_spike;

pub(crate) use apply::{
    apply_graphics_environment_to_invocation, build_runtime_plan_summary, resolve_operational_plan,
    resolve_operational_plan_with_profile, runtime_graphics_plan_enabled, InvocationPlan,
    OperationalRuntimeInput,
};
pub(crate) use benchmark::{
    host_clk_tck, host_uptime_seconds, process_age_seconds, runtime_benchmark_enabled,
};
pub(crate) use benchmark_store::{attach_benchmark_run, begin_benchmark_capture};
pub(crate) use compatibility::{
    assess_compatibility, compatibility_ipc, gepard_runtime_check, gepard_subject_warnings,
    legacy_gepard_runner_check, recommendation_to_gepard_profile, runtime_compat_enabled,
    AssessedRuntime,
};
pub(crate) use encode::server_path_token16;
pub(crate) use environment::GraphicsEnvironmentError;
pub(crate) use host_gpu::probe_host_gpu;
pub(crate) use identity::{
    prefix_v3_write_enabled, resolve_prefix_binding, resolve_prefix_binding_for_managed_descriptor,
    PrefixBinding, PrefixIdentityStatus,
};
pub(crate) use inspection::inspect_subject;
pub(crate) use model::InvocationTarget;
pub(crate) use model::{ArtifactArchitecture, RuntimePlan, RuntimeProfile};
pub(crate) use observation::{
    classify_run_outcome, runtime_observe_enabled, OutcomeInput, PlanAvailability, RunOutcome,
    StartupFailureClass,
};
pub(crate) use observation::{FingerprintEnvelopeIpc, ObservationProcessIpc};
pub(crate) use observation_store::lookup_observation_for_identity;
pub(crate) use observation_store::{
    delete_observations, enqueue_persist_finished, enqueue_persist_started,
    enqueue_persist_unreached, export_observations, list_observations, new_observation_id,
    ObservationFinishedPayload, ObservationStartedPayload,
};
pub(crate) use probe::{paths_match, probe_runner, RunnerProbe};
pub(crate) use resolver::DgVoodooState;
pub(crate) use session_anchor::{
    runtime_anchor_for_operation, session_anchor_unavailable, ExecutionFacts, SessionAnchorV2,
};

#[cfg(test)]
pub(crate) use session_anchor::session_anchor_from_context;

pub(crate) fn operational_session_anchor(
    ctx: &crate::utils::WineContext,
    plan: Option<&model::RuntimePlan>,
    webview2_required: bool,
) -> SessionAnchorV2 {
    match plan {
        Some(plan) => {
            runtime_anchor_for_operation(plan, &ctx.identity, ExecutionFacts { webview2_required })
        }
        None => session_anchor_unavailable(),
    }
}
pub(crate) use shadow::{
    observe_legacy_runtime, runtime_shadow_enabled, DgVoodooObservation, LegacyRuntimeInput,
    ShadowOperation,
};
