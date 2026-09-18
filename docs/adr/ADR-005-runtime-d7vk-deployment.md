# ADR-005: deployment y support envelope de D7VK

| Campo                 | Valor                                                                                    |
| --------------------- | ---------------------------------------------------------------------------------------- |
| Estado                | Aceptado (spike Fase 6A)                                                                 |
| Fecha                 | 2026-09-18                                                                               |
| Alcance               | Pin D7VK v2.2, harness de install/restore en scratch, decisión go/no-go para Fase 6B     |
| Veredicto             | `blocked-insufficient-evidence`                                                          |
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
| Side-by-side junto al exe bajo Wine 7.16 old-WoW64 | **Desconocido** | README upstream; sin smoke live |
| Side-by-side bajo Proton-CachyOS/UMU | **Desconocido** | Sin smoke live |
| Reemplazo `system32`/`syswow64` + conservación DirectDraw real | **Confirmado** (FS en scratch) / **Desconocido** (carga Wine) | Harness `install_prefix_owned` + restore; sin proceso Wine |
| `ddraw=n,b`, overrides y casing | **Confirmado** (merge) | `DllOverrideSet` + `DllName` lowercase; test dgVoodoo vs `game-dir/d7vk-spike` |
| Vulkan API/extensions del release | **Confirmado** (documentación) | README FAQ: Vulkan 1.4 para línea principal; build usa SPIR-V `vulkan1.3` — no sustituye el FAQ |
| Variables / logs del release | **Confirmado** (documentación) | `D7VK_LOG_PATH`, `D7VK_LOG_LEVEL`; HUD `DXVK_HUD` (v≥1.3 usa prefijo D7VK para logs) |
| Clientes mixtos DDraw/GDI o D3D9 directo | **Desconocido** en RO | `pe.rs` advierte DDraw+D3D9; GDI mixto: README “generally not expected to work” |
| Checksum y layout reproducible | **Confirmado** | Pin size/digest + golden JSON |
| Uninstall / interruption | **Confirmado** | Tests harness + fault injection |
| Patcher / OpenSetup / game mismo plan | **No aplica** en 6A | Sin profile productivo; 6B |
| Contribución `PrefixFingerprint` | **Confirmado** (diseño) | Side-by-side: no (ADR-002); prefix-owned: sí si se eligiera en 6B |

## 4. Deployment candidato

### 4.1 Side-by-side (game dir)

- Copiar `d7vk-v2.2/x32/ddraw.dll` como `ddraw.dll` junto al ejecutable.
- Override Wine: `ddraw=n,b` (`DllLoadOrder::NativeThenBuiltin`).
- Owner candidato: `OwnershipDomain::GameDirOverlay`, `component_id` `game-dir/d7vk-spike` (spike); productivo pendiente de ADR en 6B.
- **No** contribuye a `PrefixFingerprint`.

### 4.2 Prefix-owned (`ddraw_` delegation)

Algoritmo probado en scratch (no en prefix de usuario):

1. Exigir `drive_c/windows/syswow64/ddraw.dll` (Wine) regular.
2. Rechazar si `ddraw_.dll` ya existe.
3. `rename(ddraw.dll → ddraw_.dll)`.
4. Copiar payload D7VK a `ddraw.dll`.
5. Restore: quitar D7VK `ddraw.dll`, `rename(ddraw_.dll → ddraw.dll)`.

**Owner definitivo:** **Desconocido** hasta matriz live. No elegir entre side-by-side y prefix-owned en producto.

### 4.3 Mutual exclusion con dgVoodoo

- dgVoodoo y D7VK no pueden reclamar `ddraw.dll` en el mismo target: `DllOverrideSet` produce `OverrideConflict`.
- Harness rechaza scratch con `.ro-launcher-dgvoodoo.json` o par `ddraw`+`d3dimm` en game dir.

## 5. Constraints upstream (no promover a Validated)

- D7VK no implementa DirectDraw completo; delega a Wine/native `ddraw`.
- Reemplazar Wine `ddraw.dll` sin conservar copia como `ddraw_.dll` **no** es válido.
- Vulkan 1.4 requerido según README principal; hosts sin 1.4 deben rechazarse en 6B (eligibility), no forzar Sarek desde el launcher sin decisión explícita.
- Sin modificar, hookear ni evadir Gepard/GameGuard.

## 6. Matriz live (no ejecutada en este spike)

Todas las celdas: `not-run`. Ejecutar con [`scripts/d7vk-spike-live.sh`](../scripts/d7vk-spike-live.sh) y scratch **fuera** de `~/.local/share/ro-launcher/`.

| Anchor runner | Startup | Render visual | GDI mixto | D3D9 directo (si PE) | Cleanup | Logs D7VK vs WineD3D |
| ------------- | ------- | ------------- | --------- | -------------------- | ------- | -------------------- |
| Wine 7.16 old-WoW64 | not-run | not-run | not-run | not-run | not-run | not-run |
| Proton-CachyOS 11 UMU | not-run | not-run | not-run | not-run | not-run | not-run |

Registrar cuando se ejecute: GPU/vendor/driver, `vulkaninfo` API, runner path+hash, imports PE, `gepard.dll` SHA-256, profile gráfico baseline (dgVoodoo+DXVK).

## 7. Algoritmo de veredicto

1. Pin: size/digest/HTTPS distintos del golden → **`no-go`**
2. Zip sin `ddraw.dll` x86 → **`no-go`**
3. Tests harness (restore, collision, zip unsafe, catálogo len 3) fallan → **`no-go`**
4. Harness verde y matriz live `not-run` → **`blocked-insufficient-evidence`** ← **estado actual**
5. Live completo en ambos anchors sin bypass Gepard → **`go`** o **`go-reduced-scope`**
6. Live muestra fallback WineD3D silencioso, GDI roto, DLL stale o corrupción visual → **`no-go`** o **`go-reduced-scope`** con constraint explícita

**Fase 6B** solo con **`go`** o **`go-reduced-scope`**. **`blocked-insufficient-evidence`** y **`no-go`** prohíben 6B.

## 8. Fuera de alcance (6A)

- Toggle UI, catálogo curated `Validated`, descarga automática productiva (`ensure_catalog_artifact`).
- Variant `GraphicsProfile` / `GraphicsPlan` productivo.
- DXVK-Sarek, múltiples releases configurables, benchmarks, AutoTune.
- Cambios en spawn, `ro-sessiond`, compatibility catalog, prefixes del usuario.

## 9. Implementación del harness

- Módulo test-only: [`src-tauri/src/tools/runtime/d7vk_spike.rs`](../src-tauri/src/tools/runtime/d7vk_spike.rs).
- Contrato: [`RO_RUNTIME_PHASE_6A_CONTRACT.md`](../RO_RUNTIME_PHASE_6A_CONTRACT.md).
- Lock: `OperationGuard::acquire("d7vk-spike", scratch)` exclusivo; namespaces `prefix`/`dgvoodoo`/`runtime` no usados por el spike.
