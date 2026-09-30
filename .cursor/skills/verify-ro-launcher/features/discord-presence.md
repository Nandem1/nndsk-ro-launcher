# Discord presence

Discord presence lets a user turn Rich Presence on or off from the left rail. With no game client, the toggle only stores the choice. It does not publish a Discord activity.

## Sub-features

- `presence-off` starts unchecked on an empty home.
- `presence-on` stores `richPresenceEnabled: true` in the isolated settings file.
- `presence-off-again` stores `richPresenceEnabled: false`.

## How to get to it (user POV)

- Look at the `Discord Rich Presence` panel in the left rail.
- Choose the switch in that panel.

## Driving it with verify-ro-launcher

Preconditions:

- `doctor` reports the isolated data directory and a page on port `5173`.
- No game client is running in this verification instance.
- `/tmp/ro-launcher-verify/home/.local/share/ro-launcher/settings.json` either does not exist or has `"richPresenceEnabled": false`.

- **Initial off.** Run `node .cursor/skills/verify-ro-launcher/scripts/control.mjs snapshot --aria --path /tmp/ro-launcher-verify/evidence/discord-presence/off.aria.txt`. The snapshot contains heading `Discord Rich Presence` and `switch` with `checked=false`.
- **Turn on.** Choose the switch. Run `node .cursor/skills/verify-ro-launcher/scripts/control.mjs click --role switch --within-heading "Discord Rich Presence"`. The click JSON has `"ok": true` and `"ariaChecked": "true"`.
- **Stored value.** Read `/tmp/ro-launcher-verify/home/.local/share/ro-launcher/settings.json`. `richPresenceEnabled` is `true`. A snapshot at `/tmp/ro-launcher-verify/evidence/discord-presence/on.aria.txt` shows `checked=true`.
- **Turn off.** Run the same click command again. `ariaChecked` is `false` and `settings.json` has `"richPresenceEnabled": false`.
- **Proof.** Run `node .cursor/skills/verify-ro-launcher/scripts/control.mjs screenshot --path /tmp/ro-launcher-verify/evidence/discord-presence/on.png` while the switch is on. The image shows the `RO-Launcher` heading and the Discord panel.

## Gotchas

- The switch has no accessible name. `--within-heading "Discord Rich Presence"` is what scopes the click. A bare `--role switch` would hit the first switch on the page.
- The panel copy mentions Discord even while the switch is off. The proof is `aria-checked` plus `settings.json`.
- With no running client, enabling the switch does not open a Discord IPC socket. Do not describe a missing socket as a failure.
- Do not point this recipe at the user's real `~/.local/share/ro-launcher/settings.json`.
