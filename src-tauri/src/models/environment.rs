//! Versioned UI contract. Capability observations never change runner/prefix identity.
use serde::Serialize;

use super::dependency::RuntimeCheckSeverity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EnvironmentDomain {
    System,
    Tools,
    Runner,
    Prefix,
    Graphics,
    Feature,
    Compatibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EnvironmentIntent {
    Launch,
    Prepare,
    Rebuild,
    Feature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LaunchReadiness {
    Ready,
    NeedsPreparation,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolProvider {
    ProtonTools,
    WineHostTools,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HostFamily {
    Arch,
    Other,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum EnvironmentAction {
    Recheck,
    Prepare,
    HostGuide {
        family: HostFamily,
        packages: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentIssue {
    pub code: String,
    pub domain: EnvironmentDomain,
    pub severity: RuntimeCheckSeverity,
    pub affects: Vec<EnvironmentIntent>,
    pub title: String,
    pub message: String,
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentAssessment {
    pub schema_version: u8,
    pub provider: ToolProvider,
    pub launch: LaunchReadiness,
    pub preparation_available: bool,
    pub rebuild_available: bool,
    pub issues: Vec<EnvironmentIssue>,
    pub actions: Vec<EnvironmentAction>,
}
