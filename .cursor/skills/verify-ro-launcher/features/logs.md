# Logs

Logs lets a user switch the bottom panel between the game channel and the tools channel. Each channel has its own empty message. Switching channels does not change servers or settings.

## Sub-features

- `logs-game` shows the game channel empty copy.
- `logs-tools` shows the tools channel empty copy.
- `logs-return` restores the game channel.

## How to get to it (user POV)

- Look at the `Logs` panel under the tool view.
- Choose the `Juego` button in that panel.
- Choose the `Tools` button in that panel.

## Driving it with verify-ro-launcher

Preconditions:

- `doctor` reports the isolated data directory and a page on port `5173`.
- This run has not launched a game and has not emitted tool output, so both channels are empty.

- **Game channel.** The panel opens on `Juego`. Run `node .cursor/skills/verify-ro-launcher/scripts/control.mjs snapshot --aria --path /tmp/ro-launcher-verify/evidence/logs/game.aria.txt`. The snapshot contains heading `Logs`, button `Juego`, and the text `Wine / setup / lanzamiento...`.
- **Tools channel.** Choose `Tools`. Run `node .cursor/skills/verify-ro-launcher/scripts/control.mjs click --role button --name "Tools"`. A snapshot at `/tmp/ro-launcher-verify/evidence/logs/tools.aria.txt` contains `AutoPot / PID / memoria...` and does not contain `Wine / setup / lanzamiento...`.
- **Return.** Choose `Juego`. Run `node .cursor/skills/verify-ro-launcher/scripts/control.mjs click --role button --name "Juego"`. The game empty copy is back.
- **Proof.** Run `node .cursor/skills/verify-ro-launcher/scripts/control.mjs screenshot --path /tmp/ro-launcher-verify/evidence/logs/tools.png` while the tools channel is visible. The image shows the `RO-Launcher` heading and the tools empty copy.

## Gotchas

- `Tools` is also a word in other sentences. Click `--name "Tools"` matches the button label exactly, not a substring.
- Log lines are in memory for this window. Cleanup discards them. The screenshot is the retained proof.
- A startup error can add a line before the empty copy. If the snapshot contains an error line, record it and do not describe the channel as empty.
