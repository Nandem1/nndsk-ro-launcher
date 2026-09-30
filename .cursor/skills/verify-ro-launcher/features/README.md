# RO-Launcher verification map

This directory is the maintained source for verifying the user-facing behavior of RO-Launcher. Read the index before driving the app, then use the matching feature file as the recipe.

## Baseline preconditions

- Launch with `node .cursor/skills/verify-ro-launcher/scripts/control.mjs launch` from the repository root.
- The isolated data directory is `/tmp/ro-launcher-verify/home/.local/share/ro-launcher`.
- `doctor` must report that URL, that data directory, and a listener owned by this run.
- Never drive an `RO-Launcher` window that was not started by this verification run. The installed AppImage shares the window title and the real `~/.local/share/ro-launcher` directory.
- Start from the empty isolated home unless the feature file says otherwise. Do not copy the user's servers, prefixes, or settings into it.
- Only one verification instance can exist. Ports `5173` and `9222` are exclusive.

## Driving conventions

- Start every recipe from the baseline state unless its preconditions say otherwise.
- Prefer the button name, heading text, or `aria-pressed` / `aria-checked` value over CSS selectors and coordinates.
- Treat every command as literal. Keep quoted names and flags unchanged.
- Run browser actions through `node .cursor/skills/verify-ro-launcher/scripts/control.mjs`.
- Restore mutated isolated files after a write. Do not remove proof artifacts during cleanup.

## Proof and skip reporting

- Capture the user action and the resulting state, not only the final screen.
- UI proof includes the `click` JSON, an ARIA snapshot, and a screenshot that shows the `RO-Launcher` heading.
- A settings or server write also needs the isolated JSON file read back after the click.
- Record the feature ID and entry point used with every artifact.
- Report an unreachable path with the attempted command and the unmet precondition.
- Do not report a skipped entry point as verified through a different path.
- Do not launch Wine, reset a prefix, or install dgVoodoo as part of a UI proof.

## Feature entry contract

Each feature file starts with an H1 title and one paragraph describing the user-visible behavior. It then uses exactly four H2 sections in this order.

1. `Sub-features` lists short IDs with one line for each behavior.
2. `How to get to it (user POV)` lists every user entry point.
3. `Driving it with verify-ro-launcher` starts with `Preconditions:` and uses labeled bullets that pair each user action with an exact command and observable result.
4. `Gotchas` lists traps that can waste or invalidate a verification run.

## Features

- [Tool views](./tool-views.md) covers the Combate and Buffs tabs, the panels each one shows, and the return to Combate.
- [Servers](./servers.md) covers the empty list, opening the add dialog, and cancelling it without saving.
- [Launch readiness](./launch-readiness.md) covers the disabled launch control and the tools placeholder when no server exists.
- [Logs](./logs.md) covers the Juego and Tools channels and their empty copy.
- [Discord presence](./discord-presence.md) covers the idle toggle and the isolated settings file it writes.
