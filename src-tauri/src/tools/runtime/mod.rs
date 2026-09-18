//! Dominio tipado del runtime en modo shadow.
//!
//! La autoridad operacional sigue en los adapters legacy. Este módulo sólo resuelve, compara y
//! registra divergencias redacted; no modifica invocaciones, prefixes, manifests ni persistencia.

mod environment;
mod model;
mod probe;
mod resolver;
mod shadow;

pub(crate) use shadow::{
    observe_legacy_runtime, runtime_shadow_enabled, DgVoodooObservation, LegacyRuntimeInput,
    ShadowOperation,
};
