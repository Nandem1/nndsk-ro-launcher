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
crates/ro-tools-linux/    Adaptadores Linux — memoria /proc, uinput, procesos Wine, keyboard evdev
crates/ro-inputd/         Binario sidecar — grab de teclado via evdev + passthrough uinput (JSON stdio)
src-tauri/src/
  commands/               Handlers Tauri delgados (nombre 1:1 con tools/ cuando aplica)
  tools/                  Servicios de aplicación (una carpeta por feature)
  state/                  Estado compartido (proceso, sesiones, repositorios y avisos)
  models/                 DTOs IPC (serde camelCase, alineados con TypeScript)
  utils/                  Infra compartida (Wine, paths, eventos, JSON, webview) — no mover a tools
```

### Mapa `tools/` (completo)

| Módulo | Rol |
|--------|-----|
| `tools/autopot/` | AutoPot: PID, loop, perfiles |
| `tools/autobuff/` | AutoBuff: PID, reglas, loop y estado live |
| `tools/spammer/` | Spammer: lifecycle ro-inputd, scheduler de 10 ms e input uinput |
| `tools/server_tools/` | OpenSetup, patcher, dgVoodoo |
| `tools/launcher/` | Lanzar/detener juego y cleanup de las tres herramientas |
| `tools/prefix/` | Setup/reset WINEPREFIX (winetricks, marker) |
| `tools/runners/` | Descubrir Wine/Proton instalados |
| `tools/deps/` | Agregar checks de dependencias |
| `tools/input/` | Worker uinput persistente, colas priorizadas, métricas + InputGateway |

`commands/servers.rs` y `commands/settings.rs` delegan en repositorios serializados. Estos
canonicalizan configuraciones legacy, rotan `.bak` y recuperan archivos corruptos sin cambiar
los payloads IPC. `GameProcessHandle` modela `Idle | Launching | Running` con generaciones;
`SessionController` serializa start/replace/stop de AutoPot, AutoBuff y Spammer.

### Añadir una feature nueva

1. Dominio reutilizable → `crates/ro-tools-*` (solo si no depende de Tauri/OS)
2. Orquestación → `tools/<feature>/session.rs` (+ módulos internos)
3. Handler IPC → `commands/<feature>.rs` (delgado)
4. UI → `src/features/<feature>/`

### AutoPot — flujo de capas

```
commands/autopot.rs          invoke handlers (start/stop/config/status)
  → tools/autopot/session.rs  resuelve PID, valida uinput y arranca servicio
    → tools/autopot/service.rs   ciclo de vida (AutopotHandle)
      → tools/autopot/loop_runner.rs   tokio loop + eventos de estado
        → ro-tools-core AutopotEngine   lógica DT_AP (tick HP/SP)
          → ro-tools-linux ProcMemoryReader + CombatUinput
```

### Spammer — flujo de capas

```
commands/spammer.rs          invoke handlers (start/stop/update_config/status)
  → tools/spammer/session.rs  valida uinput preparado, delega a SpammerHandle
    → tools/spammer/service.rs   ciclo de vida (SpammerHandle)
      → tools/spammer/loop_runner.rs   tokio loop; spawna ro-inputd como subprocess
        → crates/ro-inputd              grab evdev, passthrough uinput, JSON stdio
        → ro-tools-core SpammerEngine   tick atómico: key_down → click → key_up
          → tools/input UinputWriter
```

**ro-inputd** es un binario sidecar bundleado junto al ejecutable principal. Lo encuentra en
`find_ro_inputd()` (busca relativo al exe). Comunicación: args `--triggers F1,F2 --json`,
recibe `{"type":"stop"}` por stdin, emite `ready`/`trigger`/`fatal`/`shutdown` por stdout.

El Spammer exige que **el juego esté corriendo**. `update_spammer_config` reemplaza la sesión de
forma serializada: espera el cleanup anterior antes de publicar la nueva.

Teclas válidas para el Spammer: `F1`–`F9`, `0`–`9` y `A`–`Z`. Al mantener **Alt + tecla** el evento
pasa por el passthrough en lugar de activar spam (comportamiento intencional).

### AutoBuff — flujo de capas

AutoBuff comparte resolución de PID, `SessionController`, `InputGateway` y perfiles con AutoPot.
Su configuración activa se actualiza mediante `watch` sin reiniciar el loop.

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
    autobuff/                      ← panel AutoBuff, reglas, store, hooks
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
| `*.json.bak` | Previous valid version used for automatic recovery |
| `*.json.corrupt-*` | Preserved invalid input after a successful recovery |
| `prefix/` | Wine prefix directory |
| `prefix/.ro-launcher-configured` | Marker file written after `setup_prefix` completes |

---

## Tauri commands

Los comandos pueden ser síncronos o asíncronos según su trabajo. Las operaciones largas usan
Tokio y comunican progreso mediante eventos.

### Events emitted to frontend

```
ro-launcher://log            { line: string }       — stdout/stderr Wine
ro-launcher://tool-log       { line: string }       — AutoPot, Spammer, input, launch hints
ro-launcher://progress       { step: string, percent: number }
ro-launcher://game-exit      { code: number }
ro-launcher://autopot-status { AutopotStatusEvent }
ro-launcher://autobuff-status { AutobuffStatusEvent }
ro-launcher://spammer-status { SpammerStatusEvent }
```

### `check_dependencies` → `DependencyStatus`

Checks: `wine-cachyos` or `wine` binary, `winetricks`, DXVK at `{prefix}/drive_c/windows/system32/d3d9.dll`, marker file, audio driver, acceso a `/dev/uinput` y al grupo `input`.

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
- `update_spammer_config(config)` — reemplazo serializado, sin solapar sesiones
- `get_spammer_status()` → `SpammerStatusEvent` — snapshot sincrónico

### `list_client_profiles` → `ClientProfile[]`

Returns the embedded memory address profiles from `src-tauri/resources/client_profiles.json` (compatible with 4RTools format). Used by AutoPot to locate HP/SP values in the game process. Profile selection is per-server via `AutopotConfig.profileId`; `undefined` means auto-detect by matching the game exe name against each profile's `exeNames` globs.

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

## Shared types

Rust (`models/`) and TypeScript (`shared/types.ts`) mirror each other via `serde(rename_all = "camelCase")`.

```typescript
interface ServerConfig {
  id: string
  name: string
  executablePath: string   // absolute path to .exe
  patcherPath?: string
  winePrefix?: string      // per-server prefix override
  runner?: string          // per-server runner override (path to wine/proton binary)
  autopot?: AutopotConfig
  autobuff?: AutobuffConfig
  spammer?: SpammerConfig
}

interface AutopotConfig {
  enabled: boolean
  hpKey: string            // F1–F9 or 0–9
  spKey: string
  hpPercent: number        // trigger threshold
  spPercent: number
  delayMs: number
  profileId?: string       // undefined = auto-detect via exeNames; set to a ClientProfile.id to pin
  hpBaseOverride?: string  // hex string to override base HP address from profile
}

interface SpammerConfig {
  enabled: boolean
  delayMs: number          // clamped 5–100ms
  keys: string[]           // F1–F9, 0–9 or A–Z
}

interface ClientProfile {
  id: string
  label: string
  exeNames: string[]       // glob patterns matched against game exe for auto-detect
  hpBase: number           // memory address for HP base
  nameAddress: number      // memory address for character name
}
```

Memory profiles are embedded in `src-tauri/resources/client_profiles.json`. `tools/autopot/profiles.rs` loads them at startup via `include_str!` and caches with `OnceLock`. `ro-tools-core::resolve_profile` picks the first profile whose `exeNames` matches the running exe.

---

## Frontend state

Each feature has a Zustand store:

- `servers.store.ts` — `servers[]`, `selectedId`, CRUD + persistence via `list_servers`/`save_servers`
- `launcher.store.ts` — `status: 'idle'|'setting-up'|'launching'|'running'|'error'`, `setupProgress`, `error`
- `logs.store.ts` — `gameLogs[]` + `toolLogs[]` (FIFO, max 200 c/u)
- `settings.store.ts` — `runners[]`, `selectedRunner` (path), persisted via `load_settings`/`save_settings`
- `autopot.store.ts` — estado en vivo vía `ro-launcher://autopot-status`
- `autobuff.store.ts` — estado en vivo vía `ro-launcher://autobuff-status`
- `spammer.store.ts` — `status: SpammerStatusEvent`, `busy`, `userEnabled` vía `ro-launcher://spammer-status`

---

## Code style

**TypeScript/React:** 2-space indentation, single quotes, no semicolons. Named exports only. Feature-specific logic stays in `src/features/<domain>/`; shared code goes to `src/shared/`. File naming: `PascalCase.tsx` for components, `useThing.ts` for hooks, `<domain>.store.ts` for Zustand stores, `<domain>.logic.ts` for pure logic modules.

**Rust:** `rustfmt` enforced (`cargo fmt --all`). Snake_case for modules and functions. IPC payloads live in `src-tauri/src/models/`; keep command handlers thin (no business logic).

---

## Constraints

- Never block the Tauri main thread — all subprocess calls use `tokio::process::Command`
- Don't hardcode absolute paths outside of `servers.json` (user data) — use `dirs`/`home_dir()` in Rust
- Don't attempt to handle Gepard Shield — it's a server-side concern
- Don't use `std::process::Command` for Wine/winetricks — only `tokio::process::Command`
- Wine log filtering happens in `utils/process.rs` — preserve the `fixme:` filter
- AutoPot domain logic belongs in `ro-tools-core`; OS adapters in `ro-tools-linux`; never invert this
- Spammer keys are restricted to F1–F9, 0–9 and A–Z; validated in `ro-tools-core/spammer/keys.rs`; `delay_ms` is clamped to 5–100ms
- `ro-inputd` requires the user to be in the `input` group (evdev grab); if not, it exits with a fatal JSON message
- Window is fixed 1280×820px (non-resizable) — don't design UI that needs more space
- AppImage on Arch/CachyOS: build with `npm run tauri:build:appimage` (`NO_STRIP=true`); output at `target/release/bundle/appimage/`
