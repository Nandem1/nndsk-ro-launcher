# Contrato de implementación — Runtime Fase 7

| Campo | Valor |
| --- | --- |
| Estado | Fase 7 implementada en código |
| Fecha | 2026-09-18 |
| Alcance | `RuntimeFingerprint` v1, observaciones locales, flag `RO_LAUNCHER_RUNTIME_OBSERVE` |
| Lectura obligatoria | `AGENTS.md`, ADR-002, ADR-004 §5, plan §Fase 7, Fases 2–6A contracts |
| Autoridad operacional | Fingerprints en anclas de sesión; observaciones no alteran spawn ni catálogo curated |

## 1. Qué es operacional

1. **`src-tauri/src/tools/runtime/runtime_fingerprint.rs`** — encoding `RuntimeFingerprintInput` v1 y `compute_runtime_fingerprint_from_input`.
2. **`session_anchor.rs`** — `runtime_anchor_for_operation`, `session_anchor_unavailable`, `operational_session_anchor`.
3. **`observation.rs` / `observation_store.rs` / `host_gpu.rs`** — schema v1, store append, retention, export/delete IPC.
4. **Launch** — reorden scan/plan/anchor; `try_exit_status`; persistencia tras `mark_running`, en `spawn_exit_task` y en fallo de arranque sin `mark_running`.
5. **Commands** — `list_runtime_observations`, `export_runtime_observations`, `delete_runtime_observations`.
6. **Frontend** — línea y acciones en `AdvancedSettings.tsx`.

## 2. Flag `RO_LAUNCHER_RUNTIME_OBSERVE`

Parser: `runtime_observe_enabled() <=> env != Some("0")`. Default (unset): ON.

| Valor | Escritura observaciones | IPC list/export/delete |
| --- | --- | --- |
| unset / ≠ `0` | sí (best-effort, no bloquea launch) | sí |
| `0` | no crea archivos nuevos | sí (datos existentes) |

Independiente de `RO_LAUNCHER_RUNTIME_GRAPHICS`, `RO_LAUNCHER_RUNTIME_COMPAT`, `RO_LAUNCHER_RUNTIME_SHADOW`, `RO_LAUNCHER_PREFIX_V3`, `RO_LAUNCHER_SESSION_SUPERVISOR`.

## 3. Goldens

- [`contract-fixtures/runtime-fingerprint-v1.json`](../contract-fixtures/runtime-fingerprint-v1.json) — Proton managed, Wine 7.16 DXVK y Wine 7.16 dgVoodoo; incluye `canonicalFieldIds`. Los inputs sintéticos fijan digests de prefix/material observado para que el golden no dependa del runtime instalado ni de la ruta absoluta del checkout; la identidad productiva sí conserva su location token y hashes observados reales.
- [`contract-fixtures/runtime-observation-v1.json`](../contract-fixtures/runtime-observation-v1.json) — ejemplo finished redacted.
- [`contract-fixtures/runtime-plan-summary.json`](../contract-fixtures/runtime-plan-summary.json) — `planId` Proton y Wine 7.16 dgVoodoo actualizados al digest v1.

Verificación independiente: Test A (`runtime_fingerprint_input_from_plan`) y Test B (árbol `CanonicalValue` escrito a mano) deben producir el mismo `digestSha256Hex`.

## 4. Store

- Ruta: `~/.local/share/ro-launcher/observations/obs-{uuid}.json`
- Escritura: `replace_json` (temp + rename)
- Lock: `OBSERVATIONS_LOCK` (`Mutex<()>`), nunca en ruta crítica de launch (spawn_blocking fire-and-forget)
- Retención: 200 registros / 90 días
- Export: destino fuera de `app_data_dir`; omite `server_local_id` y `client_id` (token 16 hex)
- Launch: `Started` tras `mark_running`; `Finished` en `spawn_exit_task`. Si el proceso de juego no aparece, un solo `spawn_blocking` escribe `Started`+`Finished` (`StartupTimeout` / `StartupFailed` / `UserStop`) sin `mark_running`.

## 5. Rollback

`RO_LAUNCHER_RUNTIME_OBSERVE=0` deja de escribir; borrar observaciones no toca servers, prefixes ni catálogo Gepard.

## 6. Pendiente (fuera de Fase 7)

- Smokes live Sakura/Honey y AppImage instalada.

`npm run tauri:dev` arrancó en esta pasada: sidecars, Vite en `:5173` y `cargo run` de `ro-launcher` sin panic. No sustituye la matriz live Sakura/Honey.

## 7. Prohibido (Fase 7)

- Promover observaciones a `Validated` o mutar `shipped_compatibility_records`.
- Cambiar protocolo `ro-sessiond` / memoria.
- D7VK productivo, upload automático. El harness de benchmark vive en Fase 8 (`benchmarks/`, ADR-006).
- Persistir args/credenciales/paths crudos en observaciones.
