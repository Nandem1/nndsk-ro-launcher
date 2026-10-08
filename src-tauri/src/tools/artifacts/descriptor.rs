use crate::tools::runtime::ArtifactArchitecture;

pub const MANAGED_RUNNER_ID: &str = "nndsk-ro-proton-0.1.0-dev.2";
pub const MANAGED_RUNNER_LABEL: &str = "nndsk-ro-proton 0.1.0-dev.2";
pub(crate) const LOCAL_MANAGED_RUNNER_ID: &str = "nndsk-ro-proton-0.1.0-dev.1";
pub(crate) const LEGACY_MANAGED_RUNNER_ID: &str = "ro-proton-cachyos-11.0-20260702-slr";
pub(crate) const LEGACY_MANAGED_RUNNER_LABEL: &str = "proton-cachyos-11.0-20260702-slr-x86_64";
pub(crate) const NNDSK_RUNTIME_SOURCE_COMMIT: &str = "93bda981a6e8b7bdadb46fa3497f84df8dfc6fe2";
pub(crate) const NNDSK_RUNTIME_SHA256: &str =
    "208e6d4735c9a25c4f14b6465712a1069ce53aaf4d5a12eb5a3967abe962ead4";
const LOCAL_NNDSK_RUNTIME_SHA256: &str =
    "75b0c916ccf6e7fcd64ed2afe576c2c0bc6a75ef63a8d74ad6dd9bee629d4d9f";
const NNDSK_RUNTIME_URL: &str = "https://github.com/Nandem1/nndsk-ro-proton/releases/download/v0.1.0-dev.2/nndsk-ro-proton-0.1.0-dev.2-linux-x86_64.tar.zst";
pub(crate) const MANAGED_DXVK_ID: &str = "dxvk-2.6.2";
pub(crate) const UMU_ID: &str = "umu-launcher-1.4.0";

pub(crate) const PROTON_SHA512: &str =
    "c8a050077b1d420e5b691dc487eaa998fe03b99b7e05e6ee3e16c8d4bd9f4c9ff5d9f80e5f6cd1a3f6bb5194bf1481fca9f91999f710d505b68ad97aa5592c7b";
pub(crate) const UMU_SHA256: &str =
    "138ce4b8843608a257d4bee88191ca78a989778bcefd8abb3c1d1aaac3ac6fb8";
pub(crate) const DXVK_SHA256: &str =
    "17761876556afd55736cb895d184f5a1c55d43350f1b1e3b129f8d28706d7992";

const PROTON_ARCHIVE_NAME: &str = "proton-cachyos-11.0-20260702-slr-x86_64.tar.xz";
const PROTON_ARCHIVE_ROOT: &str = "proton-cachyos-11.0-20260702-slr-x86_64";
const PROTON_URL: &str =
    "https://github.com/CachyOS/proton-cachyos/releases/download/cachyos-11.0-20260702-slr/proton-cachyos-11.0-20260702-slr-x86_64.tar.xz";
const PROTON_SIZE: u64 = 328_233_608;
const PROTON_VERSION: &str = "11.0-20260702-slr";

const UMU_ARCHIVE_NAME: &str = "umu-launcher-1.4.0-zipapp.tar";
const UMU_ARCHIVE_ROOT: &str = "umu";
const UMU_URL: &str =
    "https://github.com/Open-Wine-Components/umu-launcher/releases/download/1.4.0/umu-launcher-1.4.0-zipapp.tar";
const UMU_SIZE: u64 = 430_080;
const UMU_VERSION: &str = "1.4.0";

const DXVK_ARCHIVE_NAME: &str = "dxvk-2.6.2.tar.gz";
const DXVK_ARCHIVE_ROOT: &str = "dxvk-2.6.2";
const DXVK_URL: &str =
    "https://github.com/doitsujin/dxvk/releases/download/v2.6.2/dxvk-2.6.2.tar.gz";
const DXVK_SIZE: u64 = 10_107_492;
const DXVK_VERSION: &str = "2.6.2";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ArtifactDescriptor {
    pub id: &'static str,
    pub kind: ArtifactKind,
    pub version: &'static str,
    pub source: ArtifactSource,
    pub expected_size: u64,
    pub digest: ExpectedDigest,
    pub archive: ArchiveLayout,
    pub platform: &'static str,
    pub architectures: &'static [ArtifactArchitecture],
    pub recipe_revision: u64,
    pub payload: PayloadValidatorId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ArtifactKind {
    ProtonCachyos,
    NndskRoProton,
    UmuLauncher,
    Dxvk,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArtifactSource {
    Https { url: &'static str },
    // This preservation build is verified locally; no downloadable release exists yet.
    LocalOnly { source_commit: &'static str },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExpectedDigest {
    Sha256(&'static str),
    Sha512(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArchiveLayout {
    Tar {
        archive_name: &'static str,
        root: &'static str,
    },
    TarGz {
        archive_name: &'static str,
        root: &'static str,
    },
    TarXz {
        archive_name: &'static str,
        root: &'static str,
    },
    TarZst {
        archive_name: &'static str,
        root: &'static str,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PayloadValidatorId {
    ProtonCachyosSlr,
    NndskRoProton,
    UmuZipapp,
    DxvkPrefixDlls,
}

impl ArchiveLayout {
    pub(crate) fn archive_name(self) -> &'static str {
        match self {
            ArchiveLayout::Tar { archive_name, .. }
            | ArchiveLayout::TarGz { archive_name, .. }
            | ArchiveLayout::TarXz { archive_name, .. }
            | ArchiveLayout::TarZst { archive_name, .. } => archive_name,
        }
    }

    pub(crate) fn archive_root(self) -> &'static str {
        match self {
            ArchiveLayout::Tar { root, .. }
            | ArchiveLayout::TarGz { root, .. }
            | ArchiveLayout::TarXz { root, .. }
            | ArchiveLayout::TarZst { root, .. } => root,
        }
    }
}

const UMU_DESCRIPTOR: ArtifactDescriptor = ArtifactDescriptor {
    id: UMU_ID,
    kind: ArtifactKind::UmuLauncher,
    version: UMU_VERSION,
    source: ArtifactSource::Https { url: UMU_URL },
    expected_size: UMU_SIZE,
    digest: ExpectedDigest::Sha256(UMU_SHA256),
    archive: ArchiveLayout::Tar {
        archive_name: UMU_ARCHIVE_NAME,
        root: UMU_ARCHIVE_ROOT,
    },
    platform: "linux-x86_64",
    architectures: &[ArtifactArchitecture::X86_64],
    recipe_revision: 1,
    payload: PayloadValidatorId::UmuZipapp,
};

const PROTON_DESCRIPTOR: ArtifactDescriptor = ArtifactDescriptor {
    id: LEGACY_MANAGED_RUNNER_ID,
    kind: ArtifactKind::ProtonCachyos,
    version: PROTON_VERSION,
    source: ArtifactSource::Https { url: PROTON_URL },
    expected_size: PROTON_SIZE,
    digest: ExpectedDigest::Sha512(PROTON_SHA512),
    archive: ArchiveLayout::TarXz {
        archive_name: PROTON_ARCHIVE_NAME,
        root: PROTON_ARCHIVE_ROOT,
    },
    platform: "linux-x86_64",
    architectures: &[ArtifactArchitecture::X86_64],
    recipe_revision: 1,
    payload: PayloadValidatorId::ProtonCachyosSlr,
};

const NNDSK_RUNTIME_DESCRIPTOR: ArtifactDescriptor = ArtifactDescriptor {
    id: MANAGED_RUNNER_ID,
    kind: ArtifactKind::NndskRoProton,
    version: "0.1.0-dev.2",
    source: ArtifactSource::Https {
        url: NNDSK_RUNTIME_URL,
    },
    expected_size: 399_605_131,
    digest: ExpectedDigest::Sha256(NNDSK_RUNTIME_SHA256),
    archive: ArchiveLayout::TarZst {
        archive_name: "nndsk-ro-proton-0.1.0-dev.2-linux-x86_64.tar.zst",
        root: "nndsk-ro-proton",
    },
    platform: "linux-x86_64",
    architectures: &[ArtifactArchitecture::X86, ArtifactArchitecture::X86_64],
    recipe_revision: 1,
    payload: PayloadValidatorId::NndskRoProton,
};

// Preserve existing receipts, selections and prefix fingerprints. Never relabel dev.1.
const NNDSK_LOCAL_DESCRIPTOR: ArtifactDescriptor = ArtifactDescriptor {
    id: LOCAL_MANAGED_RUNNER_ID,
    kind: ArtifactKind::NndskRoProton,
    version: "0.1.0-dev.1",
    source: ArtifactSource::LocalOnly {
        source_commit: NNDSK_RUNTIME_SOURCE_COMMIT,
    },
    expected_size: 398_862_860,
    digest: ExpectedDigest::Sha256(LOCAL_NNDSK_RUNTIME_SHA256),
    archive: ArchiveLayout::TarZst {
        archive_name: "nndsk-ro-proton-0.1.0-dev.1-linux-x86_64.tar.zst",
        root: "nndsk-ro-proton",
    },
    platform: "linux-x86_64",
    architectures: &[ArtifactArchitecture::X86, ArtifactArchitecture::X86_64],
    recipe_revision: 1,
    payload: PayloadValidatorId::NndskRoProton,
};

const DXVK_DESCRIPTOR: ArtifactDescriptor = ArtifactDescriptor {
    id: MANAGED_DXVK_ID,
    kind: ArtifactKind::Dxvk,
    version: DXVK_VERSION,
    source: ArtifactSource::Https { url: DXVK_URL },
    expected_size: DXVK_SIZE,
    digest: ExpectedDigest::Sha256(DXVK_SHA256),
    archive: ArchiveLayout::TarGz {
        archive_name: DXVK_ARCHIVE_NAME,
        root: DXVK_ARCHIVE_ROOT,
    },
    platform: "linux-x86_64",
    architectures: &[ArtifactArchitecture::X86, ArtifactArchitecture::X86_64],
    recipe_revision: 1,
    payload: PayloadValidatorId::DxvkPrefixDlls,
};

static CATALOG: [ArtifactDescriptor; 5] = [
    UMU_DESCRIPTOR,
    PROTON_DESCRIPTOR,
    DXVK_DESCRIPTOR,
    NNDSK_RUNTIME_DESCRIPTOR,
    NNDSK_LOCAL_DESCRIPTOR,
];

#[allow(dead_code)]
pub(crate) fn catalog_all() -> &'static [ArtifactDescriptor] {
    &CATALOG
}

pub(crate) fn catalog_descriptor(id: &str) -> Option<&'static ArtifactDescriptor> {
    CATALOG.iter().find(|descriptor| descriptor.id == id)
}

pub(crate) fn expected_digest_hex(digest: ExpectedDigest) -> &'static str {
    match digest {
        ExpectedDigest::Sha256(value) | ExpectedDigest::Sha512(value) => value,
    }
}

pub(crate) fn decode_hex_digest(hex: &str) -> Vec<u8> {
    hex.chars()
        .collect::<Vec<_>>()
        .chunks(2)
        .map(|pair| u8::from_str_radix(&pair.iter().collect::<String>(), 16).unwrap_or(0))
        .collect()
}

pub(crate) fn architectures_json(architectures: &[ArtifactArchitecture]) -> Vec<String> {
    architectures
        .iter()
        .map(|arch| match arch {
            ArtifactArchitecture::X86 => "x86".to_string(),
            ArtifactArchitecture::X86_64 => "x86-64".to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_urls_sizes_digests_and_paths_match_phase2_constants() {
        let proton = catalog_descriptor(LEGACY_MANAGED_RUNNER_ID).unwrap();
        assert_eq!(proton.expected_size, 328_233_608);
        assert_eq!(proton.archive.archive_name(), PROTON_ARCHIVE_NAME);
        assert_eq!(proton.archive.archive_root(), PROTON_ARCHIVE_ROOT);
        assert!(matches!(
            proton.source,
            ArtifactSource::Https { url: PROTON_URL }
        ));
        assert_eq!(expected_digest_hex(proton.digest), PROTON_SHA512);

        let umu = catalog_descriptor(UMU_ID).unwrap();
        assert_eq!(umu.expected_size, 430_080);
        assert_eq!(expected_digest_hex(umu.digest), UMU_SHA256);

        let dxvk = catalog_descriptor(MANAGED_DXVK_ID).unwrap();
        assert_eq!(dxvk.expected_size, 10_107_492);
        assert_eq!(expected_digest_hex(dxvk.digest), DXVK_SHA256);
    }

    #[test]
    fn source_digest_bytes_match_hex_constants() {
        let proton = catalog_descriptor(LEGACY_MANAGED_RUNNER_ID).unwrap();
        assert_eq!(
            decode_hex_digest(expected_digest_hex(proton.digest)),
            decode_hex_digest(PROTON_SHA512)
        );
    }

    #[test]
    fn preservation_build_is_pinned_without_claiming_a_public_download() {
        let runtime = catalog_descriptor(LOCAL_MANAGED_RUNNER_ID).unwrap();
        assert_eq!(runtime.kind, ArtifactKind::NndskRoProton);
        assert_eq!(runtime.expected_size, 398_862_860);
        assert_eq!(
            runtime.digest,
            ExpectedDigest::Sha256(LOCAL_NNDSK_RUNTIME_SHA256)
        );
        assert_eq!(runtime.archive.archive_root(), "nndsk-ro-proton");
        assert_eq!(
            runtime.source,
            ArtifactSource::LocalOnly {
                source_commit: NNDSK_RUNTIME_SOURCE_COMMIT
            }
        );
        assert_ne!(runtime.id, LEGACY_MANAGED_RUNNER_ID);
    }

    #[test]
    fn published_runtime_is_pinned_and_distinct_from_preservation_build() {
        let public = catalog_descriptor(MANAGED_RUNNER_ID).unwrap();
        let local = catalog_descriptor(LOCAL_MANAGED_RUNNER_ID).unwrap();
        assert_eq!(
            public.source,
            ArtifactSource::Https {
                url: NNDSK_RUNTIME_URL
            }
        );
        assert_eq!(public.expected_size, 399_605_131);
        assert_eq!(public.digest, ExpectedDigest::Sha256(NNDSK_RUNTIME_SHA256));
        assert_ne!(public.id, local.id);
        assert_ne!(public.digest, local.digest);
        assert_eq!(public.recipe_revision, local.recipe_revision);
    }
}
