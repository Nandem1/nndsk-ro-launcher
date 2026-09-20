use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCheck {
    pub id: String,
    pub severity: RuntimeCheckSeverity,
    pub message: String,
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeCheckSeverity {
    Ok,
    Warning,
    Error,
    Pending,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum CompatibilityAssessmentIpc {
    Validated { evidence_id: String },
    Experimental { evidence_id: String },
    Incompatible { evidence_id: String, reason: String },
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityRecommendationIpc {
    pub profile: String,
    pub evidence_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityStatus {
    pub assessment: CompatibilityAssessmentIpc,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommendation: Option<CompatibilityRecommendationIpc>,
    pub gepard_file_version: Option<String>,
    pub gepard_sha256_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePlanSummary {
    pub plan_id: String,
    pub selection_source: &'static str,
    pub graphics_profile: &'static str,
    pub dxvk_provider: &'static str,
    pub dxvk_component_id: String,
    pub overlay_verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<CompatibilityStatus>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyStatus {
    pub wine: bool,
    pub winetricks: bool,
    pub dxvk: bool,
    pub prefix_configured: bool,
    pub audio_ok: bool,
    pub audio_driver: String,
    pub audio_stack: String,
    pub audio_warning: Option<String>,
    pub input_group_ok: bool,
    pub input_group_warning: Option<String>,
    pub uinput_input_ok: bool,
    pub uinput_input_warning: Option<String>,
    pub prefix_ok: bool,
    pub prefix_warning: Option<String>,
    pub dxvk_ok: bool,
    pub dxvk_warning: Option<String>,
    pub runner_kind: String,
    pub runner_ok: bool,
    pub runner_warning: Option<String>,
    pub prefix_path: String,
    pub prefix_scope: String,
    pub prefix_managed: bool,
    pub ready_to_launch: bool,
    pub can_setup: bool,
    pub can_reset: bool,
    pub checks: Vec<RuntimeCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_plan: Option<RuntimePlanSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<CompatibilityStatus>,
}
