use crate::utils::RunnerKind;

use super::encode::{fingerprint_digest, CanonicalValue, RUNTIME_FINGERPRINT_DOMAIN};
use super::fingerprint::{
    encode_artifact_identity, PrefixFingerprint, RuntimeFingerprint, Wow64LayoutFingerprint,
};
use super::identity::PrefixIdentityStatus;
use super::model::{
    CapabilityEvidence, ComponentProvenance, FingerprintDigest, GraphicsProfile, ObservedMaterial,
    ObservedMaterialRole, PrefixArchitecture, RuntimePlan, SyncPlan, Wow64Layout,
    BASE_SETUP_RECIPE_REVISION, ENVIRONMENT_BUILDER_REVISION, FINGERPRINT_SCHEMA_VERSION,
    INVOCATION_POLICY_REVISION, RUNTIME_PLAN_RECIPE_REVISION,
};
use super::resolver::WINETRICKS_DXVK_RECIPE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FingerprintCapability<T> {
    Known(T),
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObservedPrefixArchitectureV1 {
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UmuProvenanceV1 {
    RunnerManaged,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuntimeOverlayIdentityV1 {
    None,
    DgVoodoo { component_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuntimeDxvkIdentityV1 {
    RunnerOwned { component_id: String },
    ManagedPrefix { artifact_id: String },
    WinetricksPrefix { recipe_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeFingerprintInput {
    pub runtime_plan_recipe_revision: u64,
    pub runner_kind: RunnerKind,
    pub runner_identity: super::model::RunnerIdentity,
    pub umu_provenance: UmuProvenanceV1,
    pub wow64_layout: FingerprintCapability<Wow64LayoutFingerprint>,
    pub supported_prefix_architectures: FingerprintCapability<Vec<PrefixArchitecture>>,
    pub sync_plan: SyncPlan,
    pub graphics_profile: GraphicsProfile,
    pub dxvk: RuntimeDxvkIdentityV1,
    pub overlay: RuntimeOverlayIdentityV1,
    pub prefix_binding_status: PrefixIdentityStatus,
    pub desired_prefix_fingerprint: PrefixFingerprint,
    pub observed_prefix_architecture: ObservedPrefixArchitectureV1,
    pub base_setup_recipe_revision: u64,
    pub webview2_required: bool,
    pub invocation_policy_revision: u64,
    pub environment_builder_revision: u64,
}

pub(crate) fn runtime_fingerprint_input_from_plan(
    plan: &RuntimePlan,
    binding: &super::identity::PrefixBinding,
) -> RuntimeFingerprintInput {
    let runner = plan.runner();
    let wow64 = map_wow64(&runner.capabilities().wow64_layout);
    let architectures = map_architectures(&runner.capabilities().supported_prefix_architectures);
    let umu = match runner.resolved().kind() {
        RunnerKind::Proton => UmuProvenanceV1::RunnerManaged,
        RunnerKind::Wine => UmuProvenanceV1::None,
    };
    let graphics = plan.graphics();
    let overlay = match graphics.overlay() {
        None => RuntimeOverlayIdentityV1::None,
        Some(overlay) => RuntimeOverlayIdentityV1::DgVoodoo {
            component_id: overlay.component_id.as_str().to_string(),
        },
    };
    let dxvk = map_dxvk(plan.graphics().dxvk_provider());
    RuntimeFingerprintInput {
        runtime_plan_recipe_revision: RUNTIME_PLAN_RECIPE_REVISION,
        runner_kind: runner.resolved().kind(),
        runner_identity: runner.identity().clone(),
        umu_provenance: umu,
        wow64_layout: wow64,
        supported_prefix_architectures: architectures,
        sync_plan: runner.sync_plan(),
        graphics_profile: graphics.profile(),
        dxvk,
        overlay,
        prefix_binding_status: binding.status,
        desired_prefix_fingerprint: binding.desired_fingerprint.clone(),
        observed_prefix_architecture: ObservedPrefixArchitectureV1::Unknown,
        base_setup_recipe_revision: BASE_SETUP_RECIPE_REVISION,
        webview2_required: plan.webview2_required(),
        invocation_policy_revision: INVOCATION_POLICY_REVISION,
        environment_builder_revision: ENVIRONMENT_BUILDER_REVISION,
    }
}

pub(crate) fn compute_runtime_fingerprint_from_input(
    input: &RuntimeFingerprintInput,
) -> RuntimeFingerprint {
    let root = encode_runtime_fingerprint_input(input);
    let digest = fingerprint_digest(RUNTIME_FINGERPRINT_DOMAIN, &root);
    RuntimeFingerprint {
        digest: FingerprintDigest {
            schema_version: FINGERPRINT_SCHEMA_VERSION,
            algorithm: "sha256".to_string(),
            digest,
        },
    }
}

fn map_wow64(
    evidence: &CapabilityEvidence<Wow64Layout>,
) -> FingerprintCapability<Wow64LayoutFingerprint> {
    match evidence {
        CapabilityEvidence::Known { value, .. } => FingerprintCapability::Known(match value {
            Wow64Layout::OldWow64 => Wow64LayoutFingerprint::OldWow64,
            Wow64Layout::NewWow64 => Wow64LayoutFingerprint::NewWow64,
            Wow64Layout::Wine32Only => Wow64LayoutFingerprint::Wine32Only,
        }),
        CapabilityEvidence::Unknown { .. } => FingerprintCapability::Unknown,
    }
}

fn map_architectures(
    evidence: &CapabilityEvidence<std::collections::BTreeSet<PrefixArchitecture>>,
) -> FingerprintCapability<Vec<PrefixArchitecture>> {
    match evidence {
        CapabilityEvidence::Known { value, .. } => {
            FingerprintCapability::Known(value.iter().copied().collect())
        }
        CapabilityEvidence::Unknown { .. } => FingerprintCapability::Unknown,
    }
}

fn map_dxvk(provider: &super::model::DxvkProvider) -> RuntimeDxvkIdentityV1 {
    if provider.is_managed_prefix() {
        return RuntimeDxvkIdentityV1::ManagedPrefix {
            artifact_id: provider
                .managed_dxvk_artifact_id()
                .unwrap_or("dxvk-2.6.2")
                .to_string(),
        };
    }
    let component = provider.component_id();
    if component == WINETRICKS_DXVK_RECIPE {
        return RuntimeDxvkIdentityV1::WinetricksPrefix {
            recipe_id: WINETRICKS_DXVK_RECIPE.to_string(),
        };
    }
    RuntimeDxvkIdentityV1::RunnerOwned {
        component_id: component.to_string(),
    }
}

fn encode_runtime_fingerprint_input(input: &RuntimeFingerprintInput) -> CanonicalValue {
    CanonicalValue::record(vec![
        (
            "base-setup-recipe-revision".to_string(),
            CanonicalValue::U64(input.base_setup_recipe_revision),
        ),
        (
            "desired-prefix-fingerprint".to_string(),
            CanonicalValue::Bytes(input.desired_prefix_fingerprint.digest.digest.to_vec()),
        ),
        ("dxvk".to_string(), encode_dxvk(&input.dxvk)),
        (
            "environment-builder-revision".to_string(),
            CanonicalValue::U64(input.environment_builder_revision),
        ),
        (
            "graphics-profile".to_string(),
            encode_graphics_profile(input.graphics_profile),
        ),
        (
            "invocation-policy-revision".to_string(),
            CanonicalValue::U64(input.invocation_policy_revision),
        ),
        (
            "observed-prefix-architecture".to_string(),
            CanonicalValue::enum_variant("unknown", CanonicalValue::empty_record()),
        ),
        ("overlay".to_string(), encode_overlay(&input.overlay)),
        ("overlay-config-digest".to_string(), CanonicalValue::Null),
        (
            "prefix-binding-status".to_string(),
            encode_prefix_binding_status(input.prefix_binding_status),
        ),
        (
            "repairable-dependency-identities".to_string(),
            CanonicalValue::Sequence(Vec::new()),
        ),
        (
            "runner-identity".to_string(),
            encode_runner_identity(&input.runner_identity),
        ),
        (
            "runner-kind".to_string(),
            encode_runner_kind(input.runner_kind),
        ),
        (
            "runtime-plan-recipe-revision".to_string(),
            CanonicalValue::U64(input.runtime_plan_recipe_revision),
        ),
        (
            "supported-prefix-architectures".to_string(),
            encode_architectures(&input.supported_prefix_architectures),
        ),
        ("sync-plan".to_string(), encode_sync_plan(input.sync_plan)),
        (
            "umu-provenance".to_string(),
            encode_umu(&input.umu_provenance),
        ),
        (
            "webview2-required".to_string(),
            CanonicalValue::Bool(input.webview2_required),
        ),
        (
            "wow64-layout".to_string(),
            encode_wow64(&input.wow64_layout),
        ),
    ])
}

fn encode_runner_kind(kind: RunnerKind) -> CanonicalValue {
    CanonicalValue::enum_variant(
        match kind {
            RunnerKind::Wine => "wine",
            RunnerKind::Proton => "proton",
        },
        CanonicalValue::empty_record(),
    )
}

fn encode_graphics_profile(profile: GraphicsProfile) -> CanonicalValue {
    CanonicalValue::enum_variant(
        match profile {
            GraphicsProfile::Dxvk => "dxvk",
            GraphicsProfile::DgVoodooDxvk => "dgvoodoo-dxvk",
        },
        CanonicalValue::empty_record(),
    )
}

fn encode_sync_plan(sync: SyncPlan) -> CanonicalValue {
    CanonicalValue::enum_variant(
        match sync {
            SyncPlan::WineServer => "wine-server",
            SyncPlan::Esync => "esync",
            SyncPlan::Fsync => "fsync",
            SyncPlan::RunnerManaged => "runner-managed",
        },
        CanonicalValue::empty_record(),
    )
}

fn encode_prefix_binding_status(status: PrefixIdentityStatus) -> CanonicalValue {
    CanonicalValue::enum_variant(
        match status {
            PrefixIdentityStatus::V3Verified => "v3-verified",
            PrefixIdentityStatus::LegacyV2RunnerMatched => "legacy-v2-runner-matched",
            PrefixIdentityStatus::Unknown => "unknown",
            PrefixIdentityStatus::Incompatible => "incompatible",
        },
        CanonicalValue::empty_record(),
    )
}

fn encode_umu(umu: &UmuProvenanceV1) -> CanonicalValue {
    match umu {
        UmuProvenanceV1::RunnerManaged => {
            CanonicalValue::enum_variant("runner-managed", CanonicalValue::empty_record())
        }
        UmuProvenanceV1::None => CanonicalValue::Null,
    }
}

fn encode_wow64(wow64: &FingerprintCapability<Wow64LayoutFingerprint>) -> CanonicalValue {
    match wow64 {
        FingerprintCapability::Unknown => {
            CanonicalValue::enum_variant("unknown", CanonicalValue::empty_record())
        }
        FingerprintCapability::Known(value) => CanonicalValue::enum_variant(
            match value {
                Wow64LayoutFingerprint::OldWow64 => "old-wow64",
                Wow64LayoutFingerprint::NewWow64 => "new-wow64",
                Wow64LayoutFingerprint::Wine32Only => "wine32-only",
                Wow64LayoutFingerprint::Unknown => "unknown",
            },
            CanonicalValue::empty_record(),
        ),
    }
}

fn encode_architectures(
    architectures: &FingerprintCapability<Vec<PrefixArchitecture>>,
) -> CanonicalValue {
    match architectures {
        FingerprintCapability::Unknown => {
            CanonicalValue::enum_variant("unknown", CanonicalValue::empty_record())
        }
        FingerprintCapability::Known(values) => {
            let encoded = values
                .iter()
                .map(|arch| {
                    CanonicalValue::enum_variant(
                        match arch {
                            PrefixArchitecture::X86 => "x86",
                            PrefixArchitecture::X86_64 => "x86-64",
                        },
                        CanonicalValue::empty_record(),
                    )
                })
                .collect();
            CanonicalValue::enum_variant("known", CanonicalValue::Sequence(encoded))
        }
    }
}

fn encode_dxvk(dxvk: &RuntimeDxvkIdentityV1) -> CanonicalValue {
    match dxvk {
        RuntimeDxvkIdentityV1::RunnerOwned { component_id } => CanonicalValue::enum_variant(
            "runner-owned",
            CanonicalValue::record(vec![(
                "component-id".to_string(),
                CanonicalValue::String(component_id.clone()),
            )]),
        ),
        RuntimeDxvkIdentityV1::ManagedPrefix { artifact_id } => CanonicalValue::enum_variant(
            "managed-prefix",
            CanonicalValue::record(vec![(
                "artifact-id".to_string(),
                CanonicalValue::String(artifact_id.clone()),
            )]),
        ),
        RuntimeDxvkIdentityV1::WinetricksPrefix { recipe_id } => CanonicalValue::enum_variant(
            "winetricks-prefix",
            CanonicalValue::record(vec![(
                "recipe-id".to_string(),
                CanonicalValue::String(recipe_id.clone()),
            )]),
        ),
    }
}

fn encode_overlay(overlay: &RuntimeOverlayIdentityV1) -> CanonicalValue {
    match overlay {
        RuntimeOverlayIdentityV1::None => {
            CanonicalValue::enum_variant("none", CanonicalValue::empty_record())
        }
        RuntimeOverlayIdentityV1::DgVoodoo { component_id } => CanonicalValue::enum_variant(
            "dgvoodoo",
            CanonicalValue::record(vec![(
                "component-id".to_string(),
                CanonicalValue::String(component_id.clone()),
            )]),
        ),
    }
}

fn encode_runner_identity(identity: &super::model::RunnerIdentity) -> CanonicalValue {
    let provenance = encode_component_provenance(&identity.provenance);
    let observed = identity
        .observed_material
        .roles
        .iter()
        .map(encode_observed_material)
        .collect();
    CanonicalValue::record(vec![
        ("provenance".to_string(), provenance),
        (
            "observed-material".to_string(),
            CanonicalValue::Sequence(observed),
        ),
    ])
}

fn encode_observed_material(material: &ObservedMaterial) -> CanonicalValue {
    CanonicalValue::record(vec![
        (
            "digest".to_string(),
            match &material.digest {
                Some(bytes) => CanonicalValue::Bytes(bytes.clone()),
                None => CanonicalValue::Null,
            },
        ),
        (
            "role".to_string(),
            CanonicalValue::enum_variant(
                observed_role_id(material.role),
                CanonicalValue::empty_record(),
            ),
        ),
    ])
}

fn observed_role_id(role: ObservedMaterialRole) -> &'static str {
    match role {
        ObservedMaterialRole::Entrypoint => "entrypoint",
        ObservedMaterialRole::WineServer => "wine-server",
        ObservedMaterialRole::ProtonInnerWine => "proton-inner-wine",
        ObservedMaterialRole::ProtonVersion => "proton-version",
        ObservedMaterialRole::UmuEntrypoint => "umu-entrypoint",
        ObservedMaterialRole::WineTkgConfig => "wine-tkg-config",
    }
}

fn encode_component_provenance(provenance: &ComponentProvenance) -> CanonicalValue {
    match provenance {
        ComponentProvenance::ArtifactReceipt(receipt) => CanonicalValue::enum_variant(
            "artifact-receipt",
            CanonicalValue::record(vec![(
                "identity".to_string(),
                encode_artifact_identity(&receipt.identity),
            )]),
        ),
        ComponentProvenance::BundledResource { resource_id } => CanonicalValue::enum_variant(
            "bundled-resource",
            CanonicalValue::record(vec![(
                "resource-id".to_string(),
                CanonicalValue::String(resource_id.clone()),
            )]),
        ),
        ComponentProvenance::ExternalObserved(observed) => CanonicalValue::enum_variant(
            "external-observed",
            CanonicalValue::record(vec![
                (
                    "completeness".to_string(),
                    CanonicalValue::Bool(observed.complete),
                ),
                (
                    "roles".to_string(),
                    CanonicalValue::Sequence(
                        observed
                            .roles
                            .iter()
                            .map(|role| {
                                CanonicalValue::enum_variant(
                                    observed_role_id(*role),
                                    CanonicalValue::empty_record(),
                                )
                            })
                            .collect(),
                    ),
                ),
            ]),
        ),
        ComponentProvenance::LegacyUnpinned(legacy) => CanonicalValue::enum_variant(
            "legacy-unpinned",
            CanonicalValue::record(vec![(
                "recipe-id".to_string(),
                CanonicalValue::String(legacy.recipe_id.clone()),
            )]),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::artifacts::PROTON_SHA512;
    use crate::tools::runners::MANAGED_RUNNER_ID;
    use crate::tools::runtime::encode::fingerprint_digest;
    use crate::tools::runtime::fingerprint::managed_proton_prefix_fingerprint_input;
    use crate::tools::runtime::identity::{
        PrefixBinding, PrefixIdentityStatus, RuntimeEligibility,
    };
    use crate::tools::runtime::probe::probe_runner;
    use crate::tools::runtime::resolver::{
        profile_from_legacy, resolve_runtime, DgVoodooState, LegacyProfileInput,
        RuntimeResolutionInput,
    };
    use crate::utils::{PrefixLocation, PrefixScope, ResolvedRunner};
    use sha2::Digest;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    fn prefix_binding() -> super::super::identity::PrefixBinding {
        use crate::tools::runtime::fingerprint::compute_prefix_fingerprint;
        PrefixBinding {
            status: PrefixIdentityStatus::LegacyV2RunnerMatched,
            desired_fingerprint: compute_prefix_fingerprint(
                &managed_proton_prefix_fingerprint_input(),
            ),
            location: PrefixLocation {
                path: "/tmp/runtime-fp-prefix".to_string(),
                scope: PrefixScope::Isolated,
                managed: true,
                server_id: Some("fixture-server".to_string()),
            },
            eligibility: RuntimeEligibility::Eligible,
        }
    }

    fn wine716_fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../contract-fixtures/wine716-old-wow64-runner")
    }

    fn wine716_old_wow64_runner() -> (ResolvedRunner, super::super::probe::RunnerProbe) {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        let root = wine716_fixture_root();
        let wine_bin = root.join("bin/wine");
        let wineserver = root.join("bin/wineserver");
        if !wine_bin.is_file() {
            fs::create_dir_all(root.join("lib/wine/i386-unix")).unwrap();
            fs::create_dir_all(root.join("lib/wine/x86_64-unix")).unwrap();
            fs::write(root.join("lib/wine/i386-unix/libwine.so"), b"x").unwrap();
            fs::write(root.join("lib/wine/x86_64-unix/libwine.so"), b"x").unwrap();
            fs::create_dir_all(root.join("bin")).unwrap();
            fs::write(&wine_bin, "#!/bin/sh\necho 'wine-7.16'\n").unwrap();
            fs::write(&wineserver, "#!/bin/sh\nexit 0\n").unwrap();
            for path in [&wine_bin, &wineserver] {
                let mut permissions = fs::metadata(path).unwrap().permissions();
                permissions.set_mode(0o755);
                fs::set_permissions(path, permissions).unwrap();
            }
        }
        let runner = ResolvedRunner::test_wine(wine_bin, wineserver);
        let probe = probe_runner(&runner);
        (runner, probe)
    }

    const WINE716_FIXTURE_PREFIX_DIGEST: &str =
        "b16b4bb4d6bd3d66d01cf0da25f2056f3533c3464dc31067c93f67a52b92f4af";
    const MANAGED_PROTON_ENTRYPOINT_DIGEST: &str =
        "54af56937a136b38daf369c0d057ed741bfb08b0c3fd8a55f58e70d347e757c9";
    const MANAGED_UMU_ENTRYPOINT_DIGEST: &str =
        "0f7593c794b6c43b752f6e4c8ebbe6434a5786369e3f64ba8b40e6e467b761f6";

    fn wine716_prefix_binding() -> super::super::identity::PrefixBinding {
        use super::super::fingerprint::prefix_fingerprint_from_hex;
        use super::super::identity::PrefixBinding;

        // The product prefix fingerprint intentionally includes the canonical external-runner
        // location. A cross-machine runtime golden must pin that upstream identity instead of
        // deriving it from CARGO_MANIFEST_DIR (which differs between a workstation and CI).
        let desired = prefix_fingerprint_from_hex(WINE716_FIXTURE_PREFIX_DIGEST);
        PrefixBinding {
            status: PrefixIdentityStatus::LegacyV2RunnerMatched,
            desired_fingerprint: desired,
            location: PrefixLocation {
                path: "/tmp/runtime-fp-wine716-prefix".to_string(),
                scope: PrefixScope::Isolated,
                managed: true,
                server_id: Some("fixture-server".to_string()),
            },
            eligibility: RuntimeEligibility::Eligible,
        }
    }

    fn wine716_plan(dgvoodoo: bool) -> RuntimePlan {
        let (runner, probe) = wine716_old_wow64_runner();
        let profile = profile_from_legacy(LegacyProfileInput {
            server_runner: Some(runner.runner_path().to_string_lossy().as_ref()),
            default_runner: None,
            resolved: &runner,
            dgvoodoo: DgVoodooState::verified(dgvoodoo),
        })
        .unwrap();
        let binding = wine716_prefix_binding();
        let location = binding.location.clone();
        resolve_runtime(RuntimeResolutionInput {
            profile,
            resolved: &runner,
            prefix: location,
            prefix_binding: binding,
            probe,
            dgvoodoo: DgVoodooState::verified(dgvoodoo),
            webview2_required: false,
        })
        .unwrap()
    }

    fn managed_proton_plan(dgvoodoo: bool) -> RuntimePlan {
        let managed_proton = crate::utils::resolved_managed_proton_descriptor();
        let profile = profile_from_legacy(LegacyProfileInput {
            server_runner: None,
            default_runner: None,
            resolved: &managed_proton,
            dgvoodoo: DgVoodooState::verified(dgvoodoo),
        })
        .unwrap();
        let location = prefix_binding().location.clone();
        resolve_runtime(RuntimeResolutionInput {
            profile,
            resolved: &managed_proton,
            prefix: location,
            prefix_binding: prefix_binding(),
            probe: managed_proton_fixture_probe(),
            dgvoodoo: DgVoodooState::verified(dgvoodoo),
            webview2_required: false,
        })
        .unwrap()
    }

    fn managed_proton_fixture_probe() -> super::super::probe::RunnerProbe {
        use super::super::model::PayloadVerification;
        use super::super::probe::probe_managed_proton_descriptor;

        let mut probe = probe_managed_proton_descriptor(PayloadVerification::ShapeVerified);
        probe.identity.observed_material.roles = BTreeSet::from([
            ObservedMaterial {
                role: ObservedMaterialRole::Entrypoint,
                digest: Some(hex_to_bytes(MANAGED_PROTON_ENTRYPOINT_DIGEST)),
            },
            ObservedMaterial {
                role: ObservedMaterialRole::UmuEntrypoint,
                digest: Some(hex_to_bytes(MANAGED_UMU_ENTRYPOINT_DIGEST)),
            },
        ]);
        probe
    }

    #[test]
    fn overlay_changes_runtime_fingerprint_not_prefix_binding_digest() {
        let plain =
            runtime_fingerprint_input_from_plan(&managed_proton_plan(false), &prefix_binding());
        let overlay =
            runtime_fingerprint_input_from_plan(&managed_proton_plan(true), &prefix_binding());
        let fp_plain = compute_runtime_fingerprint_from_input(&plain);
        let fp_overlay = compute_runtime_fingerprint_from_input(&overlay);
        assert_ne!(fp_plain.digest.hex_digest(), fp_overlay.digest.hex_digest());
        assert_eq!(
            plain.desired_prefix_fingerprint.digest.hex_digest(),
            overlay.desired_prefix_fingerprint.digest.hex_digest()
        );
    }

    fn assert_fixture_digest(case: &str, digest: &str) {
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../contract-fixtures/runtime-fingerprint-v1.json");
        let fixture = std::fs::read_to_string(fixture_path).expect("fixture");
        let json: serde_json::Value = serde_json::from_str(&fixture).unwrap();
        let expected = json[case]["digestSha256Hex"]
            .as_str()
            .unwrap_or_else(|| panic!("missing fixture case {case}; digest={digest}"));
        assert_eq!(digest, expected);
    }

    #[test]
    fn wine716_managed_dxvk_runtime_fingerprint_matches_fixture() {
        let plan = wine716_plan(false);
        let binding = wine716_prefix_binding();
        let fp = compute_runtime_fingerprint_from_input(&runtime_fingerprint_input_from_plan(
            &plan, &binding,
        ));
        assert_fixture_digest("wine716ManagedDxvk", &fp.digest.hex_digest());
    }

    #[test]
    fn wine716_dgvoodoo_changes_runtime_fingerprint_not_prefix() {
        let plain = wine716_plan(false);
        let overlay = wine716_plan(true);
        let binding = wine716_prefix_binding();
        let fp_plain = compute_runtime_fingerprint_from_input(
            &runtime_fingerprint_input_from_plan(&plain, &binding),
        );
        let fp_overlay = compute_runtime_fingerprint_from_input(
            &runtime_fingerprint_input_from_plan(&overlay, &binding),
        );
        assert_ne!(fp_plain.digest.hex_digest(), fp_overlay.digest.hex_digest());
        assert_eq!(
            runtime_fingerprint_input_from_plan(&plain, &binding)
                .desired_prefix_fingerprint
                .digest
                .hex_digest(),
            runtime_fingerprint_input_from_plan(&overlay, &binding)
                .desired_prefix_fingerprint
                .digest
                .hex_digest()
        );
    }

    #[test]
    fn wine716_dgvoodoo_runtime_fingerprint_matches_fixture() {
        let plan = wine716_plan(true);
        let binding = wine716_prefix_binding();
        let fp = compute_runtime_fingerprint_from_input(&runtime_fingerprint_input_from_plan(
            &plan, &binding,
        ));
        assert_fixture_digest("wine716DgVoodooDxvk", &fp.digest.hex_digest());
    }

    #[test]
    fn managed_proton_runtime_fingerprint_matches_fixture() {
        let plan = managed_proton_plan(false);
        let binding = prefix_binding();
        let input = runtime_fingerprint_input_from_plan(&plan, &binding);
        let fp = compute_runtime_fingerprint_from_input(&input);
        assert_fixture_digest("managedProtonDxvk", &fp.digest.hex_digest());
    }

    const CANONICAL_FIELD_IDS: [&str; 19] = [
        "base-setup-recipe-revision",
        "desired-prefix-fingerprint",
        "dxvk",
        "environment-builder-revision",
        "graphics-profile",
        "invocation-policy-revision",
        "observed-prefix-architecture",
        "overlay",
        "overlay-config-digest",
        "prefix-binding-status",
        "repairable-dependency-identities",
        "runner-identity",
        "runner-kind",
        "runtime-plan-recipe-revision",
        "supported-prefix-architectures",
        "sync-plan",
        "umu-provenance",
        "webview2-required",
        "wow64-layout",
    ];

    fn fixture_json() -> serde_json::Value {
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../contract-fixtures/runtime-fingerprint-v1.json");
        serde_json::from_str(&std::fs::read_to_string(fixture_path).expect("fixture")).unwrap()
    }

    fn record_field_ids(value: &CanonicalValue) -> Vec<String> {
        match value {
            CanonicalValue::Record(fields) => fields.iter().map(|(id, _)| id.clone()).collect(),
            _ => Vec::new(),
        }
    }

    fn hex_to_bytes(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).expect("hex"))
            .collect()
    }

    fn sha256_file(path: &std::path::Path) -> Option<Vec<u8>> {
        let bytes = std::fs::read(path).ok()?;
        Some(sha2::Sha256::digest(bytes).to_vec())
    }

    fn observed_role(role: &str, digest: Option<Vec<u8>>) -> CanonicalValue {
        CanonicalValue::record(vec![
            (
                "digest".to_string(),
                digest
                    .map(CanonicalValue::Bytes)
                    .unwrap_or(CanonicalValue::Null),
            ),
            (
                "role".to_string(),
                CanonicalValue::enum_variant(role, CanonicalValue::empty_record()),
            ),
        ])
    }

    fn proton_observed_material() -> CanonicalValue {
        CanonicalValue::Sequence(vec![
            observed_role(
                "entrypoint",
                Some(hex_to_bytes(MANAGED_PROTON_ENTRYPOINT_DIGEST)),
            ),
            observed_role(
                "umu-entrypoint",
                Some(hex_to_bytes(MANAGED_UMU_ENTRYPOINT_DIGEST)),
            ),
        ])
    }

    fn managed_proton_artifact_identity_tree() -> CanonicalValue {
        CanonicalValue::record(vec![
            (
                "architectures".to_string(),
                CanonicalValue::Sequence(vec![CanonicalValue::enum_variant(
                    "x86-64",
                    CanonicalValue::empty_record(),
                )]),
            ),
            (
                "artifact-id".to_string(),
                CanonicalValue::String(MANAGED_RUNNER_ID.to_string()),
            ),
            (
                "install-recipe-revision".to_string(),
                CanonicalValue::U64(1),
            ),
            (
                "platform".to_string(),
                CanonicalValue::String("linux-x86_64".to_string()),
            ),
            ("schema-version".to_string(), CanonicalValue::U64(1)),
            (
                "source-digest".to_string(),
                CanonicalValue::record(vec![
                    (
                        "algorithm".to_string(),
                        CanonicalValue::String("sha512".to_string()),
                    ),
                    (
                        "bytes".to_string(),
                        CanonicalValue::Bytes(hex_to_bytes(PROTON_SHA512)),
                    ),
                ]),
            ),
        ])
    }

    fn wine_observed_material() -> CanonicalValue {
        let root = wine716_fixture_root();
        CanonicalValue::Sequence(vec![
            observed_role("entrypoint", sha256_file(&root.join("bin/wine"))),
            observed_role("wine-server", sha256_file(&root.join("bin/wineserver"))),
        ])
    }

    fn hand_tree_managed_proton() -> CanonicalValue {
        CanonicalValue::record(vec![
            (
                "base-setup-recipe-revision".to_string(),
                CanonicalValue::U64(1),
            ),
            (
                "desired-prefix-fingerprint".to_string(),
                CanonicalValue::Bytes(hex_to_bytes(
                    "a23f2a940a9cb8e5110b0e454ad68a35a22af760c005bcd0ebdf47ab44330270",
                )),
            ),
            (
                "dxvk".to_string(),
                CanonicalValue::enum_variant(
                    "runner-owned",
                    CanonicalValue::record(vec![(
                        "component-id".to_string(),
                        CanonicalValue::String("runner/dxvk".to_string()),
                    )]),
                ),
            ),
            (
                "environment-builder-revision".to_string(),
                CanonicalValue::U64(1),
            ),
            (
                "graphics-profile".to_string(),
                CanonicalValue::enum_variant("dxvk", CanonicalValue::empty_record()),
            ),
            (
                "invocation-policy-revision".to_string(),
                CanonicalValue::U64(1),
            ),
            (
                "observed-prefix-architecture".to_string(),
                CanonicalValue::enum_variant("unknown", CanonicalValue::empty_record()),
            ),
            (
                "overlay".to_string(),
                CanonicalValue::enum_variant("none", CanonicalValue::empty_record()),
            ),
            ("overlay-config-digest".to_string(), CanonicalValue::Null),
            (
                "prefix-binding-status".to_string(),
                CanonicalValue::enum_variant(
                    "legacy-v2-runner-matched",
                    CanonicalValue::empty_record(),
                ),
            ),
            (
                "repairable-dependency-identities".to_string(),
                CanonicalValue::Sequence(Vec::new()),
            ),
            (
                "runner-identity".to_string(),
                CanonicalValue::record(vec![
                    ("observed-material".to_string(), proton_observed_material()),
                    (
                        "provenance".to_string(),
                        CanonicalValue::enum_variant(
                            "artifact-receipt",
                            CanonicalValue::record(vec![(
                                "identity".to_string(),
                                managed_proton_artifact_identity_tree(),
                            )]),
                        ),
                    ),
                ]),
            ),
            (
                "runner-kind".to_string(),
                CanonicalValue::enum_variant("proton", CanonicalValue::empty_record()),
            ),
            (
                "runtime-plan-recipe-revision".to_string(),
                CanonicalValue::U64(1),
            ),
            (
                "supported-prefix-architectures".to_string(),
                CanonicalValue::enum_variant("unknown", CanonicalValue::empty_record()),
            ),
            (
                "sync-plan".to_string(),
                CanonicalValue::enum_variant("runner-managed", CanonicalValue::empty_record()),
            ),
            (
                "umu-provenance".to_string(),
                CanonicalValue::enum_variant("runner-managed", CanonicalValue::empty_record()),
            ),
            ("webview2-required".to_string(), CanonicalValue::Bool(false)),
            (
                "wow64-layout".to_string(),
                CanonicalValue::enum_variant("unknown", CanonicalValue::empty_record()),
            ),
        ])
    }

    fn hand_tree_wine716(overlay: bool) -> CanonicalValue {
        let prefix = wine716_prefix_binding()
            .desired_fingerprint
            .digest
            .digest
            .to_vec();
        let overlay_value = if overlay {
            CanonicalValue::enum_variant(
                "dgvoodoo",
                CanonicalValue::record(vec![(
                    "component-id".to_string(),
                    CanonicalValue::String("game-dir/dgvoodoo".to_string()),
                )]),
            )
        } else {
            CanonicalValue::enum_variant("none", CanonicalValue::empty_record())
        };
        let graphics = if overlay { "dgvoodoo-dxvk" } else { "dxvk" };
        CanonicalValue::record(vec![
            (
                "base-setup-recipe-revision".to_string(),
                CanonicalValue::U64(1),
            ),
            (
                "desired-prefix-fingerprint".to_string(),
                CanonicalValue::Bytes(prefix),
            ),
            (
                "dxvk".to_string(),
                CanonicalValue::enum_variant(
                    "managed-prefix",
                    CanonicalValue::record(vec![(
                        "artifact-id".to_string(),
                        CanonicalValue::String("dxvk-2.6.2".to_string()),
                    )]),
                ),
            ),
            (
                "environment-builder-revision".to_string(),
                CanonicalValue::U64(1),
            ),
            (
                "graphics-profile".to_string(),
                CanonicalValue::enum_variant(graphics, CanonicalValue::empty_record()),
            ),
            (
                "invocation-policy-revision".to_string(),
                CanonicalValue::U64(1),
            ),
            (
                "observed-prefix-architecture".to_string(),
                CanonicalValue::enum_variant("unknown", CanonicalValue::empty_record()),
            ),
            ("overlay".to_string(), overlay_value),
            ("overlay-config-digest".to_string(), CanonicalValue::Null),
            (
                "prefix-binding-status".to_string(),
                CanonicalValue::enum_variant(
                    "legacy-v2-runner-matched",
                    CanonicalValue::empty_record(),
                ),
            ),
            (
                "repairable-dependency-identities".to_string(),
                CanonicalValue::Sequence(Vec::new()),
            ),
            (
                "runner-identity".to_string(),
                CanonicalValue::record(vec![
                    ("observed-material".to_string(), wine_observed_material()),
                    (
                        "provenance".to_string(),
                        CanonicalValue::enum_variant(
                            "external-observed",
                            CanonicalValue::record(vec![
                                ("completeness".to_string(), CanonicalValue::Bool(false)),
                                (
                                    "roles".to_string(),
                                    CanonicalValue::Sequence(vec![
                                        CanonicalValue::enum_variant(
                                            "entrypoint",
                                            CanonicalValue::empty_record(),
                                        ),
                                        CanonicalValue::enum_variant(
                                            "wine-server",
                                            CanonicalValue::empty_record(),
                                        ),
                                    ]),
                                ),
                            ]),
                        ),
                    ),
                ]),
            ),
            (
                "runner-kind".to_string(),
                CanonicalValue::enum_variant("wine", CanonicalValue::empty_record()),
            ),
            (
                "runtime-plan-recipe-revision".to_string(),
                CanonicalValue::U64(1),
            ),
            (
                "supported-prefix-architectures".to_string(),
                CanonicalValue::enum_variant(
                    "known",
                    CanonicalValue::Sequence(vec![
                        CanonicalValue::enum_variant("x86", CanonicalValue::empty_record()),
                        CanonicalValue::enum_variant("x86-64", CanonicalValue::empty_record()),
                    ]),
                ),
            ),
            (
                "sync-plan".to_string(),
                CanonicalValue::enum_variant("wine-server", CanonicalValue::empty_record()),
            ),
            ("umu-provenance".to_string(), CanonicalValue::Null),
            ("webview2-required".to_string(), CanonicalValue::Bool(false)),
            (
                "wow64-layout".to_string(),
                CanonicalValue::enum_variant("old-wow64", CanonicalValue::empty_record()),
            ),
        ])
    }

    fn assert_hand_tree(case: &str, tree: CanonicalValue) {
        let json = fixture_json();
        let expected_ids = json["canonicalFieldIds"]
            .as_array()
            .expect("canonicalFieldIds")
            .iter()
            .map(|value| value.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(expected_ids, CANONICAL_FIELD_IDS.map(str::to_string));
        assert_eq!(record_field_ids(&tree), expected_ids);
        let digest = fingerprint_digest(RUNTIME_FINGERPRINT_DOMAIN, &tree);
        let expected = json[case]["digestSha256Hex"].as_str().expect("digest");
        assert_eq!(
            FingerprintDigest {
                schema_version: FINGERPRINT_SCHEMA_VERSION,
                algorithm: "sha256".to_string(),
                digest,
            }
            .hex_digest(),
            expected
        );
    }

    #[test]
    fn managed_proton_hand_canonical_tree_matches_fixture() {
        assert_hand_tree("managedProtonDxvk", hand_tree_managed_proton());
    }

    #[test]
    fn wine716_hand_canonical_tree_matches_fixture() {
        assert_hand_tree("wine716ManagedDxvk", hand_tree_wine716(false));
    }

    #[test]
    fn wine716_dgvoodoo_hand_canonical_tree_matches_fixture() {
        assert_hand_tree("wine716DgVoodooDxvk", hand_tree_wine716(true));
    }
}
