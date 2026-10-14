# ADR-003: catálogo de artefactos y receipts

| Campo                 | Valor                                                                                        |
| --------------------- | -------------------------------------------------------------------------------------------- |
| Estado                | Aceptado e implementado                                                                      |
| Fecha                 | 2026-09-17                                                                                   |
| Alcance               | UMU, Proton-CachyOS y DXVK 2.6.2 administrados                                               |
| Decisión              | Pipeline común descriptor → fetch/verify → safe extract → receipt → Ready → activación atómica |
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
7. **Activación:** receipt durable y Ready dentro del árbol privado antes del commit; nunca se restaura una generación intercambiada (F-06, sección 6).
8. **`PayloadVerification`:** no se persiste; probe sigue `ShapeVerified` cuando layout pasa, no `SourceAndPayloadVerified`.

## 3. Schema evolution

| schema_version | Lectura                   | Escritura nueva                            |
| -------------- | ------------------------- | ------------------------------------------ |
| 1              | Sí                        | No (sólo elevación desde ensure bajo lock) |
| 2              | Sí                        | Sí (preparación, antes de activar)                        |
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

## 6. F-06: preparación, commit y recuperación

Descarga e importación local convergen en `import_archive_locked` → `prepare` → `activate`.
Online conserva HTTPS/pins; import copia y verifica un snapshot privado. Ambas verifican
tamaño/checksum y usan las mismas defensas de extracción. El task dueño conserva
`OperationGuard("runtime", runtime)` y sus paths aunque se cancele el waiter; el extractor
bloqueante recibe guard y cleanup juntos.

`prepare` valida payload y escribe RuntimeReceiptV2 en `.ro-launcher-runtime.json`:
create-new temp → write-all → fsync archivo → rename → fsync del árbol preparado.
Luego exige `artifact_cache_state_at(prepared) == Ready`. Los archivos requeridos no pueden
depender de directorios symlink fuera del árbol trasladable. No hay escritura crítica en final
tras activar. Schema, filename, pins y lectura v1 no cambian. La elevación v1 exige identidad
positiva y payload Ready; el rename deja v1 válido o v2 completo. Receipts no regulares,
mayores de 64 KiB, corruptos, desconocidos o de otra identidad no autorizan mutación; FIFO
no bloquea la lectura.

`F = runtime/<id>`, `S = runtime/.<id>.activation`, bajo guard exclusivo:

1. Reconciliar S y barrer residuos antes de adquirir paths nuevos.
2. Mover el árbol ya Ready a S con `RENAME_NOREPLACE`; sincronizar ambos padres.
3. F ausente: `renameat2(S, F, RENAME_NOREPLACE)`. Una aparición concurrente nunca se sustituye.
4. F incompleto con receipt v1/v2 exactamente coincidente: sólo UMU/DXVK permiten
   `renameat2(S, F, RENAME_EXCHANGE)`. Runners mantienen `Refuse` para destinos non-Ready.
   Un F ya Ready se reutiliza. No se adopta payload sin receipt ni identidad histórica inferida.
5. El syscall exitoso es el commit. F nunca falta durante exchange. Fsync de runtime después.
6. Limpiar S; errores posteriores jamás intercambian OLD de vuelta a F.

El plan permite conservar OLD **non-Ready** mientras se prepara su reparación. El contrato
productivo es ausente / OLD receipted non-Ready / NEW Ready, no reemplazo automático de runners
o generaciones ya Ready. Las pruebas también intercambian dos generaciones sintéticas Ready
mediante el mismo primitive para verificar visibilidad.

| Final F                          | Slot S                                      | Acción                                         | Autoridad destructiva |
| -------------------------------- | ------------------------------------------- | ---------------------------------------------- | --------------------- |
| Ausente                          | Ausente                                     | Missing estable                                | Ninguna               |
| Ready                            | Ausente                                     | Reutilizar                                     | Ninguna sobre F       |
| Ausente                          | Ready o preparación con receipt coincidente | Descartar preparación y reintentar             | Sólo S identificado   |
| OLD Ready o owned-incomplete     | NEW Ready                                   | Conservar OLD, descartar NEW                   | Sólo S identificado   |
| NEW Ready                        | OLD con receipt coincidente                 | NEW gana, limpiar OLD                          | Sólo S identificado   |
| NEW Ready                        | OLD parcial con tombstone coincidente       | Continuar cleanup, jamás restaurar             | Sólo S identificado   |
| Ausente / owned-incomplete       | Preparación parcial con tombstone           | Continuar descarte, final intacto              | Sólo S identificado   |
| Foreign/ambiguo o tombstone en F | Cualquiera                                  | Preservar todo y rechazar                      | Ninguna               |
| Ausente / owned-incomplete       | Foreign/ambiguo                             | Preservar y rechazar activación                | Ninguna               |
| Ready                            | Foreign/ambiguo                             | Preservar S y diagnosticar; F sigue utilizable | Ninguna sobre S       |

No hace falta distinguir NEW-prepared de OLD-exchanged en S: F es siempre la autoridad,
S jamás restaura. Antes de borrar payload, se renombra su receipt a
`.ro-launcher-artifact-cleanup` y se sincroniza S. Ese tombstone es sólo cleanup, no otro
receipt Ready. Se borran entradas sin seguir symlinks, se sincroniza S, se borra el tombstone
al final, se quita el directorio y se sincroniza runtime. Muerte entre borrar tombstone y
rmdir deja un directorio vacío **ambiguo**: se conserva; F Ready sigue utilizable.

Wrappers privados `.id.staging-<pid>-<ns>`, `.id.import-<pid>-<ns>` y
`.id.download-<pid>-<ns>` llevan el mismo receipt antes de extraer/descargar; prueba ownership
del payload parcial. La barrida exige nombres exactos **y** receipt/tombstone coincidente.
Los antiguos `.id.previous-<ns>` sólo se limpian con identidad positiva y F Ready; jamás se
restauran ni se elige «el más nuevo». Residuos legacy sin receipt y nombres extranjeros se
conservan con diagnóstico. Muerte durante mkdir/escritura inicial puede dejar un wrapper sin
evidencia: se conserva, no se adopta por el nombre.

Lectores sin lock fijan el inode abierto y validan a través de `/proc/self/fd`. Si canonical
cambió durante lectura/cleanup, reintentan sobre el inode actual: no publican la invalidez
temporal de OLD borrándose como estado de NEW.

`activation_tests.rs` usa subprocesses, rendezvous stdout/stdin y SIGKILL en receipt temporal,
preparación Ready, NOREPLACE/exchange, fsync posterior y cleanup. Mata recuperación dos veces,
reacquiere guard sin borrar locks, preserva special files/datos extranjeros y prueba race
NOREPLACE, cancelación real del importer y lectores concurrentes. Hooks sólo `cfg(test)`;
no variable productiva de pausa. Se califica **muerte de proceso/SIGKILL**. El orden de fsync
está implementado; **power loss no está calificado**, ni se infiere persistencia de todas las
páginas del payload ante corte eléctrico.
