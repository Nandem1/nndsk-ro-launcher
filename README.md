# RO-Launcher

Launcher dedicado para **Ragnarok Online** en Linux. Gestiona el WINEPREFIX, dependencias, variables de entorno y herramientas del cliente automáticamente. El usuario elige servidor y pulsa **JUGAR**.

**Developed by: nndsk**

---

## Características

- **Multi-servidor aislado** — cada cliente recibe un entorno independiente
- **Selector de .exe** — diálogo nativo para elegir el cliente (sin escribir rutas a mano)
- **Setup por perfil** — DXVK, Gecko, vcredist, d3dx9, fuentes y WebView2 cuando el ejecutable de la estrategia activa o el override manual lo requiere
- **Contrato de lanzamiento** — inicio directo o mediante patcher, argv sin shell y placeholders efímeros para credenciales
- **Detección de herramientas** por carpeta del servidor:
  - **OpenSetup / Setup** — prioriza `opensetup.exe` si coexisten ambos
  - **Patcher** — detecta `*patcher*.exe` y variantes del nombre del servidor
  - **dgVoodoo** — verifica `D3DImm.dll`, `DDraw.dll`, `dgVoodoo.conf` y `dgVoodooCpl.exe`
- **dgVoodoo embebido y reversible** — instalación transaccional, ownership por hash y respaldo de archivos originales
- **Runtime Ragnarok administrado** — una versión probada de Proton + UMU, sin selección ni dependencias Wine del sistema
- **AutoPot** — HP/SP automático por lectura de memoria del cliente (perfiles 4RTools)
- **Spammer** — spam de teclas con trigger por hotkey (F1–F9, 0–9, A–Z) vía `ro-inputd` + uinput persistente a 10 ms
- **Audio** — detección PulseAudio/ALSA con avisos si falta el driver adecuado
- **Logs en tiempo real** — salida de Wine/Proton/DXVK y herramientas (AutoPot, Spammer, input)

---

## Requisitos

### Sistema

| Componente | Notas |
|------------|-------|
| **Linux x86_64** | Probado en CachyOS/Arch; compatible con otras distros |
| **Vulkan + drivers GPU** | Necesario para DXVK (AMD/NVIDIA/Intel) |
| **Conexión a Internet** | Necesaria la primera vez para descargar y verificar el runtime administrado |
| **curl** | Fallback para descargar Wine Gecko cuando la distro no lo instala globalmente |
| **`/dev/uinput` + grupo `input`** | Input de AutoPot/Spammer/AutoBuff y captura evdev de `ro-inputd` |

> En Wayland (Hyprland, etc.) el juego se lanza vía Xwayland automáticamente. La UI del launcher también fuerza backend X11 para WebKit (incluido AppImage).

---

## Runtime Ragnarok

El launcher utiliza exclusivamente **Proton-CachyOS 11.0 (2026-07-02 SLR)** mediante
[`umu-launcher` 1.4.0](https://github.com/Open-Wine-Components/umu-launcher/releases/tag/1.4.0).
No selecciona ni ejecuta Protons instalados en Steam o Wine del sistema. Esta versión incluye una
revisión de DXVK posterior a la corrección del bloqueo de compilación de shaders que afectaba a
clientes RO con dgVoodoo.

En el primer **Preparar/Jugar**, descarga aproximadamente 314 MiB a
`~/.local/share/ro-launcher/runtime/`, comprueba tamaño y checksum oficial, extrae en staging y sólo
entonces activa el runtime. Las instalaciones incompletas no se usan. UMU puede descargar además
los componentes de Steam Linux Runtime que necesite.

| Componente fijado | Verificación |
|-------------------|--------------|
| `proton-cachyos-11.0-20260702-slr-x86_64.tar.xz` | SHA-512 oficial |
| `umu-launcher-1.4.0-zipapp.tar` | SHA-256 oficial |

> Detectar un anti-cheat no implica compatibilidad ni permiso. El launcher muestra una advertencia
> y exige confirmación antes de instalar dgVoodoo; la política final depende del servidor.

El artefacto Proton proviene del
[release oficial de CachyOS](https://github.com/CachyOS/proton-cachyos/releases/tag/cachyos-11.0-20260702-slr).
Actualizar el runtime es una decisión versionada del launcher: no cambia por una actualización del
sistema y no requiere instalar paquetes manualmente.

---

## Instalación y uso

### Desarrollo

```bash
# Arch / CachyOS
sudo pacman -S base-devel git nodejs npm rustup
rustup default stable

git clone <repo-url> ro-launcher
cd ro-launcher
npm install
npm run tauri:dev
```

Comprobaciones de mantenimiento antes de enviar cambios:

```bash
npm test
npm run build
npm run lint
npm run format:check
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

> `tauri:dev` exporta `GDK_BACKEND=x11` y `WEBKIT_DISABLE_DMABUF_RENDERER=1` para evitar ventana negra del WebView en Wayland.

### Build de producción

```bash
# Todos los bundles (deb, AppImage, etc.)
npm run tauri:build

# Solo AppImage (recomendado para distribución portable)
npm run tauri:build:appimage
```

Los artefactos quedan en `target/release/bundle/` (raíz del repo, no dentro de `src-tauri/`).

En Arch/CachyOS, `tauri:build:appimage` ya incluye `NO_STRIP=true` (workaround para `linuxdeploy` con librerías `.relr.dyn`).

### AppImage

```bash
npm run tauri:build:appimage
chmod +x target/release/bundle/appimage/RO-Launcher_*_amd64.AppImage
./target/release/bundle/appimage/RO-Launcher_*_amd64.AppImage
```

Para actualizar: borra el `.AppImage` anterior y usa el nuevo. Tus datos en `~/.local/share/ro-launcher/` no se tocan.

### Uso rápido

1. Abre **RO-Launcher**
2. Pulsa **+** y selecciona el `.exe` del cliente
3. Configura estrategia y argumentos; una línea representa exactamente un `argv`
   - Activa **Forzar WebView2** si el diagnóstico PE es inconcluso y el ejecutable activo usa una interfaz web; en modo directo, un patcher opcional no bloquea el juego
4. Revisa **Herramientas** — instala dgVoodoo sólo si el cliente lo necesita y el servidor lo permite
5. Pulsa **JUGAR**

En el primer lanzamiento se descarga el runtime y se configura el WINEPREFIX automáticamente
(puede tardar unos minutos). Un entorno administrado creado con otro runner se respalda, reconstruye
y valida antes de iniciar; una ruta con datos sin manifiesto nunca se elimina automáticamente.

---

## Pipeline gráfico

```
Direct3D 9 ───────────────────────→ DXVK ─→ Vulkan
DirectDraw 1-7 ─→ dgVoodoo (opcional) ─→ D3D11 ─→ DXVK ─→ Vulkan
```

Variables críticas aplicadas al lanzar el juego:

```bash
WINEPREFIX="$HOME/.local/share/ro-launcher/prefixes/<server-hash>"
WAYLAND_DISPLAY=                    # fuerza Xwayland
DXVK_ASYNC=1
DXVK_CONFIG=d3d9.forceSamplerTypeSpecConstants=True
WINE_LARGE_ADDRESS_AWARE=1
WINEDLLOVERRIDES=d3dimm=n,b;ddraw=n,b  # sólo con dgVoodoo verificado
```

---

## Datos del usuario

Todo se guarda en `~/.local/share/ro-launcher/`:

| Archivo / carpeta | Contenido |
|-------------------|-----------|
| `servers.json` | Lista de servidores configurados |
| `settings.json` | Identidad del runtime administrado (compatibilidad de formato) |
| `*.json.bak` | Copia de seguridad de la versión anterior de cada configuración |
| `*.json.corrupt-*` | Configuración inválida preservada después de recuperar un backup |
| `prefix/` | WINEPREFIX compartido legacy |
| `prefixes/<server-hash>/` | Entornos aislados administrados |
| `<prefix>/.ro-launcher-prefix.json` | Manifiesto versionado con scope, servidor, runner y componentes |
| `runtime/` | Proton y UMU fijados y verificados por el launcher |

---

## dgVoodoo

El launcher incluye una plantilla en `src-tauri/resources/dgvoodoo/` (archivos embebidos en el build).

| Acción | Comportamiento |
|--------|----------------|
| **Instalar** | Respalda conflictos, escribe atómicamente, revierte fallos y registra ownership |
| **Configurar** | Abre `dgVoodooCpl.exe` con el runtime administrado |
| **Desinstalar** | Quita sólo binarios sin modificar, preserva un `dgVoodoo.conf` editado y restaura originales |

La instalación es **manual** (botón Instalar) — no se copia automáticamente al jugar.
Los overrides sólo se activan cuando `D3DImm.dll` y `DDraw.dll` coinciden con la plantilla confiable
incluida; el manifiesto local nunca se usa como fuente de confianza.

---

## AutoPot, Spammer y AutoBuff

| Herramienta | Requisitos | Notas |
|-------------|------------|-------|
| **AutoPot** | Juego corriendo, `/dev/uinput`, perfil de memoria | HP/SP por una lectura de memoria; loop estable con mínimo de 10 ms |
| **Spammer** | Juego corriendo, `/dev/uinput`, grupo `input` | Worker uinput persistente; hotkeys F1–F9, 0–9 y A–Z; Alt+tecla pasa el evento sin spam |
| **AutoBuff** | Juego corriendo, `/dev/uinput`, perfil de memoria | Reglas de buffs por status ID con actualización de configuración en vivo |

Todas las herramientas comparten un único worker uinput persistente.
La configuración de las tres herramientas se guarda por servidor en `servers.json`.

### Migración y recuperación de configuración

Al iniciar, el launcher valida y migra automáticamente configuraciones antiguas al formato actual
sin cambiar el array superior de `servers.json`. Antes de reescribir conserva el original como
`.bak`. Si el archivo principal está corrupto y el backup es válido, restaura el backup de forma
atómica, preserva el archivo dañado como `.corrupt-*` y muestra un aviso no bloqueante. Si ambos
son inválidos, inicia en modo degradado para evitar pérdida de datos.

---

## Stack técnico

| Capa | Tecnología |
|------|------------|
| Shell | [Tauri v2](https://v2.tauri.app/) |
| Backend | Rust + Tokio, crates `ro-tools-core`, `ro-tools-linux`, `ro-inputd` |
| Frontend | React 18, TypeScript, Tailwind CSS, Zustand, Vite |
| Arquitectura | Feature-Sliced Design |

---

## Solución de problemas

### Pantalla negra al jugar

- Revisa el diagnóstico PE: un cliente puede usar DirectDraw y Direct3D 9 simultáneamente
- Verifica DXVK para Direct3D 9; dgVoodoo sólo es necesario para la cadena DirectDraw si el cliente lo requiere
- Usa **Rearmar entorno** si el diagnóstico indica un manifiesto o componente incompatible
- Si aparece WebView2 como faltante, usa **Reparar/Rearmar entorno**
- Para un PE protegido o carga dinámica, activa **Forzar WebView2** en la edición del servidor

### Pantalla negra del launcher (UI) / error GBM

- En desarrollo: `npm run tauri:dev` (ya incluye los flags de Wayland)
- En AppImage/binario release: la app aplica `GDK_BACKEND=x11` y `WEBKIT_DISABLE_DMABUF_RENDERER=1` al arranque
- Si aún falla, exporta manualmente: `GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 ./RO-Launcher_*.AppImage`

### Sin audio

- Instala `lib32-libpulse` para PulseAudio: `sudo pacman -S lib32-libpulse` (Arch/CachyOS)
- El banner de audio en la UI indica el driver activo

### `mmap() error Cannot allocate memory`

Wine reservando memoria virtual para el proceso de 32 bits. Si el juego crashea:

```bash
# Aumentar límite de mapas de memoria (temporal)
sudo sysctl vm.max_map_count=2147483642
```

---

## Licencia y créditos

Proyecto personal. **dgVoodoo** y **Proton** son software de terceros con sus propias licencias — la plantilla dgVoodoo incluida proviene de una configuración personal probada en clientes RO privados.

- **[proton-cachyos](https://github.com/CachyOS/proton-cachyos)** — CachyOS / loathingkernel
- **[dgVoodoo2](http://dege.freeweb.hu/)** — Dege
- **[DXVK](https://github.com/doitsujin/dxvk)** — doitsujin
- **Developed by [nndsk](https://github.com/nndsk)**
