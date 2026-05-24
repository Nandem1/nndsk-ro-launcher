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

No test framework is configured.

---

## Architecture

Feature-Sliced Design. Each feature owns its store + UI components and is self-contained.

```
src/
  app/App.tsx                      ← root layout, calls loadServers() on mount
  features/
    servers/                       ← server list, add/remove, selection
    launcher/                      ← launch flow, progress, error states
    logs/                          ← real-time log panel (max 200 lines)
    settings/                      ← runner selector, global settings
src-tauri/src/
  lib.rs                           ← Tauri app init + generate_handler! registration
  commands/
    check.rs    → check_dependencies()
    setup.rs    → setup_prefix()
    launch.rs   → launch_game()
    servers.rs  → list_servers(), save_servers()
    runners.rs  → list_runners()
    settings.rs → load_settings(), save_settings()
  models/server.rs                 ← ServerConfig struct (serde camelCase)
```

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
ro-launcher://log        { line: string }
ro-launcher://progress   { step: string, percent: number }
ro-launcher://game-exit  { code: number }
ro-launcher://error      { message: string }
```

### `check_dependencies` → `DependencyStatus`

Checks: `wine-cachyos` or `wine` binary, `winetricks`, DXVK at `{prefix}/drive_c/windows/system32/d3d9.dll`, and the configured marker file.

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

Rust (`models/server.rs`) and TypeScript (`features/servers/servers.config.ts`) share the same structure via `serde(rename_all = "camelCase")`:

```typescript
interface ServerConfig {
  id: string
  name: string
  executablePath: string   // absolute path to .exe
  patcherPath?: string
  winePrefix?: string      // per-server prefix override
  runner?: string          // per-server runner override (path to wine/proton binary)
}
```

---

## Frontend state

Each feature has a Zustand store:

- `servers.store.ts` — `servers[]`, `selectedId`, CRUD + persistence via `list_servers`/`save_servers`
- `launcher.store.ts` — `status: 'idle'|'setting-up'|'launching'|'running'|'error'`, `setupProgress`, `error`
- `logs.store.ts` — `logs: string[]` (FIFO, max 200), `addLog`, `clearLogs`
- `settings.store.ts` — `runners[]`, `selectedRunner` (path), persisted via `load_settings`/`save_settings`

---

## Constraints

- Never block the Tauri main thread — all subprocess calls use `tokio::process::Command`
- Don't hardcode absolute paths outside of `servers.json` (user data) — use `dirs`/`home_dir()` in Rust
- Don't attempt to handle Gepard Shield — it's a server-side concern
- Don't use `std::process::Command` for Wine/winetricks — only `tokio::process::Command`
- Wine log filtering happens in `launch.rs` — preserve the `fixme:` filter
- Window is fixed 500×720px (non-resizable) — don't design UI that needs more space
