# RO-Launcher

Ragnarok Online launcher for Linux x86_64, built with Tauri, React and Rust. It downloads the runtime,
prepares dependencies and keeps a separate Wine environment for each server and runner.

## Install and play

1. Download the [AppImage](https://github.com/Nandem1/nndsk-ro-launcher/releases/latest), make it executable and open it.
2. Add your client's `.exe` and click **Preparar entorno** to prepare its environment.
3. Configure the client through **Setup**, run its patcher if updates are needed and click **Jugar** to play.

You need a graphical session and a Vulkan driver for your GPU. The primary runtime also requires
Python 3.10 or later. You do not need system Wine. The launcher checks host requirements before
preparing the environment. See the [first-run guide](docs/FIRST_RUN.md) if preparation fails.

The AppImage supports [signed updates](docs/features/UPDATER.md). Releases include a
[SHA-256 checksum](https://github.com/Nandem1/nndsk-ro-launcher/releases/latest/download/RO-Launcher_amd64.AppImage.sha256).

## Runners

- [nndsk-ro-proton](docs/NNDSK_RO_PROTON.md) is the primary runtime. The launcher manages its download and UMU.
- [Wine 7.16 Staging/TkG amd64](docs/WINE_716_RUNTIME.md) is the per-server fallback for clients that need old-WoW64.

The launcher offers these two downloads and preserves existing legacy selections. Switching runners
does not migrate or delete the previous prefix. Compatibility depends on the client build and its protection.
The launcher does not modify or disable Gepard.

## Tools

- Manages patchers, Setup and multiple running clients.
- Uses DXVK for Direct3D and offers dgVoodoo for DirectDraw.
- Includes AutoPot, AutoBuff, a spammer and Discord Rich Presence.

Automation requires input device permissions. Playing does not. `ro-sessiond` allows memory reads
from clients started by the launcher with `ptrace_scope=1`, without changing host security policy.

Settings and environments live in `~/.local/share/ro-launcher/`. **Rearmar entorno** rebuilds an
environment while keeping the previous one until completion. The launcher does not adopt directories containing unknown data.

## Development

Requires Node.js, npm, stable Rust and the Tauri v2 dependencies for Linux.

```bash
npm ci
npm run tauri:dev
```

Build the AppImage with `npm run tauri:build:appimage`. Artifacts go to `target/release/bundle/`.
See [AGENTS.md](AGENTS.md) for tests and contribution rules, and [docs/README.md](docs/README.md)
for architecture and technical contracts.

## Third-party projects

[nndsk-ro-proton](https://github.com/Nandem1/nndsk-ro-proton) ·
[Proton-CachyOS](https://github.com/CachyOS/proton-cachyos) ·
[umu-launcher](https://github.com/Open-Wine-Components/umu-launcher) ·
[DXVK](https://github.com/doitsujin/dxvk) ·
[dgVoodoo2](http://dege.freeweb.hu/)

Developed by [nndsk](https://github.com/nndsk).
