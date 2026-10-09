use crate::models::runner::RunnerInfo;
use crate::tools::runners::{
    managed_proton_path, managed_wine716_path, MANAGED_RUNNER_ID, MANAGED_RUNNER_LABEL,
    MANAGED_WINE716_ID, MANAGED_WINE716_LABEL,
};

/// Only the two supported products are offered, even before their on-demand download.
/// Saved paths outside this catalog remain untouched and can still be resolved for existing
/// sessions/prefixes; discovery does not advertise arbitrary system Wine or Steam Proton builds.
pub fn discover_runners() -> Result<Vec<RunnerInfo>, String> {
    Ok(vec![
        RunnerInfo {
            id: MANAGED_RUNNER_ID.to_string(),
            name: MANAGED_RUNNER_LABEL.to_string(),
            path: managed_proton_path().to_string_lossy().to_string(),
        },
        RunnerInfo {
            id: MANAGED_WINE716_ID.to_string(),
            name: MANAGED_WINE716_LABEL.to_string(),
            path: managed_wine716_path().to_string_lossy().to_string(),
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offers_exactly_the_two_supported_runners_regardless_of_installed_externals() {
        let runners = discover_runners().unwrap();
        assert_eq!(runners.len(), 2);
        assert_eq!(runners[0].id, MANAGED_RUNNER_ID);
        assert_eq!(runners[0].path, managed_proton_path().to_string_lossy());
        assert_eq!(runners[1].id, MANAGED_WINE716_ID);
        assert_eq!(runners[1].path, managed_wine716_path().to_string_lossy());
        assert_ne!(runners[0].path, runners[1].path);
    }
}
