use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCheck {
    pub id: String,
    pub severity: RuntimeCheckSeverity,
    pub message: String,
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeCheckSeverity {
    Ok,
    Warning,
    Error,
    Pending,
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
}
