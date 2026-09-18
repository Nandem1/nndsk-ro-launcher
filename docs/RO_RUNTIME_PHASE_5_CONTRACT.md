# Contrato de implementación — Runtime Fase 5

| Campo | Valor |
| --- | --- |
| Estado | Fase 5 implementada en código |
| Fecha | 2026-09-18 |
| Alcance | Catálogo Gepard exacto, flag `RO_LAUNCHER_RUNTIME_COMPAT`, IPC `compatibility` |
| Lectura obligatoria | `AGENTS.md`, ADR-004, plan §Fase 5, Fases 0–4 contracts |
| Autoridad operacional | Assessment en deps/tools; spawn/setup/prefix sin cambio |

## 1. Qué es operacional

1. **`src-tauri/src/tools/runtime/compatibility.rs`** — catálogo curated, `assess_compatibility`, flag.
2. **`src-tauri/src/tools/runtime/inspection.rs`** — `inspect_subject` → `gepard.dll` SHA-256.
3. **`deps/check.rs`** — `RuntimeCheck` `gepard-runner` y `DependencyStatus.compatibility` con COMPAT ON.
4. **`server_tools/pe.rs`** — warnings de sujeto vía `gepard_subject_warnings`.
5. **`gepard.rs`** — hashes delegan al catálogo; legacy COMPAT=0 intacto.
6. **Frontend** — `CompatibilityStatus` en types; línea en AdvancedSettings.

## 2. Flag `RO_LAUNCHER_RUNTIME_COMPAT`

Parser: `runtime_compat_enabled() <=> env != Some("0")`. Default (unset): ON.

| Valor | Deps / IPC |
| --- | --- |
| unset / ≠ `0` | Assessment exacto + `compatibility` + check nuevo |
| `0` | Predicados legacy `is_proton` / `is_wine_7_16`; sin campo `compatibility` |

Independiente de `RO_LAUNCHER_RUNTIME_GRAPHICS`, `RO_LAUNCHER_RUNTIME_SHADOW`, `RO_LAUNCHER_PREFIX_V3`, `RO_LAUNCHER_SESSION_SUPERVISOR`.

## 3. Goldens

- [`contract-fixtures/runtime-compatibility-catalog-v1.json`](../contract-fixtures/runtime-compatibility-catalog-v1.json)
- [`contract-fixtures/runtime-plan-summary.json`](../contract-fixtures/runtime-plan-summary.json) — campo `compatibility` opcional aditivo

## 4. Records shipped

| evidence_id | gepard fileVersion | runtime spec |
| --- | --- | --- |
| `gepard-26.8.26.1-proton-cachyos-11` | 26.8.26.1 | Managed Proton CachyOS 11 |
| `gepard-26.9.3.1-wine-7.16-old-wow64-dxvk-2.6.2` | 26.9.3.1 | Wine 7.16 old-WoW64 + DXVK 2.6.2 managed |

## 5. Rollback

`RO_LAUNCHER_RUNTIME_COMPAT=0` restaura mensajes y predicados Gepard previos. No muta prefixes,
manifests ni settings.

## 6. Pendiente (fuera de Fase 5)

- `compute_runtime_fingerprint` completo y `plan_id` discriminante.
- Observaciones locales persistidas (Fase 7).
- Smokes live Sakura/Honey con `gepard.dll` real en máquina de desarrollo.

## 7. Prohibido (Fase 5)

- Forzar runner; bloquear launch; usar `executableSha256` como sujeto Validated.
- Records `Incompatible` productivos; match por server id; D7VK; modificar anti-cheat.
