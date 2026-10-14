use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::descriptor::{
    architectures_json, expected_digest_hex, ArtifactDescriptor, ArtifactKind, ArtifactSource,
    ExpectedDigest,
};
pub(crate) const MARKER_FILE: &str = ".ro-launcher-runtime.json";
pub(crate) const RUNTIME_SCHEMA_V1: u32 = 1;
pub(crate) const RUNTIME_SCHEMA_V2: u32 = 2;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeMarkerV1 {
    pub schema_version: u32,
    pub artifact_id: String,
    pub digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeReceiptV2 {
    pub schema_version: u32,
    pub artifact_id: String,
    pub kind: ArtifactKind,
    pub version: String,
    pub digest: String,
    pub digest_algorithm: DigestAlgorithmJson,
    pub source: ReceiptSourceV2,
    pub platform: String,
    pub architectures: Vec<String>,
    pub recipe_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum ReceiptSourceV2 {
    #[serde(rename = "https")]
    Https { url: String },
    #[serde(rename = "localOnly")]
    LocalOnly {
        #[serde(rename = "sourceCommit")]
        source_commit: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DigestAlgorithmJson {
    Sha256,
    Sha512,
}

pub(crate) fn receipt_v2_from_descriptor(descriptor: &ArtifactDescriptor) -> RuntimeReceiptV2 {
    let (digest_algorithm, digest) = match descriptor.digest {
        ExpectedDigest::Sha256(value) => (DigestAlgorithmJson::Sha256, value),
        ExpectedDigest::Sha512(value) => (DigestAlgorithmJson::Sha512, value),
    };
    RuntimeReceiptV2 {
        schema_version: RUNTIME_SCHEMA_V2,
        artifact_id: descriptor.id.to_string(),
        kind: descriptor.kind,
        version: descriptor.version.to_string(),
        digest: digest.to_string(),
        digest_algorithm,
        source: match descriptor.source {
            ArtifactSource::Https { url } => ReceiptSourceV2::Https {
                url: url.to_string(),
            },
            ArtifactSource::LocalOnly { source_commit } => ReceiptSourceV2::LocalOnly {
                source_commit: source_commit.to_string(),
            },
        },
        platform: descriptor.platform.to_string(),
        architectures: architectures_json(descriptor.architectures),
        recipe_revision: descriptor.recipe_revision,
    }
}

pub(crate) fn stored_receipt_matches_descriptor(
    descriptor: &ArtifactDescriptor,
    receipt: &RuntimeReceiptV2,
) -> bool {
    let expected = receipt_v2_from_descriptor(descriptor);
    receipt.schema_version == RUNTIME_SCHEMA_V2
        && receipt.artifact_id == expected.artifact_id
        && receipt.kind == expected.kind
        && receipt.version == expected.version
        && receipt.digest == expected.digest
        && receipt.digest_algorithm == expected.digest_algorithm
        && receipt.platform == expected.platform
        && receipt.recipe_revision == expected.recipe_revision
        && receipt.source == expected.source
        && architectures_set(&receipt.architectures) == architectures_set(&expected.architectures)
}

fn architectures_set(values: &[String]) -> BTreeSet<String> {
    values.iter().cloned().collect()
}

pub(crate) fn runtime_marker_v1_matches(
    descriptor: &ArtifactDescriptor,
    marker: &RuntimeMarkerV1,
) -> bool {
    marker.schema_version == RUNTIME_SCHEMA_V1
        && matches!(descriptor.source, ArtifactSource::Https { .. })
        && marker.artifact_id == descriptor.id
        && marker.digest == expected_digest_hex(descriptor.digest)
}

pub(crate) fn parse_stored_receipt(
    marker_path: &Path,
) -> Result<Option<ParsedStoredReceipt>, String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(marker_path)
        .map_err(|error| format!("No se pudo leer {}: {error}", marker_path.display()))?;
    if !file
        .metadata()
        .is_ok_and(|m| m.is_file() && m.len() <= 65_536)
    {
        return Err("El receipt debe ser un archivo regular de hasta 64 KiB".into());
    }
    let mut content = Vec::new();
    file.take(65_537)
        .read_to_end(&mut content)
        .map_err(|error| error.to_string())?;
    if content.len() > 65_536 {
        return Err("Receipt demasiado grande".into());
    }
    let value: serde_json::Value = serde_json::from_slice(&content)
        .map_err(|error| format!("JSON inválido en {}: {error}", marker_path.display()))?;
    let schema = value
        .get("schemaVersion")
        .and_then(|version| version.as_u64())
        .ok_or_else(|| format!("schemaVersion ausente en {}", marker_path.display()))?;
    match schema {
        1 => {
            let marker: RuntimeMarkerV1 = serde_json::from_value(value)
                .map_err(|error| format!("marker v1 inválido: {error}"))?;
            Ok(Some(ParsedStoredReceipt::V1(marker)))
        }
        2 => {
            let receipt: RuntimeReceiptV2 = serde_json::from_value(value)
                .map_err(|error| format!("receipt v2 inválido: {error}"))?;
            Ok(Some(ParsedStoredReceipt::V2(receipt)))
        }
        _ => Ok(None),
    }
}

/// Same filename and schema. The temp is private to this writer (create_new); the receipt
/// rename is atomic, so elevation readers keep either valid v1 or complete v2.
pub(super) fn write_receipt(descriptor: &ArtifactDescriptor, root: &Path) -> Result<(), String> {
    let target = root.join(MARKER_FILE);
    let temp = root.join(format!(
        ".{MARKER_FILE}.tmp-{}-{}",
        std::process::id(),
        super::unique_suffix()
    ));
    let bytes = serde_json::to_vec_pretty(&receipt_v2_from_descriptor(descriptor))
        .map_err(|error| error.to_string())?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)
        .map_err(|error| error.to_string())?;
    let result = (|| {
        output
            .write_all(&bytes)
            .and_then(|_| output.sync_all())
            .map_err(|error| error.to_string())?;
        #[cfg(test)]
        super::install::checkpoint("receipt_temp_synced")?;
        std::fs::rename(&temp, &target).map_err(|error| error.to_string())?;
        #[cfg(test)]
        super::install::checkpoint("receipt_renamed")?;
        File::open(root)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| error.to_string())
    })();
    if result.is_err() {
        match std::fs::remove_file(&temp) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => crate::utils::report("artifacts.receipt", error.to_string()),
        }
    }
    result
}

pub(super) fn elevate_v1(descriptor: &ArtifactDescriptor, root: &Path) -> Result<(), String> {
    if super::artifact_cache_state_at(descriptor, root) != super::ArtifactCacheState::Ready {
        return Ok(());
    }
    if matches!(parse_stored_receipt(&root.join(MARKER_FILE))?, Some(ParsedStoredReceipt::V1(ref marker)) if runtime_marker_v1_matches(descriptor, marker))
    {
        write_receipt(descriptor, root)?;
    }
    Ok(())
}

pub(crate) enum ParsedStoredReceipt {
    V1(RuntimeMarkerV1),
    V2(RuntimeReceiptV2),
}

pub(crate) fn stored_identity_matches_descriptor(
    descriptor: &ArtifactDescriptor,
    parsed: &ParsedStoredReceipt,
) -> bool {
    match parsed {
        ParsedStoredReceipt::V1(marker) => runtime_marker_v1_matches(descriptor, marker),
        ParsedStoredReceipt::V2(receipt) => stored_receipt_matches_descriptor(descriptor, receipt),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::artifacts::descriptor::{
        catalog_descriptor, DXVK_SHA256, LEGACY_MANAGED_RUNNER_ID, MANAGED_DXVK_ID,
        MANAGED_RUNNER_ID, NNDSK_RUNTIME_SOURCE_COMMIT, PROTON_SHA512, UMU_ID, UMU_SHA256,
    };
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct ReceiptFixtures {
        proton: RuntimeReceiptV2,
        umu: RuntimeReceiptV2,
        dxvk: RuntimeReceiptV2,
        #[serde(rename = "futureSchema")]
        future_schema: RuntimeReceiptV2,
    }

    #[test]
    fn runtime_marker_schema_one_fixture_matches_only_the_pinned_artifact() {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RuntimeMarkerFixtures {
            schema1: RuntimeMarkerV1,
            future_schema: RuntimeMarkerV1,
        }
        let fixtures: RuntimeMarkerFixtures = serde_json::from_str(include_str!(
            "../../../../contract-fixtures/runtime-markers.json"
        ))
        .unwrap();
        let proton = catalog_descriptor(LEGACY_MANAGED_RUNNER_ID).unwrap();
        assert!(runtime_marker_v1_matches(proton, &fixtures.schema1));
        assert!(!runtime_marker_v1_matches(proton, &fixtures.future_schema));
        assert!(!runtime_marker_v1_matches(
            catalog_descriptor(UMU_ID).unwrap(),
            &fixtures.schema1
        ));
    }

    #[test]
    fn runtime_receipt_v2_fixture_matches_catalog() {
        let fixtures: ReceiptFixtures = serde_json::from_str(include_str!(
            "../../../../contract-fixtures/runtime-receipts-v2.json"
        ))
        .unwrap();
        let proton = catalog_descriptor(LEGACY_MANAGED_RUNNER_ID).unwrap();
        assert!(stored_receipt_matches_descriptor(proton, &fixtures.proton));
        assert_eq!(fixtures.proton.digest, PROTON_SHA512);

        let umu = catalog_descriptor(UMU_ID).unwrap();
        assert!(stored_receipt_matches_descriptor(umu, &fixtures.umu));
        assert_eq!(fixtures.umu.digest, UMU_SHA256);

        let dxvk = catalog_descriptor(MANAGED_DXVK_ID).unwrap();
        assert!(stored_receipt_matches_descriptor(dxvk, &fixtures.dxvk));
        assert_eq!(fixtures.dxvk.digest, DXVK_SHA256);

        assert!(!stored_receipt_matches_descriptor(
            proton,
            &fixtures.future_schema
        ));
    }

    #[test]
    fn same_version_display_different_digest_is_different_identity() {
        let proton = catalog_descriptor(LEGACY_MANAGED_RUNNER_ID).unwrap();
        let mut foreign = receipt_v2_from_descriptor(proton);
        foreign.digest = DXVK_SHA256.to_string();
        assert!(!stored_receipt_matches_descriptor(proton, &foreign));
    }

    #[test]
    fn local_runtime_receipt_records_source_identity_without_fabricating_https() {
        let descriptor =
            catalog_descriptor(super::super::descriptor::LOCAL_MANAGED_RUNNER_ID).unwrap();
        let receipt = receipt_v2_from_descriptor(descriptor);
        let value = serde_json::to_value(&receipt).unwrap();
        assert_eq!(value["source"]["type"], "localOnly");
        assert_eq!(value["source"]["sourceCommit"], NNDSK_RUNTIME_SOURCE_COMMIT);
        assert!(value["source"].get("url").is_none());
        let decoded = serde_json::from_value(value).unwrap();
        assert!(stored_receipt_matches_descriptor(descriptor, &decoded));
        let mut altered = receipt;
        altered.source = ReceiptSourceV2::LocalOnly {
            source_commit: "00".repeat(20),
        };
        assert!(!stored_receipt_matches_descriptor(descriptor, &altered));
        assert!(!runtime_marker_v1_matches(
            descriptor,
            &RuntimeMarkerV1 {
                schema_version: RUNTIME_SCHEMA_V1,
                artifact_id: descriptor.id.to_string(),
                digest: expected_digest_hex(descriptor.digest).to_string(),
            }
        ));
    }

    #[test]
    fn published_runtime_receipt_binds_the_exact_versioned_https_source() {
        let descriptor = catalog_descriptor(MANAGED_RUNNER_ID).unwrap();
        let receipt = receipt_v2_from_descriptor(descriptor);
        assert!(stored_receipt_matches_descriptor(descriptor, &receipt));
        let mut altered = receipt;
        altered.source = ReceiptSourceV2::Https {
            url: "https://example.com/other.tar.zst".to_string(),
        };
        assert!(!stored_receipt_matches_descriptor(descriptor, &altered));
        let local = catalog_descriptor(super::super::descriptor::LOCAL_MANAGED_RUNNER_ID).unwrap();
        assert!(!stored_receipt_matches_descriptor(
            descriptor,
            &receipt_v2_from_descriptor(local)
        ));
    }
}
