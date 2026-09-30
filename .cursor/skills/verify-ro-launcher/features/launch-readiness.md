# Launch readiness

Launch readiness tells the user whether a server can be started. With no server selected, the launch control stays disabled and the tools panel asks for a server. This check does not start Wine.

## Sub-features

- `launch-disabled` keeps the launch control disabled when the list is empty.
- `launch-tools-placeholder` shows the tools panel empty state for the same home.

## How to get to it (user POV)

- Look at the launch button at the bottom of the left rail.
- Look at the `Herramientas` panel above the tool tabs.

## Driving it with verify-ro-launcher

Preconditions:

- `doctor` reports the isolated data directory and a page on port `5173`.
- The server list is empty.
- No game client is running inside this verification instance.

- **Disabled control.** Capture the rail. Run `node .cursor/skills/verify-ro-launcher/scripts/control.mjs snapshot --aria --path /tmp/ro-launcher-verify/evidence/launch-readiness/idle.aria.txt` and `node .cursor/skills/verify-ro-launcher/scripts/control.mjs screenshot --path /tmp/ro-launcher-verify/evidence/launch-readiness/idle.png`. The launch button is `disabled`. The tools snapshot contains heading `Herramientas` and the text `Selecciona un servidor`. The screenshot shows the `RO-Launcher` heading.
- **Do not activate it.** Do not `click` the launch button. A disabled control has no user result, and an enabled one would leave this UI check and start environment setup.

## Gotchas

- The disabled label can read `Preparar entorno` before any server exists. The proof is the `disabled` flag plus the tools placeholder, not the label alone.
- Clicking an enabled `Jugar` or `Preparar entorno` starts runner and prefix work. That is outside this feature.
- The installed AppImage can show a real server and an enabled `Jugar` button at the same time. `doctor` must still point at port `5173` and the isolated home before this snapshot counts.
