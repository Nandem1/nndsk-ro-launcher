# Actualizaciones del AppImage (Tauri updater)

Cómo firmar, publicar y rotar la clave del updater oficial de RO-Launcher.

## Generar el par de claves

Ejecuta fuera del repositorio (el privado nunca entra en git):

```bash
npm run tauri signer generate -- -w ~/.tauri/nndsk-ro-launcher.key --ci -p ''
```

La CLI escribe:

- privado: `~/.tauri/nndsk-ro-launcher.key`
- público: `~/.tauri/nndsk-ro-launcher.key.pub`

Copia el **contenido** del `.pub` a `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`.
No pongas una ruta de fichero. El valor del repo es una clave local de verificación;
antes de publicar v0.1.0 genera el par de producción y sustituye el pubkey.

`plugins.updater.requireSignedVersion` está en `true`. Hace falta `@tauri-apps/cli`
2.12 o posterior. Ese CLI escribe `version:` en el comentario de confianza de
minisign. Un `latest.json` que anuncie otra versión con un artefacto viejo pero
bien firmado se rechaza. El CLI 2.11.2 firmaba sin `version:` y el plugin
rechazaría todos los updates.

## Secretos y variables de GitHub

En el entorno `release` del repositorio:

| Nombre | Tipo | Uso |
| --- | --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | Secret | Contenido del `.key` privado |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Secret | Puede quedar vacío si la clave no tiene contraseña |
| `RO_LAUNCHER_DISCORD_APPLICATION_ID` | Variable | Solo dígitos; se embebe en el binario de release |

No subas el privado, la contraseña ni un Application Secret / bot token.

## Ciclo de release

1. Sube la versión en `package.json`, `src-tauri/tauri.conf.json` y `src-tauri/Cargo.toml` (mismo valor).
2. Confirma invariantes: `node scripts/check-release-invariants.mjs`.
3. Commit en la rama de entrega.
4. Crea el tag `vX.Y.Z` (debe coincidir con las tres versiones).
5. Push del tag. Actions corre `.github/workflows/release.yml`.
6. Revisa el GitHub Release en borrador (`latest.json`, AppImage, `.sig`).
7. Publica el release cuando el draft esté bien.

El job limpia `DISCORD_APPLICATION_ID` heredado, exige el ID numérico y la clave de firma,
compila sidecars en release, firma el AppImage y comprueba `ro-inputd` / `ro-sessiond` en `usr/bin`.

## Rotación de pubkey

Si cambias `plugins.updater.pubkey`, los clientes que aún llevan el pubkey antiguo
rechazarán firmas nuevas como no auténticas. Planifica un release firmado con la clave
vieja que ya embeba el pubkey nuevo, o pide reinstalar el AppImage una vez.

## AppImageLauncher

El updater sustituye el fichero `APPIMAGE` en su sitio. No llama a `ail-cli`.
La entrada de escritorio de AppImageLauncher sigue apuntando a la misma ruta.
Si el tempdir no está en el mismo mount, la fase pasa a error de instalación.

En desarrollo (`tauri:dev`) la fase es `unavailable` / AppImage-only: no hay red ni
reemplazo de `current_exe`.
