use tokio::process::Command;

pub fn apply_prefix_env(cmd: &mut Command, prefix_path: &str) {
    cmd.env("WINEPREFIX", prefix_path)
        .env("WAYLAND_DISPLAY", "");
}

pub fn apply_game_env(cmd: &mut Command, use_dgvoodoo: bool) {
    cmd.env("DXVK_ASYNC", "1")
        .env("DXVK_CONFIG", "d3d9.forceSamplerTypeSpecConstants=True")
        .env("WINE_LARGE_ADDRESS_AWARE", "1");
    if use_dgvoodoo {
        cmd.env("WINEDLLOVERRIDES", "d3dimm=n,b;ddraw=n,b");
    }
}

pub fn apply_tool_env(cmd: &mut Command, needs_dgvoodoo_overrides: bool) {
    cmd.env("DXVK_ASYNC", "1")
        .env("WINE_LARGE_ADDRESS_AWARE", "1");
    if needs_dgvoodoo_overrides {
        cmd.env("WINEDLLOVERRIDES", "d3dimm=n,b;ddraw=n,b");
    }
}

pub fn pipe_output(cmd: &mut Command) {
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
}
