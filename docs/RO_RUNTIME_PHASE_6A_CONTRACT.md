# Contrato de implementación — Runtime Fase 6A

| Campo | Valor |
| --- | --- |
| Estado | Spike implementado; veredicto ADR-005 `blocked-insufficient-evidence` |
| Fecha | 2026-09-18 |
| Alcance | Pin D7VK v2.2, harness install/restore, ADR-005 |
| Lectura obligatoria | `AGENTS.md`, ADR-005, plan §Fase 6A, Fases 0–5 contracts |
| Autoridad operacional | **Ninguna** — spawn/setup/deps siguen Fase 4; catálogo sigue 3 artefactos |

## 1. Qué es operacional

**Nada del spike es autoridad de producto.** El launcher productivo no descarga, instala ni selecciona D7VK.

1. **`src-tauri/src/tools/runtime/d7vk_spike.rs`** — solo `#[cfg(test)]`; pin, extract zip seguro, install side-by-side y prefix-owned en scratch, restore, fault injection.
2. **`contract-fixtures/d7vk-spike-v2.2.json`** — pin verificable del release.
3. **`docs/adr/ADR-005-runtime-d7vk-deployment.md`** — deployment, §12.3, veredicto.
4. **`scripts/d7vk-spike-live.sh`** — protocolo live opt-in; no integrado en npm/CI.

## 2. Feature flags

**No se introduce flag.** `RO_LAUNCHER_RUNTIME_GRAPHICS`, `RO_LAUNCHER_RUNTIME_SHADOW`, `RO_LAUNCHER_PREFIX_V3`, `RO_LAUNCHER_RUNTIME_COMPAT`, `RO_LAUNCHER_SESSION_SUPERVISOR` conservan semántica Fases 1–5.

## 3. Goldens

- [`contract-fixtures/d7vk-spike-v2.2.json`](../contract-fixtures/d7vk-spike-v2.2.json)

Tests: `cargo test -p ro-launcher --lib d7vk -- --test-threads=1`

Test opcional con red: `pinned_release_zip_matches_fixture` (`#[ignore]`).

## 4. Veredicto ADR-005

| Resultado | Significado |
| --------- | ----------- |
| `blocked-insufficient-evidence` | Harness OK; matriz live `not-run` — **estado actual** |
| `no-go` | Pin/layout/harness fallan |
| `go` / `go-reduced-scope` | Requiere matriz live en ambos anchors — **no alcanzado** |

Fase **6B prohibida** hasta `go` o `go-reduced-scope`.

## 5. Rollback

Revertir el commit de Fase 6A elimina harness y documentación. No muta datos de usuario. Catálogo `CATALOG` y `GraphicsProfile` sin cambios.

## 6. Pendiente (fuera de 6A)

- Matriz live Sakura/Honey (o cliente RO controlado) con `scripts/d7vk-spike-live.sh`.
- Decisión owner side-by-side vs prefix-owned con evidencia de carga Wine.
- Fase 6B vertical slice si ADR-005 pasa a `go`.

## 7. Prohibido (Fase 6A)

- Tercer variant en `GraphicsProfile` / `GraphicsPlan` productivo.
- Entrada en `CATALOG` o `ensure_catalog_artifact` para D7VK.
- Toggle UI, settings, compatibility records D7VK.
- Mutar prefixes o game dirs del usuario fuera de scratch en tests.
- Afirmar superioridad vs dgVoodoo+DXVK sin A/B live.
