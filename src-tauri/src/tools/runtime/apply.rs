use std::ffi::OsStr;
use std::ffi::OsString;

use crate::utils::{ProcessEnv, RunnerInvocation, WineContext};

use super::environment::{EnvironmentChange, GraphicsEnvironment, GraphicsEnvironmentError};
use super::model::{
    GraphicsProfile, InvocationTarget, RunnerSelectionSource, RuntimePlan, RuntimeProfile,
};
use super::resolver::{
    profile_from_legacy, resolve_runtime, DgVoodooState, LegacyProfileInput,
    RuntimeResolutionError, RuntimeResolutionInput,
};

pub(crate) fn runtime_graphics_plan_enabled() -> bool {
    runtime_graphics_plan_enabled_from(
        std::env::var("RO_LAUNCHER_RUNTIME_GRAPHICS")
            .ok()
            .as_deref(),
    )
}

pub(crate) fn runtime_graphics_plan_enabled_from(value: Option<&str>) -> bool {
    value != Some("0")
}

pub(crate) struct OperationalRuntimeInput<'a> {
    pub server_runner: Option<&'a str>,
    pub default_runner: Option<&'a str>,
    pub context: &'a WineContext,
    pub dgvoodoo: DgVoodooState,
    pub webview2_required: bool,
}

pub(crate) fn resolve_operational_plan(
    input: OperationalRuntimeInput<'_>,
) -> Result<RuntimePlan, RuntimeResolutionError> {
    resolve_operational_plan_with_profile(input).map(|(_, plan)| plan)
}

pub(crate) fn resolve_operational_plan_with_profile(
    input: OperationalRuntimeInput<'_>,
) -> Result<(RuntimeProfile, RuntimePlan), RuntimeResolutionError> {
    let profile = profile_from_legacy(LegacyProfileInput {
        server_runner: input.server_runner,
        default_runner: input.default_runner,
        resolved: &input.context.resolved,
        dgvoodoo: input.dgvoodoo,
    })?;
    let plan = resolve_runtime(RuntimeResolutionInput {
        profile: profile.clone(),
        resolved: &input.context.resolved,
        prefix: input.context.location.clone(),
        prefix_binding: input.context.identity.clone(),
        probe: input.context.probe.clone(),
        dgvoodoo: input.dgvoodoo,
        webview2_required: input.webview2_required,
    })?;
    Ok((profile, plan))
}

pub(crate) fn build_runtime_plan_summary(
    plan_id: String,
    profile: &RuntimeProfile,
    plan: &RuntimePlan,
    overlay_verified: bool,
) -> crate::models::dependency::RuntimePlanSummary {
    let provider = plan.graphics().dxvk_provider();
    crate::models::dependency::RuntimePlanSummary {
        plan_id,
        selection_source: selection_source_label(profile.selection_source()),
        graphics_profile: graphics_profile_label(profile.graphics()),
        dxvk_provider: dxvk_provider_label(provider),
        dxvk_component_id: provider.component_id().to_string(),
        overlay_verified,
    }
}

fn selection_source_label(source: RunnerSelectionSource) -> &'static str {
    match source {
        RunnerSelectionSource::ServerOverride => "serverOverride",
        RunnerSelectionSource::GlobalSetting => "globalSetting",
        RunnerSelectionSource::ProductDefault => "productDefault",
    }
}

fn graphics_profile_label(profile: GraphicsProfile) -> &'static str {
    match profile {
        GraphicsProfile::Dxvk => "dxvk",
        GraphicsProfile::DgVoodooDxvk => "dgVoodooDxvk",
    }
}

fn dxvk_provider_label(provider: &super::model::DxvkProvider) -> &'static str {
    match provider.kind_label() {
        "runner-owned" => "runnerOwned",
        "managed-prefix" => "managedPrefix",
        "winetricks-prefix" => "winetricksPrefix",
        _ => "unknown",
    }
}

pub(crate) struct InvocationPlan<'a> {
    pub target: InvocationTarget,
    pub plan: &'a RuntimePlan,
}

impl InvocationPlan<'_> {
    pub fn environment(&self) -> Result<GraphicsEnvironment, GraphicsEnvironmentError> {
        self.plan
            .graphics()
            .environment_for(self.target, self.plan.prefix())
    }
}

const GUARDED_ENV_KEYS: &[&str] = &[
    "WINEDLLOVERRIDES",
    "WINE_LARGE_ADDRESS_AWARE",
    "DXVK_CONFIG_FILE",
    "DXVK_LOG_PATH",
    "DXVK_STATE_CACHE_PATH",
];

pub(crate) fn apply_graphics_environment<E: ProcessEnv>(
    env: &mut E,
    graphics: &GraphicsEnvironment,
) {
    env.unset_env("WINEDLLOVERRIDES");
    for (key, contribution) in graphics.variables.changes() {
        match &contribution.change {
            EnvironmentChange::Set(value) => env.set_env(key, value),
            EnvironmentChange::Unset => env.unset_env(key),
        }
    }
    let rendered = graphics.dll_overrides.render_wine();
    if !rendered.is_empty() {
        env.set_env("WINEDLLOVERRIDES", rendered);
    }
}

pub(crate) fn apply_graphics_environment_to_invocation(
    invocation: &mut RunnerInvocation,
    graphics: &GraphicsEnvironment,
) -> Result<(), GraphicsEnvironmentError> {
    let expected = expected_guarded_values(graphics)?;
    for key in GUARDED_ENV_KEYS {
        let existing = invocation_env_value(invocation, key);
        if let Some(existing) = existing {
            let expected_value = expected.get(*key).and_then(|value| value.as_ref());
            if expected_value != Some(&existing) {
                return Err(GraphicsEnvironmentError::EnvironmentConflict(
                    super::environment::EnvironmentConflict {
                        key: (*key).to_string(),
                    },
                ));
            }
        }
    }
    apply_graphics_environment(invocation, graphics);
    Ok(())
}

fn expected_guarded_values(
    graphics: &GraphicsEnvironment,
) -> Result<std::collections::BTreeMap<String, Option<OsString>>, GraphicsEnvironmentError> {
    let mut expected = std::collections::BTreeMap::new();
    for key in GUARDED_ENV_KEYS {
        if *key == "WINEDLLOVERRIDES" {
            let rendered = graphics.dll_overrides.render_wine();
            expected.insert(
                key.to_string(),
                if rendered.is_empty() {
                    None
                } else {
                    Some(OsString::from(rendered))
                },
            );
            continue;
        }
        let value = match graphics.variables.changes().get(*key) {
            None => None,
            Some(contribution) => match &contribution.change {
                EnvironmentChange::Set(value) => Some(value.clone()),
                EnvironmentChange::Unset => None,
            },
        };
        expected.insert((*key).to_string(), value);
    }
    Ok(expected)
}

fn invocation_env_value(invocation: &RunnerInvocation, key: &str) -> Option<OsString> {
    let key_os = OsStr::new(key);
    invocation
        .env
        .iter()
        .rev()
        .find(|(existing, _)| existing == key_os)
        .and_then(|(_, value)| value.clone())
}

#[cfg(test)]
mod tests {
    #![allow(deprecated)]

    use std::collections::BTreeMap;

    use super::*;
    use crate::tools::prefix::MANAGED_DXVK_COMPONENT;
    use crate::tools::runtime::model::{
        ArtifactId, ComponentId, ComponentProvenance, DgVoodooOverlayPlan, DxvkProvider,
        GraphicsPlan,
    };
    use crate::utils::{apply_game_env, PrefixLocation, PrefixScope, RunnerInvocation};

    #[derive(Default)]
    struct EnvCapture {
        values: BTreeMap<String, Option<OsString>>,
    }

    impl ProcessEnv for EnvCapture {
        fn set_env(&mut self, key: impl AsRef<OsStr>, val: impl AsRef<OsStr>) {
            self.values.insert(
                key.as_ref().to_string_lossy().to_string(),
                Some(val.as_ref().to_os_string()),
            );
        }

        fn unset_env(&mut self, key: impl AsRef<OsStr>) {
            self.values
                .insert(key.as_ref().to_string_lossy().to_string(), None);
        }
    }

    fn prefix_location(path: &str) -> PrefixLocation {
        PrefixLocation {
            path: path.to_string(),
            scope: PrefixScope::Isolated,
            managed: true,
            server_id: Some("fixture".to_string()),
        }
    }

    fn managed_dgvoodoo_plan() -> GraphicsPlan {
        GraphicsPlan::dgvoodoo_dxvk(
            DgVoodooOverlayPlan {
                provenance: ComponentProvenance::BundledResource {
                    resource_id: "bundled/dgvoodoo".to_string(),
                },
                component_id: ComponentId::new("game-dir/dgvoodoo").unwrap(),
            },
            DxvkProvider::managed_prefix(
                ArtifactId::new(MANAGED_DXVK_COMPONENT).expect("artifact id"),
            ),
        )
    }

    fn wine_override_claims(capture: &EnvCapture) -> BTreeMap<String, String> {
        let Some(value) = capture
            .values
            .get("WINEDLLOVERRIDES")
            .and_then(|value| value.as_ref())
        else {
            return BTreeMap::new();
        };
        value
            .to_string_lossy()
            .split(';')
            .filter(|entry| !entry.is_empty())
            .map(|entry| {
                let (name, order) = entry
                    .split_once('=')
                    .expect("override entry must contain '='");
                (name.to_string(), order.to_string())
            })
            .collect()
    }

    #[test]
    fn runtime_graphics_flag_defaults_to_enabled() {
        assert!(runtime_graphics_plan_enabled_from(None));
        assert!(!runtime_graphics_plan_enabled_from(Some("0")));
        assert!(runtime_graphics_plan_enabled_from(Some("1")));
    }

    #[test]
    fn structured_env_matches_contract_fixture_for_game_and_maintenance() {
        let fixture = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../contract-fixtures/runtime-graphics-env-v1.json"
        ))
        .expect("fixture");
        let parsed: serde_json::Value = serde_json::from_str(&fixture).expect("json");
        let prefix = prefix_location("/tmp/prefix");
        let plan = managed_dgvoodoo_plan();
        for (target, key) in [
            (InvocationTarget::Game, "managedDxvkWithOverlay"),
            (
                InvocationTarget::MaintenancePatcher,
                "managedDxvkMaintenancePatcher",
            ),
        ] {
            let expected = parsed[key].as_object().expect("object");
            let structured = plan
                .environment_for(target, &prefix)
                .expect("structured environment");
            let mut capture = EnvCapture::default();
            apply_graphics_environment(&mut capture, &structured);
            let claims = wine_override_claims(&capture);
            assert_eq!(claims.len(), expected.len(), "target {:?}", target);
            for (dll, order) in &claims {
                assert_eq!(
                    expected.get(dll).and_then(|v| v.as_str()),
                    Some(order.as_str())
                );
            }
        }
    }

    #[test]
    fn structured_env_matches_legacy_matrix_for_managed_dxvk() {
        let prefix = prefix_location("/tmp/prefix");
        let plan = managed_dgvoodoo_plan();
        let cases = [
            (InvocationTarget::Game, true),
            (InvocationTarget::LaunchPatcher, true),
            (InvocationTarget::MaintenancePatcher, false),
            (InvocationTarget::OpenSetup, true),
            (InvocationTarget::GraphicsControlPanel, true),
        ];
        for (target, use_dgvoodoo) in cases {
            let structured = plan
                .environment_for(target, &prefix)
                .expect("structured environment");
            let mut structured_capture = EnvCapture::default();
            apply_graphics_environment(&mut structured_capture, &structured);

            let mut legacy_capture = EnvCapture::default();
            apply_game_env(&mut legacy_capture, use_dgvoodoo, true, &prefix.path);

            assert_eq!(
                wine_override_claims(&structured_capture),
                wine_override_claims(&legacy_capture),
                "target {:?}",
                target
            );
            assert_eq!(
                structured_capture.values.get("WINE_LARGE_ADDRESS_AWARE"),
                legacy_capture.values.get("WINE_LARGE_ADDRESS_AWARE")
            );
        }
    }

    #[test]
    fn conflicting_wine_overrides_are_rejected_before_apply() {
        let prefix = prefix_location("/tmp/prefix");
        let graphics = managed_dgvoodoo_plan()
            .environment_for(InvocationTarget::Game, &prefix)
            .expect("environment");
        let mut invocation = RunnerInvocation {
            program: std::path::PathBuf::from("/usr/bin/wine"),
            args: vec![],
            cwd: std::path::PathBuf::from("/game"),
            env: vec![(
                OsString::from("WINEDLLOVERRIDES"),
                Some(OsString::from("d3d9=b")),
            )],
        };
        assert!(apply_graphics_environment_to_invocation(&mut invocation, &graphics).is_err());
    }
}
