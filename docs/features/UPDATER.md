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

Instalar y reiniciar son operaciones distintas. La instalación atestigua que no hay
clientes antes de descargar y otra vez antes de la activación del AppImage en el host.
No cierra instalaciones guiadas, presencia, herramientas, sesiones ni input; tanto
un error de instalación como `readyToRestart` dejan el launcher utilizable.

El reinicio exige `readyToRestart`. Bajo el mutex del registro de clientes comprueba
que no hay clientes iniciando, ejecutando o deteniéndose y reserva la admisión de
lanzamientos. Luego reserva la admisión de instalaciones guiadas bajo su propio
mutex, solo si no hay workers activos. Si esa segunda reserva falla, se libera la
primera. Las reservas no cancelan trabajos y su rollback no revierte el cierre
terminal de instalaciones de una salida real. Ambas permanecen cerradas al llamar
a `AppHandle::restart()`; `RunEvent::Exit` es el dueño de la limpieza de servicios.
Los fallos de limpieza se registran en `diagnostics.log`.

En Tauri 2.11.2 `AppHandle::restart() -> !` no retorna: desde el worker del comando
solicita la salida al event loop, que entrega `RunEvent::Exit` antes de relanzar.
Si falla esa solicitud, Tauri usa su reinicio directo; esa ruta de emergencia no
garantiza el callback de salida. No se ha calificado todavía el flujo completo
con AppImages firmados.

Límite del hardening F-02: estas reservas cubren clientes y ejecución de instalaciones
guiadas. Preparación/reset de prefixes, importación de artefactos y herramientas de
servidor conservan sus propios contratos; aún pueden entrar antes de que el cierre
de sesiones de salida los rechace. Según `FINAL_HARDENING_PLAN.md`, F-02 puede
integrarse primero, pero no debe publicarse una estable sin la recuperación F-05/F-06.
F-04 impide un segundo desktop sobre la misma raíz de datos; los guards por recurso
siguen coordinando la CLI de importación.

El updater sustituye el fichero `APPIMAGE` en su sitio. No llama a `ail-cli`.
La entrada de escritorio de AppImageLauncher sigue apuntando a la misma ruta.
La activación usa un temporal hermano en el mismo directorio, sin fallback a copy/delete.

En desarrollo (`tauri:dev`) la fase es `unavailable` / AppImage-only: no hay red ni
reemplazo de `current_exe`.

## Activación atómica del AppImage (F-03)

Tauri conserva el endpoint, el manifiesto, la descarga y la verificación minisign,
incluida la coincidencia con la versión firmada (`requireSignedVersion`). El
manifiesto por sí solo no está firmado. Solo después de que `Update::download`
devuelva bytes autenticados, el launcher activa el AppImage; no llama a
`Update::install` ni añade otro verificador criptográfico.

El destino viene de `packaged_appimage(app)`: el `APPIMAGE` capturado en `Env` del
host por Tauri, la misma fuente que el plugin 2.12.0 usa para su `extract_path`
privado. Ni el WebView ni el manifiesto eligen una ruta. Se exige una ruta absoluta
a un archivo regular ejecutable y se rechaza un destino symlink. Se comprueba la
identidad de formato del payload: cabecera ELF64 little-endian completa y marcador
type-2 `AI\x02` en offset 8, como el artefacto de este producto y la
[especificación AppImage](https://github.com/AppImage/AppImageSpec/blob/master/draft.md#type-2-image-format).
Gzip, tar y otros paquetes fallan cerrados; no se extraen archivos. Esta defensa
de formato no interpreta todo ELF/SquashFS; la autenticidad y la integridad de la
descarga siguen siendo responsabilidad exclusiva de Tauri.

Protocolo, con el gate F-01 retenido:

1. Validar el payload sin mutación; inspeccionar destino y guardar su modo.
2. Abrir el directorio padre para su fsync; rechazar un padre symlink.
3. Resolver el nombre reservado `.<nombre>.update-tmp` en ese mismo directorio.
   Retirar un temporal regular anterior del mismo usuario y con un solo hardlink.
   Un symlink, directorio, FIFO, hardlink o archivo de otro usuario se conserva y
   produce un error. No se trunca ni se sigue un objeto existente.
4. Crear el temporal exclusivamente (`O_CREAT|O_EXCL|O_NOFOLLOW|O_CLOEXEC`, 0600).
5. `write_all` de los bytes verificados; chmod al modo original; `sync_all` del
   archivo para sincronizar contenido y metadatos del modo final.
6. `rename(2)` del temporal **sobre** el destino: único punto de commit.
7. `sync_all` del directorio padre. El rename consume el nombre temporal.

No se mueve el AppImage antiguo a un backup. Antes del rename, cualquier error
ordinario conserva el destino antiguo e intenta retirar solo el temporal recién
creado; un fallo de limpieza se incluye en el error. Un SIGKILL puede dejar ese
temporal, incluso parcial; el siguiente intento lo retira y lo crea otra vez.
Después del rename, el destino nuevo es autoritativo. Si el fsync del directorio
falla se informa que el AppImage ya fue activado pero su durabilidad no pudo
confirmarse; no se intenta rollback. El proceso/mount actual puede seguir usando
el inode antiguo hasta el reinicio explícito.

La prueba negativa es inspección del código **bloqueado** de
`tauri-plugin-updater 2.12.0`, `updater.rs:1053–1127`: elegía un tempdir en el
mismo dispositivo, movía el destino a `current_app.AppImage`, escribía directamente
en el destino original y luego restauraba permisos. En el error ordinario intentaba
renombrar el backup de vuelta; el drop del tempdir lo eliminaba en éxito. Entre
el primer rename y la escritura el destino faltaba, y durante la escritura podía
contener bytes parciales. No había fsync del archivo ni del directorio.

Los tests de F-03 usan procesos reales y pipes, sin sleeps: SIGKILL inmediatamente
antes del rename conserva bytes y modo antiguos, deja un temporal completo y el
retry funciona; SIGKILL inmediatamente después del rename, **antes del fsync del
padre**, conserva bytes y modo nuevos. También se prueba un short write real con
`RLIMIT_FSIZE` en un hijo desechable. Los hooks solo existen en el binario de tests.
Esto califica interrupción de proceso/SIGKILL en directorios desechables, **no**
pérdida de alimentación: el orden de fsync está implementado pero no calificado
contra power-loss. No es una calificación E2E de AppImages firmados N-1 → N.
La propiedad de instancia F-04 excluye dos desktops sobre la misma raíz de datos
antes de que cualquiera pueda competir por este temporal; no se añade otro mutex
para AppImage.

## Propiedad de instancia desktop (F-04)

La entrada desktop adquiere `OperationGuard("instance", app_data_dir())` antes de
configurar WebKit, construir `Builder`, `GameState`, repositorios, updater o
diagnósticos. La autoridad existente `utils/paths.rs` define la raíz como
`$HOME/.local/share/ro-launcher`: allí viven settings, servidores, instalaciones,
runtime y `locks/`. CWD, ejecutable, AppImage, servidor y prefix no eligen la clave.
`XDG_DATA_HOME` no sustituye esta raíz. La normalización existente de OperationGuard
resuelve aliases de HOME incluso antes de crear los padres ausentes.

Un guard local envuelve el callback completo de inicialización y `.run()`, incluido
`RunEvent::Exit`, la limpieza de F-02 y el reinicio. Los locks siguen siendo registros
POSIX exclusivos (`fcntl(F_SETLK)`), por proceso, con descriptor `FD_CLOEXEC`.
No se heredan con fork ni sobreviven a la muerte del dueño, incluido SIGKILL.
Los archivos `locks/<sha256>.lock` no se truncan ni eliminan: un archivo persistente
no representa propiedad persistente y todos los participantes abren el mismo inode.

El sucesor puede arrancar antes de la muerte de su predecesor. En la dependencia
bloqueada Tauri 2.11.2, `app.rs:1422–1428` ejecuta nuestro callback de salida y
`cleanup_before_exit()` **antes** de `process::restart`; `process.rs:74–91` hace
`Command::spawn()` y enseguida `exit(0)`. Por eso se permite **un segundo** de handoff,
con reintentos cada 25 ms y tiempo monotónico (`Instant`). Este margen cubre la
planificación del spawn/exit final, no la duración de la limpieza previa. No garantiza
el handoff si el predecesor queda suspendido después del spawn durante más de un
segundo. Los fallbacks de restart también usan ese mismo spawn/exit; F-02 conserva
su limitación de entrega del callback en esos fallbacks.

Solo `Busy` se reintenta; los errores de filesystem/permisos fallan inmediatamente
y no se clasifican como otra instancia. Si otro desktop conserva la raíz al vencer
el plazo, el perdedor escribe `RO-Launcher ya está abierto para <raíz>` a stderr y
sale con código 1 antes de inicializar estado, diagnósticos o ventanas. No roba el
lock, no termina al dueño ni agrega IPC para enfocar su ventana.

`--import-runtime /ruta/absoluta/archive.tar.zst` despacha antes de la entrada desktop
y conserva únicamente sus guards runtime/artefactos. La nueva propiedad no cambia
el gate/candidato de F-01, la reserva y limpieza de F-02 ni la activación de F-03.
La recuperación de prefixes F-05 y de artefactos F-06 permanece pendiente.

El AppImage instalado y `npm run tauri:dev` sobre el mismo HOME deben entrar en
conflicto. Para desarrollar en paralelo usa un HOME absoluto desechable, además de
XDG config/cache/data aislados si quieres aislar también las bibliotecas de escritorio:

```bash
dev_root=$(mktemp -d /tmp/ro-launcher-dev.XXXXXX)
env HOME="$dev_root" XDG_DATA_HOME="$dev_root/data" \
  XDG_CONFIG_HOME="$dev_root/config" XDG_CACHE_HOME="$dev_root/cache" \
  CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}" \
  RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}" npm run tauri:dev
```

Raíces distintas pueden coexistir. No hay un selector de raíz en el producto, pero
HOME aislados pueden ejecutar el mismo fichero AppImage. F-04 no serializa updates
de ese fichero entre raíces diferentes: para desktops instalados aislados usa
copias AppImage separadas. El desarrollo sin empaquetar no activa AppImages.
La exclusividad es propiedad cooperativa del producto, no una defensa frente a un
proceso malicioso del mismo usuario que borre o sustituya los archivos de lock.
