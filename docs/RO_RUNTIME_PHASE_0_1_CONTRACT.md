# Contrato de implementación — Runtime Fases 0 y 1

| Campo                             | Valor                                             |
| --------------------------------- | ------------------------------------------------- |
| Estado                            | Fases 0 y 1 implementadas; aceptación runtime pendiente |
| Fecha                             | 2026-09-17                                        |
| Alcance                           | Sólo Fase 0 y Fase 1                              |
| Lectura obligatoria               | `AGENTS.md`, ADR-001, ADR-002                     |
| Autoridad operacional al terminar | Código legacy; el resolver nuevo permanece shadow |

Este documento es el handoff normativo. No reemplaza el roadmap; fija las decisiones que un agente
implementador no debe volver a diseñar.

## 1. Fase 0 — Characterization y corrección segura del default

### 1.1 Scope

1. Congelar mediante tests la conducta actual de runner, prefix, DXVK, dgVoodoo, target environments
   y requirements.
2. Añadir fixtures mínimos de manifests existentes.
3. Corregir la discrepancia del default general sin sobrescribir una elección persistida.
4. No introducir aún el módulo productivo de runtime ni cambiar autoridades.

### 1.2 Decisión exacta para settings/default

El sistema debe distinguir internamente:

```rust
enum SettingsDocumentOrigin {
    Missing,
    Persisted,
    Recovered,
}
```

La detección de `Missing` ocurre bajo el mismo `SETTINGS_LOCK` que la lectura. No cambia el JSON ni
se serializa por IPC.

- `Missing`: devolver `managed_proton_path()` como default efectivo. Discovery ya expone esa ruta
  aunque todavía no esté instalada.
- `Persisted` o `Recovered`: conservar `defaultRunner` exactamente, incluido system/custom Wine.
- un valor persistido no disponible no se reemplaza ni se guarda automáticamente como managed;
  se conserva y la UI lo muestra como no disponible para que el usuario decida;
- un `server.runner` no vacío continúa ganando al global y tampoco se sustituye por recommendation;
- no reescribir `settings.json` sólo por leerlo;
- no cambiar `AppSettings` serializado ni añadir un campo “was explicit”.

`AppSettings::default()` puede permanecer como fallback interno mientras el repository reemplaza el
valor únicamente para `Missing`; no se debe codificar una ruta managed duplicada en `models`.

### 1.3 Files likely affected

- `src-tauri/src/utils/settings.rs`: origin de documento bajo lock;
- `src-tauri/src/state/mod.rs`: aplicar el product default managed sólo a `Missing`;
- `src/features/settings/settings.logic.ts`: conservar runner persistido aunque discovery no lo
  encuentre;
- `src/features/settings/settings.store.ts`: representar/mostrar selección no disponible sin
  persistir una sustitución;
- tests colocados en los anteriores;
- characterization en `utils/runner.rs`, `utils/prefix.rs`, `utils/wine.rs`,
  `tools/prefix/setup.rs`, `tools/launcher/session.rs`, `tools/server_tools/session.rs` y
  `tools/deps/check.rs`.

No es obligatorio tocar todos si los tests existentes ya prueban una rama de forma equivalente.

### 1.4 Entregables

- [x] Test `server override > global persistido > product default managed`.
- [x] Test de primera instalación sin `settings.json`: Proton-CachyOS managed efectivo.
- [x] Test de upgrade con `/usr/bin/wine`, Wine portable y Proton externo persistidos: sin overwrite.
- [x] Test de runner persistido temporalmente ausente: se conserva y no se llama `saveSettings`.
- [x] Table test `Proton -> Runner`, `Wine 7.16 -> Managed`, otro Wine -> `Winetricks`.
- [x] Fixture prefix manifest schema 2 válido, corrupto, future-schema y runner mismatch.
- [x] Fixture runtime marker schema 1 y dgVoodoo manifest schema 2.
- [x] Tests de environment actual para Game, LaunchPatcher, MaintenancePatcher, OpenSetup y CPL.
- [x] Tests de managed DXVK con/sin dgVoodoo, incluido orden/contents actuales de overrides.
- [x] Test de patcher manual distinto del launch-patcher.
- [x] Test de dgVoodoo config editable versus wrapper modificado.
- [x] Matriz documentada de los dos anchors: Sakura y Honey, sin ampliar predicates.

### 1.5 Acceptance criteria

- una instalación sin settings resuelve el managed Proton;
- cualquier `defaultRunner` preexistente sobrevive exactamente a load/discovery;
- cada rama legacy que comparará Fase 1 tiene al menos un test;
- los tests prueban el environment efectivo y prefix/runner, no sólo labels;
- ningún manifest/schema/path se escribe de otra forma por esta fase.

### 1.6 Tests y gates

Primero tests Rust/TypeScript focalizados. Después:

```text
npm run lint
npm run format:check
npm test
npm run build
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

El cambio de default requiere smoke `npm run tauri:dev` para:

- primera instalación/default Proton managed;
- server override Wine 7.16;
- settings persistido system/custom Wine.

Registrar runner path/version, prefix y env efectivos. No afirmar runtime validation que no se
ejecutó.

### 1.7 Prohibido

- nuevos schemas o paths de prefix;
- persistir profiles/plans/fingerprints;
- cambiar `ResolvedRunner` o `ro-sessiond`;
- activar environment nuevo;
- “mejorar” predicates Gepard;
- D7VK, catálogo de artifacts o UI de profiles;
- reescribir settings existentes para normalizarlos al default.

### 1.8 Rollback

Los tests/fixtures se revierten sin datos. La corrección del default se revierte restaurando el
resolver de `Missing`; no toca settings existentes ni prefixes. Si la UI no puede representar un
runner ausente sin pérdida, Fase 0 no está terminada.

### 1.9 Registro de implementación

Implementado el 2026-09-17. El origen del documento se captura bajo `SETTINGS_LOCK`; el repository
aplica la ruta de Proton administrado sólo a `Missing` y no la persiste implícitamente. La UI
conserva una selección no detectada, la presenta como tal y no llama `saveSettings`. Los fixtures
versionados viven en `contract-fixtures/` y characterization cubre precedencia, DXVK, prefix,
targets y seguridad de dgVoodoo sin crear schemas ni paths nuevos.

## 2. Fase 1 — Dominio tipado en shadow mode

### 2.1 Scope

Crear `src-tauri/src/tools/runtime/` con modelo, probes/adapters, resolver puro y comparador shadow.
Integrarlo en los comandos superiores sólo para observar. Mantener todos los writers/spawns legacy.

No se añade IPC/UI en Fase 1. Un summary público se difiere hasta que haya un consumidor real y una
política de redaction probada.

### 2.2 Tipos exactos acordados

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

struct RuntimePlan {
    runner: RunnerPlan,
    graphics: GraphicsPlan,
    prefix: PrefixLocation,
    webview2_required: bool,
}

struct RunnerPlan {
    resolved: ResolvedRunner,
    identity: RunnerIdentity,
    capabilities: RunnerCapabilities,
    sync: SyncPlan,
}

enum GraphicsProfile { Dxvk, DgVoodooDxvk }

enum GraphicsPlan {
    Dxvk { dxvk: DxvkProvider },
    DgVoodooDxvk { overlay: DgVoodooOverlayPlan, dxvk: DxvkProvider },
}

enum DxvkProvider {
    RunnerOwned { component: RunnerComponentId },
    ManagedPrefix { artifact_id: ArtifactId },
    WinetricksPrefix { provenance: ComponentProvenance },
}

enum InvocationTarget {
    Game,
    LaunchPatcher,
    MaintenancePatcher,
    OpenSetup,
    GraphicsControlPanel,
}
```

`RunnerIdentity`, `RunnerCapabilities`, `SyncPlan`, `GraphicsEnvironment`, `DllOverrideSet`,
`EnvironmentDelta`, provenance y ownership siguen exactamente ADR-001. Los fields internos pueden
ser privados y usar smart constructors; no se exponen structs parcialmente construibles.

No crear en Fase 1:

- `ClientInspection` público;
- `DependencyPlan`;
- `PrefixPlan`;
- `RuntimeProfileId` persistente;
- `CompatibilityAssessment` productivo;
- `RuntimeFingerprint` o `PrefixFingerprint` (son Fase 2).

### 2.3 Ownership

| Estado efectivo                  | Owner                               |
| -------------------------------- | ----------------------------------- |
| DXVK Proton                      | runner                              |
| DLL/config DXVK 2.6.2 instaladas | prefix                              |
| DXVK instalado por winetricks    | prefix, provenance `LegacyUnpinned` |
| dgVoodoo wrappers/config/CPL     | game-dir overlay                    |
| runners externos                 | host/external                       |

Artifact store no es owner de las copias instaladas. dgVoodoo no se añade a prefix components ni a
su manifest. Cada owner combina dominio + component ID; dos componentes prefix-owned distintos no
son el mismo owner y no pueden reclamar la misma DLL por coincidencia de ubicación.

### 2.4 Public APIs previstas

Los nombres pueden adaptarse al style Rust, pero la responsabilidad no cambia:

```rust
fn probe_runner(resolved: &ResolvedRunner) -> RunnerProbe;

fn profile_from_legacy(input: LegacyProfileInput<'_>) -> RuntimeProfile;

fn resolve_runtime(input: RuntimeResolutionInput) -> Result<RuntimePlan, RuntimeResolutionError>;

impl GraphicsPlan {
    fn environment_for(
        &self,
        target: InvocationTarget,
        prefix: &PrefixLocation,
    ) -> Result<GraphicsEnvironment, GraphicsEnvironmentError>;
}

fn merge_environment_deltas<'a>(
    deltas: impl IntoIterator<Item = &'a EnvironmentDelta>,
) -> Result<EnvironmentDelta, EnvironmentConflict>;

fn capture_legacy_runtime(input: LegacySnapshotInput<'_>) -> LegacyRuntimeSnapshot;

fn compare_shadow(
    legacy: &LegacyRuntimeSnapshot,
    plan: &RuntimePlan,
) -> ShadowComparison;
```

`RuntimeResolutionInput` contiene facts ya disponibles: profile solicitado, `ResolvedRunner`,
`PrefixLocation`, un `RunnerProbe` tomado una vez, estado dgVoodoo verificado y requirement WebView2.
No acepta `DxvkProvision`: el resolver nuevo debe derivarlo y el comparador debe poder detectar una
divergencia. El constructor rechaza un `RunnerRequest` que no corresponda al `ResolvedRunner`
recibido; shadow registra esa situación como `runner-identity` y legacy continúa.

`LegacyRuntimeSnapshot` es el adapter boundary temporal. Captura las decisiones actuales después de
que legacy las resolvió; no se convierte en modelo de dominio ni se usa para ejecutar el plan nuevo.

### 2.5 Autoridad legacy que permanece

- `resolve_server_wine_context_with_runner` elige runner y prefix;
- `DxvkProvision::for_runner` elige provisioning real;
- `setup_runtime_prefix` instala y escribe schema 2;
- `apply_game_env`/`apply_tool_env` producen el environment real;
- launcher y server-tools deciden y hacen spawn;
- `runtime_prefix_blockers` bloquea launches;
- Gepard legacy sólo recomienda/advierte como hoy;
- `RunnerSessionRegistry` y `ro-sessiond` conservan lifecycle actual.

### 2.6 Shadow comparison contract

Comparar por operación superior:

- runner kind, entrypoint canonical token y selection source;
- sync normalizado;
- prefix scope/managed/custom, server id y path token legacy;
- provider/owner DXVK y component requirement managed;
- dgVoodoo configured+verified;
- policy de los cinco targets;
- DLL claims, variables gráficas y `WINE_LARGE_ADDRESS_AWARE=1` normalizados por target;
- WebView2 requirement;
- recipe base fija (Gecko, vcrun2019, d3dx9, corefonts, fallback y audio);
- recommendation Gepard legacy y ausencia de sustitución del runner.

No comparar mensajes localizados, map ordering, paths crudos, log/cache contents ni readiness
transitoria.

Cada mismatch produce una categoría estable, por ejemplo:

```text
runner-identity
selection-source
sync-plan
prefix-binding
dxvk-provider
dgvoodoo-state
target-policy
dll-overrides
environment-delta
webview2-requirement
base-recipe
legacy-recommendation
shadow-resolution-error
```

Loggear una sola vez por operación, con tokens/digests truncados y sin command line, credenciales o
paths personales. Tests fallan ante divergencia no allowlisted. No mantener allowlist permanente:
cada entrada necesita issue, reason y fecha de eliminación.

### 2.7 Semántica operacional de shadow

El shadow puede resolver, comparar, loggear y fallar tests. No puede cambiar runner, prefix,
provisioning, environment, spawn, blocker, IPC o persistencia.

Un error interno de shadow se registra y legacy continúa. Incluso una divergencia security-relevant
no crea un bloqueo nuevo en Fase 1; debe abrir un issue/test y se corrige antes de transferir
authority. Un blocker legacy existente sigue actuando normalmente.

Rollback de release:

```text
RO_LAUNCHER_RUNTIME_SHADOW=0
```

La flag deshabilita por completo probes/comparison nuevos y está enabled por defecto sólo después de
pasar characterization. No afecta `RO_LAUNCHER_SESSION_SUPERVISOR`.

### 2.8 Lifetime y performance

- resolver una vez por launch/setup/deps/tool y pasar `&RuntimePlan`;
- compartir el mismo `RunnerProbe` dentro de la operación; no repetir `wine --version`;
- no guardar el plan en hot state ni clonarlo en polling;
- no escanear árboles de runner;
- no hacer hashing/fingerprinting en Fase 1;
- no añadir locks globales ni mantener locks sync a través de `.await`;
- memory/input/presence/ro-sessiond no conocen estos tipos.

### 2.9 Entregables

- [x] módulo runtime con tipos cerrados y smart constructors;
- [x] probe con `Known`/`Unknown`, incluido old/new-WoW64 sin inferencia por versión;
- [x] resolver puro de los dos graphics profiles;
- [x] `environment_for` estructurado para cinco targets, sólo observacional;
- [x] merge conflictivo de DLL/env cubierto por tests;
- [x] adapters legacy y shadow comparator en launch, setup, deps y server tools;
- [x] logs redacted y flag de rollback;
- [x] table tests de Proton managed/externo, Wine 7.16 y Wine genérico;
- [x] parity tests de los dos anchors y Gepard unknown;
- [x] prueba de que shadow failure no altera ni bloquea el resultado legacy.

### 2.10 Acceptance criteria

Para el corpus actual, shadow produce paridad en todos los campos de §2.6. Los tests hacen imposible
construir managed DXVK sobre runner-owned DXVK, dos owners de una DLL o un graphics plan sin owner.
No cambia ningún command/env/path/schema/IPC observable y la flag elimina todo el trabajo shadow.

### 2.11 Tests y adversarial pass

Además de gates completos, ejecutar smoke de Proton managed y Wine 7.16, launch directo, patcher
handoff, maintenance patcher, OpenSetup, multi-client y shadow disabled. Revisar AppImage sanitation,
error/cancellation y que los logs no expongan paths/args.

### 2.12 Prohibido

- conectar el nuevo environment al spawn;
- escribir manifest v3 o fingerprint;
- declarar Sakura validated por versión 7.16;
- cambiar Gepard/GameGuard o el cliente;
- añadir D7VK/WineD3D/plugins/DAG;
- añadir summary IPC sin consumidor aprobado;
- mover runtime domain a `ro-tools-core`;
- enseñar graphics/fingerprint a `ro-sessiond`.

### 2.13 Registro de implementación y validación

Implementado el 2026-09-17 en `src-tauri/src/tools/runtime/` e integrado de forma exclusivamente
observacional en launch, setup/reset, deps y server tools. El código legacy conserva toda autoridad
operacional y `RO_LAUNCHER_RUNTIME_SHADOW=0` evita probes y comparación nuevos.

Validación automatizada ejecutada:

- `npm run lint`, `npm run format:check`, `npm test` (109 tests) y `npm run build`;
- `cargo fmt --all -- --check`;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- `cargo test --workspace --all-features` (345 tests aprobados; dos ignorados por requerir
  hardware/permisos manuales);
- `npm run tauri:dev` arrancó con shadow habilitado y con
  `RO_LAUNCHER_RUNTIME_SHADOW=0`.

No se ejecutaron clientes reales ni una AppImage instalada. Siguen pendientes los smokes de primera
instalación/default Proton, settings system/custom persistidos, override Wine 7.16, Sakura y Honey,
launch directo, ambos flujos de patcher, OpenSetup, multi-client, sanitización de AppImage y
fallos/cancelación. Por eso los entregables de código de §1.4 y §2.9 están completos, pero el
acceptance runtime de §1.5–§1.6 y §2.10–§2.11 no está cerrado.

## 3. Decision table para implementadores

| Question                                       | Decision                                                                                                                      | Reason                                          | Evidence                                      | Implementation consequence                | Can revisit when                    |
| ---------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------- | --------------------------------------------- | ----------------------------------------- | ----------------------------------- |
| GraphicsProfile variants F1                    | Sólo `Dxvk`, `DgVoodooDxvk`                                                                                                   | Son las dos topologías reales                   | booleans/env y dgVoodoo actuales              | enum cerrado; no D7VK                     | ADR-005 después del spike           |
| RuntimeProfile persistence                     | No se persiste en F0–F2                                                                                                       | Config actual basta y evita migración prematura | `servers.json`/`settings.json` guardan runner | adapter efímero                           | UI real de profiles                 |
| RuntimePlan lifetime                           | Una operación superior                                                                                                        | manifests/game dir pueden cambiar               | callers resuelven por comando                 | crear una vez, pasar por referencia       | al anclar leases en F2/F4           |
| RuntimeFingerprint inputs                      | Stack resuelto, prefix binding, recipes y env semántico; no subject/host                                                      | separa runtime de evidencia                     | ADR-002 §2                                    | implementar recién F2 con golden encoding | schema v2 por ADR                   |
| PrefixFingerprint inputs                       | Sólo runner/material/layout/arch policy/recipe y estado isolation-critical                                                    | evita proliferación                             | ownership actual                              | set dependency crítico vacío en v1        | nuevo estado realmente incompatible |
| Prefix path derivation                         | `<server16>-v3-<prefix24>`; full IDs en manifest                                                                              | ruta no es autoridad                            | protecciones v2 actuales                      | collision=mismatch hard error             | path schema v4                      |
| v2 alias rules                                 | Sólo topología legacy y runner kind/path exactos; `LegacyV2RunnerMatched`                                                     | metadata no prueba artifacts históricos         | manifest schema 2                             | read old, no rename/rewrite               | migración explícita opt-in          |
| Runner identity                                | Provenance + roles/digests acotados; locator separado                                                                         | path/version solos no bastan                    | runner/discovery code                         | no tree hash; external stays incomplete   | ADR-003/manual receipt              |
| old/new-WoW64                                  | `StructuralProbeV1` sólo afirma old con ambos Unix dirs no vacíos; new/32-only requieren receipt validator; 7.16 no lo prueba | predicate actual sólo lee version               | `is_wine_7_16`, discovery label               | Unknown no respalda Validated             | nuevo validator versionado por ADR  |
| DXVK ownership                                 | Proton runner; managed/winetricks prefix                                                                                      | instalación actual difiere                      | `DxvkProvision` y setup                       | variants mutuamente excluyentes           | nuevo provider real                 |
| dgVoodoo ownership                             | game-dir overlay, manifest separado                                                                                           | archivos/backups actuales                       | `server_tools/dgvoodoo.rs`                    | nunca prefix component                    | no revisit sin migration ADR        |
| Target environments                            | Cinco targets; coherentes pero no idénticos                                                                                   | patcher manual ya difiere                       | launcher/server-tools call sites              | tabla exhaustiva y tests                  | nuevo target real                   |
| DLL override conflict                          | mismo owner+valor idempotente; cualquier otro claim error                                                                     | last-write-wins oculta owner                    | `ProcessEnv::set_env`                         | BTreeMap + preflight                      | nuevo load order real               |
| Unknown compatibility                          | Ausencia/insuficiencia de evidencia                                                                                           | no inventar compatibilidad                      | Gepard exact hashes                           | conservar elección, warning               | evidence exacta                     |
| Explicit user override                         | No se sustituye por recommendation/default                                                                                    | selección y recommendation son distintas        | server override + settings                    | preservar incluso si no disponible        | acción explícita del usuario        |
| Artifact provenance                            | Receipt/bundled/external/legacy; identity separada de integrity                                                               | marker actual sólo prueba shape                 | `managed.rs` validators                       | corrupt=ineligible, no digest nuevo       | ADR-003                             |
| Shadow divergence behavior                     | log redacted + test failure; launch legacy continúa                                                                           | shadow no tiene authority                       | phase boundary                                | nunca bloquear/mutar                      | Fase 4 tras parity                  |
| Compatibility vs eligibility vs recommendation | Tres conceptos separados                                                                                                      | un enum no responde tres preguntas              | predicates/labels actuales divergen           | no meterlos en RuntimePlan                | ADR-004                             |
| ClientInspection in F1                         | Eliminado/deferred                                                                                                            | no es necesario para paridad                    | scans tienen otros consumers                  | usar facts mínimos legacy                 | Fase 5 compatibility                |
| DependencyPlan in F1                           | Eliminado                                                                                                                     | receta base fija; una variación WebView2        | `RuntimeRequirements` actual                  | bool + recipe legacy                      | segunda policy real                 |
| PrefixPlan in F1                               | Eliminado                                                                                                                     | duplicaría `PrefixLocation`                     | prefix utilities actuales                     | reuse tipo existente                      | Fase 2 si identity exige wrapper    |
| External runner completeness                   | **DEFERRED:** no identity completa sin receipt/build manifest                                                                 | hash de árbol no es solución                    | layout externo arbitrario                     | `ExternalObserved`, compatibility Unknown | Fase 3/5 con evidencia concreta     |
| D7VK deployment/release                        | **DEFERRED:** spike 6A fijó pin v2.2 y harness; variant persistido solo tras ADR-005 `go`/`go-reduced-scope`                  | no existe consumidor productivo                 | `d7vk_spike.rs` + ADR-005                   | ningún variant en F1                      | ADR-005 (`blocked-insufficient-evidence`) |
| Artifact catalog completo                      | **DEFERRED:** F1 sólo requiere provenance semantic                                                                            | no diseñar package manager                      | managed mechanisms actuales                   | tipos mínimos/adapters                    | ADR-003/Fase 3                      |
| Compatibility catalog/schema                   | **DEFERRED:** semántica cerrada, records/store aún sin segundo consumer                                                       | F1 sólo compara recommendation legacy           | exact Gepard hashes actuales                  | ningún catalog/write en F1                | ADR-004/Fase 5                      |
| Profile IDs y UX                               | **DEFERRED:** falta selección de profile productiva                                                                           | evitar persistencia prematura                   | config actual expresa runner                  | adapter efímero                           | Fase 4 o UI posterior               |
| Captura de benchmark                           | **DEFERRED:** falta workload/protocolo reproducible                                                                           | no influye en paridad del runtime               | no hay harness actual                         | cero métricas/score en F1                 | ADR-006/Fase 8                      |

## 4. Adversarial architecture review

| Caso intentado                                       | Resultado exigido                                                                              |
| ---------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| 1. Sakura + Wine 7.16 con layout equivocado          | version conocida, layout `Unknown`/`NewWow64`; no exact Validated; shadow no cambia legacy     |
| 2. Honey con Proton externo                          | `ExternalObserved`, no confundir con receipt managed; elección explícita se conserva           |
| 3. DXVK Proton + managed DXVK                        | constructor rechaza dos providers/owners antes de mutar                                        |
| 4. dgVoodoo modificado por usuario                   | config válida entra al runtime identity; wrapper modificado queda no verificado y no se fuerza |
| 5. patcher manual vs launch patcher                  | targets distintos y policy explícita; ningún caller inventa boolean                            |
| 6. mismo runner path con contenido actualizado       | roles observados cambian identity/fingerprints; external incompleto nunca se declara exacto    |
| 7. v2 válido con artifacts internos cambiados        | revalidar; `LegacyUnpinned`/Unknown o repair; nunca promover metadata histórica                |
| 8. custom prefix user-owned                          | sin path v3, adopción, rename, delete ni manifest rewrite automático                           |
| 9. Gepard unknown                                    | compatibility `Unknown`; no fuerza runner/profile                                              |
| 10. dos clients y cambio de profile                  | F2 session anchor rechaza fingerprint distinto hasta liberar todas las leases                  |
| 11. receipt válido con payload corrupto              | identity estable, eligibility `Ineligible`, reparación transaccional                           |
| 12. D7VK reclama `ddraw.dll` de dgVoodoo             | futuro variant mutuamente excluyente y `DllOverrideSet` produce conflict                       |
| 13. runtime fingerprint cambia por detalle no-prefix | prefix fingerprint/path no cambia; por ejemplo sync/dgVoodoo                                   |
| 14. prefix fingerprint omite cambio ABI/material     | runner/layout/arch/prefix DXVK/recipe críticos están incluidos; golden matrix debe fallar      |

Conclusión del review: la API normal no permite doble owner, provider gráfico incompleto ni
promotion de evidencia unknown. La incertidumbre inevitable de v2 y runners externos queda
representada, no escondida. Ninguna decisión de Fase 0–1 requiere una migración big-bang.
