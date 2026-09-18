//! Dominio tipado del runtime. Identidad de prefix y gráficos operacionales bajo flag.
#![allow(dead_code)]

mod apply;
mod compatibility;
mod encode;
mod environment;
mod fingerprint;
mod identity;
mod inspection;
mod managed_identity;
mod material;
mod model;
mod probe;
mod resolver;
mod session_anchor;
mod shadow;

#[cfg(test)]
mod d7vk_spike;

pub(crate) use apply::{
    apply_graphics_environment_to_invocation, build_runtime_plan_summary, resolve_operational_plan,
    resolve_operational_plan_with_profile, runtime_graphics_plan_enabled, InvocationPlan,
    OperationalRuntimeInput,
};
pub(crate) use compatibility::{
    assess_compatibility, compatibility_ipc, gepard_runtime_check, gepard_subject_warnings,
    legacy_gepard_runner_check, recommendation_to_gepard_profile, runtime_compat_enabled,
    AssessedRuntime,
};
pub(crate) use environment::GraphicsEnvironmentError;
pub(crate) use identity::{
    prefix_v3_write_enabled, resolve_prefix_binding, resolve_prefix_binding_for_managed_descriptor,
    PrefixBinding, PrefixIdentityStatus,
};
pub(crate) use inspection::inspect_subject;
pub(crate) use model::InvocationTarget;
pub(crate) use model::{ArtifactArchitecture, RuntimePlan, RuntimeProfile};
pub(crate) use probe::{paths_match, probe_runner, RunnerProbe};
pub(crate) use resolver::DgVoodooState;
pub(crate) use session_anchor::{session_anchor_from_context, SessionAnchorV2};
pub(crate) use shadow::{
    observe_legacy_runtime, runtime_shadow_enabled, DgVoodooObservation, LegacyRuntimeInput,
    ShadowOperation,
};
