# Servers

Servers lets a user see the saved server list and open a dialog to add one. With an empty isolated home the list tells the user to add a server. Cancelling the dialog leaves the list empty.

## Sub-features

- `servers-empty` shows the empty list and the add button.
- `servers-open-add` opens the add dialog from that button.
- `servers-cancel-add` closes the dialog and does not create a server.

## How to get to it (user POV)

- Look at the `Servidor` panel in the left rail.
- Choose the `Agregar servidor` button in that panel.
- Choose `Cerrar configuración` in the dialog, or press the backdrop behind the dialog.

## Driving it with verify-ro-launcher

Preconditions:

- `doctor` reports the isolated data directory and a page on port `5173`.
- `/tmp/ro-launcher-verify/home/.local/share/ro-launcher/servers.json` is absent, or it is `[]`.
- No add dialog is open.

- **Empty list.** Capture the rail before opening the dialog. Run `node .cursor/skills/verify-ro-launcher/scripts/control.mjs snapshot --aria --path /tmp/ro-launcher-verify/evidence/servers/empty.aria.txt`. The snapshot contains heading `Servidor`, button `Agregar servidor`, and the text `Sin servidores — agrega uno con +`.
- **Open dialog.** Choose `Agregar servidor`. Run `node .cursor/skills/verify-ro-launcher/scripts/control.mjs click --role button --name "Agregar servidor"`. A later snapshot contains heading `Agregar servidor`, button `Cerrar configuración`, and button `Agregar`.
- **Cancel.** Choose `Cerrar configuración`. Run `node .cursor/skills/verify-ro-launcher/scripts/control.mjs click --role button --name "Cerrar configuración"`. The dialog headings are gone and the empty-list text is back.
- **No write.** Read `/tmp/ro-launcher-verify/home/.local/share/ro-launcher/servers.json`. It is still absent or `[]`.
- **Proof.** Run `node .cursor/skills/verify-ro-launcher/scripts/control.mjs screenshot --path /tmp/ro-launcher-verify/evidence/servers/empty.png` after the cancel. The image shows the `RO-Launcher` heading and the empty `Servidor` panel.

## Gotchas

- Saving with `Agregar` writes `servers.json` in the isolated directory. This recipe stops at cancel. Do not fill the form unless a later proof creates a disposable server and deletes that file afterward.
- The game executable picker opens a native file dialog. Do not drive that dialog from the inspector.
- `Agregar servidor` is the button's accessible name. The visible control is only a plus icon.
