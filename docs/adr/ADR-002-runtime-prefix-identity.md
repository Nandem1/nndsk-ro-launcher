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
