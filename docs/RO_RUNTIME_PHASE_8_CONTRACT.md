# Contrato de implementación — Runtime Fase 8

| Campo | Valor |
| --- | --- |
| Estado | Fase 8 implementada en código |
| Fecha | 2026-09-18 |
| Alcance | Harness A/B opt-in, CSV v1, comparación sin score, flag `RO_LAUNCHER_RUNTIME_BENCHMARK` |
| Lectura obligatoria | `AGENTS.md`, ADR-006, plan §Fase 8, Fase 7 contract |
| Autoridad operacional | IPC benchmark; `launch_game` sin cambios; catálogo Gepard sin cambios |

## 1. Qué es operacional

1. **`src-tauri/src/tools/runtime/benchmark.rs`** — tipos v1, flag, CSV, percentiles, comparability, gates.
2. **`src-tauri/src/tools/runtime/benchmark_store.rs`** — store `benchmarks/`, attach/capture/import/visual.
3. **`src-tauri/src/commands/benchmarks.rs`** — IPC listado en ADR-006.
4. **`GameProcessHandle::running_facts_for`** — lectura de cliente en ejecución.
5. **`RunnerSessionRegistry::plan_id_for_prefix`** — validación de plan bajo supervisor.
6. **Frontend** — bloque en `AdvancedSettings.tsx`.

## 2. Flag `RO_LAUNCHER_RUNTIME_BENCHMARK`

Parser: `runtime_benchmark_enabled() <=> env != Some("0")`. Default (unset): ON.

| Valor | Mutación (start/begin/finish/import/visual) | list/export/delete/compare |
| --- | --- | --- |
| unset / ≠ `0` | sí | sí |
| `0` | `"runtime-benchmark-disabled"` | sí (datos existentes) |

Independiente de `RO_LAUNCHER_RUNTIME_OBSERVE`, `RO_LAUNCHER_RUNTIME_GRAPHICS`, `RO_LAUNCHER_RUNTIME_COMPAT`, `RO_LAUNCHER_RUNTIME_SHADOW`, `RO_LAUNCHER_PREFIX_V3`, `RO_LAUNCHER_SESSION_SUPERVISOR`.

## 3. Goldens

- [`contract-fixtures/runtime-benchmark-metrics-v1.json`](../contract-fixtures/runtime-benchmark-metrics-v1.json)
- [`contract-fixtures/runtime-benchmark-csv-v1.csv`](../contract-fixtures/runtime-benchmark-csv-v1.csv)
- [`contract-fixtures/runtime-benchmark-run-v1.json`](../contract-fixtures/runtime-benchmark-run-v1.json)
- [`contract-fixtures/runtime-benchmark-comparison-v1.json`](../contract-fixtures/runtime-benchmark-comparison-v1.json)

Verificación: Test A (`compute_frametime_metrics`) y Test B (percentiles literales) deben coincidir con el golden.

## 4. Store

- Ruta: `~/.local/share/ro-launcher/benchmarks/run-{uuid}.json`
- Escritura: `replace_json` (temp + rename)
- Lock: `BENCHMARKS_LOCK` (`Mutex<()>`)
- Retención: 100 registros / 90 días
- Export: destino fuera de `app_data_dir`; omite `client_id` y `server_local_id`

## 5. Rollback

`RO_LAUNCHER_RUNTIME_BENCHMARK=0` deja de crear runs; borrar `benchmarks/` no toca observations, servers ni prefixes.

## 6. Pendiente (fuera de Fase 8)

- Smokes live Sakura/Honey y AppImage instalada.
- Captura automática in-process de frametimes.

## 7. Prohibido (Fase 8)

- Modificar `launch_game`, env gráfico de spawn, protocolo `ro-sessiond`, catálogo curated.
- Score/winner/ranking, AutoTune activo, D7VK, overlays inyectados.
