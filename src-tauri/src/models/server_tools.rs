use serde::Serialize;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub found: bool,
    pub path: Option<String>,
    pub label: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DgVoodooStatus {
    pub cpl: ToolInfo,
    pub d3dimm_dll: ToolInfo,
    pub ddraw_dll: ToolInfo,
    pub conf: ToolInfo,
    pub configured: bool,
    pub needs_install: bool,
    pub can_auto_install: bool,
    pub can_uninstall: bool,
    pub issues: Vec<String>,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClientDiagnostics {
    pub architecture: Option<String>,
    pub graphics_apis: Vec<String>,
    pub managed_patcher: bool,
    pub webview2_required: bool,
    pub pe_analysis_conclusive: bool,
    pub gepard_present: bool,
    pub gameguard_present: bool,
    pub warnings: Vec<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ServerToolsStatus {
    pub game_dir: String,
    pub open_setup: ToolInfo,
    pub patcher: ToolInfo,
    pub dgvoodoo: DgVoodooStatus,
    pub diagnostics: ClientDiagnostics,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstallDgVoodooResult {
    pub installed: Vec<String>,
    pub status: ServerToolsStatus,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UninstallDgVoodooResult {
    pub removed: Vec<String>,
    pub status: ServerToolsStatus,
}
