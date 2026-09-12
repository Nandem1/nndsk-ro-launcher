use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::models::server::ServerConfig;
use crate::utils::find_file_case_insensitive;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GepardRunnerProfile {
    ModernProton,
    Wine716Legacy,
}

impl GepardRunnerProfile {
    pub fn stack_label(self) -> &'static str {
        match self {
            Self::ModernProton => "Proton-CachyOS 11 + DXVK 3.0.1",
            Self::Wine716Legacy => "Wine 7.16 old-WoW64 + DXVK-Sarek 1.10.x",
        }
    }

    pub fn remediation(self) -> &'static str {
        match self {
            Self::ModernProton => {
                "Selecciona el Proton-CachyOS 11 administrado o un Proton moderno"
            }
            Self::Wine716Legacy => {
                "Selecciona Wine 7.16 portable old-WoW64; DXVK-Sarek conservará Vulkan"
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ValidatedGepardBuild {
    pub product_version: &'static str,
    pub file_version: &'static str,
    pub sha256: &'static str,
    pub runner: GepardRunnerProfile,
}

pub const VALIDATED_GEPARD_BUILDS: [ValidatedGepardBuild; 2] = [
    ValidatedGepardBuild {
        product_version: "3.0",
        file_version: "26.8.26.1",
        sha256: "e2f624d2e3451e68e46783e86d75a8b6787567a96bd33357d74b184dfcec6c13",
        runner: GepardRunnerProfile::ModernProton,
    },
    ValidatedGepardBuild {
        product_version: "3.0",
        file_version: "26.9.3.1",
        sha256: "db4653ddf6aea88a502f10e200a300a05e8e4d65e7cfe65eb5b2ff0779e2e4f5",
        runner: GepardRunnerProfile::Wine716Legacy,
    },
];

#[derive(Debug, Clone)]
pub struct GepardInspection {
    pub sha256: Option<String>,
    pub build: Option<&'static ValidatedGepardBuild>,
}

pub fn inspect_gepard(game_dir: &Path) -> Option<GepardInspection> {
    let path = find_file_case_insensitive(game_dir, "gepard.dll")?;
    let sha256 = sha256_file(&path).ok();
    let build = sha256.as_deref().and_then(validated_build);
    Some(GepardInspection { sha256, build })
}

fn validated_build(sha256: &str) -> Option<&'static ValidatedGepardBuild> {
    VALIDATED_GEPARD_BUILDS
        .iter()
        .find(|build| sha256.eq_ignore_ascii_case(build.sha256))
}

pub fn recommended_gepard_build(server: &ServerConfig) -> Option<&'static ValidatedGepardBuild> {
    game_dir(server).and_then(inspect_gepard)?.build
}

fn game_dir(server: &ServerConfig) -> Option<&Path> {
    Path::new(&server.executable_path).parent()
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_database_maps_only_exact_hashes() {
        let modern = &VALIDATED_GEPARD_BUILDS[0];
        let legacy = &VALIDATED_GEPARD_BUILDS[1];
        assert_eq!(
            validated_build(modern.sha256).map(|build| build.runner),
            Some(GepardRunnerProfile::ModernProton)
        );
        assert_eq!(
            validated_build(&legacy.sha256.to_ascii_uppercase()).map(|build| build.runner),
            Some(GepardRunnerProfile::Wine716Legacy)
        );
        assert!(validated_build(
            "db4653ddf6aea88a502f10e200a300a05e8e4d65e7cfe65eb5b2ff0779e2e4f0"
        )
        .is_none());
    }
}
