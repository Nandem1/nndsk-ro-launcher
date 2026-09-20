# ADR-003: catálogo de artefactos y receipts

| Campo                 | Valor                                                                                        |
| --------------------- | -------------------------------------------------------------------------------------------- |
| Estado                | Aceptado e implementado                                                                      |
| Fecha                 | 2026-09-17                                                                                   |
| Alcance               | UMU, Proton-CachyOS y DXVK 2.6.2 administrados                                               |
| Decisión              | Pipeline común descriptor → fetch/verify → safe extract → install → receipt; markers v1 + v2 |
| Autoridad relacionada | `AGENTS.md`, `ADR-001-runtime-graphics-domain.md`, `ADR-002-runtime-prefix-identity.md`      |

## 1. Contexto

El pipeline anterior descargaba, verificaba, extraía e instalaba tres artefactos desde
`tools/runners/managed.rs`, con lógica y receipts acoplados. Los fingerprints ya consumían una
`ArtifactIdentity`, pero el marker schema 1 sólo conservaba `artifactId` y digest de fuente.

La extracción a un módulo común conserva URLs, tamaños, digests y paths `runtime/<id>`. No introduce
package manager, mirrors ni flags de transición.

## 2. Decisión

1. **Módulo** `src-tauri/src/tools/artifacts/` con descriptor, allowlist, fetch, extract seguro, install transaccional y receipt v1/v2 en el mismo archivo `.ro-launcher-runtime.json`.
2. **`ArtifactReceipt` in-memory** permanece ADR-002 (`identity` + `payload_verification`). En disco, **RuntimeReceiptV2** proyecta identidad estable (kind, version, source, platform, architectures, recipe_revision).
3. **Allowlist** = URL exacta del catálogo; sólo `https://`. Redirects de reqwest se conservan (GitHub releases).
4. **Extract:** rechazar paths absolutos, `..` fuera de staging y symlinks que escapen; permitir symlinks relativos internos (Proton protonfixes).
5. **Payload:** archivos requeridos deben ser regulares (no symlink); `is_executable` sin cambio semántico.
6. **Locks:** `OperationGuard` exclusivo namespace `runtime` en `ensure_*`; `artifact_ready` sin lock; elevación v1→v2 sólo bajo lock en `ensure_catalog_artifact`.
7. **Restore:** fallo post-backup restaura el directorio anterior; error de restore no se traga.
8. **`PayloadVerification`:** no se persiste; probe sigue `ShapeVerified` cuando layout pasa, no `SourceAndPayloadVerified`.

## 3. Schema evolution

| schema_version | Lectura                   | Escritura nueva                            |
| -------------- | ------------------------- | ------------------------------------------ |
| 1              | Sí                        | No (sólo elevación desde ensure bajo lock) |
| 2              | Sí                        | Sí (install commit)                        |
| >2             | No (not ready; no borrar) | No                                         |

## 4. Alternativas rechazadas

- Package manager o URL desde `servers.json`
- Host-allowlist de redirects
- Rechazar todos los symlinks en extract
- Feature flag `RO_LAUNCHER_ARTIFACT_*`
- Persistir `payload_verification` en JSON
- Segundo filename de receipt

## 5. Consecuencias

- Los futuros artefactos reutilizan el mismo pipeline añadiendo descriptor + validator cerrado;
  D7VK queda excluido por el `no-go` de ADR-005.
- Un binario antiguo que no entiende schema 2 puede reinstalar tras rollback si sólo ve v2; el
  payload previo se preserva hasta el commit exitoso de la instalación.
