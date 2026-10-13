# ADR-002: fingerprints e identidad de prefix

| Campo                 | Valor                                                                                                   |
| --------------------- | ------------------------------------------------------------------------------------------------------- |
| Estado                | Aceptado e implementado                                                                                 |
| Fecha                 | 2026-09-16                                                                                              |
| Decisión              | Separar identidad de aislamiento e identidad de ejecución; escribir prefix v3 sin migración destructiva |
| Autoridad relacionada | [`AGENTS.md`](../../AGENTS.md), [arquitectura vigente](../RO_RUNTIME_ARCHITECTURE_PLAN.md)              |

## Contexto

El path legacy separaba servidor y runner, pero un path o una versión textual no demostraban qué
bytes, layout WoW64 o provider prefix-owned habían creado el entorno. Tampoco toda diferencia de
ejecución exige otro prefix: sync, dgVoodoo o una receta repairable no deben multiplicar directorios.

Se necesitan dos identidades con ciclos de vida distintos y una migración que no ponga datos del
usuario en riesgo.

## Decisión

### Dos fingerprints

`PrefixFingerprint` representa sólo el material de aislamiento que no debe cambiar in-place:

- revisión material de la receta;
- kind, locator e identidad material acotada del runner;
- layout WoW64 conocido o `Unknown` explícito;
- policy de arquitectura;
- DXVK prefix-owned y dependencias futuras declaradas `IsolationCritical`.

`RuntimeFingerprint` describe la ejecución completa y añade:

- capabilities y sync efectivos;
- profile gráfico, provider DXVK y overlay;
- estado de binding del prefix;
- recetas base y requirement WebView2;
- revisiones de plan, invocación y environment.

La salud temporal no altera una identidad: vuelve el entorno ineligible o requiere repair. Si el
repair instala material diferente, entonces cambia el input correspondiente.

Quedan fuera de ambos fingerprints: server id, cliente/Gepard, GPU/driver, PID, timestamps,
credenciales, command lines, paths personales, logs, labels, assessment y benchmark. El server id
actúa como namespace externo; sujeto, host y proceso se registran como evidencia separada.

### Provenance

Los fingerprints consumen identidades tipadas:

- artefacto administrado con ID, digest de fuente, plataforma, arquitecturas y revisión de install;
- recurso bundled con digest/revisión;
- material externo observado por roles acotados;
- receta legacy marcada explícitamente como no fijada.

`PayloadVerification` es health mutable y no entra al digest. Un marker o una versión textual no se
promueven a prueba de integridad. Para un runner externo, el locator es un token SHA-256 del path
canónico; el path personal no aparece en fingerprints ni exports.

### Encoding

Ambos fingerprints usan SHA-256 con dominios y encoding canónico versionados:

```text
SHA-256("ro-launcher/runtime-fingerprint/v1\0" || canonical(runtime_input))
SHA-256("ro-launcher/prefix-fingerprint/v1\0"  || canonical(prefix_input))
```

El encoder fija tags, enteros big-endian, orden de campos/sets e IDs estables en lowercase. No usa
`Debug`, `Display`, JSON, localización ni orden de `HashMap`. Goldens fijan bytes y digest. Cambiar la
semántica o el esquema requiere un dominio/version nuevos.

### Path administrado v3

```text
server_token = first_16_hex(SHA-256(domain || length || server_id))
directory    = prefixes/<server_token>-v3-<first_24_hex(prefix_digest)>
```

El nombre corto no es autoridad. El manifest v3 conserva server id y digest completos, runner kind
y path operacional, y componentes instalados. El digest compromete el resto de la identidad material
calculada por el resolver. Para reutilizar el binding se exige:

1. path bajo la raíz administrada, sin escape ni symlink indebido;
2. manifest v3 legible;
3. server id y fingerprint completos exactos;
4. que la identidad deseada vuelva a producir el mismo digest;
5. health apto o repair seguro bajo guard antes de ejecutar.

Un mismatch en un path existente es colisión/estado ajeno: se rechaza y preserva. No se prueba un
suffix alternativo, no se adopta el directorio y no se reescribe su historia.

### Migración v2 → v3

La estrategia es `read old / write new / no physical migration automática`.

| Situación                                          | Acción                                                   |
| -------------------------------------------------- | -------------------------------------------------------- |
| v3 exacto y sano                                   | reutilizar                                               |
| v3 exacto con componente repairable                | reparar bajo guards; rebuild transaccional si hace falta |
| v3 incompatible o schema futuro                    | rechazar y preservar                                     |
| v2 exacto para server + runner + estructura legacy | reutilizar como `LegacyV2RunnerMatched`                  |
| v2 con repair seguro                               | reparar sin promoverlo a v3                              |
| v2 desconocido/corrupto/ajeno                      | rechazar y preservar                                     |
| no existe candidato                                | crear v3 transaccionalmente                              |
| custom prefix                                      | conservar path y ownership; no adoptar, migrar ni borrar |

El alias v2 sólo aplica a topologías que el código legacy podía producir. No afirma provenance
histórica exacta y por sí solo no respalda un assessment `Validated`.

### Sesión y concurrencia

El registry está keyed por locator canónico del prefix. Dentro de esa entrada, `SessionAnchorV2`
guarda el `RuntimeFingerprint` completo y usa su digest como `plan_id`.

Mientras existan leases de operación, cliente, memoria o request, otro plan no puede mutar el mismo
prefix ni sustituir su anchor. Cambiar un overlay game-dir también cambia el runtime fingerprint.
Detener un cliente no libera el anchor si otro sigue usándolo.

Un runner actualizado bajo el mismo path se vuelve otra identidad si cambian los roles materiales
observados. No se confía en nombre de directorio, versión reportada o label para ocultar esa mutación.

### Transacción y recuperación de prefixes administrados (F-05)

Preparar un prefix administrado limpio y reconstruirlo son transacciones recuperables tras la muerte
del proceso. No hay journal ni archivo de intención fuera del prefix. La evidencia es:

- `B` = `<padre>/.<nombre>.reset-backup`: un único respaldo con nombre fijo, en el mismo directorio
  y sistema de archivos. Un reset no empieza mientras exista.
- `M` = `<prefix>/.ro-launcher-provisioning`: marcador escrito antes de provisionar. Registra
  schema, scope, serverId, ruta y fingerprint. No contiene instrucciones; sólo prueba que el
  launcher creó esa raíz para esa ubicación y aún no la confirmó. Se escribe en un temporal fijo,
  con `fsync` del archivo, rename y `fsync` del prefix.
- El manifiesto v2/v3 existente es el único registro de commit. Se escribe con temporal, `fsync`,
  rename y `fsync` del directorio del prefix. Para un path v3 debe coincidir además el digest que
  codifica la ruta.

Reset: `rename(R, B)` + `fsync(padre)`; crear `R` y `M`; provisionar; quiesce; `syncfs`; manifiesto
(commit); borrar `M` + `fsync(R)`; borrar `B` + `fsync(padre)`. La preparación inicial es igual sin
`B`. Un fallo ordinario, tras quiesce, usa la misma función de recuperación que un arranque tras
SIGKILL; no existe un segundo algoritmo de rollback.

| `R` \ `B`                     | ausente                       | restaurable                | parcial o ajeno                       |
| ----------------------------- | ----------------------------- | -------------------------- | ------------------------------------- |
| ausente o vacío               | nada                          | restaurar `B`              | rechazar                              |
| `M` válido sin manifiesto     | retirar `R`                   | retirar `R`, restaurar `B` | rechazar                              |
| manifiesto confirmado (± `M`) | retirar `M`                   | retirar `M` y `B`          | retirar si parcial; rechazar si ajeno |
| legacy válido                 | nada                          | rechazar                   | rechazar                              |
| datos sin evidencia, symlink  | nada (rige el rechazo actual) | rechazar                   | rechazar                              |

"Restaurable" exige un directorio real, sin symlink, en el mismo sistema de archivos, sin marcador
de instalación de juego y con un manifiesto de esta ubicación, o marcador legacy más estructura
completa. "Rechazar" conserva todo y devuelve un error accionable con ambas rutas. Un manifiesto
confirmado siempre gana: nunca se restaura `B` sobre un prefix nuevo confirmado. Borrar `R` o `B`
deja su evidencia para el final, y restaurar es un único `renameat2(RENAME_NOREPLACE)`. Por eso una
recuperación interrumpida deja otro estado de la tabla y la siguiente converge. Repetirla no hace
nada.

Reglas que no cambian: un prefix custom nunca se recupera, mueve, restaura ni borra. Un directorio
administrado con datos sin evidencia no se adopta. Un prefix con instalación de juego no entra a
reset, y la recuperación nunca descarta un árbol que contenga ese marcador. Los respaldos
`.<nombre>.reset-backup-<pid>-<ms>` de versiones anteriores nunca se borran. Sólo se restaura uno
si la raíz no existe y es el único válido; en otro caso se informan en diagnostics.

Orden: setup/reset concilian bajo el guard exclusivo `prefix` antes de iniciar sesión y vuelven a
comprobar bajo el guard después de `RunnerOperation::begin`. El bootstrap del supervisor concilia
tras apagar leftovers y antes de soltar su guard. Si la raíz cambió, la operación termina pidiendo
reintento, para reevaluar runner, manifiesto y protección. La resolución de identidad sigue apuntando
a la ruta de una transacción sin commit en lugar de saltar a otro candidato. Ninguna acción
destructiva corre si hay procesos usando el prefix.

Calificación: probado contra SIGKILL en cada paso del reset, de la preparación inicial y de la propia
recuperación, también con `ro-sessiond` real y un runner inerte. El orden de `fsync` está
implementado, pero la pérdida de energía no está calificada.

## Alternativas rechazadas

- Un único fingerprint para prefix, host, cliente y sesión.
- Hashear todo el árbol de un runner o prefix.
- Incluir health, paths personales, GPU o resultados en identidad estable.
- Renombrar/mover automáticamente prefixes v2.
- Adoptar directorios no vacíos o sobrescribir un manifest incompatible.
- Crear un contador de suffix ante colisión; requiere una revisión explícita del path schema.

## Consecuencias

- Cambiar runner/layout/provider material obtiene otro prefix sin destruir el anterior.
- Cambios repairable o de ejecución pueden compartir prefix pero siguen siendo distinguibles en
  observaciones y benchmarks.
- Los prefixes custom conservan control del usuario.
- La migración permite rollback: datos v2 y schemas futuros se preservan intactos.
