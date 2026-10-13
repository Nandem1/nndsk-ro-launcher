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
No pongas una ruta de fichero. El pubkey del repo es el material público de verificación.

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

La versión de la app es un solo `X.Y.Z`. El skill [release-ro-launcher](../../.cursor/skills/release-ro-launcher/SKILL.md) y `npm run version:bump` la escriben en los cinco ficheros. No la edites a mano.
Si el usuario no fija versión, el modelo debe elegir y justificar major/minor/patch
según el delta completo desde el último tag publicado del launcher. El criterio
vive en la skill; preparar una versión no autoriza todavía el bump ni publicar.

`v0.1.0` ya está publicado. El siguiente corte es un bump. No muevas ese tag.

Cortes siguientes:

1. Elegir y justificar el bump con la skill, verificar su dry-run y, una vez autorizado, ejecutar `npm run version:bump -- <bump>` (o el `X.Y.Z` explícito del usuario).
2. Commit del bump.
3. Push de `main` y CI verde completo (`quality` y `release-cache`). `ci.yml` no corre en tags `v*.*.*`; `release.yml` se niega a firmar un commit sin ese CI verde.
4. Tag `vX.Y.Z` (debe coincidir con `npm run version:show`).
5. Push del tag. Actions corre `.github/workflows/release.yml`.
6. Revisa el GitHub Release en borrador (`latest.json`, AppImage, `.sig`, `release-smoke.txt`).
7. Escribe las notas que verá el updater en `latest.json` (`notes`).
8. El usuario publica el release cuando el draft esté bien; el agente no lo publica.

`release.yml` tiene cuatro jobs y solo uno ve la clave privada:

1. `release-config` lee la variable pública `RO_LAUNCHER_DISCORD_APPLICATION_ID` del entorno
   `release`. No ejecuta código del repositorio ni referencia secretos.
2. `build` (sin secretos) exige CI verde, corre los checks estáticos y la política de audit,
   compila un AppImage **sin firmar** (`createUpdaterArtifacts:false` vía `--config`; el codegen de
   Tauri no embebe ese campo, el binario es el mismo) y ejecuta
   `scripts/smoke-release-artifact.sh` sobre ese archivo: `ro-launcher` arranca por `AppRun`,
   `ro-inputd` responde su protocolo JSON y el `ro-sessiond` extraído pasa `orphan_reparent`
   (exige `ptrace_scope=1`). Sube el AppImage probado y publica su SHA-256 como output.
3. `sign` (entorno `release`) instala el CLI fijado con `npm ci --ignore-scripts`, comprueba el
   SHA-256, y un único paso recibe la clave para ejecutar
   `tauri signer sign --app-version X.Y.Z`. Luego verifica la firma con el pubkey embebido.
4. `publish` (`contents: write`, sin clave) vuelve a verificar hash y firma con
   `scripts/updater-release.mjs`, crea el draft, sube AppImage, `.sig`, `RO-Launcher_amd64.AppImage.sha256`
   y `release-smoke.txt`, y genera `latest.json` con `linux-x86_64` y `linux-x86_64-appimage`
   apuntando a la URL de API del asset (igual que hacía `tauri-action`).

Si `publish` falla después de crear el draft, borra ese draft antes de relanzar el job: la
creación falla a propósito si el release ya existe.
`scripts/check-release-invariants.mjs` rechaza volver a `tauri-action`, referenciar la clave fuera
del paso de firma, acciones sin SHA, `continue-on-error`, `|| true` o el opt-out de Yama.

## Actualizar los SHA de GitHub Actions

Todas las `uses:` remotas están fijadas a un commit completo con el tag revisado como comentario.
Para actualizar una, revisa el changelog del tag nuevo y resuelve su commit:

```bash
git ls-remote https://github.com/actions/checkout 'refs/tags/v4^{}' 'refs/tags/v4'
```

Usa el commit pelado (`^{}`) cuando el tag es anotado. Sustituye SHA y comentario en
`ci.yml`, `release.yml`, `dependency-audit.yml` y `.github/actions/*` a la vez, y ejecuta
`node scripts/check-release-invariants.mjs`.
Publicar el draft es lo que hace visible
`https://github.com/Nandem1/nndsk-ro-launcher/releases/latest/download/latest.json`
para los AppImage instalados.

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
