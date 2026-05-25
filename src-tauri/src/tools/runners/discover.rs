use std::collections::HashSet;
use std::path::Path;

use crate::models::runner::RunnerInfo;
use crate::utils::{
    canonical_runner_path, proton_runner_id, proton_search_dirs, PROTON_CACHYOS_SLR,
    SYSTEM_WINE_CANDIDATES,
};

pub fn discover_runners() -> Result<Vec<RunnerInfo>, String> {
    let mut runners = vec![];
    let mut seen = HashSet::new();

    let home = std::env::var("HOME").unwrap_or_default();

    for search_dir in proton_search_dirs(&home) {
        let dir = Path::new(&search_dir);
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            let mut proton_entries: Vec<_> = entries.flatten().collect();
            proton_entries.sort_by_key(|e| e.file_name());
            for entry in proton_entries {
                let proton_bin = entry.path().join("proton");
                if !proton_bin.exists() {
                    continue;
                }
                let Some(canonical) = canonical_runner_path(&proton_bin) else {
                    continue;
                };
                if !seen.insert(canonical.to_string_lossy().to_string()) {
                    continue;
                }

                let tool_name = entry.file_name().to_string_lossy().to_string();
                let label = if tool_name == PROTON_CACHYOS_SLR {
                    format!("{tool_name} (recomendado Gepard)")
                } else {
                    tool_name
                };

                runners.push(RunnerInfo {
                    id: proton_runner_id(&entry.file_name().to_string_lossy()),
                    name: label,
                    path: proton_bin.to_string_lossy().to_string(),
                });
            }
        }
    }

    for (path, label) in SYSTEM_WINE_CANDIDATES {
        if !Path::new(path).exists() {
            continue;
        }
        let Some(canonical) = canonical_runner_path(Path::new(path)) else {
            continue;
        };
        if !seen.insert(canonical.to_string_lossy().to_string()) {
            continue;
        }

        runners.push(RunnerInfo {
            id: Path::new(path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            name: format!("{} ({})", label, path),
            path: path.to_string(),
        });
    }

    runners.sort_by_key(runner_sort_key);

    Ok(runners)
}

fn runner_sort_key(runner: &RunnerInfo) -> (u8, String) {
    let priority = if runner.id.contains(PROTON_CACHYOS_SLR) {
        0
    } else if runner.id.starts_with("proton-") {
        1
    } else {
        2
    };
    (priority, runner.name.clone())
}
