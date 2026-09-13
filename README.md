# RO-Launcher

Launcher de Ragnarok Online para Linux, construido con Tauri, React y Rust. Administra runners,
WINEPREFIX, dependencias y herramientas por servidor sin depender de la versión de Wine instalada
por el sistema.

## Funciones principales

- Perfiles aislados por servidor y runner.
- Proton-CachyOS + UMU administrados y verificados por el launcher.
- Wine/Proton externos y Wine portable desde el directorio de datos.
- Detección PE, patcher, WebView2, Gepard, GameGuard, DirectDraw y Direct3D.
- dgVoodoo reversible, multi-client, AutoPot, AutoBuff, spammer y Discord Rich Presence.

## Matriz Gepard validada

Las recomendaciones se aplican sólo cuando el SHA-256 de `gepard.dll` coincide exactamente.

| Gepard | SHA-256 | Evidencia local | Perfil recomendado |
|---|---|---|---|
| 3.0 · `26.8.26.1` | `e2f624d2e3451e68e46783e86d75a8b6787567a96bd33357d74b184dfcec6c13` | HoneyRO; misma DLL en Dracarys | Proton-CachyOS 11 + DXVK 3.0.1 |
| 3.0 · `26.9.3.1` | `db4653ddf6aea88a502f10e200a300a05e8e4d65e7cfe65eb5b2ff0779e2e4f5` | SakuraRO; `3::110::12` con new WoW64 | Wine 7.16 old-WoW64 + DXVK 2.6.2 |
| Cualquier otra build | — | No validada | Sin recomendación automática |

Ambas DLL y clientes son PE32/i386. HoneyRO quedó validado con el runtime administrado
`proton-cachyos-11.0-20260702-slr`; SakuraRO quedó validado con Wine 7.16 legacy. El launcher no
modifica Gepard, el cliente ni el tráfico del juego: sólo selecciona una capa de compatibilidad.

La selección es por servidor. Cambiar runner crea o reutiliza otro prefix; nunca migra ni elimina
el anterior. Un hash desconocido sólo genera una advertencia.

### Pipeline gráfico

```text
Direct3D 8/9/11 ─────────────────────→ DXVK ───────→ Vulkan
DirectDraw 1-7 ─→ dgVoodoo (opcional) ─→ D3D11 ───→ DXVK ─→ Vulkan
```

Wine 7.16 recibe las DLL oficiales x86/x64 de DXVK 2.6.2, además de configuración, cache y logs
privados. El
mismo backend se aplica al juego, OpenSetup y patcher para que todos enumeren la GPU real. No se
habilitan `PROTON_USE_WOW64`, NTSync ni un HUD de diagnóstico. DXVK se descarga desde su release
oficial y se verifica por tamaño y SHA-256 antes de instalarlo.

## Wine 7.16 portable

El launcher descubre distribuciones con esta estructura:

```text
~/.local/share/ro-launcher/runners/<nombre>/bin/wine
~/.local/share/ro-launcher/runners/<nombre>/bin/wineserver
~/.local/share/ro-launcher/runners/<nombre>/lib/wine/i386-unix/
```

Para el perfil Sakura, `bin/wine --version` debe informar `wine-7.16` y debe existir el runtime Unix
i386. `WINEARCH=win32` no convierte un Wine pure-WoW64 en Wine legacy. El launcher administra una
distribución portable ya instalada; todavía no descarga Wine 7.16.

Un Wine 7.16 TkG que declare los patches `fsync-unix-staging` y `fsync_futex_waitv` en
`wine-tkg-config.txt` activa `WINEFSYNC=1` automáticamente. Una build staging sin FSYNC usa ESYNC y
una build vanilla conserva wineserver. NTSync no se fuerza: Wine 7.16 no lo implementa.

## Uso

1. Agrega el ejecutable del cliente.
2. Edita el servidor y elige el runner recomendado por la tabla.
3. Pulsa **Preparar** o **Rearmar entorno** cuando el diagnóstico lo solicite.
4. Configura resolución y API desde **Setup**.
5. Instala dgVoodoo sólo para clientes DirectDraw y cuando el servidor lo permita.
6. Pulsa **Jugar**.

El primer preparado descarga aproximadamente 314 MiB del runtime administrado y verifica tamaño y
checksum antes de activarlo.

## Desarrollo

Requiere Linux x86_64, Vulkan, Node.js/npm, Rust estable y dependencias de Tauri v2.

```bash
npm install
npm run tauri:dev
```

Antes de un commit:

```bash
npm run lint
npm run format:check
npm test
npm run build
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Builds: `npm run tauri:build` o `npm run tauri:build:appimage`. Los artefactos quedan en
`target/release/bundle/`.

## Datos locales

Todo vive en `~/.local/share/ro-launcher/`:

- `servers.json` y `settings.json`: configuración.
- `runners/`: Wine portable.
- `runtime/`: Proton, UMU y DXVK 2.6.2 verificados.
- `prefixes/<server-hash>-<runner-hash>/`: entorno aislado.
- `<prefix>/.ro-launcher-prefix.json`: runner y componentes instalados.
- `<prefix>/.ro-launcher-dxvk/`: configuración, cache y logs de DXVK.

**Rearmar entorno** conserva el anterior hasta terminar correctamente. Directorios sin manifiesto
válido no se adoptan ni eliminan automáticamente.

## Diagnóstico breve

- `3::110::12`: confirma el hash y usa el perfil exacto de la tabla.
- GPU genérica o rendimiento bajo: confirma que el diagnóstico indique DXVK, no WineD3D.
- DirectDraw en blanco: instala dgVoodoo y conserva DXVK como backend D3D11.
- Patcher en blanco: marca WebView2 como requisito y rearma el entorno.
- UI negra en Wayland: usa `npm run tauri:dev`; el script fuerza X11 para WebKit.

## Terceros

[proton-cachyos](https://github.com/CachyOS/proton-cachyos) ·
[umu-launcher](https://github.com/Open-Wine-Components/umu-launcher) ·
[DXVK](https://github.com/doitsujin/dxvk) ·
[dgVoodoo2](http://dege.freeweb.hu/)

Desarrollado por [nndsk](https://github.com/nndsk).
