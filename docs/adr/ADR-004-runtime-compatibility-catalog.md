# ADR-004: catálogo de compatibilidad basado en evidencia

| Campo                 | Valor                                                                                                                        |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Estado                | Aceptado e implementado                                                                                                      |
| Fecha                 | 2026-09-18                                                                                                                   |
| Alcance               | Assessments Gepard exactos sobre runtime resuelto                                                                            |
| Decisión              | Catálogo curated v1, `CompatibilityRuntimeSpec`, sujeto = SHA-256 de `gepard.dll`                                            |
| Autoridad relacionada | [`AGENTS.md`](../../AGENTS.md), [ADR-001](ADR-001-runtime-graphics-domain.md), [ADR-002](ADR-002-runtime-prefix-identity.md) |

## 1. Contexto

Las dos reglas en `server_tools/gepard.rs` mapean hashes exactos a `GepardRunnerProfile`, pero
`deps/check.rs` validaba compatibilidad con predicados amplios (`is_proton()`, `is_wine_7_16()`).
Eso contradice `AGENTS.md`: Sakura exige Wine 7.16 old-WoW64 y DXVK 2.6.2 managed; Honey exige
Proton-CachyOS 11 administrado, no cualquier Proton.

El `RuntimeFingerprint` y los session facts ya están completos. El catálogo v1 deliberadamente no
usa ese digest para matching: su contrato es una especificación legible de runtime.

## 2. Decisiones cerradas

1. **Match key ≠ `RuntimeFingerprint`.** El record v1 usa `CompatibilityRuntimeSpec`.
2. **Sujeto Validated = solo `gepard.dll` SHA-256.** No `executableSha256` de presence ni hash de
   cliente en deps/launch.
3. **Sin records `Incompatible` ni `Experimental` shipped.** Mismatch de runtime → `Unknown` +
   `RuntimeRecommendation` cuando el hash curated coincide.
4. **El resolver no cambia runner, prefix, env ni spawn.** `server.runner` no vacío sigue ganando.
5. **Honey Validated:** Proton managed `ro-proton-cachyos-11.0-20260702-slr` (`is_managed_proton` +
   `ArtifactReceipt` con ese `artifact_id`). Otro Proton → `Unknown`.
6. **Sakura Validated:** `is_wine_7_16_version` en versión Known, `Wow64Layout::OldWow64` Known
   (StructuralProbeV1), `DxvkProvider` ManagedPrefix `dxvk-2.6.2`.
7. **dgVoodoo / `GraphicsProfile` no entran al spec.**
8. **PayloadVerification y salud de prefix no entran** al assessment.
9. **Catálogo shipped = constantes Rust + golden JSON.** `LocalObservation` no promueve a curated.
10. **Un solo almacén de hashes Gepard** en el módulo de catálogo; `gepard.rs` delega.
11. **Launch no consulta assessment** para `ready_to_launch`.
12. **Flag** `RO_LAUNCHER_RUNTIME_COMPAT`: ON si `env != "0"`; default ON.
13. **Sin locks nuevos** para lectura de `gepard.dll`; sin cache entre operaciones.
14. **Labels Validated** no usan `stack_label()` legacy (`DXVK 3.0.1`); describen el record curated.

## 3. Taxonomía

```rust
enum CompatibilityAssessment {
    Validated { evidence_id: EvidenceId },
    Experimental { evidence_id: EvidenceId },
    Incompatible { evidence_id: EvidenceId, reason: FailureClass },
    Unknown,
}
```

- **Validated:** sujeto hash exacto AND `CompatibilityRuntimeSpec` observado igual al record curated.
- **Unknown:** ausencia de hash, hash desconocido, runtime no demostrable, mismatch, o provenance no
  curated.
- **Recommendation:** perfil sugerido cuando el hash curated coincide; no sustituye selección explícita
  ni fuerza runner.

`RuntimeEligibility` de prefix (ADR-002) permanece separado; no se fusiona con compatibility.

## 4. Matching

1. `SubjectObservation::Hash` con 64 hex (normalizado lowercase).
2. Record curated con `gepard_sha256` igual (case-insensitive).
3. `observed_runtime_spec(plan, probe)` → `Option<CompatibilityRuntimeSpec>`.
4. Si `observed == record.runtime` y record `Validated` con mismo `evidence_id` → `Validated`.
5. Cualquier otra combinación → `Unknown` (recomendación conservada si hash matcheó).

No wildcard por familia de runner, server id ni nombre de servidor.

## 5. Curated vs local

Sólo `EvidenceProvenance::CuratedShipped` alimenta el resolver productivo. `LocalObservation` y
futuras observaciones locales producen `Unknown` hasta revisión manual. El store `observations/` no
alimenta el resolver productivo ni `shipped_compatibility_records`.

## 6. Rollback

`RO_LAUNCHER_RUNTIME_COMPAT=0` restaura el check `gepard-runner` legacy (`is_proton` /
`is_wine_7_16`) y omite `DependencyStatus.compatibility`. No reescribe datos en disco.

## 7. Fuera de alcance

Crowdsourcing, backend remoto, fuzzy hash, `RuntimeFingerprint` digest, D7VK, AutoTune, bloqueo de
launch, modificación de Gepard/GameGuard, match por `servers.json` labels.

## 8. Consecuencias

- UI y deps pueden citar `evidence_id` exacto en Validated.
- Shadow puede mapear recommendation a `GepardRunnerProfile` sin ampliar predicados de Validated.
- El catálogo v1 conserva `CompatibilityRuntimeSpec` como match key por decisión, no por ausencia
  del `RuntimeFingerprint`.
