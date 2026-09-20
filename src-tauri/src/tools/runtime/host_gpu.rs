use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HostGpuObservationIpc {
    pub completeness: String,
    pub cards: Vec<HostGpuCardIpc>,
    pub vulkan_api: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HostGpuCardIpc {
    pub sysfs_card: String,
    pub vendor_id: Option<String>,
    pub device_id: Option<String>,
    pub driver: Option<String>,
}

pub(crate) fn probe_host_gpu() -> HostGpuObservationIpc {
    probe_host_gpu_from(Path::new("/sys/class/drm"))
}

pub(crate) fn probe_host_gpu_from(drm_root: &Path) -> HostGpuObservationIpc {
    if !drm_root.is_dir() {
        return HostGpuObservationIpc {
            completeness: "unavailable".to_string(),
            cards: Vec::new(),
            vulkan_api: None,
        };
    }
    let mut cards = Vec::new();
    if let Ok(entries) = fs::read_dir(drm_root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !is_card_dir(&name) {
                continue;
            }
            if let Some(card) = read_card(entry.path(), &name) {
                cards.push(card);
            }
        }
    }
    if cards.is_empty() {
        return HostGpuObservationIpc {
            completeness: "unavailable".to_string(),
            cards,
            vulkan_api: None,
        };
    }
    let all_known = cards
        .iter()
        .all(|card| card.vendor_id.is_some() && card.device_id.is_some() && card.driver.is_some());
    let completeness = if all_known {
        "known".to_string()
    } else {
        "partial".to_string()
    };
    HostGpuObservationIpc {
        completeness,
        cards,
        vulkan_api: None,
    }
}

fn is_card_dir(name: &str) -> bool {
    name.strip_prefix("card")
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit()))
}

fn read_card(card_path: PathBuf, sysfs_card: &str) -> Option<HostGpuCardIpc> {
    let device = card_path.join("device");
    let vendor_id = read_hex_id(device.join("vendor"));
    let device_id = read_hex_id(device.join("device"));
    let driver = read_driver_name(device.join("driver"));
    Some(HostGpuCardIpc {
        sysfs_card: sysfs_card.to_string(),
        vendor_id,
        device_id,
        driver,
    })
}

fn read_hex_id(path: PathBuf) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?.trim().to_ascii_lowercase();
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with("0x") {
        Some(raw)
    } else if raw.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Some(format!("0x{raw}"))
    } else {
        None
    }
}

fn read_driver_name(path: PathBuf) -> Option<String> {
    fs::read_link(path).ok().and_then(|target| {
        target
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn probe_host_gpu_from_fixture_dir() {
        let root = std::env::temp_dir().join(format!(
            "ro-launcher-gpu-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let card = root.join("card0").join("device");
        fs::create_dir_all(&card).unwrap();
        fs::write(card.join("vendor"), "0x10de").unwrap();
        fs::write(card.join("device"), "0x2504").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(root.join("nvidia"), card.join("driver")).unwrap();
        }

        let observed = probe_host_gpu_from(&root);
        assert_eq!(observed.completeness, "known");
        assert_eq!(observed.cards.len(), 1);
        assert_eq!(observed.cards[0].driver.as_deref(), Some("nvidia"));
    }
}
