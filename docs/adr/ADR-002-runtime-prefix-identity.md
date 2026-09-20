# ADR-002: identidad de runtime y prefix

| Campo                 | Valor                                                                                                       |
| --------------------- | ----------------------------------------------------------------------------------------------------------- |
| Estado                | Aceptado                                                                                                    |
| Fecha                 | 2026-09-16                                                                                                  |
| Alcance               | Fase 2 y contratos que Fase 1 debe preservar                                                                |
| Decisión              | Dos fingerprints con dominios distintos, encoding binario canónico, path v3 derivado y alias v2 conservador |
| Autoridad relacionada | `AGENTS.md`, `ADR-001-runtime-graphics-domain.md`                                                           |

## 1. Problema

La identidad actual de un prefix administrado es servidor + path canónico del runner. El manifest
schema 2 prueba scope, server, kind, path y una lista de componentes, pero no prueba bytes del
runner, old/new-WoW64, arquitectura solicitada, recipe ni provenance de componentes.

Usar toda diferencia del runtime para el path sería incorrecto: dgVoodoo vive en game dir, sync y
variables pueden cambiar sin volver incompatible el estado del prefix, y logs/benchmarks no deben
proliferar environments. Usar sólo runner path también es insuficiente: un runner puede actualizarse
en el mismo path y un provider DXVK prefix-owned puede cambiar materialmente.

Se separan por ello:

- `RuntimeFingerprint`: identidad semántica del runtime resuelto que se intentará ejecutar;
- `PrefixFingerprint`: identidad de la receta y estado material incompatible que exige aislamiento.

Ninguno de los dos demuestra salud actual. Integrity/readiness pertenece a
`RuntimeEligibility` y a validators, no al digest de identidad.

## 2. Definiciones normativas

### 2.1 `RuntimeFingerprint`

Identifica una resolución de runtime suficientemente para:

- explicar y correlacionar decisiones;
- asociarla a un sujeto exacto en compatibility evidence;
- detectar staleness del runtime plan;
- asociarla a un host y workload en benchmark records;
- anclar una sesión y rechazar cambios de profile incompatibles mientras está en uso.

No es el ID del servidor, del cliente, del benchmark ni del launch request completo. Un record de
evidencia usa una clave compuesta:

```text
SubjectFingerprint(client + anti-cheat)
+ server identity
+ RuntimeFingerprint
+ host/driver identity cuando corresponda
```

Separar `SubjectFingerprint` evita introducir `ClientInspection` en Fase 1–2 y evita que el término
“runtime” mezcle el binario de juego con la materialización que lo ejecuta. Cambiar el cliente sin
cambiar requirements no crea otro runtime fingerprint; sí crea otro subject y por tanto otra clave
de compatibilidad/benchmark. La staleness de argumentos, executable path u otros campos de server
sigue usando la key de configuración existente además del runtime fingerprint.

El fingerprint detecta staleness del **plan**, no sustituye un health check. Un cache de readiness
debe estar ligado además a la metadata/invalidation key de los archivos validados o revalidarse en
cada operación superior; no puede seguir verde sólo porque el fingerprint planeado no cambió.

La representación canónica `RuntimeFingerprintInputV1` contiene, en este orden semántico:

1. revisión de la receta del runtime plan;
2. `RunnerKind` y `RunnerIdentity` material;
3. provenance de UMU para planes Proton;
4. capabilities que cambian semántica (`Wow64Layout`, conjunto de arquitecturas y evidencia
   explícitamente unknown);
5. `SyncPlan`;
6. `GraphicsProfile` y `GraphicsPlan` completos: owner, artifact/resource identity y recipe;
7. binding del prefix: v3 o alias v2, junto al `PrefixFingerprint` deseado completo y la
   arquitectura observada del prefix cuando existe;
8. revisión de receta base de setup y `webview2_required`;
9. identities/receipts planeados para dependencias repairable relevantes;
10. revisión de la policy de `InvocationTarget` y del builder de environment;
11. settings semánticos de ejecución que alteran comportamiento, por ejemplo digest de config
    dgVoodoo/DXVK efectivo y DLL claims normalizados.

El binding de prefix es un enum cerrado:

```rust
enum PrefixBindingIdentityV1 {
    ManagedV3 {
        fingerprint: PrefixFingerprint,
    },
    LegacyV2 {
        desired: PrefixFingerprint,
        status: LegacyV2Status,
    },
    Custom {
        observed_manifest: ObservedManifestIdentity,
    },
}
```

`LegacyV2Status` sólo admite `RunnerMatched`, `Unknown` o `Incompatible`; únicamente el primero
puede llegar a launch por el alias. `Custom` registra `Unknown` cuando no existe evidencia, sin
adoptar ni escribir. El locator físico se registra como token diagnóstico y en el session anchor,
pero no cambia el runtime fingerprint: server/prefix instance ya se asocian por separado en logs,
evidence y benchmark records.

Para componentes `LegacyUnpinned` se encodea esa provenance, no bytes inventados. La identidad
planeada no cambia porque un health check pase a `Corrupt`; el mismo runtime se vuelve inelegible y
debe repararse. Si una reparación instala otra identidad material, entonces sí cambia el input.

La proyección canónica de `CapabilityEvidence` encodea `known + value` o `unknown`. La fuente y el
motivo diagnóstico se conservan en el plan/summary, pero no entran al digest: aprender una mejor
explicación del mismo valor no crea otra identidad. Pasar de `Unknown` a un valor conocido sí cambia
el fingerprint.

### 2.2 Lo que no entra a `RuntimeFingerprint`

- server id o nombre;
- client/Gepard/GameGuard hash: pertenecen a `SubjectFingerprint`;
- GPU, vendor, driver, Vulkan version o kernel: pertenecen al host record;
- PID, `(pid,start_time)`, timestamps de launch o duración;
- credenciales, tokens, argumentos del patcher o command lines;
- paths personales crudos;
- paths de logs/cache y sus contenidos;
- flags efímeros de debug/tracing;
- readiness, mensajes de error, estado de download o progreso;
- labels de UI y texto localizado;
- resultado de benchmark o compatibility status.

### 2.3 `PrefixFingerprint`

Representa exclusivamente la receta de aislamiento material para un prefix administrado:

> Entra sólo aquello que puede dejar estado incompatible dentro del prefix y que no debe repararse
> in-place como parte de la misma identidad.

`PrefixFingerprintInputV1` contiene:

1. revisión de la receta ABI/material del prefix;
2. `RunnerKind`;
3. `RunnerPrefixIdentityV1`, la proyección material acotada del runner;
4. `RunnerLocatorIdentityV1`;
5. `Wow64Layout` conocido o `Unknown` explícito;
6. policy de arquitectura solicitada: `Win32`, `Win64` o `RunnerDefault`;
7. provider gráfico prefix-owned, su artifact/recipe/arquitecturas o `None` explícito;
8. set ordenado de dependencias futuras clasificadas `IsolationCritical`.

La arquitectura solicitada, no un hecho descubierto demasiado tarde, determina el path. El schema
v3 también registra la arquitectura observada desde `system.reg`; debe satisfacer la policy. En la
migración actual se conserva `RunnerDefault`, de modo que Fase 2 no fuerza `WINEARCH` ni cambia el
resultado de Wine. Si una fase futura pide Win32 o Win64 explícito, obtiene otro fingerprint.
Para `RunnerDefault`, la primera creación fija el valor observado en el manifest; reuso exige que
`system.reg` siga coincidiendo con ese valor. No se permite que un mismo v3 cambie de arquitectura
in-place. La arquitectura observada entra además al `RuntimeFingerprint`.

`RunnerLocatorIdentityV1` es `Managed { artifact_id, entrypoint_role }` para el runtime administrado
y `External { location_token }` para un entrypoint externo. Mantiene el contrato de no compartir
prefix entre locators externos distintos incluso cuando sus bytes observados coincidan, sin hacer
que un artifact managed aún no instalado dependa de canonicalización. Sólo esa identidad, no el path,
entra al encoding/export. El manifest conserva el path operacional verificable.

`RunnerPrefixIdentityV1` incluye artifact identity managed o, para externos, los roles
entrypoint/wineserver (Wine) y script/inner-Wine (Proton) realmente observados. Excluye reported
version como texto, UMU y `wine-tkg-config.txt`: esos datos cambian runtime/sync, pero por sí solos
no prueban otro ABI o estado material del prefix. `Wow64Layout` se añade por separado. No se usa el
`RunnerIdentity` completo como atajo porque produciría prefixes nuevos por cambios sólo de sync.

### 2.4 Lo que no entra a `PrefixFingerprint`

- server id: separa la ruta como namespace externo, no la receta;
- dgVoodoo o cualquier overlay exclusivamente game-dir;
- UMU, porque no escribe el prefix como componente material independiente del Proton elegido;
- sync mode;
- variables y DLL overrides que sólo afectan procesos;
- WebView2, Gecko, vcrun2019, d3dx9, corefonts, font fallback o audio actuales: son repairable;
- GPU/driver, client, anti-cheat, benchmark o target de invocación;
- health/integrity temporal, logs, cache y debug;
- custom prefix path.

Una dependencia sólo entra en una revisión futura si un ADR la clasifica explícitamente
`IsolationCritical`. Fase 2 tiene un set vacío; no deja al implementador decidir caso a caso.

Una recipe revision incluida en `PrefixFingerprint` se incrementa sólo cuando cambia el estado
material/ABI producido o su interpretación, no por refactor, logging, mensajes o cambios de
orquestación equivalentes. Las revisiones de recipes repairable pueden cambiar
`RuntimeFingerprint` sin separar el prefix.

## 3. Matriz obligatoria de contribución

`Sí` significa que el valor semántico/identity entra. La salud actual nunca entra.

| Caso                                             | RuntimeFingerprint    | PrefixFingerprint  | Decisión exacta                                                    |
| ------------------------------------------------ | --------------------- | ------------------ | ------------------------------------------------------------------ |
| runner distinto                                  | Sí                    | Sí                 | material identity y locator identity cambian                       |
| mismo path, entrypoint/companion ABI actualizado | Sí                    | Sí                 | cambia `RunnerPrefixIdentityV1`; no se confía sólo en path         |
| Wine old-WoW64 vs new-WoW64                      | Sí                    | Sí                 | variant conocido distinto; `Unknown` se encodea como tal           |
| policy de arquitectura Win32/Win64/RunnerDefault | Sí                    | Sí                 | forma parte de la receta de creación                               |
| arquitectura observada conforme                  | Sí                    | No adicional       | valida policy y distingue ejecución; la policy ya identifica path  |
| arquitectura observada no conforme               | No nueva identidad    | No nueva identidad | `Ineligible`; no adoptar/lanzar                                    |
| DXVK prefix-owned distinto                       | Sí                    | Sí                 | artifact/recipe/arquitectura cambian                               |
| DXVK runner-owned distinto                       | Sí                    | Sí                 | cambia `RunnerIdentity`; no se añade un component prefix duplicado |
| DXVK winetricks actual                           | Sí (`LegacyUnpinned`) | Sí (recipe legacy) | no puede respaldar `Validated` exacto sin nueva evidencia          |
| dgVoodoo version/wrappers/config                 | Sí                    | No                 | overlay game-dir con manifest propio                               |
| D7VK game-dir                                    | No                    | No                 | ADR-005 `no-go`; no existe plan productivo                         |
| D7VK prefix-owned                                | No                    | No                 | ADR-005 prohíbe reubicar el mismo payload para rodear Gepard       |
| WebView2                                         | Sí                    | No                 | requirement/recipe repairable; workaround Wine 7.16 se conserva    |
| Gecko                                            | Sí                    | No                 | repairable                                                         |
| vcrun2019/d3dx9                                  | Sí                    | No                 | recipe repairable actual                                           |
| fonts/corefonts/font fallback                    | Sí                    | No                 | recipe repairable actual                                           |
| audio efectivo                                   | Sí                    | No                 | configuración repairable actual                                    |
| dependencia futura `IsolationCritical`           | Sí                    | Sí                 | exige ADR/revisión de schema                                       |
| sync mode                                        | Sí                    | No                 | environment/capability, no estado aislable del prefix              |
| sólo `wine-tkg-config.txt`                       | Sí                    | No                 | cambia evidencia/sync, no la proyección material del runner        |
| config sólo en environment                       | Sí si semántica       | No                 | paths de logs/debug se excluyen                                    |
| UMU identity                                     | Sí                    | No                 | invocación Proton, no owner del prefix                             |
| GPU/vendor/driver                                | No                    | No                 | host evidence separado                                             |
| server id                                        | No                    | No                 | namespace de path/evidence separado                                |
| PID/timestamp/credentials                        | No                    | No                 | efímero o sensible                                                 |

## 4. Provenance e integrity

Fingerprints consumen identities, no un package manager. El contrato mínimo es:

```rust
struct ArtifactId(String);

struct Digest {
    algorithm: DigestAlgorithm, // sha256 o sha512 para artifacts actuales
    bytes: Vec<u8>,
}

struct ArtifactIdentity {
    schema_version: u32,
    artifact_id: ArtifactId,
    source_digest: Digest,
    platform: PlatformId,
    architectures: ArchitectureSet,
    install_recipe_revision: u32,
}

struct ArtifactReceipt {
    identity: ArtifactIdentity,
    payload_verification: PayloadVerification,
}

struct BundledResourceReceipt {
    schema_version: u32,
    resource_id: ResourceId,
    bundle_revision: BundleRevision,
    resource_digest: Digest,
}

struct ExternalObserved {
    observation_schema: u32,
    roles: Vec<ObservedMaterial>,
    completeness: ObservationCompleteness,
}

struct LegacyUnpinned {
    recipe_id: RecipeId,
    reason: LegacyReason,
}
```

Normas:

- `ArtifactId` es namespaced, ASCII y estable; no es un display/version label;
- source digest identifica el payload descargado esperado;
- `PayloadVerification` distingue `SourceAndPayloadVerified`, `ShapeVerified`, `Unverified` y
  `Corrupt`;
- los fingerprints consumen `ArtifactIdentity`, nunca el estado mutable `payload_verification`;
- un runtime marker schema 1 actual sólo permite `ShapeVerified` después de validar marker+layout;
- `ExternalObserved` enumera roles concretos; nunca implica hash de árbol completo;
- `LegacyUnpinned` es identidad explícitamente incompleta, no ausencia silenciosa;
- `Corrupt` no cambia el fingerprint planeado: produce `RuntimeEligibility::Ineligible` y repair;
- receipts completos de ADR-003 podrán refinar campos sin reinterpretar receipts viejos.

## 5. Encoding canónico

### 5.1 Algoritmo y envelope

Ambos fingerprints usan SHA-256 y se muestran como 64 caracteres hex lowercase.

```text
RuntimeFingerprint = SHA-256(
  "ro-launcher/runtime-fingerprint/v1\0" || canonical(RuntimeFingerprintInputV1)
)

PrefixFingerprint = SHA-256(
  "ro-launcher/prefix-fingerprint/v1\0" || canonical(PrefixFingerprintInputV1)
)
```

El valor persistido contiene siempre:

```text
schemaVersion = 1
algorithm     = "sha256"
digest        = 64 hex lowercase
```

### 5.2 Canonical Value Encoding v1

No se usa `Debug`, `Display`, JSON, labels ni el orden incidental de un `HashMap`. El encoder sólo
acepta estos valores y escribe bytes exactos:

| Tag byte | Valor        | Payload                                            |
| -------- | ------------ | -------------------------------------------------- |
| `0x00`   | null/absence | ninguno                                            |
| `0x01`   | false        | ninguno                                            |
| `0x02`   | true         | ninguno                                            |
| `0x03`   | `u64`        | 8 bytes big-endian                                 |
| `0x04`   | string       | `u32` big-endian length + UTF-8 bytes              |
| `0x05`   | bytes        | `u32` big-endian length + raw bytes                |
| `0x06`   | sequence     | `u32` count + encoded values                       |
| `0x07`   | record       | `u32` field count + encoded fields                 |
| `0x08`   | enum variant | encoded stable variant id + encoded record payload |

Un field de record es `encoded string(field_id)` seguido de su valor. Reglas:

1. `field_id` y enum variant IDs son ASCII lowercase `kebab-case`, fijados por golden tests;
2. records ordenan fields por bytes ASCII de `field_id`;
3. absence se encodea como `0x00`; omitir el field está prohibido;
4. sets se encodean como sequence ordenada lexicográficamente por el encoding completo de cada
   elemento; listas semánticamente ordenadas conservan orden;
5. no hay floats, platform `usize`, strings localizados ni normalización implícita;
6. IDs validados admiten sólo `[a-z0-9][a-z0-9._/-]*`;
7. todo integer usa `u64`; valores signed/negativos no forman parte de v1;
8. un cambio de significado, field o normalización requiere schema/domain v2.

Los golden tests fijan bytes y digest, no sólo igualdad entre dos ejecuciones.

### 5.3 Tokens de ubicación externa

En Linux, después de canonicalizar el path cuando existe:

```text
ExternalRunnerLocationToken = SHA-256(
  "ro-launcher/runner-location/v1\0"
  || u32_be(byte_length)
  || raw OsStr bytes
)
```

Un runner externo debe existir y canonicalizar antes de calcular el token; si no, eligibility es
`Ineligible` y no se crea/adopta un v3. No se alterna silenciosamente entre raw/canonical. Un runner
managed usa su locator estable por artifact/role y no pasa por este algoritmo.

El token no reemplaza el path operacional guardado en manifest. Evita incorporar un path personal
al fingerprint o a evidencia exportable. Prefix locators usan el mecanismo de redaction/token local
ya existente para diagnóstico y el canonical path interno del session registry; no forman parte de
ninguno de los dos fingerprints.

## 6. Path administrado v3

Para un server id UTF-8 válido:

```text
ServerPathToken = first_16_hex(SHA-256(
  "ro-launcher/server-path/v1\0" || u32_be(length) || server_id_bytes
))

directory = prefixes/<ServerPathToken>-v3-<first_24_hex(PrefixFingerprint.digest)>
```

El nombre contiene 64 bits de namespace de server y 96 bits del prefix digest sólo para mantener
una ruta manejable. La autoridad es el manifest v3, que guarda:

- server id completo;
- `PrefixFingerprint` completo con schema y algoritmo;
- runner kind, path operacional y `RunnerLocatorIdentityV1` completo;
- provenance/identity material del runner;
- architecture policy y arquitectura observada;
- receipts/component identities;
- revisión de receta.

Al encontrar el directorio:

1. debe pasar las protecciones actuales de raíz, symlink y managed path;
2. debe existir manifest v3 legible;
3. server id y digest completos deben coincidir;
4. runner locator/material, architecture y components deben revalidarse;
5. cualquier mismatch de full digest bajo el mismo nombre es colisión o estado ajeno: hard error;
6. nunca se prueba otro suffix, adopta el directorio, reescribe el manifest ni elimina el contenido.

Para un locator externo, revalidar incluye el path canónico exacto. Para el managed locator,
artifact ID + entrypoint role son identidad y el path guardado es diagnóstico: un cambio de raíz de
datos no crea otro fingerprint si el receipt/material completo coincide. Ese path sólo se actualiza
en una escritura explícita posterior del manifest, nunca por el mero hecho de abrirlo.

La resolución de una colisión requiere una revisión futura del path schema. No se improvisa un
contador porque haría que la ruta volviera a ser autoridad accidental.

Custom prefixes no usan esta derivación. Se conservan exactamente, nunca se renombran, adoptan o
eliminan automáticamente. Puede calcularse identidad diagnóstica, pero Fase 2 no escribe manifest
v3 ni cambia ownership de un custom prefix.

## 7. Migración schema 2 -> schema 3

Se mantiene la estrategia `read old / write new / no physical migration automática`.

### 7.1 Estados de binding

```rust
enum PrefixIdentityStatus {
    V3Verified,
    LegacyV2RunnerMatched,
    Unknown,
    Incompatible,
}
```

`LegacyV2RunnerMatched` significa exactamente: schema 2, scope/server y canonical runner kind/path
coinciden y las validaciones legacy actuales pasan. No significa que los bytes históricos del
runner, artifacts o arquitectura original estén demostrados. El nombre `LegacyVerified...` se
rechaza porque sugeriría más evidencia de la disponible.

### 7.2 Orden de resolución

Para un prefix administrado:

1. resolver `RuntimeProfile` y el `PrefixFingerprint` deseado sin mutar;
2. derivar path v3 y, si existe, aceptar sólo `V3Verified` exacto;
3. si v3 no existe y la solicitud representa la topología legacy actual, buscar el path v2 exacto
   derivado por el algoritmo vigente servidor+runner;
4. revalidar v2 con las protecciones actuales, schema 2 exacto, scope/server, runner kind/path,
   estructura, architecture observada, components esperados y shape/readiness observable;
5. si pasa, usarlo como `LegacyV2RunnerMatched`; no escribir schema 3, no renombrar y no afirmar
   provenance histórica;
6. si faltan componentes repairable, conservar la ruta v2 y usar el repair legacy bajo sus guards;
   si necesita rebuild/reemplazo, éste sigue siendo transaccional; su manifest sigue siendo
   schema 2;
7. si no hay candidato aceptable, provisionar un v3 nuevo mediante el flujo transaccional;
8. preservar cualquier v2 rechazado; nunca borrarlo ni convertirlo en staging.

La topología legacy actual significa las decisiones que el código anterior ya podía producir:
DXVK runner-owned en Proton, managed 2.6.2 en Wine 7.16, winetricks en otro Wine y dgVoodoo como
overlay separado. Un futuro provider prefix-owned distinto no puede usar el alias v2 por similitud.

### 7.3 Cuándo se reutiliza, repara, crea o rechaza

| Situación                                                                | Acción                                                                                                                             |
| ------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------- |
| v3 full match y health correcto                                          | reutilizar v3                                                                                                                      |
| v3 identity match con componente repairable ausente/corrupto             | repair bajo los guards vigentes; si requiere rebuild/reemplazo, éste es transaccional; no cambiar digest por health                |
| v3 full digest/runner/architecture incompatible                          | rechazar; preservar; crear otro path sólo si la identidad deseada deriva naturalmente otro digest                                  |
| v2 exacto por server+runner, manifest/estructura válidos                 | reutilizar como `LegacyV2RunnerMatched`                                                                                            |
| v2 válido con requirement repairable faltante                            | reparar en v2 con código legacy; cualquier rebuild sigue siendo transaccional; no promover a v3                                    |
| v2 con artifacts internos modificados pero no identificables exactamente | conservar, provenance `LegacyUnpinned`/`Unknown`; no afirmar `Validated`; bloquear sólo si validator legacy detecta unsafe/corrupt |
| v2 con schema futuro/corrupto, runner distinto o directorio no adoptable | `Incompatible`; preservar y no lanzar como ese plan                                                                                |
| no existe v2/v3                                                          | crear v3 transaccionalmente                                                                                                        |
| el usuario pide un profile materialmente distinto                        | crear/usar su v3; preservar v2                                                                                                     |
| rearmado explícito desde legacy a identidad exacta                       | crear v3; no mover user data automáticamente                                                                                       |
| custom prefix                                                            | mantener path/ownership actual; sin migración automática                                                                           |

En el primer upgrade no existe evidencia para saber si un runner fue actualizado anteriormente
bajo el mismo path. Fase 2 no falsifica esa historia: el v2 puede seguir funcionando en modo legacy,
pero queda `LegacyV2RunnerMatched` y compatibility exacta es `Unknown` salvo nueva evidencia. Después
de crear v3, cualquier cambio de los digests acotados produce otra identidad.

SakuraRO Wine 7.16 + DXVK 2.6.2 y Proton-CachyOS actuales conservan así su v2 cuando pasa las mismas
validaciones que antes. Un usuario sólo obtiene v3 en instalación nueva, rearmado explícito o una
selección materialmente distinta. No se pierde user data por upgrade.

## 8. Session identity y concurrencia

El registry actual está keyed por prefix y ancla kind+path. Fase 2 lo refuerza, no lo reemplaza:

```text
SessionAnchorV2 = canonical prefix locator
                + RunnerLocatorIdentityV1
                + full RuntimeFingerprint
```

Mientras exista operation/client/request lease:

- otro plan con fingerprint distinto para el mismo prefix no puede mutar ni iniciar una sesión;
- un cambio de graphics profile que sólo vive en game dir también cambia runtime fingerprint y se
  rechaza mientras el entorno compartido está activo;
- detener un cliente no libera el anchor si quedan otros clientes/requests;
- no se envía el fingerprint a `ro-sessiond` ni se cambia su protocolo;
- process identity sigue siendo `(pid,start_time)`.

El anchor no autoriza filesystem mutation. Las operaciones exclusivas y guards actuales siguen
siendo necesarios para prefix y game dir.

## 9. Runner actualizado en el mismo path

La identity observable acotada incluye como mínimo:

- Wine: digest de entrypoint y wineserver; digest/absence explícita de `wine-tkg-config.txt`;
- Proton: digest del script, inner Wine cuando esté localizable y contenido del version file;
- managed artifacts: proyección estable `ArtifactIdentity` del receipt; el nivel real de payload
  verification afecta eligibility, no identity;
- externos: `ExternalObserved` y su completeness.

Un cambio en entrypoint/wineserver o script/inner-Wine cambia ambos fingerprints. Un cambio sólo en
version text o `wine-tkg-config.txt` cambia `RuntimeFingerprint`, no `PrefixFingerprint`. Para un
artifact managed, cambiar su `ArtifactIdentity` cambia ambos porque el payload completo esperado es
otro. Una modificación sólo en un archivo no observado de un runner externo puede no detectarse;
por eso `ExternalObserved` nunca se describe como identidad completa ni puede por sí solo satisfacer
exact `Validated`. Hashear árboles completos en cada launch se rechaza por costo, estabilidad y poco
valor probatorio.

## 10. Caching y performance

| Trabajo                                          | Clase                  | Política                               |
| ------------------------------------------------ | ---------------------- | -------------------------------------- |
| canonical encoding + SHA-256 de structs pequeños | launch/setup cold path | una vez por plan                       |
| hash de roles de runner acotados                 | launch/setup path      | una vez por resolución; cache opcional |
| lectura de manifest y `system.reg`               | launch/setup path      | revalidar antes de launch/mutation     |
| payload verification completa                    | setup/repair path      | no ejecutar por frame ni tick          |
| fingerprints en memory/input/session polling     | hot path               | prohibido                              |

Un cache de archivo, si se introduce, usa:

```text
canonical locator + device + inode + size + mtime_ns + ctime_ns
```

como invalidation key. Un cache de inspección de game dir añade identity del manifest dgVoodoo y
metadata de los archivos exactos usados. Error, ausencia o cambio invalida. No se cachea por label,
versión reportada ni path solamente.

Los planes se pasan por referencia/`Arc` sólo donde una lease lo requiera; no se clonan en polling.
Fase 1 no introduce cache global ni lock. Fase 2 puede usar cache local acotado, fuera de locks de
session y nunca manteniendo un lock síncrono a través de `.await`.

## 11. Contradicciones resueltas

| Problema                                                         | Evidencia                                                                            | Decisión                                                              | Impacto en roadmap                                                 |
| ---------------------------------------------------------------- | ------------------------------------------------------------------------------------ | --------------------------------------------------------------------- | ------------------------------------------------------------------ |
| planning mezclaba client con runtime fingerprint                 | client/Gepard tienen lifecycle y consumidores distintos                              | subject fingerprint separado; runtime fingerprint sólo stack resuelto | `ClientInspection` se difiere; Fase 2 no necesita hashear clientes |
| label Sakura afirma old-WoW64 pero predicate sólo comprueba 7.16 | `is_wine_7_16()` ejecuta `--version`; el layout sólo aparece como label en discovery | old/new/unknown capability explícita; nunca inferir por versión       | exact validation queda Fase 5                                      |
| marker managed parece receipt de payload                         | `artifact_ready()` valida marker y shape, no hashes del árbol instalado              | provenance e integrity separados                                      | ADR-003 debe mejorar receipts sin que Fase 2 invente certeza       |
| prefix fingerprint podía absorber todo dependency                | setup actual instala una receta repairable fija                                      | sólo isolation-critical entra; set vacío en v1                        | se elimina `DependencyPlan` de Fase 1                              |
| dgVoodoo podría tratarse como component del prefix               | manifest/backups y archivos están en game dir                                        | sólo runtime fingerprint; manifest separado                           | ningún write de prefix schema contiene dgVoodoo                    |
| v2 carece de material identity histórica                         | schema 2 sólo guarda kind/path/components strings                                    | estado `LegacyV2RunnerMatched`, no “verified” exacto                  | upgrade conserva v2 sin reinterpretarlo                            |
| architecture observada se conoce después de crear                | actual setup usa default del runner                                                  | fingerprint usa policy; manifest valida resultado                     | Fase 2 no cambia `WINEARCH`                                        |

## 12. Alternativas rechazadas

- un solo fingerprint para path, compatibility y benchmark;
- FNV/path hashes actuales como autoridad;
- JSON “estable” dependiendo del orden actual de serde/maps;
- `Debug`/display strings como input;
- full raw paths dentro de digests exportables;
- digest truncado como prueba suficiente;
- migrar o renombrar físicamente v2 al abrirlo;
- reescribir v2 como v3 con evidencia inferida;
- crear un prefix nuevo por dgVoodoo, sync, logs o flags de debug;
- omitir runner location porque dos archivos tengan el mismo digest;
- hash recursivo del runner en cada launch.

## 13. Consecuencias y criterio de cumplimiento

Fase 2 queda cerrada cuando:

- golden tests fijan el encoding y ambos domain separators;
- la matriz de §3 se refleja en tests positivos y negativos;
- path v3 sólo acepta full manifest match;
- v2 válido sigue seleccionable sin rewrite físico ni claims nuevos;
- directorios ambiguos, collisions, schema futuro y custom prefixes no se adoptan/eliminan;
- un runner actualizado en un role observado cambia la identidad;
- artifacts corruptos producen ineligibility/repair, no otro digest arbitrario;
- session registry rechaza un profile diferente mientras existan leases;
- ninguna operación de fingerprinting entra a loops de memoria, input, presence o `ro-sessiond`.
