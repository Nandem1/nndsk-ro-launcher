use std::collections::BTreeMap;
use std::ffi::OsString;

use crate::utils::{dxvk_cache_path, dxvk_config_path, dxvk_log_path, PrefixLocation};

use super::model::{
    ComponentId, DgVoodooOverlayPlan, DxvkProvider, GraphicsPlan, InvocationTarget,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum OwnershipDomain {
    Runner,
    Prefix,
    GameDirOverlay,
    HostExternal,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ComponentOwner {
    domain: OwnershipDomain,
    component_id: ComponentId,
}

impl ComponentOwner {
    pub(super) fn new(domain: OwnershipDomain, component_id: ComponentId) -> Self {
        Self {
            domain,
            component_id,
        }
    }

    pub(super) fn domain(&self) -> OwnershipDomain {
        self.domain
    }

    pub(super) fn component_id(&self) -> &str {
        self.component_id.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct DllName(String);

impl DllName {
    pub(super) fn parse(value: &str) -> Result<Self, GraphicsEnvironmentError> {
        let normalized = value.to_ascii_lowercase();
        let normalized = normalized.strip_suffix(".dll").unwrap_or(&normalized);
        if normalized.is_empty()
            || !normalized
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(GraphicsEnvironmentError::InvalidDllName);
        }
        Ok(Self(format!("{normalized}.dll")))
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DllLoadOrder {
    NativeThenBuiltin,
}

impl DllLoadOrder {
    fn wine_value(self) -> &'static str {
        match self {
            Self::NativeThenBuiltin => "n,b",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DllClaim {
    pub(super) name: DllName,
    pub(super) load_order: DllLoadOrder,
    pub(super) owner: ComponentOwner,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct DllOverrideSet {
    claims: BTreeMap<DllName, DllClaim>,
}

impl DllOverrideSet {
    pub(super) fn claim(
        &mut self,
        name: &str,
        load_order: DllLoadOrder,
        owner: ComponentOwner,
    ) -> Result<(), OverrideConflict> {
        let name = DllName::parse(name).map_err(|_| OverrideConflict {
            dll: "invalid-dll-name".to_string(),
        })?;
        let claim = DllClaim {
            name: name.clone(),
            load_order,
            owner,
        };
        match self.claims.get(&name) {
            None => {
                self.claims.insert(name, claim);
                Ok(())
            }
            Some(existing) if existing == &claim => Ok(()),
            Some(_) => Err(OverrideConflict {
                dll: name.as_str().to_string(),
            }),
        }
    }

    pub(super) fn merge(&mut self, other: &Self) -> Result<(), OverrideConflict> {
        for claim in other.claims.values() {
            self.claim(claim.name.as_str(), claim.load_order, claim.owner.clone())?;
        }
        Ok(())
    }

    pub(super) fn claims(&self) -> &BTreeMap<DllName, DllClaim> {
        &self.claims
    }

    pub(crate) fn render_wine(&self) -> String {
        self.claims
            .values()
            .map(|claim| {
                let name = claim.name.as_str().trim_end_matches(".dll");
                format!("{name}={}", claim.load_order.wine_value())
            })
            .collect::<Vec<_>>()
            .join(";")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OverrideConflict {
    pub(crate) dll: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum EnvironmentChange {
    Set(OsString),
    Unset,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct EnvironmentContributorId(String);

impl EnvironmentContributorId {
    pub(super) fn new(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EnvironmentContribution {
    pub(super) change: EnvironmentChange,
    pub(super) contributor: EnvironmentContributorId,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct EnvironmentDelta {
    changes: BTreeMap<String, EnvironmentContribution>,
}

impl EnvironmentDelta {
    pub(super) fn set(
        &mut self,
        key: &str,
        value: impl Into<OsString>,
        contributor: EnvironmentContributorId,
    ) -> Result<(), EnvironmentConflict> {
        self.contribute(key, EnvironmentChange::Set(value.into()), contributor)
    }

    #[allow(
        dead_code,
        reason = "unset is part of the closed phase-1 environment contract"
    )]
    pub(super) fn unset(
        &mut self,
        key: &str,
        contributor: EnvironmentContributorId,
    ) -> Result<(), EnvironmentConflict> {
        self.contribute(key, EnvironmentChange::Unset, contributor)
    }

    fn contribute(
        &mut self,
        key: &str,
        change: EnvironmentChange,
        contributor: EnvironmentContributorId,
    ) -> Result<(), EnvironmentConflict> {
        if key == "WINEDLLOVERRIDES" || key.is_empty() || key.contains('=') || key.contains('\0') {
            return Err(EnvironmentConflict {
                key: key.to_string(),
            });
        }
        let contribution = EnvironmentContribution {
            change,
            contributor,
        };
        match self.changes.get(key) {
            None => {
                self.changes.insert(key.to_string(), contribution);
                Ok(())
            }
            Some(existing) if existing == &contribution => Ok(()),
            Some(_) => Err(EnvironmentConflict {
                key: key.to_string(),
            }),
        }
    }

    pub(super) fn changes(&self) -> &BTreeMap<String, EnvironmentContribution> {
        &self.changes
    }
}

pub(super) fn merge_environment_deltas<'a>(
    deltas: impl IntoIterator<Item = &'a EnvironmentDelta>,
) -> Result<EnvironmentDelta, EnvironmentConflict> {
    let mut merged = EnvironmentDelta::default();
    for delta in deltas {
        for (key, contribution) in delta.changes() {
            merged.contribute(
                key,
                contribution.change.clone(),
                contribution.contributor.clone(),
            )?;
        }
    }
    Ok(merged)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EnvironmentConflict {
    pub(crate) key: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GraphicsEnvironment {
    pub(super) dll_overrides: DllOverrideSet,
    pub(super) variables: EnvironmentDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GraphicsEnvironmentError {
    InvalidDllName,
    OverrideConflict(OverrideConflict),
    EnvironmentConflict(EnvironmentConflict),
}

impl GraphicsEnvironmentError {
    pub(crate) fn conflict_key(&self) -> Option<&str> {
        match self {
            Self::EnvironmentConflict(conflict) => Some(conflict.key.as_str()),
            Self::OverrideConflict(conflict) => Some(conflict.dll.as_str()),
            Self::InvalidDllName => None,
        }
    }
}

impl From<OverrideConflict> for GraphicsEnvironmentError {
    fn from(value: OverrideConflict) -> Self {
        Self::OverrideConflict(value)
    }
}

impl From<EnvironmentConflict> for GraphicsEnvironmentError {
    fn from(value: EnvironmentConflict) -> Self {
        Self::EnvironmentConflict(value)
    }
}

impl GraphicsEnvironment {
    fn merge(&mut self, other: &Self) -> Result<(), GraphicsEnvironmentError> {
        self.dll_overrides.merge(&other.dll_overrides)?;
        self.variables = merge_environment_deltas([&self.variables, &other.variables])?;
        Ok(())
    }
}

impl GraphicsPlan {
    pub(crate) fn environment_for(
        &self,
        target: InvocationTarget,
        prefix: &PrefixLocation,
    ) -> Result<GraphicsEnvironment, GraphicsEnvironmentError> {
        let mut result = runtime_base_environment()?;
        result.merge(&dxvk_environment(self.dxvk_provider(), prefix)?)?;
        if target.receives_overlay() {
            if let Some(overlay) = self.overlay() {
                result.merge(&dgvoodoo_environment(overlay)?)?;
            }
        }
        Ok(result)
    }
}

fn runtime_base_environment() -> Result<GraphicsEnvironment, GraphicsEnvironmentError> {
    let mut environment = GraphicsEnvironment::default();
    environment.variables.set(
        "WINE_LARGE_ADDRESS_AWARE",
        "1",
        EnvironmentContributorId::new("runtime-base"),
    )?;
    Ok(environment)
}

fn dxvk_environment(
    provider: &DxvkProvider,
    prefix: &PrefixLocation,
) -> Result<GraphicsEnvironment, GraphicsEnvironmentError> {
    let mut environment = GraphicsEnvironment::default();
    if !provider.is_managed_prefix() {
        return Ok(environment);
    }

    let component = ComponentId::new(provider.component_id())
        .map_err(|_| GraphicsEnvironmentError::InvalidDllName)?;
    let owner = ComponentOwner::new(OwnershipDomain::Prefix, component);
    for dll in ["d3d8", "d3d9", "d3d10core", "d3d11", "dxgi"] {
        environment
            .dll_overrides
            .claim(dll, DllLoadOrder::NativeThenBuiltin, owner.clone())?;
    }
    let contributor = EnvironmentContributorId::new(provider.component_id());
    environment.variables.set(
        "DXVK_CONFIG_FILE",
        dxvk_config_path(&prefix.path).into_os_string(),
        contributor.clone(),
    )?;
    environment.variables.set(
        "DXVK_LOG_PATH",
        dxvk_log_path(&prefix.path).into_os_string(),
        contributor.clone(),
    )?;
    environment.variables.set(
        "DXVK_STATE_CACHE_PATH",
        dxvk_cache_path(&prefix.path).into_os_string(),
        contributor,
    )?;
    Ok(environment)
}

fn dgvoodoo_environment(
    overlay: &DgVoodooOverlayPlan,
) -> Result<GraphicsEnvironment, GraphicsEnvironmentError> {
    let mut environment = GraphicsEnvironment::default();
    let owner = ComponentOwner::new(
        OwnershipDomain::GameDirOverlay,
        overlay.component_id.clone(),
    );
    for dll in ["d3dimm", "ddraw"] {
        environment
            .dll_overrides
            .claim(dll, DllLoadOrder::NativeThenBuiltin, owner.clone())?;
    }
    Ok(environment)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(domain: OwnershipDomain, id: &str) -> ComponentOwner {
        ComponentOwner::new(domain, ComponentId::new(id).unwrap())
    }

    #[test]
    fn dll_names_are_normalized_and_rendered_deterministically() {
        let mut overrides = DllOverrideSet::default();
        overrides
            .claim(
                "D3D9.DLL",
                DllLoadOrder::NativeThenBuiltin,
                owner(OwnershipDomain::Prefix, "dxvk"),
            )
            .unwrap();
        overrides
            .claim(
                "ddraw",
                DllLoadOrder::NativeThenBuiltin,
                owner(OwnershipDomain::GameDirOverlay, "dgvoodoo"),
            )
            .unwrap();
        assert_eq!(overrides.render_wine(), "d3d9=n,b;ddraw=n,b");
        assert!(DllName::parse("../d3d9").is_err());
        assert!(DllName::parse("d3d9=n,b").is_err());
    }

    #[test]
    fn a_dll_cannot_have_two_owners() {
        let mut overrides = DllOverrideSet::default();
        overrides
            .claim(
                "d3d9",
                DllLoadOrder::NativeThenBuiltin,
                owner(OwnershipDomain::Runner, "runner/dxvk"),
            )
            .unwrap();
        let conflict = overrides
            .claim(
                "d3d9.dll",
                DllLoadOrder::NativeThenBuiltin,
                owner(OwnershipDomain::Prefix, "dxvk-2.6.2"),
            )
            .unwrap_err();
        assert_eq!(conflict.dll, "d3d9.dll");
    }

    #[test]
    fn environment_merge_is_idempotent_only_for_the_same_contributor() {
        let mut first = EnvironmentDelta::default();
        first
            .set(
                "DXVK_LOG_PATH",
                "/prefix/logs",
                EnvironmentContributorId::new("dxvk"),
            )
            .unwrap();
        let same = first.clone();
        assert!(merge_environment_deltas([&first, &same]).is_ok());

        let mut conflicting = EnvironmentDelta::default();
        conflicting
            .set(
                "DXVK_LOG_PATH",
                "/other/logs",
                EnvironmentContributorId::new("dxvk"),
            )
            .unwrap();
        assert_eq!(
            merge_environment_deltas([&first, &conflicting])
                .unwrap_err()
                .key,
            "DXVK_LOG_PATH"
        );
    }

    #[test]
    fn dgvoodoo_and_d7vk_spike_cannot_share_ddraw() {
        let mut overrides = DllOverrideSet::default();
        overrides
            .claim(
                "ddraw",
                DllLoadOrder::NativeThenBuiltin,
                owner(OwnershipDomain::GameDirOverlay, "game-dir/dgvoodoo"),
            )
            .unwrap();
        overrides
            .claim(
                "d3dimm",
                DllLoadOrder::NativeThenBuiltin,
                owner(OwnershipDomain::GameDirOverlay, "game-dir/dgvoodoo"),
            )
            .unwrap();
        let conflict = overrides
            .claim(
                "ddraw.dll",
                DllLoadOrder::NativeThenBuiltin,
                owner(OwnershipDomain::GameDirOverlay, "game-dir/d7vk-spike"),
            )
            .unwrap_err();
        assert_eq!(conflict.dll, "ddraw.dll");
    }

    #[test]
    fn same_d7vk_owner_claim_is_idempotent() {
        let mut overrides = DllOverrideSet::default();
        let spike = owner(OwnershipDomain::GameDirOverlay, "game-dir/d7vk-spike");
        overrides
            .claim("ddraw", DllLoadOrder::NativeThenBuiltin, spike.clone())
            .unwrap();
        overrides
            .claim("DDRAW", DllLoadOrder::NativeThenBuiltin, spike)
            .unwrap();
        assert_eq!(overrides.render_wine(), "ddraw=n,b");
    }

    #[test]
    fn wine_dll_overrides_can_only_come_from_the_structured_set() {
        let mut delta = EnvironmentDelta::default();
        assert!(delta
            .set(
                "WINEDLLOVERRIDES",
                "d3d9=n,b",
                EnvironmentContributorId::new("custom")
            )
            .is_err());
    }
}
