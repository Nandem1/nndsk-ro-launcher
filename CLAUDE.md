# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

## What this is

Launcher dedicado para Ragnarok Online en Linux. Gestiona el WINEPREFIX,
dependencias y variables de entorno automáticamente. El usuario solo elige
servidor y hace click en Jugar.

Stack: **Tauri v2** (Rust backend + React 18 frontend), TypeScript, Tailwind CSS, Zustand, Vite.
Target OS: Linux x86_64 (CachyOS/Arch primary, Ubuntu secondary).

---

## Build & development commands

```bash
# Full app (Tauri shell + Vite dev server)
npm run tauri:dev

# Production build (all bundles: deb, AppImage, etc.)
npm run tauri:build

# AppImage only (portable distro; NO_STRIP=true for Arch/CachyOS)
npm run tauri:build:appimage

# Frontend only (no Tauri window)
npm run dev

# Frontend tests
npm test

# Run a single frontend test file
npx vitest run src/features/spammer/spammer.logic.test.ts

# Type-check
npx tsc --noEmit

# Rust tests
cargo test --workspace

# Format Rust
cargo fmt --all
```

Bundles de producción: `target/release/bundle/` (raíz del repo).

`tauri:dev` sets `GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1` — required to prevent a black WebView on Wayland during development.

Release builds (AppImage, binary) apply the same WebKit vars at startup via `utils/webview.rs` → `configure_linux_webview_env()` (called from `lib.rs` before Tauri init). Only set them manually if overriding.

---

## Architecture

Cargo workspace con cuatro capas en el backend. El frontend usa Feature-Sliced Design.

### Capas backend (de adentro hacia afuera)

```
crates/ro-tools-core/     Dominio puro — sin OS (engine, config, ports, profiles, dgvoodoo, spammer)
crates/ro-tools-linux/    Adaptadores Linux — memoria /proc, ydotool, procesos Wine, keyboard evdev
crates/ro-inputd/         Binario sidecar — grab de teclado via evdev + passthrough uinput (JSON stdio)
src-tauri/src/
  commands/               Handlers Tauri delgados (nombre 1:1 con tools/ cuando aplica)
  tools/                  Servicios de aplicación (una carpeta por feature)
  state/                  Estado compartido (GameState: pid, autopot, spammer, input, ydotoold)
  models/                 DTOs IPC (serde camelCase, alineados con TypeScript)
  utils/                  Infra compartida (Wine, paths, eventos, JSON, webview) — no mover a tools
```

### Mapa `tools/` (completo)

| Módulo | Rol |
|--------|-----|
| `tools/autopot/` | AutoPot: PID, loop, perfiles |
| `tools/spammer/` | Spammer: lifecycle ro-inputd, loop de spam via ydotool |
| `tools/server_tools/` | OpenSetup, patcher, dgVoodoo |
| `tools/launcher/` | Lanzar/detener juego, cleanup AutoPot |
| `tools/prefix/` | Setup/reset WINEPREFIX (winetricks, marker) |
| `tools/runners/` | Descubrir Wine/Proton instalados |
| `tools/deps/` | Agregar checks de dependencias |
| `tools/input/` | Ciclo de vida ydotoold + InputGateway |

`commands/servers.rs` y `commands/settings.rs` son CRUD JSON puro — delegan directo a
`utils/` y **no** necesitan capa `tools/`.

### Añadir una feature nueva

1. Dominio reutilizable → `crates/ro-tools-*` (solo si no depende de Tauri/OS)
2. Orquestación → `tools/<feature>/session.rs` (+ módulos internos)
3. Handler IPC → `commands/<feature>.rs` (delgado)
4. UI → `src/features/<feature>/`

### AutoPot — flujo de capas

```
commands/autopot.rs          invoke handlers (start/stop/config/status)
  → tools/autopot/session.rs  resuelve PID, valida ydotool, arranca servicio
    → tools/autopot/service.rs   ciclo de vida (AutopotHandle)
      → tools/autopot/loop_runner.rs   tokio loop + eventos de estado
        → ro-tools-core AutopotEngine   lógica DT_AP (tick HP/SP)
          → ro-tools-linux ProcMemoryReader + LazyYdotoolInput
```

### Spammer — flujo de capas

```
commands/spammer.rs          invoke handlers (start/stop/update_config/status)
  → tools/spammer/session.rs  asegura ydotoold, delega a SpammerHandle
    → tools/spammer/service.rs   ciclo de vida (SpammerHandle)
      → tools/spammer/loop_runner.rs   tokio loop; spawna ro-inputd como subprocess
        → crates/ro-inputd              grab evdev, passthrough uinput, JSON stdio
        → ro-tools-core SpammerEngine   tick: key_down → click_left → key_up
          → ro-tools-linux LazyYdotoolInput (ydotool)
```

**ro-inputd** es un binario sidecar bundleado junto al ejecutable principal. Lo encuentra en
`find_ro_inputd()` (busca relativo al exe). Comunicación: args `--triggers F1,F2 --json`,
recibe `{"type":"stop"}` por stdin, emite `ready`/`trigger`/`fatal`/`shutdown` por stdout.

El Spammer exige que **el juego esté corriendo** (`pid` no null). `update_spammer_config` hace
restart completo (no hay canal live de reconfiguración).

Teclas válidas para el Spammer: `F1`–`F9` y `0`–`9`. Al mantener **Alt + tecla** el evento
pasa por el passthrough en lugar de activar spam (comportamiento intencional).

### Server tools — flujo de capas

```
commands/server_tools.rs       invoke handlers (scan/install/uninstall/launch)
  → tools/server_tools/session.rs   orquestación por servidor
    → tools/server_tools/scan.rs    detecta OpenSetup, patcher, dgVoodoo
    → tools/server_tools/dgvoodoo.rs  install/uninstall desde plantilla bundled
      → ro-tools-core dgvoodoo       validación de dgVoodoo.conf (sin I/O)
```

### Launcher / prefix / deps

```
commands/deps.rs       → tools/deps/check.rs         → utils/* + tools/input
commands/prefix.rs     → tools/prefix/setup.rs       → utils/prefix, winetricks
commands/launcher.rs   → tools/launcher/session.rs   → utils/wine, utils/process
commands/runners.rs    → tools/runners/discover.rs   → utils/runner
```

### Frontend (Feature-Sliced Design)

```
src/
  app/App.tsx                      ← root layout, calls loadServers() on mount
  features/
    servers/                       ← server list, add/remove, selection, dgVoodoo
    launcher/                      ← launch flow, progress, error states
    autopot/                       ← panel AutoPot, store, hooks
    spammer/                       ← panel Spammer, store, hooks
    logs/                          ← game log + tool log (max 200 lines c/u)
    settings/                      ← runner selector, system status banner
  shared/                          ← types, constants, api, hooks Tauri
```

### Tests Rust

```bash
cargo test --workspace    # ro-tools-core, ro-tools-linux, ro-inputd, src-tauri
```

Frontend: `npm test` (vitest) — tests en `servers/servers.logic.test.ts`,
`settings/settings.logic.test.ts`, `spammer/spammer.logic.test.ts`, y `logs/logs.logic.test.ts`.

---

## Data persistence

All data lives under `~/.local/share/ro-launcher/`:

| Path | Purpose |
|------|---------|
| `servers.json` | User's server list |
| `settings.json` | Global settings (`default_runner` path) |
| `prefix/` | Wine prefix directory |
| `prefix/.ro-launcher-configured` | Marker file written after `setup_prefix` completes |

---

## Tauri commands

All commands are `async`. Long-running ones (`setup_prefix`, `launch_game`) spawn a `tokio::spawn` task and return immediately — they communicate progress via events.

### Events emitted to frontend

```
ro-launcher://log            { line: string }       — stdout/stderr Wine
ro-launcher://tool-log       { line: string }       — AutoPot, Spammer, input, launch hints
ro-launcher://progress       { step: string, percent: number }
ro-launcher://game-exit      { code: number }
ro-launcher://autopot-status { AutopotStatusEvent }
ro-launcher://spammer-status { SpammerStatusEvent }
```

### `check_dependencies` → `DependencyStatus`

Checks: `wine-cachyos` or `wine` binary, `winetricks`, DXVK at `{prefix}/drive_c/windows/system32/d3d9.dll`, marker file, audio driver, and `ydotool`+`ydotoold` for AutoPot/Spammer. Also checks `input` group membership (required for `ro-inputd` to grab evdev).

### `setup_prefix`

1. Create WINEPREFIX dir (5%)
2. `wineboot -i` (10%)
3. Install Wine Gecko (20%)
4. `winetricks dxvk` (35%)
5. `winetricks vcrun2019` (55%)
6. `winetricks d3dx9` (75%)
7. `winetricks corefonts` (90%)
8. Configure audio driver (95%)
9. Write marker file (100%)

All subprocess calls set `WINEPREFIX` and `WAYLAND_DISPLAY=""`.

### `launch_game(server: ServerConfig)`

Verifies marker exists, then spawns `wine <exe>` with working dir set to the exe's parent. Pipes stdout/stderr line-by-line as `ro-launcher://log` events. Filters out `fixme:` lines (too noisy). Emits `ro-launcher://game-exit` on process exit.

### `list_runners`

Scans for system Wine (`/usr/bin/wine-cachyos`, `/usr/bin/wine`, `/usr/bin/wine64`) and Proton installations under `~/.steam/root/compatibilitytools.d/`, `~/.local/share/Steam/compatibilitytools.d/`, `/usr/share/steam/compatibilitytools.d/`.

### Spammer commands

- `start_spammer(server)` — requiere `pid` (juego corriendo); inicia `ro-inputd` + loop
- `stop_spammer()` — detiene loop y ro-inputd gracefully
- `update_spammer_config(config)` — requiere `pid`; restart completo
- `get_spammer_status()` → `SpammerStatusEvent` — snapshot sincrónico

---

## Critical env vars

### Game launch (`utils/wine.rs` → `apply_game_env`)

```rust
WINEPREFIX   = "~/.local/share/ro-launcher/prefix"  // or server override
WAYLAND_DISPLAY = ""          // forces Xwayland — black screen without this
DXVK_ASYNC   = "1"
DXVK_CONFIG  = "d3d9.forceSamplerTypeSpecConstants=True"
WINE_LARGE_ADDRESS_AWARE = "1"
WINEDLLOVERRIDES = "d3dimm=n,b;ddraw=n,b"
```

`WAYLAND_DISPLAY=""` is non-negotiable on Hyprland/Wayland. The game uses dgVoodoo (DX11 output) → DXVK (Vulkan). Without DXVK installed in the prefix the screen will be black.

### Launcher UI WebView (`utils/webview.rs`)

Set before GTK/WebKit init (release + dev):

```rust
GDK_BACKEND = "x11"
WEBKIT_DISABLE_DMABUF_RENDERER = "1"
```

Prevents black WebView / GBM buffer errors on Wayland (including AppImage). Skipped if already set in the environment.

---

## ServerConfig — shared type

Rust (`models/server.rs`) and TypeScript (`shared/types.ts`) share the same structure via `serde(rename_all = "camelCase")`:

```typescript
interface ServerConfig {
  id: string
  name: string
  executablePath: string   // absolute path to .exe
  patcherPath?: string
  winePrefix?: string      // per-server prefix override
  runner?: string          // per-server runner override (path to wine/proton binary)
  autopot?: AutopotConfig  // per-server AutoPot settings
  spammer?: SpammerConfig  // per-server Spammer settings
}
```

Perfiles de memoria embebidos en `src-tauri/resources/client_profiles.json` (compatible 4RTools).

---

## Frontend state

Each feature has a Zustand store:

- `servers.store.ts` — `servers[]`, `selectedId`, CRUD + persistence via `list_servers`/`save_servers`
- `launcher.store.ts` — `status: 'idle'|'setting-up'|'launching'|'running'|'error'`, `setupProgress`, `error`
- `logs.store.ts` — `gameLogs[]` + `toolLogs[]` (FIFO, max 200 c/u)
- `settings.store.ts` — `runners[]`, `selectedRunner` (path), persisted via `load_settings`/`save_settings`
- `autopot.store.ts` — estado en vivo vía `ro-launcher://autopot-status`
- `spammer.store.ts` — `status: SpammerStatusEvent`, `busy`, `userEnabled` vía `ro-launcher://spammer-status`

---

## Constraints

- Never block the Tauri main thread — all subprocess calls use `tokio::process::Command`
- Don't hardcode absolute paths outside of `servers.json` (user data) — use `dirs`/`home_dir()` in Rust
- Don't attempt to handle Gepard Shield — it's a server-side concern
- Don't use `std::process::Command` for Wine/winetricks — only `tokio::process::Command`
- Wine log filtering happens in `utils/process.rs` — preserve the `fixme:` filter
- AutoPot domain logic belongs in `ro-tools-core`; OS adapters in `ro-tools-linux`; never invert this
- Spammer keys are restricted to F1–F9 and 0–9; validated in `ro-tools-core/spammer/keys.rs`; `delay_ms` is clamped to 5–100ms
- `ro-inputd` requires the user to be in the `input` group (evdev grab); if not, it exits with a fatal JSON message
- Window is fixed 1280×820px (non-resizable) — don't design UI that needs more space
- AppImage on Arch/CachyOS: build with `npm run tauri:build:appimage` (`NO_STRIP=true`); output at `target/release/bundle/appimage/`
