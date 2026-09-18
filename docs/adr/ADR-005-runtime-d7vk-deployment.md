# ADR-005: deployment y support envelope de D7VK

| Campo                 | Valor                                                                                    |
| --------------------- | ---------------------------------------------------------------------------------------- |
| Estado                | Aceptado (spike Fase 6A cerrado)                                                         |
| Fecha                 | 2026-09-18                                                                               |
| Alcance               | Pin D7VK v2.2, harness de install/restore en scratch, decisión go/no-go para Fase 6B     |
| Veredicto             | `no-go`                                                                                  |
| Autoridad relacionada | `AGENTS.md`, `docs/RO_RUNTIME_ARCHITECTURE_PLAN.md` §12, ADR-001/002/003                 |

## 1. Pin del artefacto

| Campo            | Valor                                                                                              |
| ---------------- | -------------------------------------------------------------------------------------------------- |
| `artifactId`     | `d7vk-2.2`                                                                                         |
| Versión upstream | `2.2` (tag `v2.2`, publicado 2026-08-28)                                                           |
| URL              | `https://github.com/WinterSnowfall/d7vk/releases/download/v2.2/d7vk-v2.2.zip`                    |
| Tamaño           | `3375635` bytes                                                                                    |
| SHA-256 (zip)    | `1a9ffe3639ceb5e2fb1ccb25ef388354b4476bb26bc7edf88a8dc59f2774df38`                               |
| Licencia         | zlib/libpng ([LICENSE v2.2](https://raw.githubusercontent.com/WinterSnowfall/d7vk/v2.2/LICENSE)) |

Golden: [`contract-fixtures/d7vk-spike-v2.2.json`](../contract-fixtures/d7vk-spike-v2.2.json).

## 2. Layout observado del release

**Confirmado** (listado del asset verificado en implementación):

| Entrada zip                 | Notas                          |
| --------------------------- | ------------------------------ |
| `d7vk-v2.2/`                | directorio raíz                |
| `d7vk-v2.2/x32/`            | única arquitectura empaquetada |
| `d7vk-v2.2/x32/ddraw.dll`   | PE machine `0x14c` (x86)       |

- `x64RelativeSource`: **ausente** en v2.2 (solo `x32`).
- Payload `ddraw.dll` x86: `8785934` bytes; SHA-256 `b6f2f4b64a72047742bd522a9ff0242446a42b9bb1a8e361b114e48f4c402f8a`.
- No hay `d3d9.dll` adicional en el zip (a diferencia de DXVK-Sarek).

## 3. Respuestas a §12.3 (`RO_RUNTIME_ARCHITECTURE_PLAN.md`)

| Pregunta | Estado | Evidencia |
| -------- | ------ | --------- |
| Arquitectura(s) de `ddraw.dll` para clientes RO | **Confirmado** | Solo x86 en v2.2; alineado con clientes 32-bit y `pe.rs` |
| Side-by-side junto al exe bajo Wine 7.16 old-WoW64 | **Confirmado** (carga) / **`no-go`** (Gepard) | D7VK emitió `LOADING D7VK`; Gepard `26.9.3.1` marcó `illegal file ddraw.dll` |
| Side-by-side bajo Proton-CachyOS/UMU | **No ejecutado** | Mismo overlay de game dir; no necesario para el veredicto. No se disfraza el payload |
| Reemplazo `system32`/`syswow64` + conservación DirectDraw real | **Confirmado** (FS en scratch) / **No ejecutado** (carga Wine) | Harness `install_prefix_owned`; live no se persigue: seguiría siendo otro `ddraw` en proceso |
| `ddraw=n,b`, overrides y casing | **Confirmado** (merge + live) | `DllOverrideSet`; el proceso Wine 7.16 cargó D7VK nativo, no WineD3D |
| Vulkan API/extensions del release | **Confirmado** (documentación + host) | README FAQ: Vulkan 1.4; host del spike: API 1.4.351 |
| Variables / logs del release | **Confirmado** | `D7VK_LOG_PATH`, `D7VK_LOG_LEVEL`; HUD `DXVK_HUD`; banner `LOADING D7VK` en stderr |
| Clientes mixtos DDraw/GDI o D3D9 directo | **PE confirmado** / **render no alcanzado** | Sakura: strings `ddraw` + `d3d9` + `d3dx9_43` + `gdi32`; Gepard abortó antes del frame |
| Checksum y layout reproducible | **Confirmado** | Pin size/digest + golden JSON |
| Uninstall / interruption | **Confirmado** | Tests harness + restore live de dgVoodoo (hashes originales) |
| Patcher / OpenSetup / game mismo plan | **No aplica** en 6A | Sin profile productivo; 6B prohibida |
| Contribución `PrefixFingerprint` | **Confirmado** (diseño) | Side-by-side: no (ADR-002). Owner productivo no se elige: 6B no se abre |

## 4. Deployment candidato

### 4.1 Side-by-side (game dir)

- Copiar `d7vk-v2.2/x32/ddraw.dll` como `ddraw.dll` junto al ejecutable.
- Override Wine: `ddraw=n,b` (`DllLoadOrder::NativeThenBuiltin`).
- Owner candidato: `OwnershipDomain::GameDirOverlay`, `component_id` `game-dir/d7vk-spike` (spike).
- **No** contribuye a `PrefixFingerprint`.
- **Live:** Gepard trata el payload como archivo ilegal en el directorio del cliente. dgVoodoo `DDraw.dll` (~219 KiB, SHA-256 `838ee34f…`) permanece permitido; D7VK `ddraw.dll` (~8.4 MiB, SHA-256 `b6f2f4b6…`) no. No se reubica ni se disfraza el binario.

### 4.2 Prefix-owned (`ddraw_` delegation)

Algoritmo probado en scratch (no en prefix de usuario):

1. Exigir `drive_c/windows/syswow64/ddraw.dll` (Wine) regular.
2. Rechazar si `ddraw_.dll` ya existe.
3. `rename(ddraw.dll → ddraw_.dll)`.
4. Copiar payload D7VK a `ddraw.dll`.
5. Restore: quitar D7VK `ddraw.dll`, `rename(ddraw_.dll → ddraw.dll)`.

**Owner definitivo:** no se elige. Prefix-owned dejaría el mismo PE de D7VK cargado en el proceso; no se usa para rodear el escaneo de Gepard.

### 4.3 Mutual exclusion con dgVoodoo

- dgVoodoo y D7VK no pueden reclamar `ddraw.dll` en el mismo target: `DllOverrideSet` produce `OverrideConflict`.
- Harness rechaza scratch con `.ro-launcher-dgvoodoo.json` o par `ddraw`+`d3dimm` en game dir.

## 5. Constraints upstream (no promover a Validated)

- D7VK no implementa DirectDraw completo; delega a Wine/native `ddraw`.
- Reemplazar Wine `ddraw.dll` sin conservar copia como `ddraw_.dll` **no** es válido.
- Vulkan 1.4 requerido según README principal; hosts sin 1.4 deben rechazarse en 6B (eligibility), no forzar Sarek desde el launcher sin decisión explícita.
- Sin modificar, hookear ni evadir Gepard/GameGuard. Un `illegal file` de Gepard es **`no-go`**, no un bug de layout.

## 6. Matriz live

Scratch y prefix de prueba **fuera** de `~/.local/share/ro-launcher/`. Overlay temporal del game dir restaurado al terminar. Protocolo: [`scripts/d7vk-spike-live.sh`](../scripts/d7vk-spike-live.sh).

| Anchor runner | Startup | Render visual | GDI mixto | D3D9 directo (si PE) | Cleanup | Logs D7VK vs WineD3D |
| ------------- | ------- | ------------- | --------- | -------------------- | ------- | -------------------- |
| Wine 7.16 old-WoW64 | D7VK cargó; Gepard `illegal file ddraw.dll` | no alcanzado | no alcanzado | no alcanzado | restore OK (hashes dgVoodoo) | `LOADING D7VK`; no WineD3D |
| Proton-CachyOS 11 UMU | not-run | not-run | not-run | not-run | n/a | n/a |

Host del spike Wine 7.16 (2026-09-18):

- GPU: NVIDIA GeForce RTX 3060; driver 615.71.09; Vulkan instance/device API 1.4.351
- Runner: `wine-7.16-staging-tkg-amd64` (`wine-7.16.r0` TkG Staging Esync Fsync), old-WoW64
- Cliente: SakuraRO `ragexe.exe` x86; Gepard `26.9.3.1` SHA-256 `db4653ddf6aea88a502f10e200a300a05e8e4d65e7cfe65eb5b2ff0779e2e4f5`
- Baseline overlay: dgVoodoo `DDraw.dll` `838ee34f17309ca4cd515407883a247db5fa90fe58261c6615ff5cc74fe71e4d` + `D3DImm.dll` `5f93b9fb7a77acafc259a57d3294a0ee70125a8af8a0a1975af4cba818229947`
- Payload D7VK: `ddraw.dll` x86 `b6f2f4b64a72047742bd522a9ff0242446a42b9bb1a8e361b114e48f4c402f8a`

## 7. Algoritmo de veredicto

1. Pin: size/digest/HTTPS distintos del golden → **`no-go`**
2. Zip sin `ddraw.dll` x86 → **`no-go`**
3. Tests harness (restore, collision, zip unsafe, catálogo len 3) fallan → **`no-go`**
4. Harness verde y matriz live `not-run` → **`blocked-insufficient-evidence`** (estado intermedio; ya no aplica)
5. Live: Gepard/GameGuard `illegal file` sobre el payload D7VK → **`no-go`** ← **estado actual**
6. Live completo en ambos anchors **sin** rechazo de anti-cheat ni bypass → **`go`** o **`go-reduced-scope`**
7. Live muestra fallback WineD3D silencioso, GDI roto, DLL stale o corrupción visual → **`no-go`** o **`go-reduced-scope`** con constraint explícita

**Fase 6B** solo con **`go`** o **`go-reduced-scope`**. **`no-go`** prohíbe 6B. Reabrir el spike sólo si un servidor allowlistea D7VK o existe un cliente RO controlado sin esa política.

## 8. Fuera de alcance (6A)

- Toggle UI, catálogo curated `Validated`, descarga automática productiva (`ensure_catalog_artifact`).
- Variant `GraphicsProfile` / `GraphicsPlan` productivo.
- DXVK-Sarek, múltiples releases configurables, benchmarks, AutoTune.
- Cambios en spawn, `ro-sessiond`, compatibility catalog, prefixes del usuario.

## 9. Implementación del harness

- Módulo test-only: [`src-tauri/src/tools/runtime/d7vk_spike.rs`](../src-tauri/src/tools/runtime/d7vk_spike.rs).
- Contrato: [`RO_RUNTIME_PHASE_6A_CONTRACT.md`](../RO_RUNTIME_PHASE_6A_CONTRACT.md).
- Lock: `OperationGuard::acquire("d7vk-spike", scratch)` exclusivo; namespaces `prefix`/`dgvoodoo`/`runtime` no usados por el spike.
