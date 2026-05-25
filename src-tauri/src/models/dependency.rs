use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyStatus {
    pub wine: bool,
    pub winetricks: bool,
    pub dxvk: bool,
    pub prefix_configured: bool,
    pub audio_ok: bool,
    pub audio_driver: String,
    pub audio_warning: Option<String>,
    pub autopot_input_ok: bool,
    pub autopot_input_warning: Option<String>,
}
