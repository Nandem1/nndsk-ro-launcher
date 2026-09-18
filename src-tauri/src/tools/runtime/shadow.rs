use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::path::Path;

use sha2::{Digest, Sha256};
use tauri::AppHandle;

use crate::tools::prefix::DxvkProvision;
use crate::tools::server_tools::GepardRunnerProfile;
use crate::utils::{apply_game_env, emit_tool_log_opt, ProcessEnv, WineContext, WineSyncMode};

use super::environment::{
    ComponentOwner, DllLoadOrder, EnvironmentChange, GraphicsEnvironment, OwnershipDomain,
};
use super::fingerprint::{compute_prefix_fingerprint, managed_proton_prefix_fingerprint_input};
use super::identity::{PrefixBinding, PrefixIdentityStatus, RuntimeEligibility};
use super::model::{
    BaseRecipeStep, ComponentProvenance, GraphicsProfile, InvocationTarget, RunnerRequest,
    RunnerSelectionSource, RuntimePlan, RuntimeProfile, SyncPlan,
};
use super::probe::probe_runner;
use super::resolver::{
    profile_from_legacy, resolve_runtime, DgVoodooState, LegacyProfileInput, RuntimeResolutionInput,
};

fn shadow_prefix_binding(location: crate::utils::PrefixLocation) -> PrefixBinding {
    PrefixBinding {
        status: PrefixIdentityStatus::Unknown,
        desired_fingerprint: compute_prefix_fingerprint(&managed_proton_prefix_fingerprint_input()),
        location,
        eligibility: RuntimeEligibility::Eligible,
    }
}

const MANAGED_DXVK_COMPONENT: &str = "dxvk-2.6.2";
const RUNNER_DXVK_COMPONENT: &str = "runner/dxvk";
const WINETRICKS_DXVK_COMPONENT: &str = "winetricks/dxvk-legacy-unpinned";
const DGVOODOO_COMPONENT: &str = "game-dir/dgvoodoo";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShadowOperation {
    Launch,
    PrefixSetup,
    PrefixReset,
    DependencyCheck,
    ServerTool,
}

impl ShadowOperation {
    fn label(self) -> &'static str {
        match self {
            Self::Launch => "launch",
            Self::PrefixSetup => "prefix-setup",
            Self::PrefixReset => "prefix-reset",
            Self::DependencyCheck => "dependency-check",
            Self::ServerTool => "server-tool",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DgVoodooObservation {
    Verified { configured: bool },
    Unavailable,
}

impl DgVoodooObservation {
    pub(crate) fn verified(configured: bool) -> Self {
        Self::Verified { configured }
    }
}

pub(crate) struct LegacyRuntimeInput<'a> {
    pub(crate) server_runner: Option<&'a str>,
    pub(crate) default_runner: Option<&'a str>,
    pub(crate) context: &'a WineContext,
    pub(crate) dxvk: DxvkProvision,
    pub(crate) dgvoodoo: DgVoodooObservation,
    pub(crate) webview2_required: bool,
    pub(crate) recommendation: Option<GepardRunnerProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ShadowOutcome {
    Disabled,
    Match,
    Diverged(Vec<&'static str>),
    Failed(&'static str),
}

pub(crate) fn runtime_shadow_enabled() -> bool {
    runtime_shadow_enabled_from(std::env::var("RO_LAUNCHER_RUNTIME_SHADOW").ok().as_deref())
}

fn runtime_shadow_enabled_from(value: Option<&str>) -> bool {
    value != Some("0")
}

pub(crate) fn observe_legacy_runtime(
    app: Option<&AppHandle>,
    operation: ShadowOperation,
    input: LegacyRuntimeInput<'_>,
) -> ShadowOutcome {
    if !runtime_shadow_enabled() {
        return ShadowOutcome::Disabled;
    }

    let dgvoodoo = match input.dgvoodoo {
        DgVoodooObservation::Verified { configured } => DgVoodooState::verified(configured),
        DgVoodooObservation::Unavailable => {
            let reason = "dgvoodoo-observation-unavailable";
            emit_shadow_error(app, operation, input.context, reason);
            return ShadowOutcome::Failed(reason);
        }
    };
    let profile = match profile_from_legacy(LegacyProfileInput {
        server_runner: input.server_runner,
        default_runner: input.default_runner,
        resolved: &input.context.resolved,
        dgvoodoo,
    }) {
        Ok(profile) => profile,
        Err(error) => {
            let reason = error.code();
            emit_shadow_error(app, operation, input.context, reason);
            return ShadowOutcome::Failed(reason);
        }
    };
    let legacy = match capture_legacy_runtime(&input, &profile) {
        Ok(legacy) => legacy,
        Err(reason) => {
            emit_shadow_error(app, operation, input.context, reason);
            return ShadowOutcome::Failed(reason);
        }
    };
    let prefix = input.context.location.clone();
    let plan = match resolve_runtime(RuntimeResolutionInput {
        profile: profile.clone(),
        resolved: &input.context.resolved,
        prefix: prefix.clone(),
        prefix_binding: shadow_prefix_binding(prefix),
        probe: probe_runner(&input.context.resolved),
        dgvoodoo,
        webview2_required: input.webview2_required,
    }) {
        Ok(plan) => plan,
        Err(error) => {
            let reason = error.code();
            emit_shadow_error(app, operation, input.context, reason);
            return ShadowOutcome::Failed(reason);
        }
    };

    let comparison = compare_shadow(&legacy, &profile, &plan);
    if comparison.mismatches.is_empty() {
        ShadowOutcome::Match
    } else {
        let categories = comparison
            .mismatches
            .iter()
            .map(|mismatch| mismatch.category.label())
            .collect::<Vec<_>>();
        emit_tool_log_opt(
            app,
            format_shadow_divergence(operation, input.context, &categories),
        );
        ShadowOutcome::Diverged(categories)
    }
}

fn emit_shadow_error(
    app: Option<&AppHandle>,
    operation: ShadowOperation,
    context: &WineContext,
    reason: &'static str,
) {
    emit_tool_log_opt(
        app,
        format!(
            "[RuntimeShadow:{}] divergence=shadow-resolution-error reason={reason} runner={} prefix={}",
            operation.label(),
            path_token(context.resolved.runner_path()),
            path_token(Path::new(&context.prefix))
        ),
    );
}

fn format_shadow_divergence(
    operation: ShadowOperation,
    context: &WineContext,
    categories: &[&str],
) -> String {
    format!(
        "[RuntimeShadow:{}] divergence={} runner={} prefix={}",
        operation.label(),
        categories.join(","),
        path_token(context.resolved.runner_path()),
        path_token(Path::new(&context.prefix))
    )
}

fn path_token(path: &Path) -> String {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let digest = Sha256::digest(canonical.as_os_str().as_encoded_bytes());
    format!("{:016x}", digest)[..16].to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrefixSnapshot {
    scope: crate::utils::PrefixScope,
    managed: bool,
    server_token: Option<String>,
    path_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProviderSnapshot {
    kind: &'static str,
    component_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedDllClaim {
    load_order: DllLoadOrder,
    owner_domain: OwnershipDomain,
    component_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedEnvironment {
    dll_claims: BTreeMap<String, NormalizedDllClaim>,
    variables: BTreeMap<String, EnvironmentChange>,
}

#[derive(Debug, Clone)]
struct LegacyRuntimeSnapshot {
    runner_kind: crate::utils::RunnerKind,
    runner_token: String,
    selection_source: RunnerSelectionSource,
    sync: SyncPlan,
    prefix: PrefixSnapshot,
    dxvk: ProviderSnapshot,
    dgvoodoo_verified: bool,
    environments: BTreeMap<InvocationTarget, NormalizedEnvironment>,
    webview2_required: bool,
    base_recipe: Vec<BaseRecipeStep>,
    recommendation: Option<GepardRunnerProfile>,
}

fn capture_legacy_runtime(
    input: &LegacyRuntimeInput<'_>,
    profile: &RuntimeProfile,
) -> Result<LegacyRuntimeSnapshot, &'static str> {
    let dgvoodoo_verified = matches!(
        input.dgvoodoo,
        DgVoodooObservation::Verified { configured: true }
    );
    let mut environments = BTreeMap::new();
    for target in InvocationTarget::ALL {
        let use_dgvoodoo = dgvoodoo_verified && target.receives_overlay();
        environments.insert(
            target,
            capture_legacy_environment(
                use_dgvoodoo,
                input.dxvk == DxvkProvision::Managed,
                &input.context.prefix,
            )?,
        );
    }

    Ok(LegacyRuntimeSnapshot {
        runner_kind: input.context.resolved.kind(),
        runner_token: path_token(input.context.resolved.runner_path()),
        selection_source: profile.selection_source(),
        sync: match input.context.resolved.wine_sync_mode() {
            WineSyncMode::WineServer if input.context.resolved.is_proton() => {
                SyncPlan::RunnerManaged
            }
            WineSyncMode::WineServer => SyncPlan::WineServer,
            WineSyncMode::Esync => SyncPlan::Esync,
            WineSyncMode::Fsync => SyncPlan::Fsync,
        },
        prefix: prefix_snapshot(input.context),
        dxvk: legacy_provider(input.dxvk),
        dgvoodoo_verified,
        environments,
        webview2_required: input.webview2_required,
        base_recipe: BaseRecipeStep::LEGACY.to_vec(),
        recommendation: input.recommendation,
    })
}

fn prefix_snapshot(context: &WineContext) -> PrefixSnapshot {
    PrefixSnapshot {
        scope: context.location.scope,
        managed: context.location.managed,
        server_token: context.location.server_id.as_deref().map(value_token),
        path_token: path_token(Path::new(&context.location.path)),
    }
}

fn value_token(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    format!("{:016x}", digest)[..16].to_string()
}

fn legacy_provider(provision: DxvkProvision) -> ProviderSnapshot {
    match provision {
        DxvkProvision::Runner => ProviderSnapshot {
            kind: "runner-owned",
            component_id: RUNNER_DXVK_COMPONENT.to_string(),
        },
        DxvkProvision::Managed => ProviderSnapshot {
            kind: "managed-prefix",
            component_id: MANAGED_DXVK_COMPONENT.to_string(),
        },
        DxvkProvision::Winetricks => ProviderSnapshot {
            kind: "winetricks-prefix",
            component_id: WINETRICKS_DXVK_COMPONENT.to_string(),
        },
    }
}

#[derive(Default)]
struct LegacyEnvironmentCapture {
    changes: BTreeMap<String, EnvironmentChange>,
}

impl ProcessEnv for LegacyEnvironmentCapture {
    fn set_env(&mut self, key: impl AsRef<OsStr>, val: impl AsRef<OsStr>) {
        self.changes.insert(
            key.as_ref().to_string_lossy().to_string(),
            EnvironmentChange::Set(val.as_ref().to_os_string()),
        );
    }

    fn unset_env(&mut self, key: impl AsRef<OsStr>) {
        self.changes.insert(
            key.as_ref().to_string_lossy().to_string(),
            EnvironmentChange::Unset,
        );
    }
}

fn capture_legacy_environment(
    use_dgvoodoo: bool,
    use_managed_dxvk: bool,
    prefix: &str,
) -> Result<NormalizedEnvironment, &'static str> {
    let mut capture = LegacyEnvironmentCapture::default();
    apply_game_env(&mut capture, use_dgvoodoo, use_managed_dxvk, prefix);
    let overrides = capture.changes.remove("WINEDLLOVERRIDES");
    let dll_claims = match overrides {
        None => BTreeMap::new(),
        Some(EnvironmentChange::Set(value)) => parse_legacy_overrides(&value)?,
        Some(EnvironmentChange::Unset) => return Err("legacy-dll-overrides-unset"),
    };
    Ok(NormalizedEnvironment {
        dll_claims,
        variables: capture.changes,
    })
}

fn parse_legacy_overrides(
    overrides: &OsString,
) -> Result<BTreeMap<String, NormalizedDllClaim>, &'static str> {
    let overrides = overrides.to_str().ok_or("legacy-dll-overrides-non-utf8")?;
    let mut claims = BTreeMap::new();
    for entry in overrides.split(';') {
        let (name, load_order) = entry
            .split_once('=')
            .ok_or("legacy-dll-overrides-invalid")?;
        if load_order != "n,b" {
            return Err("legacy-dll-load-order-unsupported");
        }
        let name = format!("{}.dll", name.to_ascii_lowercase());
        let (owner_domain, component_id) = if matches!(name.as_str(), "d3dimm.dll" | "ddraw.dll") {
            (OwnershipDomain::GameDirOverlay, DGVOODOO_COMPONENT)
        } else {
            (OwnershipDomain::Prefix, MANAGED_DXVK_COMPONENT)
        };
        claims.insert(
            name,
            NormalizedDllClaim {
                load_order: DllLoadOrder::NativeThenBuiltin,
                owner_domain,
                component_id: component_id.to_string(),
            },
        );
    }
    Ok(claims)
}

fn normalize_plan_environment(environment: &GraphicsEnvironment) -> NormalizedEnvironment {
    let rendered_overrides = environment.dll_overrides.render_wine();
    debug_assert_eq!(
        rendered_overrides
            .split(';')
            .filter(|entry| !entry.is_empty())
            .count(),
        environment.dll_overrides.claims().len()
    );
    let dll_claims = environment
        .dll_overrides
        .claims()
        .values()
        .map(|claim| {
            (
                claim.name.as_str().to_string(),
                normalize_owner(claim.load_order, &claim.owner),
            )
        })
        .collect();
    let variables = environment
        .variables
        .changes()
        .iter()
        .map(|(key, contribution)| (key.clone(), contribution.change.clone()))
        .collect();
    NormalizedEnvironment {
        dll_claims,
        variables,
    }
}

fn normalize_owner(load_order: DllLoadOrder, owner: &ComponentOwner) -> NormalizedDllClaim {
    NormalizedDllClaim {
        load_order,
        owner_domain: owner.domain(),
        component_id: owner.component_id().to_string(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ShadowMismatchCategory {
    RunnerIdentity,
    SelectionSource,
    SyncPlan,
    PrefixBinding,
    DxvkProvider,
    DgVoodooState,
    TargetPolicy,
    DllOverrides,
    EnvironmentDelta,
    WebView2Requirement,
    BaseRecipe,
    LegacyRecommendation,
    ShadowResolutionError,
}

impl ShadowMismatchCategory {
    fn label(self) -> &'static str {
        match self {
            Self::RunnerIdentity => "runner-identity",
            Self::SelectionSource => "selection-source",
            Self::SyncPlan => "sync-plan",
            Self::PrefixBinding => "prefix-binding",
            Self::DxvkProvider => "dxvk-provider",
            Self::DgVoodooState => "dgvoodoo-state",
            Self::TargetPolicy => "target-policy",
            Self::DllOverrides => "dll-overrides",
            Self::EnvironmentDelta => "environment-delta",
            Self::WebView2Requirement => "webview2-requirement",
            Self::BaseRecipe => "base-recipe",
            Self::LegacyRecommendation => "legacy-recommendation",
            Self::ShadowResolutionError => "shadow-resolution-error",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShadowMismatch {
    category: ShadowMismatchCategory,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ShadowComparison {
    mismatches: Vec<ShadowMismatch>,
}

fn compare_shadow(
    legacy: &LegacyRuntimeSnapshot,
    profile: &RuntimeProfile,
    plan: &RuntimePlan,
) -> ShadowComparison {
    let mut categories = BTreeSet::new();
    if legacy.runner_kind != plan.runner.resolved.kind()
        || legacy.runner_token != path_token(plan.runner.resolved.runner_path())
        || plan.runner.identity.kind != legacy.runner_kind
    {
        categories.insert(ShadowMismatchCategory::RunnerIdentity);
    }
    let expected_runner_owner = match profile.runner() {
        RunnerRequest::Managed { .. } => OwnershipDomain::Runner,
        RunnerRequest::External { .. } => OwnershipDomain::HostExternal,
    };
    let planned_runner_owner = match &plan.runner.identity.provenance {
        ComponentProvenance::ArtifactReceipt(_) => OwnershipDomain::Runner,
        ComponentProvenance::ExternalObserved(_) => OwnershipDomain::HostExternal,
        ComponentProvenance::BundledResource { .. } | ComponentProvenance::LegacyUnpinned(_) => {
            OwnershipDomain::HostExternal
        }
    };
    if expected_runner_owner != planned_runner_owner {
        categories.insert(ShadowMismatchCategory::RunnerIdentity);
    }
    if legacy.selection_source != profile.selection_source() {
        categories.insert(ShadowMismatchCategory::SelectionSource);
    }
    if legacy.sync != plan.runner.sync {
        categories.insert(ShadowMismatchCategory::SyncPlan);
    }
    let planned_prefix = PrefixSnapshot {
        scope: plan.prefix.scope,
        managed: plan.prefix.managed,
        server_token: plan.prefix.server_id.as_deref().map(value_token),
        path_token: path_token(Path::new(&plan.prefix.path)),
    };
    if legacy.prefix != planned_prefix {
        categories.insert(ShadowMismatchCategory::PrefixBinding);
    }
    let planned_provider = ProviderSnapshot {
        kind: plan.graphics.dxvk_provider().kind_label(),
        component_id: plan.graphics.dxvk_provider().component_id().to_string(),
    };
    if legacy.dxvk != planned_provider {
        categories.insert(ShadowMismatchCategory::DxvkProvider);
    }
    let planned_dgvoodoo = plan.graphics.profile() == GraphicsProfile::DgVoodooDxvk;
    if legacy.dgvoodoo_verified != planned_dgvoodoo {
        categories.insert(ShadowMismatchCategory::DgVoodooState);
    }

    for target in InvocationTarget::ALL {
        let Some(legacy_environment) = legacy.environments.get(&target) else {
            categories.insert(ShadowMismatchCategory::TargetPolicy);
            continue;
        };
        let planned_environment = match plan.graphics.environment_for(target, &plan.prefix) {
            Ok(environment) => normalize_plan_environment(&environment),
            Err(_) => {
                categories.insert(ShadowMismatchCategory::ShadowResolutionError);
                continue;
            }
        };
        let legacy_overlay = has_overlay_claims(legacy_environment);
        let planned_overlay = has_overlay_claims(&planned_environment);
        let legacy_managed = has_managed_dxvk_claims(legacy_environment);
        let planned_managed = has_managed_dxvk_claims(&planned_environment);
        if legacy_overlay != planned_overlay || legacy_managed != planned_managed {
            categories.insert(ShadowMismatchCategory::TargetPolicy);
        }
        if legacy_environment.dll_claims != planned_environment.dll_claims {
            categories.insert(ShadowMismatchCategory::DllOverrides);
        }
        if legacy_environment.variables != planned_environment.variables {
            categories.insert(ShadowMismatchCategory::EnvironmentDelta);
        }
    }
    if legacy.webview2_required != plan.webview2_required {
        categories.insert(ShadowMismatchCategory::WebView2Requirement);
    }
    if legacy.base_recipe.as_slice() != plan.base_recipe() {
        categories.insert(ShadowMismatchCategory::BaseRecipe);
    }

    // Recommendations are deliberately absent from RuntimeProfile/RuntimePlan. Reaching this point
    // with the same runner identity proves that even a recommendation for another family did not
    // replace the explicit/default legacy selection.
    if legacy.recommendation.is_some()
        && (legacy.runner_kind != plan.runner.resolved.kind()
            || legacy.runner_token != path_token(plan.runner.resolved.runner_path()))
    {
        categories.insert(ShadowMismatchCategory::LegacyRecommendation);
    }

    ShadowComparison {
        mismatches: categories
            .into_iter()
            .map(|category| ShadowMismatch { category })
            .collect(),
    }
}

fn has_overlay_claims(environment: &NormalizedEnvironment) -> bool {
    ["d3dimm.dll", "ddraw.dll"]
        .iter()
        .all(|dll| environment.dll_claims.contains_key(*dll))
}

fn has_managed_dxvk_claims(environment: &NormalizedEnvironment) -> bool {
    [
        "d3d8.dll",
        "d3d9.dll",
        "d3d10core.dll",
        "d3d11.dll",
        "dxgi.dll",
    ]
    .iter()
    .all(|dll| environment.dll_claims.contains_key(*dll))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::runners::managed_proton_path;
    use crate::utils::ResolvedRunner;
    use std::ops::Deref;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn context(resolved: ResolvedRunner) -> WineContext {
        let probe = probe_runner(&resolved);
        let identity = crate::tools::runtime::resolve_prefix_binding(
            None,
            &resolved,
            &probe,
            resolved.is_wine_7_16(),
        )
        .expect("shadow test binding");
        let location = identity.location.clone();
        WineContext {
            prefix: location.path.clone(),
            location,
            resolved,
            identity,
            probe,
        }
    }

    #[test]
    fn rollback_flag_defaults_on_and_zero_disables() {
        assert!(runtime_shadow_enabled_from(None));
        assert!(runtime_shadow_enabled_from(Some("1")));
        assert!(!runtime_shadow_enabled_from(Some("0")));
        assert!(runtime_shadow_enabled_from(Some("false")));
    }

    #[test]
    fn wine_716_dgvoodoo_matches_all_five_target_policies() {
        let runner = wine_runner("wine-7.16");
        let context = context(runner.runner.clone());
        let profile = profile_from_legacy(LegacyProfileInput {
            server_runner: Some(context.resolved.runner_path().to_string_lossy().as_ref()),
            default_runner: None,
            resolved: &context.resolved,
            dgvoodoo: DgVoodooState::verified(true),
        })
        .unwrap();
        let input = LegacyRuntimeInput {
            server_runner: Some("/portable/wine"),
            default_runner: None,
            context: &context,
            dxvk: DxvkProvision::Managed,
            dgvoodoo: DgVoodooObservation::verified(true),
            webview2_required: true,
            recommendation: Some(GepardRunnerProfile::Wine716Legacy),
        };
        let legacy = capture_legacy_runtime(&input, &profile).unwrap();
        let prefix = context.location.clone();
        let plan = resolve_runtime(RuntimeResolutionInput {
            profile: profile.clone(),
            resolved: &context.resolved,
            prefix: prefix.clone(),
            prefix_binding: shadow_prefix_binding(prefix),
            probe: probe_runner(&context.resolved),
            dgvoodoo: DgVoodooState::verified(true),
            webview2_required: true,
        })
        .unwrap();
        assert!(compare_shadow(&legacy, &profile, &plan)
            .mismatches
            .is_empty());

        let maintenance = legacy
            .environments
            .get(&InvocationTarget::MaintenancePatcher)
            .unwrap();
        assert!(!has_overlay_claims(maintenance));
        assert!(has_managed_dxvk_claims(maintenance));
        for target in [
            InvocationTarget::Game,
            InvocationTarget::LaunchPatcher,
            InvocationTarget::OpenSetup,
            InvocationTarget::GraphicsControlPanel,
        ] {
            assert!(has_overlay_claims(
                legacy.environments.get(&target).unwrap()
            ));
        }
    }

    #[test]
    fn proton_anchor_and_unknown_gepard_keep_the_selected_runner() {
        let proton_path = managed_proton_path();
        let runner = ResolvedRunner::test_proton(
            proton_path.clone(),
            proton_path.parent().unwrap().to_path_buf(),
            PathBuf::from("/usr/bin/umu-run"),
        );
        let context = context(runner);
        for recommendation in [Some(GepardRunnerProfile::ModernProton), None] {
            let profile = profile_from_legacy(LegacyProfileInput {
                server_runner: None,
                default_runner: None,
                resolved: &context.resolved,
                dgvoodoo: DgVoodooState::verified(false),
            })
            .unwrap();
            let input = LegacyRuntimeInput {
                server_runner: None,
                default_runner: None,
                context: &context,
                dxvk: DxvkProvision::Runner,
                dgvoodoo: DgVoodooObservation::verified(false),
                webview2_required: false,
                recommendation,
            };
            let legacy = capture_legacy_runtime(&input, &profile).unwrap();
            let prefix = context.location.clone();
            let plan = resolve_runtime(RuntimeResolutionInput {
                profile: profile.clone(),
                resolved: &context.resolved,
                prefix: prefix.clone(),
                prefix_binding: shadow_prefix_binding(prefix),
                probe: probe_runner(&context.resolved),
                dgvoodoo: DgVoodooState::verified(false),
                webview2_required: false,
            })
            .unwrap();
            assert!(compare_shadow(&legacy, &profile, &plan)
                .mismatches
                .is_empty());
        }
    }

    #[test]
    fn a_shadow_fact_failure_is_non_blocking_and_redacted() {
        let runner = wine_runner("wine-7.16");
        let context = context(runner.runner.clone());
        let legacy_result = 37;
        let outcome = observe_legacy_runtime(
            None,
            ShadowOperation::Launch,
            LegacyRuntimeInput {
                server_runner: Some("/private/home/player/wine/bin/wine"),
                default_runner: None,
                context: &context,
                dxvk: DxvkProvision::Managed,
                dgvoodoo: DgVoodooObservation::Unavailable,
                webview2_required: false,
                recommendation: None,
            },
        );
        assert_eq!(
            outcome,
            ShadowOutcome::Failed("dgvoodoo-observation-unavailable")
        );
        assert_eq!(legacy_result, 37);

        let log = format_shadow_divergence(
            ShadowOperation::Launch,
            &context,
            &["runner-identity", "prefix-binding"],
        );
        assert!(!log.contains("/private/home/player"));
        assert!(!log.contains("private-server-name"));
        assert!(log.contains("runner-identity,prefix-binding"));
    }

    #[test]
    fn normalized_override_order_does_not_create_a_false_divergence() {
        let legacy = OsString::from("d3d8=n,b;d3d9=n,b;d3d10core=n,b;d3d11=n,b;dxgi=n,b");
        let parsed = parse_legacy_overrides(&legacy).unwrap();
        assert_eq!(
            parsed.keys().cloned().collect::<Vec<_>>(),
            [
                "d3d10core.dll",
                "d3d11.dll",
                "d3d8.dll",
                "d3d9.dll",
                "dxgi.dll"
            ]
        );
    }

    #[test]
    fn comparator_reports_stable_categories_for_adversarial_drift() {
        let runner = wine_runner("wine-7.16");
        let context = context(runner.runner.clone());
        let profile = profile_from_legacy(LegacyProfileInput {
            server_runner: Some("/portable/wine"),
            default_runner: None,
            resolved: &context.resolved,
            dgvoodoo: DgVoodooState::verified(false),
        })
        .unwrap();
        let input = LegacyRuntimeInput {
            server_runner: Some("/portable/wine"),
            default_runner: None,
            context: &context,
            dxvk: DxvkProvision::Managed,
            dgvoodoo: DgVoodooObservation::verified(false),
            webview2_required: false,
            recommendation: None,
        };
        let mut legacy = capture_legacy_runtime(&input, &profile).unwrap();
        let prefix = context.location.clone();
        let plan = resolve_runtime(RuntimeResolutionInput {
            profile: profile.clone(),
            resolved: &context.resolved,
            prefix: prefix.clone(),
            prefix_binding: shadow_prefix_binding(prefix),
            probe: probe_runner(&context.resolved),
            dgvoodoo: DgVoodooState::verified(false),
            webview2_required: false,
        })
        .unwrap();

        legacy.dxvk = legacy_provider(DxvkProvision::Winetricks);
        legacy.selection_source = RunnerSelectionSource::GlobalSetting;
        legacy
            .environments
            .get_mut(&InvocationTarget::Game)
            .unwrap()
            .variables
            .remove("WINE_LARGE_ADDRESS_AWARE");

        let categories = compare_shadow(&legacy, &profile, &plan)
            .mismatches
            .into_iter()
            .map(|mismatch| mismatch.category.label())
            .collect::<Vec<_>>();
        assert_eq!(
            categories,
            ["selection-source", "dxvk-provider", "environment-delta"]
        );
    }

    struct TestWine {
        runner: ResolvedRunner,
        root: PathBuf,
    }

    impl Deref for TestWine {
        type Target = ResolvedRunner;

        fn deref(&self) -> &Self::Target {
            &self.runner
        }
    }

    impl Drop for TestWine {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn wine_runner(version: &str) -> TestWine {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "ro-launcher-shadow-wine-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("bin")).unwrap();
        let wine = root.join("bin/wine");
        let wineserver = root.join("bin/wineserver");
        std::fs::write(&wine, format!("#!/bin/sh\necho '{version}'\n")).unwrap();
        std::fs::write(&wineserver, "#!/bin/sh\nexit 0\n").unwrap();
        for path in [&wine, &wineserver] {
            let mut permissions = std::fs::metadata(path).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(path, permissions).unwrap();
        }
        TestWine {
            runner: ResolvedRunner::test_wine(wine, wineserver),
            root,
        }
    }
}
