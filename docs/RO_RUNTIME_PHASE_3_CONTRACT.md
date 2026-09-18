# Contrato de implementación — Runtime Fase 3

| Campo | Valor |
| --- | --- |
| Estado | Fase 3 implementada en código |
| Fecha | 2026-09-17 |
| Alcance | Catálogo UMU / Proton-CachyOS / DXVK 2.6.2, receipts v2, pipeline común |
| Lectura obligatoria | `AGENTS.md`, [ADR-003](adr/ADR-003-runtime-artifact-catalog.md), plan §Fase 3 |
| Autoridad operacional | Mismo fetch/install que Fase 2; marker v1 legible; escritura v2 en install/elevación |

## 1. Qué es operacional

1. **`src-tauri/src/tools/artifacts/`** — descriptor, allowlist, fetch, extract seguro, install, receipt, payload.
2. **`ensure_catalog_artifact`** — bajo lock `runtime`; elevación v1→v2 si payload válido.
3. **`managed.rs`** — wrappers públicos sin cambio de firmas; delega al catálogo.
4. **Receipt v2** en `.ro-launcher-runtime.json` (mismo path que schema 1).
5. **Sin feature flag** de catálogo; flags de Fase 1/2/supervisor no alteradas.

## 2. Goldens

- [contract-fixtures/runtime-markers.json](../contract-fixtures/runtime-markers.json) (v1, sin cambios).
- [contract-fixtures/runtime-receipts-v2.json](../contract-fixtures/runtime-receipts-v2.json) (proton, umu, dxvk, futureSchema rechazado).
- PrefixFingerprint Proton managed: sin cambio (`a23f2a940a9cb8e5110b0e454ad68a35a22af760c005bcd0ebdf47ab44330270`).

## 3. Módulos

| Área | Ubicación |
| --- | --- |
| Catálogo | `src-tauri/src/tools/artifacts/descriptor.rs` |
| Pipeline | `fetch.rs`, `extract.rs`, `install.rs`, `receipt.rs`, `payload.rs`, `source.rs` |
| API pública runners | `src-tauri/src/tools/runners/managed.rs` |
| Identidad Fase 2 | `src-tauri/src/tools/runtime/managed_identity.rs` (sin `SourceAndPayloadVerified`) |

## 4. Pendiente (fuera de Fase 3)

- `compute_runtime_fingerprint` completo (Fase 2 §6).
- Smoke `npm run tauri:dev` / AppImage / Sakura-Honey live.

## 5. Rollback

Revert del commit de Fase 3 restaura binario pre-catálogo. Payloads bajo `runtime/<id>` intactos; schema 2 en disco sigue siendo leído por este binario; binario anterior puede reinstalar si sólo ve schema 2.

## 6. Prohibido (Fase 3)

- Cambiar URLs, digests, tamaños o paths de los tres artefactos.
- Package manager, D7VK, DXVK 3, flags nuevas de catálogo.
- Persistir `PayloadVerification` en JSON de runtime.
