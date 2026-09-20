# ADR-001: dominio de runtime y gráficos

| Campo                 | Valor                                                                                                                                           |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Estado                | Aceptado                                                                                                                                        |
| Fecha                 | 2026-09-16                                                                                                                                      |
| Alcance               | Fases 0–4; environment estructurado activo bajo `RO_LAUNCHER_RUNTIME_GRAPHICS` (default ON)                                                                 |
| Decisión              | Modelo cerrado para las dos topologías gráficas actuales, resolución shadow sin autoridad operacional y composición estructurada de environment |
| Autoridad relacionada | `AGENTS.md`, `docs/RO_RUNTIME_ARCHITECTURE_PLAN.md`                                                                                             |
| ADR relacionado       | `ADR-002-runtime-prefix-identity.md`                                                                                                            |

## 1. Contexto confirmado

La decisión de runtime actual está repartida entre varios owners:

- `utils/runner.rs` resuelve `ResolvedRunner`, genera comandos y decide sync;
- `utils/prefix.rs` deriva la ubicación y valida el manifest schema 2;
- `tools/prefix/setup.rs` decide `DxvkProvision::{Runner, Managed, Winetricks}`;
- `tools/launcher/session.rs` vuelve a decidir dgVoodoo, DXVK y el environment de game o
  launch-patcher;
- `tools/server_tools/session.rs` aplica una política distinta y deliberada a OpenSetup, patcher
  de mantenimiento y dgVoodoo CPL;
- `utils/wine.rs` convierte dos booleanos en `WINEDLLOVERRIDES` y variables DXVK;
- `tools/deps/check.rs` reconstruye las mismas decisiones para diagnóstico.

Esto justifica un valor resuelto compartido. No justifica reescribir `ResolvedRunner`, crear un
package manager, modelar un grafo libre de capas ni mover este dominio a `ro-tools-core`.

Hechos que condicionan esta decisión:

1. Proton usa su DXVK y no debe recibir una instalación managed adicional.
2. Wine 7.16 usa hoy DXVK 2.6.2 instalado dentro del prefix.
3. Otros Wine usan el verb `dxvk` de winetricks; esa procedencia no está fijada.
4. dgVoodoo vive junto al juego, tiene manifest y rollback propios, y emite D3D11 hacia DXVK.
5. Game y launch-patcher reciben hoy la política gráfica completa.
6. OpenSetup y dgVoodoo CPL reciben dgVoodoo cuando el overlay está verificado.
7. El patcher abierto manualmente desde Tools no recibe dgVoodoo, aunque sí recibe el DXVK managed
   que corresponda al runner.
8. `ProcessEnv::set_env` permite last-write-wins. Esa propiedad mecánica no es una política válida
   para ownership de DLLs.

## 2. Decisión resumida

Se adopta:

- `RuntimeProfile` como intención efímera de Fase 1;
- `RuntimePlan` como resolución concreta, inmutable y acotada a una operación superior;
- `GraphicsProfile` como enum cerrado de topologías completas;
- `GraphicsPlan` como topología resuelta con owner y provenance concretos;
- `RunnerPlan` como wrapper de evidencia alrededor de `ResolvedRunner`, sin reemplazarlo;
- cinco `InvocationTarget` semánticos;
- `DllOverrideSet` y `EnvironmentDelta` con conflictos explícitos;
- cuatro clases de ownership físico;
- shadow mode estricto: legacy ejecuta, el nuevo resolver sólo observa.

Fase 1 no introduce `ClientInspection`, `DependencyPlan`, `PrefixPlan`, persistencia de profiles,
fingerprints ni un catálogo de compatibilidad productivo. Esas abstracciones no son necesarias para
demostrar paridad del seam actual.

## 3. Profile y plan

### 3.1 `RuntimeProfile`

Representa la intención solicitada, no una promesa de reproducibilidad:

```rust
struct RuntimeProfile {
    runner: RunnerRequest,
    selection_source: RunnerSelectionSource,
    graphics: GraphicsProfile,
}

enum RunnerRequest {
    Managed { artifact_id: ArtifactId },
    External { kind_hint: RunnerKind, entrypoint: PathBuf },
}

enum RunnerSelectionSource {
    ServerOverride,
    GlobalSetting,
    ProductDefault,
}
```

Semántica:

- se deriva de la precedencia actual `server.runner > default recibido > Proton managed`;
- `External` incluye Wine portable/system y Proton externo;
- el path externo es un locator operacional local, no provenance ni evidencia exportable;
- `selection_source` permite explicar la decisión y comprobar que una recomendación no sustituyó
  la elección del usuario;
- Fase 1 no serializa este tipo ni asigna `RuntimeProfileId` persistente;
- no contiene dependencies, compatibility, readiness, GPU, paths de prefix ni opciones de debug.

Un profile no es “validated”. La compatibilidad es una relación entre sujeto, runtime resuelto y
evidencia, no una propiedad intrínseca del profile.

### 3.2 `RuntimePlan`

La forma productiva mínima de Fase 1 es:

```rust
struct RuntimePlan {
    runner: RunnerPlan,
    graphics: GraphicsPlan,
    prefix: PrefixLocation,
    webview2_required: bool,
}
```

Representa la resolución concreta para una operación superior en la máquina actual. No contiene:

- `ClientInspection` completo;
- `CompatibilityAssessment` o recommendation;
- procesos, PIDs, locks, leases o children;
- readiness mutable ni resultados de provisioning;
- credenciales, argumentos de lanzamiento o variables de diagnóstico;
- un `DependencyPlan` para una receta que todavía es fija;
- un `PrefixPlan` que sólo renombraría `PrefixLocation` en Fase 1.

`webview2_required` permanece como campo porque es la única variación de la receta base actual. La
receta fija —Gecko, vcrun2019, d3dx9, corefonts, font fallback y audio— sigue siendo autoridad legacy
y recibe un `PREFIX_RECIPE_REVISION` sólo en Fase 2.

El modo especial de instalación de WebView2 no es otro flag del profile: se deriva de
`RunnerPlan` y conserva el predicate actual, Wine 7.16 con ESYNC/FSYNC declarado. La transición
temporal Windows 7 -> install -> restauración Windows 10 incluso en error sigue dentro del
provisioner legacy; no se generaliza a otros runners.

`RuntimePlan` se crea una vez por comando superior (`launch`, `setup`, dependency check o tool), se
pasa por referencia y se descarta. No se guarda en `GameState`, no cruza los loops de input/memoria
y no se reutiliza después de una mutación de manifests, runner o game dir. Fase 2 añade fingerprints;
una fase posterior podrá anclar el plan durante leases sin convertirlo en estado mutable.

### 3.3 Por qué no es un god object

El plan sólo agrega cuatro decisiones que ya tienen varios consumidores. Datos de compatibilidad,
inspección, evidencia de ejecución y benchmark permanecen fuera. Para diagnóstico, una proyección
redacted puede construirse desde el plan, pero no forma parte del dominio ni se añade a IPC en
Fase 1.

Un resultado interno de shadow puede agrupar `RuntimeProfile`, `RuntimePlan` y
`ShadowComparison`; ese wrapper no es API pública ni se persiste.

## 4. Runner

### 4.1 Relación con `ResolvedRunner`

`ResolvedRunner` sigue siendo autoridad para:

- resolver Wine versus Proton;
- construir `RunnerInvocation`;
- localizar Wine, wineserver, Proton y UMU;
- aplicar environment propio del runner;
- crear, reparar y apagar el prefix.

No se duplica esa lógica. `RunnerPlan` añade identidad observada y capabilities:

```rust
struct RunnerPlan {
    resolved: ResolvedRunner,
    identity: RunnerIdentity,
    capabilities: RunnerCapabilities,
    sync: SyncPlan,
}
```

`RunnerIdentity` no es el display label ni sólo el resultado de `--version`:

```rust
struct RunnerIdentity {
    kind: RunnerKind,
    provenance: ComponentProvenance,
    observed_material: ObservedRunnerMaterial,
}
```

`ObservedRunnerMaterial` contiene sólo un conjunto acotado de roles y digests:

- Wine: entrypoint, wineserver y `wine-tkg-config.txt` cuando existe;
- Proton: script `proton`, inner Wine usado por esa distribución cuando se puede localizar y el
  archivo `version`;
- UMU es provenance de invocación separado; no se finge parte de los bytes de Proton.

Fase 1 no calcula nuevos hashes de archivo: reutiliza digest/provenance ya presente en markers o
representa el role como `Unknown`. Los hashes acotados de entrypoints/companions comienzan en Fase 2
según ADR-002. No se hashea recursivamente el árbol del runner.

### 4.2 Capabilities y evidencia incompleta

```rust
struct RunnerCapabilities {
    reported_version: CapabilityEvidence<String>,
    wow64_layout: CapabilityEvidence<Wow64Layout>,
    supported_prefix_architectures: CapabilityEvidence<ArchitectureSet>,
    sync_support: SyncSupport,
}

enum CapabilityEvidence<T> {
    Known { value: T, source: CapabilitySource },
    Unknown { reason: UnknownCapabilityReason },
}

enum Wow64Layout {
    OldWow64,
    NewWow64,
    Wine32Only,
}

enum SyncSupport {
    Wine {
        esync_declared: bool,
        fsync_patch_pair_declared: bool,
    },
    RunnerManaged,
}

enum SyncPlan {
    WineServer,
    Esync,
    Fsync,
    RunnerManaged,
}
```

Reglas cerradas:

- `wine --version == 7.16` sólo prueba `reported_version`;
- `StructuralProbeV1` sólo puede producir `Known(OldWow64)` cuando, desde el mismo root del
  entrypoint, existen `lib/wine/i386-unix` y `lib/wine/x86_64-unix` y ambos contienen al menos un
  archivo regular; el probe registra sus locators relativos y se cubre con fixture;
- ausencia de `i386-unix` no prueba `NewWow64`: puede ser un artifact incompleto. Fase 1 sólo
  produce `Known(NewWow64)` o `Known(Wine32Only)` desde un receipt/descriptor cuyo validator declare
  y compruebe positivamente ese layout; si no existe, usa `Unknown`;
- el label de `discover.rs` no es evidencia de capability; puede mostrar el resultado del mismo
  probe, pero no originarlo;
- si el probe no puede distinguir old/new-WoW64, la capability es `Unknown`, nunca `OldWow64` por
  nombre, ubicación o versión;
- Wine sólo obtiene `Fsync` cuando el config declara ambos patches ya exigidos por
  `wine_sync_mode`; staging puede producir `Esync`; en otro caso usa `WineServer`;
- `SyncSupport::Wine` describe únicamente mecanismos que la evidencia TkG actual autoriza al
  launcher a activar; no afirma todos los mecanismos que el binario podría soportar;
- Wine 7.16 no obtiene NTSync;
- Proton usa `RunnerManaged`: el launcher no afirma qué mecanismo interno concreto eligió Proton;
- `SyncPlan` debe ser derivable de capabilities; no acepta una preferencia arbitraria del usuario
  en Fase 1.

Lo demostrable por familia es:

| Runner                 | Identidad/provenance posible                                                                   | Capabilities demostrables                                   | Lo que no se afirma                                        |
| ---------------------- | ---------------------------------------------------------------------------------------------- | ----------------------------------------------------------- | ---------------------------------------------------------- |
| Proton-CachyOS managed | receipt/marker conocido más digests acotados y verificación de payload declarada               | kind, versión del artifact y layout que pruebe el validator | que un marker por sí solo garantice bytes locales intactos |
| Wine 7.16 portable     | `ExternalObserved` con digests acotados                                                        | versión, sync config y layout sólo si el probe lo demuestra | que toda build 7.16 sea old-WoW64 o compatible con Sakura  |
| Wine system/external   | `ExternalObserved`                                                                             | hechos observados individualmente                           | identidad exacta de una distribución completa              |
| Proton externo         | `ExternalObserved` del script, inner Wine/version cuando estén localizables, más UMU observado | kind y hechos realmente inspeccionados                      | equivalencia con Proton-CachyOS managed                    |

Una capability `Unknown` puede mantener la conducta legacy durante shadow. No puede respaldar
`Validated` ni ser convertida silenciosamente en `Known`.

## 5. Dominio gráfico inicial

Las únicas variantes productivas de Fase 1 son:

```rust
enum GraphicsProfile {
    Dxvk,
    DgVoodooDxvk,
}
```

No se agregan `D7vk`, `WineD3d`, `Custom`, `Layers(Vec<_>)` ni parameters libres. ADR-005 cerró el
spike D7VK como `no-go`; un variant sólo podría aparecer tras un ADR sustituto con autorización y
evidencia nuevas.

El plan resuelto es:

```rust
enum GraphicsPlan {
    Dxvk {
        dxvk: DxvkProvider,
    },
    DgVoodooDxvk {
        overlay: DgVoodooOverlayPlan,
        dxvk: DxvkProvider,
    },
}

enum DxvkProvider {
    RunnerOwned { component: RunnerComponentId },
    ManagedPrefix { artifact_id: ArtifactId },
    WinetricksPrefix { provenance: ComponentProvenance },
}
```

Constructores públicos, no campos públicos, garantizan:

- Proton resuelve `RunnerOwned`; `RunnerComponentId` referencia la identity/provenance ya guardada
  una sola vez en `RunnerPlan`, no copia su receipt dentro de graphics;
- Wine 7.16 actual resuelve `ManagedPrefix` para DXVK 2.6.2;
- otro Wine resuelve `WinetricksPrefix`;
- `DgVoodooDxvk` exige un overlay verificado y conserva un provider DXVK explícito;
- no puede existir dgVoodoo sin la segunda etapa D3D11 -> DXVK;
- no puede construirse managed DXVK encima de DXVK runner-owned.

### 5.1 Enum cerrado versus lista/grafo

Se intentó refutar el enum con un cliente que use DirectDraw/D3D7 y D3D9 directo. El caso no exige
un DAG libre: una topología completa puede contener un owner para `ddraw.dll`/`d3dimm.dll` y un
provider DXVK para D3D9/D3D11. Lo importante es que el variant nombre la combinación admitida.

Un grafo permitiría estados sin significado productivo —dgVoodoo y D7VK reclamando `ddraw.dll`,
dos providers de `d3d9.dll` o una cadena sin salida Vulkan— y trasladaría validación a runtime. El
enum cerrado se conserva. Si una futura topología real no cabe, se añade un variant explícito con
tests; no se habilita composición arbitraria.

## 6. Targets de invocación

La taxonomía inicial queda cerrada:

```rust
enum InvocationTarget {
    Game,
    LaunchPatcher,
    MaintenancePatcher,
    OpenSetup,
    GraphicsControlPanel,
}
```

Los cinco son necesarios:

- `Game`: ejecución directa del cliente;
- `LaunchPatcher`: patcher que forma parte de la acción Jugar y entrega el cliente;
- `MaintenancePatcher`: patcher abierto manualmente desde Tools;
- `OpenSetup`: selector de renderer que debe ver el adapter relevante;
- `GraphicsControlPanel`: dgVoodoo CPL actual y futuros paneles explícitos.

No se añade un target genérico `Tool`: ocultaría la divergencia real entre maintenance patcher,
OpenSetup y CPL.

Coherencia significa que todos los targets se derivan del mismo `GraphicsPlan` y de una tabla de
política exhaustiva. No significa environment byte por byte idéntico. La paridad inicial es:

| Target                 | DXVK prefix-owned | dgVoodoo overlay                            | Notas                                   |
| ---------------------- | ----------------- | ------------------------------------------- | --------------------------------------- |
| `Game`                 | sí                | sí si el plan lo contiene y está verificado | política completa                       |
| `LaunchPatcher`        | sí                | sí si el plan lo contiene y está verificado | misma política de entrega que Game      |
| `MaintenancePatcher`   | sí                | no                                          | conserva comportamiento actual de Tools |
| `OpenSetup`            | sí                | sí si el plan lo contiene y está verificado | enumera el adapter que usará Game       |
| `GraphicsControlPanel` | sí                | sí si el plan lo contiene y está verificado | hoy dgVoodoo CPL                        |

Para DXVK runner-owned no se fuerzan DLL overrides ni rutas prefix-owned desde graphics; el runner
conserva su environment. Todos los targets siguen recibiendo `WINE_LARGE_ADDRESS_AWARE=1` por la
política actual. Sanitización AppImage y variables de runner se componen antes de graphics, pero
siguen perteneciendo a sus owners.

## 7. DLL overrides y environment

### 7.1 Modelo mínimo

```rust
struct GraphicsEnvironment {
    dll_overrides: DllOverrideSet,
    variables: EnvironmentDelta,
}

struct DllClaim {
    name: DllName,
    load_order: DllLoadOrder,
    owner: ComponentOwner,
}

struct ComponentOwner {
    domain: OwnershipDomain,
    component_id: ComponentId,
}

enum DllLoadOrder {
    NativeThenBuiltin,
}

enum EnvironmentChange {
    Set(OsString),
    Unset,
}

struct EnvironmentContribution {
    change: EnvironmentChange,
    contributor: EnvironmentContributorId,
}
```

`DllName` se valida como basename ASCII, se normaliza a lowercase y siempre conserva el sufijo
`.dll`. No acepta path, wildcard ni fragmento de sintaxis Wine. Fase 1 sólo necesita
`NativeThenBuiltin`, equivalente a `n,b`; otras órdenes se añaden sólo con un consumidor real.

`ComponentOwner` identifica tanto el dominio físico como el componente lógico. Por ejemplo,
`prefix/dxvk-2.6.2` y un hipotético `prefix/backend-futuro-x` serían owners diferentes aunque ambos
vivieran en el prefix. Esto ilustra el modelo, no autoriza un payload concreto.
`EnvironmentContributorId` identifica de forma estable `runner`, `runtime-base` o el component ID
gráfico; no se deriva de un display label.

`DllOverrideSet` usa orden determinista por `DllName`. Su operación de merge es:

1. nombre ausente: insertar;
2. mismo nombre, mismo owner y mismo load order: idempotente;
3. mismo nombre con owner distinto o load order distinto: `OverrideConflict`;
4. nunca resolver por orden de llamada.

Se renderiza `WINEDLLOVERRIDES` exactamente una vez, ordenado por nombre normalizado. Está prohibido
introducir esa key mediante `EnvironmentDelta`.

`EnvironmentDelta` almacena `key -> EnvironmentContribution` en orden determinista. Dos
contribuciones idénticas del mismo contributor son idempotentes; cualquier diferencia de
contributor, valor o set/unset para la misma key es `EnvironmentConflict`. No se diseña un DSL ni
reglas por variable.

Fase 1 sólo construye y compara estas estructuras; no las aplica. Al activar el nuevo builder en
Fase 4, un `WINEDLLOVERRIDES` heredado y no atribuido se elimina mediante la sanitización acordada o
produce un preflight explícito. Nunca se concatena opacamente con el valor estructurado.

### 7.2 Alcance de comparación

La comparación shadow usa sólo el delta producido por runner+graphics, no el environment completo
del proceso host. Compara keys y valores efectivos normalizados, incluyendo ausencia explícita de
overrides. Paths de log/cache se comparan mediante rol (`dxvk-config`, `dxvk-log`, `dxvk-cache`) y
token redacted, no se imprimen como paths personales.

## 8. Ownership de componentes y archivos

```rust
enum OwnershipDomain {
    Runner,
    Prefix,
    GameDirOverlay,
    HostExternal,
}
```

`OwnershipDomain` describe dónde vive y quién puede mutar/restaurar el archivo efectivo;
`ComponentOwner` añade el ID lógico necesario para detectar dos componentes distintos en el mismo
dominio. El artifact store es storage y provenance, no un quinto owner de runtime.

| Componente              | Owner efectivo                                                                | Provenance                                      | Manifest/receipt de autoridad                       |
| ----------------------- | ----------------------------------------------------------------------------- | ----------------------------------------------- | --------------------------------------------------- |
| DXVK incluido en Proton | `Runner`                                                                      | receipt managed o `ExternalObserved`            | marker/receipt del runner; nunca prefix component   |
| DXVK 2.6.2 managed      | `Prefix` para las DLL/config instaladas; artifact store sólo suministra bytes | `ArtifactReceipt`                               | prefix manifest/receipt futuro más receipt global   |
| DXVK por winetricks     | `Prefix`                                                                      | `LegacyUnpinned` hasta poder fijar bytes/recipe | prefix manifest legacy, sin promover a exacto       |
| dgVoodoo                | `GameDirOverlay`                                                              | `BundledResourceReceipt`                        | `.ro-launcher-dgvoodoo.json` y backups del game dir |
| Wine/Proton/UMU externo | `HostExternal`                                                                | `ExternalObserved`                              | no adopción ni mutación por el launcher             |

dgVoodoo no entra al manifest del prefix. Su config editable y sus wrappers se validan mediante el
boundary actual. Un config modificado por el usuario puede seguir siendo un overlay válido si el
validator lo acepta; wrappers modificados dejan de estar verificados y el plan no debe forzarlos.

Un archivo efectivo sólo puede tener un `ComponentOwner` exacto. Una colisión se detecta antes de
descargar, instalar, copiar, lanzar o borrar. `HostExternal` nunca autoriza al launcher a mutar el
archivo observado.

## 9. Provenance mínima que este ADR consume

ADR-003 definirá catálogo e instalación. Para que los tipos de este ADR tengan semántica estable se
fija este contrato mínimo:

```rust
struct ArtifactId(String); // namespaced, estable, no display label

enum ComponentProvenance {
    ArtifactReceipt(ArtifactReceipt),
    BundledResource(BundledResourceReceipt),
    ExternalObserved(ExternalObserved),
    LegacyUnpinned(LegacyUnpinned),
}
```

- `ArtifactReceipt`: `ArtifactIdentity` estable —schema, `ArtifactId`, digest de fuente,
  plataforma/arquitectura y revisión de receta— más el nivel mutable de verificación del payload
  local;
- `BundledResourceReceipt`: ID estable del recurso, revisión/build del bundle y digest del manifest
  o payload relevante;
- `ExternalObserved`: roles, digests y hechos acotados realmente observados; declara explícitamente
  que no identifica todo el árbol;
- `LegacyUnpinned`: nombre de recipe/componente y razón por la que los bytes exactos no se conocen.

Un receipt válido no implica que el payload local siga íntegro. `PayloadVerification` es un dato
separado (`SourceAndPayloadVerified`, `ShapeVerified`, `Unverified`, `Corrupt`) y no forma parte de
la proyección de identity usada por fingerprints. El código actual de managed runtime sólo
demuestra marker + shape después de instalar; no debe describirse como verificación byte a byte del
payload extraído.

## 10. Compatibilidad, recomendación y elegibilidad

No se usa un único enum para tres trabajos:

```rust
enum CompatibilityAssessment {
    Validated { evidence_id: EvidenceId },
    Experimental { evidence_id: EvidenceId },
    Incompatible { evidence_id: EvidenceId, reason: FailureClass },
    Unknown,
}

enum RuntimeEligibility {
    Eligible,
    Ineligible { reasons: Vec<EligibilityReason> },
    Indeterminate { reasons: Vec<EligibilityReason> },
}

struct RuntimeRecommendation {
    profile: RecommendedProfileRef,
    evidence_id: EvidenceId,
    reason: RecommendationReason,
}
```

- compatibility responde “¿qué evidencia exacta existe para este sujeto+runtime?”;
- eligibility responde “¿puede resolverse/materializarse/lanzarse de forma segura aquí?”;
- recommendation responde “¿qué intención se sugiere y por qué?”.

Reglas cerradas:

1. `Unknown` es ausencia o insuficiencia de evidencia, no incompatibilidad.
2. `Validated` exige sujeto y runtime exactos; no existe wildcard positivo por familia.
3. `Experimental` también referencia evidencia, pero no equivale a validado.
4. `Incompatible` exige evidencia negativa exacta y no se generaliza.
5. Una selección explícita no se sustituye silenciosamente por recommendation.
6. Una recommendation no convierte un runtime inelegible en elegible.
7. Fase 1 no hace estos tipos autoridad ni implementa ADR-004. El adapter legacy de Gepard sigue
   mostrando sus recomendaciones actuales y shadow comprueba que no cambien la selección.

El predicate actual `is_wine_7_16()` o “cualquier Proton” es más amplio que el label de stack que se
muestra. Hasta Fase 5 no puede elevarse a `Validated` en el modelo nuevo.

## 11. Shadow mode de Fase 1

### 11.1 Autoridad

```text
legacy resolver = única autoridad operacional
new resolver    = observador puro
```

El resolver nuevo puede resolver, comparar, emitir una divergencia redacted y hacer fallar tests.
No puede:

- cambiar runner o prefix;
- instalar, reparar o eliminar artifacts/components;
- cambiar environment o argumentos;
- bloquear un launch por un error/divergencia exclusiva del shadow;
- escribir manifests, settings o server config;
- cambiar recommendation/compatibility visible por IPC.

Si el shadow falla, el flujo legacy continúa. Un problema que ya bloquea por una validación legacy
sigue bloqueando por esa autoridad, no porque shadow lo haya reclasificado.

### 11.2 Parity contract

Se compara, como mínimo:

1. kind y token del entrypoint canónico efectivo;
2. fuente de selección: override de server, global o product default;
3. sync efectivo normalizado;
4. prefix legacy: scope, managed/custom, path token y server id;
5. estrategia y owner de DXVK (`Runner`, `Managed`, `Winetricks`);
6. requirement/component esperado para DXVK managed;
7. decisión dgVoodoo configurado/verificado;
8. política exhaustiva de los cinco targets;
9. DLL claims, variables gráficas y la contribución base `WINE_LARGE_ADDRESS_AWARE=1` efectivas por
   target;
10. `webview2_required`;
11. recipe legacy fija: Gecko, vcrun2019, d3dx9, corefonts, font fallback y audio;
12. recommendation Gepard legacy y prueba de que no cambió la selección.

Los mensajes humanos, orden de mapa, paths crudos y readiness transitoria no forman parte de
parity. Cada divergencia usa una categoría estable y valores redacted. Se registra como máximo una
vez por operación superior.

No se añade cache ni lock global en Fase 1. El shadow consume el mismo snapshot de facts ya
resueltos por legacy cuando sea posible; no debe ejecutar `wine --version` varias veces dentro de la
misma resolución.

## 12. Performance y límites de ejecución

| Operación                               | Clase            | Regla                                                                 |
| --------------------------------------- | ---------------- | --------------------------------------------------------------------- |
| PE/Gepard scan, probes de runner        | cold/launch path | una vez por resolución superior; nunca por frame/tick                 |
| setup, payload validation, receipts     | setup path       | I/O permitido bajo operación/transaction                              |
| construcción/merge de maps pequeños     | cold/launch path | claridad y conflictos prevalecen sobre micro-optimización             |
| serialización de summary                | diagnóstico/IPC  | sólo bajo demanda; no repetir durante polling                         |
| input, memory, presence/session polling | hot/runtime loop | cero fingerprinting, file hashing, tree scan, plan clone o lock nuevo |

Si se añade cache después de Fase 1, el probe de cada archivo usa como invalidation key el locator
canónico más `device`, `inode`, `size`, `mtime_ns` y `ctime_ns`. Cualquier cambio, error o ausencia
invalida. No se cachea por display label ni sólo por path.

## 13. Alternativas rechazadas

- grafo/lista arbitraria de translation layers;
- booleans nuevos por backend;
- hacer de `RuntimePlan` un snapshot de toda inspección y compatibilidad;
- persistir el plan o profile en Fase 1;
- reimplementar invocaciones fuera de `ResolvedRunner`;
- absorber dgVoodoo dentro del prefix manifest;
- tratar marker/shape como payload hash verificado;
- usar last-write-wins para DLL/env conflicts;
- añadir WineD3D u otro backend sin un consumidor productivo; D7VK queda además prohibido por
  ADR-005.

## 14. Consecuencias

Positivas:

- un backend futuro debe añadir un variant completo, ownership, provenance, target policy y tests,
  no branches repartidos;
- las combinaciones DXVK inválidas fallan al construir el plan;
- target differences se vuelven visibles y testeables;
- Fase 1 puede retirarse sin alterar ejecución ni datos.

Costes:

- se necesita un adapter de facts legacy y un comparador temporal;
- la verificación exacta de runners externos seguirá produciendo `Unknown` en casos legítimos;
- activar el environment estructurado quedó acotado a Fase 4: con `RO_LAUNCHER_RUNTIME_GRAPHICS=0`
  el launcher sigue el builder booleano legacy; con el flag en default u otro valor distinto de `0`,
  spawn y deps derivan `GraphicsEnvironment` desde `RuntimePlan` (`docs/RO_RUNTIME_PHASE_4_CONTRACT.md`).
- assessments de compatibilidad Gepard exactos y display asociado pasan a ADR-004 y Fase 5
  (`docs/RO_RUNTIME_PHASE_5_CONTRACT.md`); Fase 1–4 no elevaban `Validated` sin evidencia de runtime.

## 15. Criterio de cumplimiento

Este ADR se considera implementado para Fase 1 cuando los tipos anteriores sólo admiten las dos
topologías actuales, el resolver shadow cubre el parity contract, toda divergencia es redacted y no
operacional, y no cambian runner, prefix, provisioning, environment, IPC persistido ni lifecycle de
`ro-sessiond`.
