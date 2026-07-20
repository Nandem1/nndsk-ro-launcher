use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolKind {
    OpenSetup,
    Patcher,
    DgVoodoo,
}

impl ToolKind {
    /// OpenSetup debe enumerar el mismo adaptador DirectDraw que usará el juego.
    /// El patcher abierto desde Herramientas es sólo de mantenimiento; el inicio
    /// mediante patcher desde Jugar aplica el entorno del juego por otra ruta.
    pub fn should_apply_dgvoodoo_overrides(self, dgvoodoo_configured: bool) -> bool {
        dgvoodoo_configured && matches!(self, Self::OpenSetup | Self::DgVoodoo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verified_dgvoodoo_is_exposed_to_opensetup_and_control_panel() {
        assert!(ToolKind::OpenSetup.should_apply_dgvoodoo_overrides(true));
        assert!(ToolKind::DgVoodoo.should_apply_dgvoodoo_overrides(true));
        assert!(!ToolKind::Patcher.should_apply_dgvoodoo_overrides(true));
    }

    #[test]
    fn unverified_wrappers_are_never_forced() {
        for tool in [ToolKind::OpenSetup, ToolKind::Patcher, ToolKind::DgVoodoo] {
            assert!(!tool.should_apply_dgvoodoo_overrides(false));
        }
    }
}
