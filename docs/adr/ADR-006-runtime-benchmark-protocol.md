# ADR-006: protocolo de benchmark reproducible

| Campo                 | Valor                                                                                    |
| --------------------- | ---------------------------------------------------------------------------------------- |
| Estado                | Aceptado                                                                                 |
| Fecha                 | 2026-09-18                                                                               |
| Alcance               | Fase 8 — harness A/B opt-in, sin AutoTune ni ranking                                     |
| Decisión              | Protocolo v1, captura `imported-csv-v1`, comparability explícita, store `benchmarks/`  |
| Autoridad relacionada | `AGENTS.md`, ADR-002 §fingerprints, Fase 7 observaciones, plan §Fase 8                   |

## 1. Contexto

La Fase 7 registra outcomes y fingerprints sin métricas de rendimiento. Comparar perfiles gráficos
requiere frametimes bajo protocolo fijo, gates de corrección visual y provenance de host/runtime.
DXVK HUD no exporta frametimes a archivo; overlays externos no están validados con Gepard.

## 2. Decisiones cerradas

1. **Harness separado del launch.** No segundo spawn; adjunta a cliente `Running` vía IPC.
2. **Store** `~/.local/share/ro-launcher/benchmarks/` distinto de `observations/` y del catálogo curated.
3. **Captura v1:** CSV importado (`frametimeMs` [, `monotonicNs`]); sin inyectar env en el juego.
4. **Métricas:** p50/p95/p99 nearest-rank, 1%/0.1% lows; sin score único ni winner.
5. **Comparability:** igualdad de spec, `server_token`, sujeto Gepard, `runner_kind`, digest de
   `prefix_fingerprint`, host GPU `known`, flags `graphics_plan`/`supervisor`, adapter y thermal.
6. **Gates:** visual passed, startup limpio, frametime disponible; `passed` no implica ganador.
7. **Flag** `RO_LAUNCHER_RUNTIME_BENCHMARK`: ON si `env != "0"`; default ON; rollback sin tocar prefixes.
8. **Observaciones** no promocionan a `Validated`; link best-effort por `client_id` + identidad.

## 3. Invalidación

Razones cerradas: `process-gone`, `plan-changed`, `warmup-too-short`, `duration-mismatch`,
`clock-jump-or-pause`, `insufficient-samples`, `linked-outcome-not-usable`, `corrupt-record`.

## 4. Privacidad

Export fuera de `app_data_dir`; omite `client_id`/`server_local_id`; redacción HOME/app_data en strings.

## 5. Fuera de alcance

AutoTune, ranking global, MangoHud/DXVK_HUD productivo, mutación de profiles, D7VK, upload remoto.

## 6. Consecuencias

- Fase 9 puede consumir `usable_for_autotune` informativo; Fase 8 no actúa sobre él.
- Contrato operacional: [`RO_RUNTIME_PHASE_8_CONTRACT.md`](../RO_RUNTIME_PHASE_8_CONTRACT.md).
