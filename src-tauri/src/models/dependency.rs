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
}
