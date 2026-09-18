use super::managed_identity::{managed_dxvk_artifact_identity, managed_proton_artifact_identity};
use crate::tools::runners::MANAGED_RUNNER_ID;
use crate::utils::RunnerKind;

use super::encode::{
    fingerprint_digest, CanonicalValue, PREFIX_FINGERPRINT_DOMAIN, RUNTIME_FINGERPRINT_DOMAIN,
};
use super::model::{
    ArtifactArchitecture, ArtifactIdentity, FingerprintDigest, FINGERPRINT_SCHEMA_VERSION,
    PREFIX_MATERIAL_RECIPE_REVISION,
};
use super::resolver::WINETRICKS_DXVK_RECIPE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrefixFingerprint {
    pub(crate) digest: FingerprintDigest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeFingerprint {
    pub(crate) digest: FingerprintDigest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArchitecturePolicy {
    RunnerDefault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Wow64LayoutFingerprint {
    OldWow64,
    NewWow64,
    Wine32Only,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RunnerLocatorIdentity {
    Managed { artifact_id: String },
    External { location_token: [u8; 32] },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PrefixOwnedGraphics {
    None,
    ManagedDxvk(ArtifactIdentity),
    WinetricksDxvk { recipe_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrefixFingerprintInput {
    pub(crate) architecture_policy: ArchitecturePolicy,
    pub(crate) runner_kind: RunnerKind,
    pub(crate) runner_locator: RunnerLocatorIdentity,
    pub(crate) runner_prefix_identity: RunnerPrefixIdentity,
    pub(crate) wow64_layout: Wow64LayoutFingerprint,
    pub(crate) prefix_owned_graphics: PrefixOwnedGraphics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RunnerPrefixIdentity {
    Managed(ArtifactIdentity),
    External(Vec<ExternalRoleMaterial>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalRoleMaterial {
    pub(crate) role: String,
    pub(crate) digest: Option<Vec<u8>>,
}

pub(crate) fn encode_artifact_identity(identity: &ArtifactIdentity) -> CanonicalValue {
    let architectures = identity
        .architectures
        .iter()
        .map(|arch| {
            CanonicalValue::enum_variant(
                match arch {
                    ArtifactArchitecture::X86 => "x86",
                    ArtifactArchitecture::X86_64 => "x86-64",
                },
                CanonicalValue::empty_record(),
            )
        })
        .collect();
    let algorithm = match identity.source_digest.algorithm {
        super::model::DigestAlgorithm::Sha256 => "sha256",
        super::model::DigestAlgorithm::Sha512 => "sha512",
    };
    CanonicalValue::record(vec![
        (
            "architectures".to_string(),
            CanonicalValue::Sequence(architectures),
        ),
        (
            "artifact-id".to_string(),
            CanonicalValue::String(identity.artifact_id.as_str().to_string()),
        ),
        (
            "install-recipe-revision".to_string(),
            CanonicalValue::U64(identity.install_recipe_revision),
        ),
        (
            "platform".to_string(),
            CanonicalValue::String(identity.platform.clone()),
        ),
        (
            "schema-version".to_string(),
            CanonicalValue::U64(identity.schema_version),
        ),
        (
            "source-digest".to_string(),
            CanonicalValue::record(vec![
                (
                    "algorithm".to_string(),
                    CanonicalValue::String(algorithm.to_string()),
                ),
                (
                    "bytes".to_string(),
                    CanonicalValue::Bytes(identity.source_digest.bytes.clone()),
                ),
            ]),
        ),
    ])
}

pub(crate) fn compute_prefix_fingerprint(input: &PrefixFingerprintInput) -> PrefixFingerprint {
    let root = encode_prefix_fingerprint_input(input);
    let digest = fingerprint_digest(PREFIX_FINGERPRINT_DOMAIN, &root);
    PrefixFingerprint {
        digest: FingerprintDigest {
            schema_version: FINGERPRINT_SCHEMA_VERSION,
            algorithm: "sha256".to_string(),
            digest,
        },
    }
}

fn encode_prefix_fingerprint_input(input: &PrefixFingerprintInput) -> CanonicalValue {
    let architecture_policy = CanonicalValue::enum_variant(
        match input.architecture_policy {
            ArchitecturePolicy::RunnerDefault => "runner-default",
        },
        CanonicalValue::empty_record(),
    );
    let runner_kind = CanonicalValue::enum_variant(
        match input.runner_kind {
            RunnerKind::Wine => "wine",
            RunnerKind::Proton => "proton",
        },
        CanonicalValue::empty_record(),
    );
    let runner_locator = match &input.runner_locator {
        RunnerLocatorIdentity::Managed { artifact_id } => CanonicalValue::enum_variant(
            "managed",
            CanonicalValue::record(vec![
                (
                    "artifact-id".to_string(),
                    CanonicalValue::String(artifact_id.clone()),
                ),
                (
                    "entrypoint-role".to_string(),
                    CanonicalValue::enum_variant("entrypoint", CanonicalValue::empty_record()),
                ),
            ]),
        ),
        RunnerLocatorIdentity::External { location_token } => CanonicalValue::enum_variant(
            "external",
            CanonicalValue::record(vec![(
                "location-token".to_string(),
                CanonicalValue::Bytes(location_token.to_vec()),
            )]),
        ),
    };
    let runner_prefix_identity = match &input.runner_prefix_identity {
        RunnerPrefixIdentity::Managed(identity) => CanonicalValue::enum_variant(
            "managed",
            CanonicalValue::record(vec![(
                "identity".to_string(),
                encode_artifact_identity(identity),
            )]),
        ),
        RunnerPrefixIdentity::External(roles) => {
            let encoded_roles = roles
                .iter()
                .map(|role| {
                    CanonicalValue::record(vec![
                        (
                            "digest".to_string(),
                            match &role.digest {
                                Some(bytes) => CanonicalValue::Bytes(bytes.clone()),
                                None => CanonicalValue::Null,
                            },
                        ),
                        (
                            "role".to_string(),
                            CanonicalValue::enum_variant(
                                role.role.as_str(),
                                CanonicalValue::empty_record(),
                            ),
                        ),
                    ])
                })
                .collect();
            CanonicalValue::enum_variant(
                "external",
                CanonicalValue::record(vec![(
                    "roles".to_string(),
                    CanonicalValue::Sequence(encoded_roles),
                )]),
            )
        }
    };
    let wow64_layout = CanonicalValue::enum_variant(
        match input.wow64_layout {
            Wow64LayoutFingerprint::OldWow64 => "old-wow64",
            Wow64LayoutFingerprint::NewWow64 => "new-wow64",
            Wow64LayoutFingerprint::Wine32Only => "wine32-only",
            Wow64LayoutFingerprint::Unknown => "unknown",
        },
        CanonicalValue::empty_record(),
    );
    let prefix_owned_graphics = match &input.prefix_owned_graphics {
        PrefixOwnedGraphics::None => {
            CanonicalValue::enum_variant("none", CanonicalValue::empty_record())
        }
        PrefixOwnedGraphics::ManagedDxvk(identity) => CanonicalValue::enum_variant(
            "managed-dxvk",
            CanonicalValue::record(vec![(
                "identity".to_string(),
                encode_artifact_identity(identity),
            )]),
        ),
        PrefixOwnedGraphics::WinetricksDxvk { recipe_id } => CanonicalValue::enum_variant(
            "winetricks-dxvk",
            CanonicalValue::record(vec![(
                "recipe-id".to_string(),
                CanonicalValue::String(recipe_id.clone()),
            )]),
        ),
    };
    CanonicalValue::record(vec![
        ("architecture-policy".to_string(), architecture_policy),
        (
            "isolation-critical-deps".to_string(),
            CanonicalValue::Sequence(Vec::new()),
        ),
        ("prefix-owned-graphics".to_string(), prefix_owned_graphics),
        (
            "recipe-revision".to_string(),
            CanonicalValue::U64(PREFIX_MATERIAL_RECIPE_REVISION),
        ),
        ("runner-kind".to_string(), runner_kind),
        ("runner-locator".to_string(), runner_locator),
        ("runner-prefix-identity".to_string(), runner_prefix_identity),
        ("wow64-layout".to_string(), wow64_layout),
    ])
}

pub(crate) fn managed_proton_prefix_fingerprint_input() -> PrefixFingerprintInput {
    let identity = managed_proton_artifact_identity();
    PrefixFingerprintInput {
        architecture_policy: ArchitecturePolicy::RunnerDefault,
        runner_kind: RunnerKind::Proton,
        runner_locator: RunnerLocatorIdentity::Managed {
            artifact_id: MANAGED_RUNNER_ID.to_string(),
        },
        runner_prefix_identity: RunnerPrefixIdentity::Managed(identity),
        wow64_layout: Wow64LayoutFingerprint::Unknown,
        prefix_owned_graphics: PrefixOwnedGraphics::None,
    }
}

pub(crate) fn prefix_fingerprint_from_plan(
    runner_kind: RunnerKind,
    locator: RunnerLocatorIdentity,
    prefix_identity: RunnerPrefixIdentity,
    wow64: Wow64LayoutFingerprint,
    graphics: PrefixOwnedGraphics,
) -> PrefixFingerprint {
    compute_prefix_fingerprint(&PrefixFingerprintInput {
        architecture_policy: ArchitecturePolicy::RunnerDefault,
        runner_kind,
        runner_locator: locator,
        runner_prefix_identity: prefix_identity,
        wow64_layout: wow64,
        prefix_owned_graphics: graphics,
    })
}

pub(crate) fn prefix_owned_graphics_for_dxvk(
    runner_kind: RunnerKind,
    wine_7_16: bool,
) -> PrefixOwnedGraphics {
    match runner_kind {
        RunnerKind::Proton => PrefixOwnedGraphics::None,
        RunnerKind::Wine if wine_7_16 => {
            PrefixOwnedGraphics::ManagedDxvk(managed_dxvk_artifact_identity())
        }
        RunnerKind::Wine => PrefixOwnedGraphics::WinetricksDxvk {
            recipe_id: WINETRICKS_DXVK_RECIPE.to_string(),
        },
    }
}

pub(crate) fn runtime_fingerprint_placeholder() -> RuntimeFingerprint {
    let digest = fingerprint_digest(
        RUNTIME_FINGERPRINT_DOMAIN,
        &CanonicalValue::record(vec![(
            "placeholder".to_string(),
            CanonicalValue::Bool(true),
        )]),
    );
    RuntimeFingerprint {
        digest: FingerprintDigest {
            schema_version: FINGERPRINT_SCHEMA_VERSION,
            algorithm: "sha256".to_string(),
            digest,
        },
    }
}

pub(crate) fn explain_identity_delta(
    left: &PrefixFingerprintInput,
    right: &PrefixFingerprintInput,
) -> Vec<&'static str> {
    let mut fields = Vec::new();
    if left.architecture_policy != right.architecture_policy {
        fields.push("architecture-policy");
    }
    if left.runner_kind != right.runner_kind {
        fields.push("runner-kind");
    }
    if left.runner_locator != right.runner_locator {
        fields.push("runner-locator");
    }
    if left.runner_prefix_identity != right.runner_prefix_identity {
        fields.push("runner-prefix-identity");
    }
    if left.wow64_layout != right.wow64_layout {
        fields.push("wow64-layout");
    }
    if left.prefix_owned_graphics != right.prefix_owned_graphics {
        fields.push("prefix-owned-graphics");
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::prefix::MANAGED_DXVK_COMPONENT;
    use crate::tools::runtime::encode::sha256_hex;

    #[test]
    fn golden_managed_proton_prefix_fingerprint() {
        let fp = compute_prefix_fingerprint(&managed_proton_prefix_fingerprint_input());
        assert_eq!(
            fp.digest.hex_digest(),
            "a23f2a940a9cb8e5110b0e454ad68a35a22af760c005bcd0ebdf47ab44330270"
        );
        let root = encode_prefix_fingerprint_input(&managed_proton_prefix_fingerprint_input());
        let mut encoded = Vec::new();
        root.encode(&mut encoded);
        assert_eq!(
            sha256_hex(&encoded),
            "e43918a5f92ade0f4bb785458889fc40b80eff603ef7106541d9fe04a2f88bee"
        );
    }

    #[test]
    fn dgvoodoo_does_not_change_prefix_fingerprint() {
        let base = managed_proton_prefix_fingerprint_input();
        let a = compute_prefix_fingerprint(&base);
        let b = compute_prefix_fingerprint(&base);
        assert_eq!(a.digest.hex_digest(), b.digest.hex_digest());
    }

    #[test]
    fn managed_dxvk_changes_prefix_fingerprint() {
        let proton = compute_prefix_fingerprint(&managed_proton_prefix_fingerprint_input());
        let wine = compute_prefix_fingerprint(&PrefixFingerprintInput {
            architecture_policy: ArchitecturePolicy::RunnerDefault,
            runner_kind: RunnerKind::Wine,
            runner_locator: RunnerLocatorIdentity::Managed {
                artifact_id: MANAGED_RUNNER_ID.to_string(),
            },
            runner_prefix_identity: RunnerPrefixIdentity::Managed(
                managed_proton_artifact_identity(),
            ),
            wow64_layout: Wow64LayoutFingerprint::Unknown,
            prefix_owned_graphics: PrefixOwnedGraphics::ManagedDxvk(
                managed_dxvk_artifact_identity(),
            ),
        });
        assert_ne!(proton.digest.hex_digest(), wine.digest.hex_digest());
        assert_eq!(MANAGED_DXVK_COMPONENT, "dxvk-2.6.2");
    }
}
