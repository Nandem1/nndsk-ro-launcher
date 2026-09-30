# Tool views

Tool views let a user switch the main tools between Combate and Buffs. Combate shows AutoPot and Spammer together. Buffs replaces them with AutoBuff. The choice stays on screen until the user picks the other tab.

## Sub-features

- `tool-views-combat` shows AutoPot and Spammer, with Combate pressed.
- `tool-views-buffs` shows AutoBuff and releases Combate.
- `tool-views-return` restores AutoPot and Spammer from Buffs.

## How to get to it (user POV)

- Choose the `Combate` button in the tool tab strip.
- Choose the `Buffs` button in the tool tab strip.

## Driving it with verify-ro-launcher

Preconditions:

- `doctor` reports the isolated data directory and a page on port `5173`.
- The page shows the `RO-Launcher` heading and `Ragnarok Online`.
- No server is required. Both tabs render their idle copy.

- **Combat baseline.** Leave the initial tab untouched. Run `node .cursor/skills/verify-ro-launcher/scripts/control.mjs snapshot --aria --path /tmp/ro-launcher-verify/evidence/tool-views/combat.aria.txt` and `node .cursor/skills/verify-ro-launcher/scripts/control.mjs screenshot --path /tmp/ro-launcher-verify/evidence/tool-views/combat.png`. The snapshot has `button "Combate" pressed=true`, `button "Buffs" pressed=false`, and headings `AutoPot` and `Spammer`. It has no `AutoBuff` heading.
- **Open Buffs.** Choose `Buffs`. Run `node .cursor/skills/verify-ro-launcher/scripts/control.mjs click --role button --name "Buffs"`. The click JSON has `"ok": true` and `"ariaPressed": "true"`.
- **Buffs result.** Capture the new panels. Run `node .cursor/skills/verify-ro-launcher/scripts/control.mjs snapshot --aria --path /tmp/ro-launcher-verify/evidence/tool-views/buffs.aria.txt` and `node .cursor/skills/verify-ro-launcher/scripts/control.mjs screenshot --path /tmp/ro-launcher-verify/evidence/tool-views/buffs.png`. The snapshot has `button "Buffs" pressed=true`, `button "Combate" pressed=false`, and heading `AutoBuff`. `AutoPot` and `Spammer` are gone. Both screenshots show the `RO-Launcher` heading.
- **Return to Combate.** Choose `Combate`. Run `node .cursor/skills/verify-ro-launcher/scripts/control.mjs click --role button --name "Combate"`. The click JSON has `"ariaPressed": "true"`. A new snapshot again contains `AutoPot` and `Spammer` and does not contain `AutoBuff`.

## Gotchas

- The tab buttons expose `aria-pressed`. The visible amber style is not the proof.
- AutoBuff's idle line `Selecciona un servidor` is expected without a server. It does not mean the tab failed.
- The installed AppImage has the same tabs. A snapshot is valid only after `doctor` ties port `9222` to this run.
- Switching tabs does not write `settings.json` or `servers.json`.
