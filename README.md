# RO-Launcher

Launcher dedicado para **Ragnarok Online** en Linux. Gestiona el WINEPREFIX, dependencias, variables de entorno y herramientas del cliente automáticamente. El usuario elige servidor y pulsa **JUGAR**.

**Developed by: nndsk**

---

## Características

- **Multi-servidor** — agrega varios clientes RO, cada uno con su propia carpeta y ejecutable
- **Selector de .exe** — diálogo nativo para elegir el cliente (sin escribir rutas a mano)
- **Setup automático del prefix** — DXVK, Gecko, vcredist, d3dx9 y marker de configuración en el primer lanzamiento
- **Detección de herramientas** por carpeta del servidor:
  - **OpenSetup / Setup** — prioriza `opensetup.exe` si coexisten ambos
  - **Patcher** — detecta `*patcher*.exe` y variantes del nombre del servidor
  - **dgVoodoo** — verifica `D3DImm.dll`, `DDraw.dll`, `dgVoodoo.conf` y `dgVoodooCpl.exe`
- **dgVoodoo embebido** — instala/desinstala desde una plantilla preconfigurada incluida en el launcher
- **Runners** — Proton (recomendado) o Wine del sistema, seleccionables desde la UI
- **Audio** — detección PulseAudio/ALSA con avisos si falta el driver adecuado
- **Logs en tiempo real** — salida de Wine/Proton/DXVK con copia de errores

---

## Requisitos

### Sistema

| Componente | Notas |
|------------|-------|
| **Linux x86_64** | Probado en CachyOS/Arch; compatible con otras distros |
| **Vulkan + drivers GPU** | Necesario para DXVK (AMD/NVIDIA/Intel) |
| **winetricks** | Setup del prefix |

> En Wayland (Hyprland, etc.) el launcher lanza vía Xwayland automáticamente.

---

## Runners

El launcher detecta y **prioriza** [`proton-cachyos-slr`](https://github.com/CachyOS/proton-cachyos) — Proton con Steam Linux Runtime, mejor compatibilidad con Gepard Shield y clientes protegidos. En la UI aparece como **proton-cachyos-slr (recomendado Gepard)**.

| Runner | Descripción |
|--------|-------------|
| **proton-cachyos-slr** | Proton + SLR — **recomendado**, máxima compatibilidad |
| proton-cachyos | Sin SLR; solo si sabes por qué la necesitas |
| wine-cachyos / wine | Fallback si no hay Proton disponible |

### Instalación de proton-cachyos-slr

```bash
# CachyOS (repo oficial)
sudo pacman -S proton-cachyos-slr

# Arch Linux (AUR)
yay -S proton-cachyos-slr
```

Manual (cualquier distro): descarga el release `-slr` desde [GitHub Releases](https://github.com/CachyOS/proton-cachyos/releases) y extrae en `~/.local/share/Steam/compatibilitytools.d/`. El launcher lo detecta al reiniciar.

> ProtonUp-Qt gestiona Proton-GE, no proton-cachyos. Para este launcher usa los métodos de arriba.

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

> `tauri:dev` exporta `GDK_BACKEND=x11` y `WEBKIT_DISABLE_DMABUF_RENDERER=1` para evitar ventana negra del WebView en Wayland.

### Build de producción

```bash
npm run tauri:build
```

El binario queda en `src-tauri/target/release/bundle/`.

### Uso rápido

1. Abre **RO-Launcher**
2. Pulsa **+** y selecciona el `.exe` del cliente
3. Elige el **Runner** (`proton-cachyos-slr` si está disponible)
4. Revisa **Herramientas** — instala dgVoodoo si hace falta
5. Pulsa **JUGAR**

En el primer lanzamiento se configura el WINEPREFIX automáticamente (puede tardar unos minutos).

---

## Pipeline gráfico

```
Cliente RO (DirectDraw/Direct3D)
        ↓
   dgVoodoo (D3DImm.dll + DDraw.dll)
        ↓
   DXVK (Vulkan)
        ↓
   GPU (AMD / NVIDIA / Intel)
```

Variables críticas aplicadas al lanzar el juego:

```bash
WINEPREFIX=~/.local/share/ro-launcher/prefix
WAYLAND_DISPLAY=                    # fuerza Xwayland
DXVK_ASYNC=1
DXVK_CONFIG=d3d9.forceSamplerTypeSpecConstants=True
WINE_LARGE_ADDRESS_AWARE=1
WINEDLLOVERRIDES=d3dimm=n,b;ddraw=n,b
```

---

## Datos del usuario

Todo se guarda en `~/.local/share/ro-launcher/`:

| Archivo / carpeta | Contenido |
|-------------------|-----------|
| `servers.json` | Lista de servidores configurados |
| `settings.json` | Runner global por defecto |
| `prefix/` | WINEPREFIX compartido |
| `prefix/.ro-launcher-configured` | Marker de setup completado |

---

## dgVoodoo

El launcher incluye una plantilla en `src-tauri/resources/dgvoodoo/` (archivos embebidos en el build).

| Acción | Comportamiento |
|--------|----------------|
| **Instalar** | Copia solo los archivos que falten en la carpeta del cliente |
| **Configurar** | Abre `dgVoodooCpl.exe` con el runner seleccionado |
| **Desinstalar** | Elimina los 4 archivos dgVoodoo de la carpeta del cliente |

La instalación es **manual** (botón Instalar) — no se copia automáticamente al jugar.

---

## Stack técnico

| Capa | Tecnología |
|------|------------|
| Shell | [Tauri v2](https://v2.tauri.app/) |
| Backend | Rust + Tokio |
| Frontend | React 18, TypeScript, Tailwind CSS, Zustand, Vite |
| Arquitectura | Feature-Sliced Design |

---

## Solución de problemas

### Pantalla negra al jugar

- Verifica que DXVK esté instalado en el prefix (primer setup)
- Confirma dgVoodoo instalado (D3DImm + DDraw)
- Usa `proton-cachyos-slr` como runner

### Pantalla negra del launcher (UI)

- Lanza con `npm run tauri:dev` (ya incluye los flags de Wayland)
- O exporta manualmente: `GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1`

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
