---
name: verify-ro-launcher
description: Drive the RO-Launcher Tauri desktop app through an isolated dev webview and prove UI behavior. Use when a change touches the launcher shell, server list, tool tabs, logs, launch readiness, or Discord presence, and the proof has to come from the real window rather than unit tests.
---

# Verify RO-Launcher

RO-Launcher is a Linux Tauri desktop app. The user-facing surface is the window titled `RO-Launcher`. During verification that window loads the Vite dev server at `http://localhost:5173` inside WebKitGTK. There is no separate user CLI.

The installed AppImage may already be open. It uses the same window title and the real data directory `~/.local/share/ro-launcher`. Never drive it, never match it by title, and never kill a process named `ro-launcher`. This skill starts a second, isolated `tauri dev` and talks only to the WebKit inspector it owns.

Only one verification instance can run. Vite refuses another server on port `5173`, and the inspector uses port `9222`.

## Launch

From the repository root:

```bash
node .cursor/skills/verify-ro-launcher/scripts/control.mjs launch
```

That command is the verification start. It keeps Cargo and npm caches on the real home, then runs `npm run tauri:dev` with:

- `HOME=/tmp/ro-launcher-verify/home`
- `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, and `XDG_CACHE_HOME` under that home
- `GDK_BACKEND=x11`
- `WEBKIT_DISABLE_DMABUF_RENDERER=1`
- `WEBKIT_INSPECTOR_HTTP_SERVER=127.0.0.1:9222`

`npm run tauri:dev` builds `ro-inputd` and `ro-sessiond`, then starts Tauri. The app data directory for this run is `/tmp/ro-launcher-verify/home/.local/share/ro-launcher`.

Ready means `launch` exits 0 and prints JSON with `"ok": true`, a `url` on port `5173`, and no `problems`. The page text includes `Ragnarok Online`. The log is `/tmp/ro-launcher-verify/tauri.log`. A cold Rust compile can take a few minutes; the helper waits up to 240 seconds.

If `launch` reports that port `5173` or `9222` is taken, do not start another copy. Run `cleanup` only when the state file belongs to a previous verification run.

## Doctor

Run this before driving whenever the UI looks stale, a command fails, or the session is not one you just launched:

```bash
node .cursor/skills/verify-ro-launcher/scripts/control.mjs doctor
```

Exit 0 means this instance is worth driving. The report must show all of the following:

- `ok` is true
- `pid` is alive and is the `npm run tauri:dev` process this skill started
- the listener on `127.0.0.1:9222` is a descendant of that pid
- that listener's `HOME` is `/tmp/ro-launcher-verify/home`
- `dataDir` is `/tmp/ro-launcher-verify/home/.local/share/ro-launcher`
- `url` contains `:5173`

Any other shape means stop. Do not click the installed app to "finish" the proof.

## Drive

The helper clicks and reads the dev webview through the inspector. Prefer accessible names and visible labels. Do not use coordinates.

```bash
node .cursor/skills/verify-ro-launcher/scripts/control.mjs click --role button --name "Buffs"
node .cursor/skills/verify-ro-launcher/scripts/control.mjs click --role switch --within-heading "Discord Rich Presence"
node .cursor/skills/verify-ro-launcher/scripts/control.mjs snapshot --aria --path /tmp/ro-launcher-verify/evidence/tool-views/buffs.aria.txt
node .cursor/skills/verify-ro-launcher/scripts/control.mjs screenshot --path /tmp/ro-launcher-verify/evidence/tool-views/buffs.png
```

`click` prints JSON. `ok: false` means the control was not found; the `candidates` list is the set of labels that were actually present. A switch inside a panel has no name of its own: pass `--within-heading` with the panel's `h2` text and omit `--name` only in that case.

`snapshot --aria` writes an indented tree of buttons, links, fields, headings, paragraphs, and switches, including `pressed`, `checked`, and `disabled`. The paragraph text is the DOM text, so a CSS uppercase label still appears as authored (`AutoBuff`, `Juego`). `screenshot` writes a PNG of the webview.

Read `.cursor/skills/verify-ro-launcher/features/README.md` and the feature file before choosing a path. A proof of one entry point does not cover the others listed there.

## Evidence

Save proof under `/tmp/ro-launcher-verify/evidence/<feature-id>/`. Capture the action and the resulting state:

- the `click` JSON
- an ARIA snapshot from before the action when the feature has a prior state
- an ARIA snapshot and a PNG after the action
- for a write, a second view of the stored file under `dataDir`

The PNG must show the `RO-Launcher` heading. Assert the resulting control state (`pressed` or `checked`) and the panel that appeared or disappeared. Do not treat a unit test, a Zustand write, or a Tauri command called from a script as the user path.

Do not launch the game, reset a prefix, or install dgVoodoo from this harness. Those paths start Wine and can touch runners outside the isolated home. The safe UI proof stops at the disabled or idle control.

Discord presence only writes `richPresenceEnabled` in the isolated `settings.json` when no game client is running. It does not open a Discord socket until a client exists. Still confirm the file instead of trusting the toggle animation.

## Cleanup

```bash
node .cursor/skills/verify-ro-launcher/scripts/control.mjs cleanup
```

This sends `SIGTERM`, then `SIGKILL`, to the process group recorded in `/tmp/ro-launcher-verify/run.json`, and only if that pid's `HOME` is still `/tmp/ro-launcher-verify/home`. It deletes `/tmp/ro-launcher-verify/home` and the state file. It leaves `/tmp/ro-launcher-verify/evidence/` in place. After cleanup, the evidence files must still exist and the installed AppImage, if it was running, must still be running.

Run `cleanup` after a failed launch too, so a broken attempt does not keep ports `5173` and `9222`.

## Helpers

The only helper is `.cursor/skills/verify-ro-launcher/scripts/control.mjs`. Invoke it with `node` from the repository root. Commands: `launch`, `doctor`, `click`, `snapshot`, `screenshot`, `cleanup`.
