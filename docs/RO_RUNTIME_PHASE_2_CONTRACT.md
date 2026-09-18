# Contrato de implementación — Runtime Fase 2

| Campo | Valor |
| --- | --- |
| Estado | Fase 2 implementada en código; `RuntimeFingerprint` completo pendiente de facts en orquestación |
| Fecha | 2026-09-17 |
| Commit local | `aaf5836` (*Implement runtime phase 2 prefix identity and session anchors*) |
| Alcance | Fingerprints, path/manifest v3, alias v2, session `plan_id`; sin env operacional (Fase 4) |
| Lectura obligatoria | `AGENTS.md`, [ADR-002](adr/ADR-002-runtime-prefix-identity.md), plan §11.2 |
| Autoridad operacional | Identidad de prefix aislado administrado; gráficos/env siguen shadow + legacy spawn |

Este documento fija decisiones del handoff Fase 2. No reemplaza ADR-002; si hay conflicto, gana ADR-002.

## 1. Qué es operacional

1. **`PrefixFingerprint`** (encoder canónico v1, goldens bloqueados en tests).
2. **`resolve_prefix_binding`** — lookup v3 siempre; write-new v3 según `RO_LAUNCHER_PREFIX_V3`.
3. **`WineContext.identity` + `WineContext.probe`** — binding y material observado antes de fijar path.
4. **Manifiesto schema 3** — `write_prefix_manifest_v3` en setup cuando el binding lo exige; alias v2 sigue schema 2.
5. **`SessionAnchorV2` / `plan_id`** — pasado a `RunnerOperation::begin`; el registry rechaza otro `plan_id` con lease activo.
6. **Deps pending managed** — `resolve_prefix_binding_for_managed_descriptor` sin ejecutable Proton en disco.
7. **Rebuild managed** — no por schema futuro (`>3`); no adoptar custom ni nonempty sin manifest.

## 2. Flag `RO_LAUNCHER_PREFIX_V3`

Parser: `prefix_v3_write_enabled() <=> env != Some("0")`. Default (unset): write-new habilitado.

| Situación (isolated + managed) | unset / ≠ `0` | `=0` |
| --- | --- | --- |
| Existe v3 + full match | `V3Verified` | igual |
| v3 mismatch | `Incompatible`; no borrar | igual |
| v2 legacy match | `LegacyV2RunnerMatched` | igual |
| Provisión nueva | path `*-v3-*`, schema 3 | path FNV, schema 2 |
| v2 existe, plan no alias | path v3 nuevo; preservar v2 | path FNV runner actual; schema 2 |
| Repair mismo v3 | schema 3 | igual |
| Repair alias v2 | schema 2 | schema 2 |

**Read-existing** (lookup v3) corre siempre. **Write-new** v3 solo con flag habilitada o repair de un v3 ya verificado.

## 3. Writer v2 vs v3

- **v2:** `write_prefix_manifest` — schema 2; repair/rebuild de alias legacy; provisión con `RO_LAUNCHER_PREFIX_V3=0`.
- **v3:** `write_prefix_manifest_v3` — requiere `prefixFingerprint` con digest completo (64 hex); path `prefixes/<token16>-v3-<first24hex(digest)>`.
- **Prohibido:** dgVoodoo en manifest de prefix; promover v2→v3 in-place por cambio de runner; rebuild por schema desconocido.

## 4. Goldens obligatorios (tests Rust)

| Artefacto | Valor |
| --- | --- |
| Record `{count:1, flag:true}` SHA-256 | `855bf92f11437f37d149d93f865ac9dff6354980f5f546a774ca88d95d03f791` |
| ServerPathToken `fixture-server` (16 hex) | `d8e0b61318835001` |
| External location `/opt/wine/bin/wine` | `991911a552070c3379dcb3f09f1389363b49e2fc09f5ba75af38fae58d453301` |
| PrefixFingerprint managed Proton | `a23f2a940a9cb8e5110b0e454ad68a35a22af760c005bcd0ebdf47ab44330270` |

Fixture JSON: [`contract-fixtures/runtime-prefix-manifest-v3.json`](../contract-fixtures/runtime-prefix-manifest-v3.json).

## 5. Módulos principales

| Área | Ubicación |
| --- | --- |
| Encoder | `src-tauri/src/tools/runtime/encode.rs` |
| Fingerprints | `src-tauri/src/tools/runtime/fingerprint.rs` |
| Binding / alias | `src-tauri/src/tools/runtime/identity.rs` |
| Identidad managed | `src-tauri/src/tools/runtime/managed_identity.rs` |
| Probe + cache | `src-tauri/src/tools/runtime/probe.rs`, `material.rs` |
| Session anchor | `src-tauri/src/tools/runtime/session_anchor.rs` |
| Prefix I/O | `src-tauri/src/utils/prefix.rs` |

## 6. Pendiente explícito (misma release lógica, siguiente iteración)

- **`compute_runtime_fingerprint`** con `RuntimePlan` + facts de dgVoodoo/WebView2 (hoy placeholder en anchor de sesión).
- Goldens adicionales de **RuntimeFingerprint** (Proton+DXVK, Wine 7.16, overlay dgVoodoo).
- Tests dedicados: collision path v3 (cubierto en `identity.rs` / `prefix.rs`); pendientes `plan_id` bajo lease, flag `=0` write-new, matriz ADR-002 §3 ampliada.
- Smoke **`npm run tauri:dev`**: prefix aislado nuevo v3, alias v2 fixture, flag `=0` (plan §12).

## 7. Gates ejecutados (2026-09-17)

```text
npm run lint
npm test
npm run build
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

No ejecutados en esta pasada: `npm run format:check`, `npm run tauri:dev`, Sakura/Honey live, AppImage instalada, multi-client juego.

## 8. Rollback y datos en disco

Revert del commit de Fase 2 restaura binario pre-v3. Prefixes con schema 3 en disco requieren este reader (mismo commit); un binario anterior sólo entiende schema 2 y los marcará incompatibles **sin borrarlos**.

## 9. Prohibido (Fase 2)

- Migrar/renombrar/borrar v2; GC de prefixes; env operacional desde `GraphicsPlan`.
- Fingerprinting en memory/input/presence/`ro-sessiond`.
- Push del commit sin revisión; modificar Gepard/validación servidor.
- Debilitar Yama; `sudo` para operación normal.
