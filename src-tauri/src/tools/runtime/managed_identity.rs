use std::collections::BTreeSet;

use crate::tools::runners::{
    managed_dxvk_source_digest_bytes, managed_proton_source_digest_bytes, managed_runtime_ready,
    managed_umu_source_digest_bytes, MANAGED_DXVK_ID, MANAGED_RUNNER_ID, UMU_ID,
};

use super::model::{
    ArtifactArchitecture, ArtifactId, ArtifactIdentity, DigestAlgorithm, PayloadVerification,
    SourceDigest, ARTIFACT_IDENTITY_SCHEMA_VERSION,
};

pub(crate) fn managed_runtime_payload_verification() -> PayloadVerification {
    if managed_runtime_ready() {
        PayloadVerification::ShapeVerified
    } else {
        PayloadVerification::Unverified
    }
}

pub(crate) fn managed_proton_artifact_identity() -> ArtifactIdentity {
    ArtifactIdentity {
        schema_version: ARTIFACT_IDENTITY_SCHEMA_VERSION,
        artifact_id: ArtifactId::new(MANAGED_RUNNER_ID).expect("managed runner id"),
        source_digest: SourceDigest {
            algorithm: DigestAlgorithm::Sha512,
            bytes: managed_proton_source_digest_bytes(),
        },
        platform: "linux-x86_64".to_string(),
        architectures: BTreeSet::from([ArtifactArchitecture::X86_64]),
        install_recipe_revision: 1,
    }
}

pub(crate) fn managed_dxvk_artifact_identity() -> ArtifactIdentity {
    ArtifactIdentity {
        schema_version: ARTIFACT_IDENTITY_SCHEMA_VERSION,
        artifact_id: ArtifactId::new(MANAGED_DXVK_ID).expect("managed dxvk id"),
        source_digest: SourceDigest {
            algorithm: DigestAlgorithm::Sha256,
            bytes: managed_dxvk_source_digest_bytes(),
        },
        platform: "linux-x86_64".to_string(),
        architectures: BTreeSet::from([ArtifactArchitecture::X86, ArtifactArchitecture::X86_64]),
        install_recipe_revision: 1,
    }
}

pub(crate) fn managed_umu_artifact_identity() -> ArtifactIdentity {
    ArtifactIdentity {
        schema_version: ARTIFACT_IDENTITY_SCHEMA_VERSION,
        artifact_id: ArtifactId::new(UMU_ID).expect("managed umu id"),
        source_digest: SourceDigest {
            algorithm: DigestAlgorithm::Sha256,
            bytes: managed_umu_source_digest_bytes(),
        },
        platform: "linux-x86_64".to_string(),
        architectures: BTreeSet::from([ArtifactArchitecture::X86_64]),
        install_recipe_revision: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_identity_still_shape_verified_not_source_and_payload() {
        assert_ne!(
            PayloadVerification::ShapeVerified,
            PayloadVerification::SourceAndPayloadVerified
        );
        if managed_runtime_ready() {
            assert_eq!(
                managed_runtime_payload_verification(),
                PayloadVerification::ShapeVerified
            );
        } else {
            assert_eq!(
                managed_runtime_payload_verification(),
                PayloadVerification::Unverified
            );
        }
    }
}
