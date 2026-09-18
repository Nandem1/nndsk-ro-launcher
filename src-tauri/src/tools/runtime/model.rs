use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::utils::{PrefixLocation, ResolvedRunner, RunnerKind};

pub(crate) const PREFIX_MATERIAL_RECIPE_REVISION: u64 = 1;
pub(crate) const RUNTIME_PLAN_RECIPE_REVISION: u64 = 1;
pub(crate) const BASE_SETUP_RECIPE_REVISION: u64 = 1;
pub(crate) const INVOCATION_POLICY_REVISION: u64 = 1;
pub(crate) const ENVIRONMENT_BUILDER_REVISION: u64 = 1;
pub(crate) const ARTIFACT_IDENTITY_SCHEMA_VERSION: u64 = 1;
pub(crate) const FINGERPRINT_SCHEMA_VERSION: u64 = 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ArtifactId(String);

impl ArtifactId {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, &'static str> {
        let value = value.into();
        if valid_stable_id(&value) {
            Ok(Self(value))
        } else {
            Err("invalid-artifact-id")
        }
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

fn valid_stable_id(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars.all(|ch| {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '.' | '_' | '/' | '-')
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RunnerRequest {
    Managed {
        artifact_id: ArtifactId,
    },
    External {
        kind_hint: RunnerKind,
        entrypoint: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RunnerSelectionSource {
    ServerOverride,
    GlobalSetting,
    ProductDefault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GraphicsProfile {
    Dxvk,
    DgVoodooDxvk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeProfile {
    runner: RunnerRequest,
    selection_source: RunnerSelectionSource,
    graphics: GraphicsProfile,
}

impl RuntimeProfile {
    pub(super) fn new(
        runner: RunnerRequest,
        selection_source: RunnerSelectionSource,
        graphics: GraphicsProfile,
    ) -> Self {
        Self {
            runner,
            selection_source,
            graphics,
        }
    }

    pub(super) fn runner(&self) -> &RunnerRequest {
        &self.runner
    }

    pub(super) fn selection_source(&self) -> RunnerSelectionSource {
        self.selection_source
    }

    pub(super) fn graphics(&self) -> GraphicsProfile {
        self.graphics
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PayloadVerification {
    ShapeVerified,
    Unverified,
    SourceAndPayloadVerified,
    Corrupt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceDigest {
    pub(crate) algorithm: DigestAlgorithm,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DigestAlgorithm {
    Sha256,
    Sha512,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactIdentity {
    pub(crate) schema_version: u64,
    pub(crate) artifact_id: ArtifactId,
    pub(crate) source_digest: SourceDigest,
    pub(crate) platform: String,
    pub(crate) architectures: BTreeSet<ArtifactArchitecture>,
    pub(crate) install_recipe_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ArtifactArchitecture {
    X86,
    X86_64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FingerprintDigest {
    pub(crate) schema_version: u64,
    pub(crate) algorithm: String,
    pub(crate) digest: [u8; 32],
}

impl FingerprintDigest {
    pub(crate) fn hex_digest(&self) -> String {
        self.digest
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ArtifactReceipt {
    pub(super) identity: ArtifactIdentity,
    pub(super) payload_verification: PayloadVerification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ObservedMaterialRole {
    Entrypoint,
    WineServer,
    ProtonInnerWine,
    ProtonVersion,
    UmuEntrypoint,
    WineTkgConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ObservedMaterial {
    pub(crate) role: ObservedMaterialRole,
    pub(crate) digest: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExternalObserved {
    pub(super) roles: BTreeSet<ObservedMaterialRole>,
    pub(super) complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LegacyUnpinned {
    pub(super) recipe_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ComponentProvenance {
    ArtifactReceipt(ArtifactReceipt),
    BundledResource { resource_id: String },
    ExternalObserved(ExternalObserved),
    LegacyUnpinned(LegacyUnpinned),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ObservedRunnerMaterial {
    pub(super) roles: BTreeSet<ObservedMaterial>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RunnerIdentity {
    pub(super) kind: RunnerKind,
    pub(super) provenance: ComponentProvenance,
    pub(super) observed_material: ObservedRunnerMaterial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CapabilitySource {
    ProcessOutput,
    VersionFile,
    StructuralProbeV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UnknownCapabilityReason {
    NotReported,
    LayoutNotProven,
    DescriptorDoesNotDeclare,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CapabilityEvidence<T> {
    Known { value: T, source: CapabilitySource },
    Unknown { reason: UnknownCapabilityReason },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum PrefixArchitecture {
    X86,
    X86_64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "positive receipt evidence for these layouts starts after phase 1"
)]
pub(super) enum Wow64Layout {
    OldWow64,
    NewWow64,
    Wine32Only,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SyncSupport {
    Wine {
        esync_declared: bool,
        fsync_patch_pair_declared: bool,
    },
    RunnerManaged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SyncPlan {
    WineServer,
    Esync,
    Fsync,
    RunnerManaged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RunnerCapabilities {
    pub(super) reported_version: CapabilityEvidence<String>,
    pub(super) wow64_layout: CapabilityEvidence<Wow64Layout>,
    pub(super) supported_prefix_architectures: CapabilityEvidence<BTreeSet<PrefixArchitecture>>,
    pub(super) sync_support: SyncSupport,
}

#[derive(Debug, Clone)]
pub(super) struct RunnerPlan {
    pub(super) resolved: ResolvedRunner,
    pub(super) identity: RunnerIdentity,
    pub(super) capabilities: RunnerCapabilities,
    pub(super) sync: SyncPlan,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ComponentId(String);

impl ComponentId {
    pub(super) fn new(value: impl Into<String>) -> Result<Self, &'static str> {
        let value = value.into();
        if valid_stable_id(&value) {
            Ok(Self(value))
        } else {
            Err("invalid-component-id")
        }
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DxvkProviderKind {
    RunnerOwned { component: ComponentId },
    ManagedPrefix { artifact_id: ArtifactId },
    WinetricksPrefix { provenance: ComponentProvenance },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DxvkProvider {
    kind: DxvkProviderKind,
}

impl DxvkProvider {
    pub(super) fn runner_owned(component: ComponentId) -> Self {
        Self {
            kind: DxvkProviderKind::RunnerOwned { component },
        }
    }

    pub(super) fn managed_prefix(artifact_id: ArtifactId) -> Self {
        Self {
            kind: DxvkProviderKind::ManagedPrefix { artifact_id },
        }
    }

    pub(super) fn winetricks_prefix(provenance: ComponentProvenance) -> Self {
        Self {
            kind: DxvkProviderKind::WinetricksPrefix { provenance },
        }
    }

    pub(super) fn kind_label(&self) -> &'static str {
        match self.kind {
            DxvkProviderKind::RunnerOwned { .. } => "runner-owned",
            DxvkProviderKind::ManagedPrefix { .. } => "managed-prefix",
            DxvkProviderKind::WinetricksPrefix { .. } => "winetricks-prefix",
        }
    }

    pub(super) fn component_id(&self) -> &str {
        match &self.kind {
            DxvkProviderKind::RunnerOwned { component } => component.as_str(),
            DxvkProviderKind::ManagedPrefix { artifact_id } => artifact_id.as_str(),
            DxvkProviderKind::WinetricksPrefix { provenance } => match provenance {
                ComponentProvenance::LegacyUnpinned(value) => &value.recipe_id,
                _ => "invalid-provenance",
            },
        }
    }

    pub(super) fn is_managed_prefix(&self) -> bool {
        matches!(self.kind, DxvkProviderKind::ManagedPrefix { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DgVoodooOverlayPlan {
    pub(super) provenance: ComponentProvenance,
    pub(super) component_id: ComponentId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum GraphicsPlanKind {
    Dxvk,
    DgVoodooDxvk { overlay: DgVoodooOverlayPlan },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GraphicsPlan {
    kind: GraphicsPlanKind,
    dxvk: DxvkProvider,
}

impl GraphicsPlan {
    pub(super) fn dxvk(dxvk: DxvkProvider) -> Self {
        Self {
            kind: GraphicsPlanKind::Dxvk,
            dxvk,
        }
    }

    pub(super) fn dgvoodoo_dxvk(overlay: DgVoodooOverlayPlan, dxvk: DxvkProvider) -> Self {
        Self {
            kind: GraphicsPlanKind::DgVoodooDxvk { overlay },
            dxvk,
        }
    }

    pub(super) fn profile(&self) -> GraphicsProfile {
        match self.kind {
            GraphicsPlanKind::Dxvk => GraphicsProfile::Dxvk,
            GraphicsPlanKind::DgVoodooDxvk { .. } => GraphicsProfile::DgVoodooDxvk,
        }
    }

    pub(super) fn dxvk_provider(&self) -> &DxvkProvider {
        &self.dxvk
    }

    pub(super) fn overlay(&self) -> Option<&DgVoodooOverlayPlan> {
        match &self.kind {
            GraphicsPlanKind::Dxvk => None,
            GraphicsPlanKind::DgVoodooDxvk { overlay } => Some(overlay),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum InvocationTarget {
    Game,
    LaunchPatcher,
    MaintenancePatcher,
    OpenSetup,
    GraphicsControlPanel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum BaseRecipeStep {
    Gecko,
    Vcrun2019,
    D3dx9,
    Corefonts,
    FontFallback,
    Audio,
}

impl BaseRecipeStep {
    pub(super) const LEGACY: [Self; 6] = [
        Self::Gecko,
        Self::Vcrun2019,
        Self::D3dx9,
        Self::Corefonts,
        Self::FontFallback,
        Self::Audio,
    ];
}

impl InvocationTarget {
    pub(super) const ALL: [Self; 5] = [
        Self::Game,
        Self::LaunchPatcher,
        Self::MaintenancePatcher,
        Self::OpenSetup,
        Self::GraphicsControlPanel,
    ];

    pub(super) fn receives_overlay(self) -> bool {
        !matches!(self, Self::MaintenancePatcher)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimePlan {
    pub(super) runner: RunnerPlan,
    pub(super) graphics: GraphicsPlan,
    pub(super) prefix: PrefixLocation,
    pub(super) webview2_required: bool,
    pub(super) prefix_binding: super::identity::PrefixBinding,
}

impl RuntimePlan {
    pub(super) fn base_recipe(&self) -> &'static [BaseRecipeStep] {
        &BaseRecipeStep::LEGACY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_ids_reject_display_text_and_syntax() {
        assert!(ArtifactId::new("dxvk-2.6.2").is_ok());
        assert!(ArtifactId::new("DXVK 2.6.2").is_err());
        assert!(ComponentId::new("game-dir/dgvoodoo").is_ok());
        assert!(ComponentId::new("../escape?").is_err());
    }
}
