# Contrato de implementación — Runtime Fase 4

| Campo | Valor |
| --- | --- |
| Estado | Fase 4 implementada en código |
| Fecha | 2026-09-18 |
| Alcance | `GraphicsPlan` como autoridad operacional DXVK/dgVoodoo; flag `RO_LAUNCHER_RUNTIME_GRAPHICS` |
| Lectura obligatoria | `AGENTS.md`, ADR-001 §7/§14, plan §Fase 4, Fases 0–3 contracts |
| Autoridad operacional | `environment_for` + `apply_graphics_environment*` cuando flag ≠ `0`; legacy booleanos sólo con flag `=0` |

## 1. Qué es operacional

1. **`src-tauri/src/tools/runtime/apply.rs`** — flag, `resolve_operational_plan`, `InvocationPlan`, aplicación de `GraphicsEnvironment` a `ProcessEnv` / `RunnerInvocation`.
2. **Launch y server tools** — resuelven plan con `ctx.probe` + `ctx.identity`; targets explícitos (`Game`, `LaunchPatcher`, `MaintenancePatcher`, `OpenSetup`, `GraphicsControlPanel`).
3. **Prefix/deps** — `DxvkProvision` derivado del plan cuando el flag está activo; `DependencyStatus.runtimePlan` opcional.
4. **Shadow** — sigue observando; usa `ctx.probe` (no reprobea); no bloquea launch por divergencia.
5. **Sin cambio** en `dgvoodoo.rs` installer, catálogo Fase 3, schemas prefix/dgvoodoo/receipt.

## 2. Flag `RO_LAUNCHER_RUNTIME_GRAPHICS`

Parser: `runtime_graphics_plan_enabled() <=> env != Some("0")`. Default (unset): ON.

| Valor | Spawn / deps / setup |
| --- | --- |
| unset / ≠ `0` | Plan operacional + `apply_graphics_environment_to_invocation` |
| `0` | `apply_game_env` / `apply_tool_env` (deprecated, retiro 2026-12-18) |

Independiente de `RO_LAUNCHER_RUNTIME_SHADOW`, `RO_LAUNCHER_PREFIX_V3`, `RO_LAUNCHER_SESSION_SUPERVISOR`.

## 3. Goldens

- [`contract-fixtures/runtime-graphics-env-v1.json`](../contract-fixtures/runtime-graphics-env-v1.json)
- [`contract-fixtures/runtime-plan-summary.json`](../contract-fixtures/runtime-plan-summary.json)

## 4. Pendiente (fuera de Fase 4)

- `compute_runtime_fingerprint` completo y `plan_id` que discrimine overlay (placeholder actual).
- Smokes live Sakura/Honey, AppImage instalada, multi-client juego.
- `expected_plan_id` IPC, UI de profiles, compatibility catalog (Fase 5).

## 5. Rollback

`RO_LAUNCHER_RUNTIME_GRAPHICS=0` restaura el builder legacy por proceso. No reescribe manifests ni prefixes.

## 6. Prohibido (Fase 4)

- D7VK, DXVK 3, mover dgVoodoo al prefix, rediseño UI, flags de catálogo, alterar protocolo `ro-sessiond`.
