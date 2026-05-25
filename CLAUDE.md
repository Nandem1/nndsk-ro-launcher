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

# Production build
npm run tauri:build

# Frontend only (no Tauri window)
npm run dev

# Type-check
npx tsc --noEmit
```

`tauri:dev` sets `GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1` — both are required to prevent a black WebView window on Wayland.

---

## Architecture

Cargo workspace con tres capas en el backend. El frontend usa Feature-Sliced Design.

### Capas backend (de adentro hacia afuera)

```
crates/ro-tools-core/     Dominio puro — sin OS (engine, config, ports, profiles, dgvoodoo)
crates/ro-tools-linux/    Adaptadores Linux — memoria /proc, ydotool, procesos Wine
src-tauri/src/
  commands/               Handlers Tauri delgados (nombre 1:1 con tools/ cuando aplica)
  tools/                  Servicios de aplicación (una carpeta por feature)
  state/                  Estado compartido (GameState: pid, autopot, ydotoold)
  models/                 DTOs IPC (serde camelCase, alineados con TypeScript)
  utils/                  Infra compartida (Wine, paths, eventos, JSON) — no mover a tools
```

### Mapa `tools/` (completo)

| Módulo | Rol |
|--------|-----|
| `tools/autopot/` | AutoPot: PID, loop, perfiles |
| `tools/server_tools/` | OpenSetup, patcher, dgVoodoo |
| `tools/launcher/` | Lanzar/detener juego, cleanup AutoPot |
| `tools/prefix/` | Setup/reset WINEPREFIX (winetricks, marker) |
| `tools/runners/` | Descubrir Wine/Proton instalados |
| `tools/deps/` | Agregar checks de dependencias |
| `tools/input/` | Ciclo de vida ydotoold |

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
    logs/                          ← game log + tool log (max 200 lines c/u)
    settings/                      ← runner selector, system status banner
  shared/                          ← types, constants, api, hooks Tauri
```

### Tests Rust

```bash
cargo test --workspace    # ro-tools-core, ro-tools-linux, src-tauri
```

Frontend: `npx tsc --noEmit` (sin framework de tests configurado aún).

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
ro-launcher://tool-log       { line: string }       — AutoPot, input, launch hints
ro-launcher://progress       { step: string, percent: number }
ro-launcher://game-exit      { code: number }
ro-launcher://autopot-status { AutopotStatusEvent }
```

### `check_dependencies` → `DependencyStatus`

Checks: `wine-cachyos` or `wine` binary, `winetricks`, DXVK at `{prefix}/drive_c/windows/system32/d3d9.dll`, marker file, audio driver, and `ydotool`+`ydotoold` for AutoPot.

### `setup_prefix`

1. Create WINEPREFIX dir (10%)
2. `wineboot -i` (20%)
3. `winetricks dxvk` (40%)
4. `winetricks vcrun2019` (70%)
5. Write marker file (100%)

All subprocess calls set `WINEPREFIX` and `WAYLAND_DISPLAY=""`.

### `launch_game(server: ServerConfig)`

Verifies marker exists, then spawns `wine <exe>` with working dir set to the exe's parent. Pipes stdout/stderr line-by-line as `ro-launcher://log` events. Filters out `fixme:` lines (too noisy). Emits `ro-launcher://game-exit` on process exit.

### `list_runners`

Scans for system Wine (`/usr/bin/wine-cachyos`, `/usr/bin/wine`, `/usr/bin/wine64`) and Proton installations under `~/.steam/root/compatibilitytools.d/`, `~/.local/share/Steam/compatibilitytools.d/`, `/usr/share/steam/compatibilitytools.d/`.

---

## Critical env vars for game launch

```rust
WINEPREFIX   = "~/.local/share/ro-launcher/prefix"  // or server override
WAYLAND_DISPLAY = ""          // forces Xwayland — black screen without this
DXVK_ASYNC   = "1"
DXVK_CONFIG  = "d3d9.forceSamplerTypeSpecConstants=True"
```

`WAYLAND_DISPLAY=""` is non-negotiable on Hyprland/Wayland. The game uses dgVoodoo (DX11 output) → DXVK (Vulkan). Without DXVK installed in the prefix the screen will be black.

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

---

## Constraints

- Never block the Tauri main thread — all subprocess calls use `tokio::process::Command`
- Don't hardcode absolute paths outside of `servers.json` (user data) — use `dirs`/`home_dir()` in Rust
- Don't attempt to handle Gepard Shield — it's a server-side concern
- Don't use `std::process::Command` for Wine/winetricks — only `tokio::process::Command`
- Wine log filtering happens in `utils/process.rs` — preserve the `fixme:` filter
- AutoPot domain logic belongs in `ro-tools-core`; OS adapters in `ro-tools-linux`; never invert this
- Window is fixed 500×720px (non-resizable) — don't design UI that needs more space
