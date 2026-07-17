use crate::models::dependency::DependencyStatus;
use crate::tools::deps::fields::{dependency_dxvk_fields, dependency_prefix_fields};
use crate::utils::audio;
use crate::utils::resolve_effective_runner;
use crate::utils::{
    is_dxvk_installed, is_prefix_configured, prefix_path, system_wine_available,
    winetricks_available,
};
use ro_tools_linux::{detect_input_permissions, detect_uinput_permissions};

pub async fn check_dependencies(runner: Option<String>) -> Result<DependencyStatus, String> {
    let wine = system_wine_available();
    let winetricks = winetricks_available();

    let prefix = prefix_path();
    let dxvk = is_dxvk_installed(&prefix);
    let prefix_configured = is_prefix_configured(&prefix);

    let resolved = resolve_effective_runner(runner).await?;
    let (audio_ok, audio_driver, audio_warning, audio_stack) =
        audio::dependency_audio_fields(&prefix, prefix_configured, &resolved).await;

    let input_perms = detect_input_permissions();
    let uinput_perms = detect_uinput_permissions();
    let (prefix_ok, prefix_warning) =
        dependency_prefix_fields(wine, winetricks, prefix_configured, &prefix);
    let (dxvk_ok, dxvk_warning) = dependency_dxvk_fields(dxvk, prefix_configured);

    Ok(DependencyStatus {
        wine,
        winetricks,
        dxvk,
        prefix_configured,
        audio_ok,
        audio_driver,
        audio_stack,
        audio_warning,
        input_group_ok: input_perms.ok,
        input_group_warning: input_perms.warning,
        uinput_input_ok: uinput_perms.ok,
        uinput_input_warning: uinput_perms.warning,
        prefix_ok,
        prefix_warning,
        dxvk_ok,
        dxvk_warning,
    })
}
