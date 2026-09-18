//! Dominio tipado del runtime. La identidad de prefix es operacional; gráficos/env en shadow.
#![allow(dead_code)]

mod encode;
mod environment;
mod fingerprint;
mod identity;
mod managed_identity;
mod material;
mod model;
mod probe;
mod resolver;
mod session_anchor;
mod shadow;

pub(crate) use identity::{
    prefix_v3_write_enabled, resolve_prefix_binding, resolve_prefix_binding_for_managed_descriptor,
    PrefixBinding, PrefixIdentityStatus,
};
pub(crate) use probe::{paths_match, probe_runner, RunnerProbe};
pub(crate) use session_anchor::{session_anchor_from_context, SessionAnchorV2};
pub(crate) use shadow::{
    observe_legacy_runtime, runtime_shadow_enabled, DgVoodooObservation, LegacyRuntimeInput,
    ShadowOperation,
};
