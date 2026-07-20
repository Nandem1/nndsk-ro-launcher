use crate::models::runner::RunnerInfo;
use crate::tools::runners::{managed_proton_path, MANAGED_RUNNER_ID, MANAGED_RUNNER_LABEL};

/// El launcher expone un único runtime probado para Ragnarok.
///
/// La ruta se devuelve aunque todavía no se haya descargado: la instalación se
/// realiza bajo demanda al preparar el entorno del primer servidor.
pub fn discover_runners() -> Result<Vec<RunnerInfo>, String> {
    Ok(vec![RunnerInfo {
        id: MANAGED_RUNNER_ID.to_string(),
        name: MANAGED_RUNNER_LABEL.to_string(),
        path: managed_proton_path().to_string_lossy().to_string(),
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_only_the_managed_ragnarok_runtime() {
        let runners = discover_runners().unwrap();
        assert_eq!(runners.len(), 1);
        assert_eq!(runners[0].id, MANAGED_RUNNER_ID);
        assert_eq!(runners[0].path, managed_proton_path().to_string_lossy());
    }
}
