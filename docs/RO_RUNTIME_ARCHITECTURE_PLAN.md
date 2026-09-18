# Arquitectura del runtime de Ragnarok Online

Planning de consolidación incremental para que RO-Launcher pueda resolver, materializar, ejecutar y
explicar runtimes reproducibles sin convertir cada backend o compatibilidad nueva en una excepción
distribuida por el launcher.

| Campo                   | Valor                                                                                                                                                                                   |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Estado                  | Fases 0 y 1 implementadas; resolver nuevo en shadow mode; autoridad legacy preservada; aceptación con clientes reales pendiente                                                        |
| Última revisión         | 2026-09-17                                                                                                                                                                              |
| Alcance                 | Resolución de runner, gráficos, dependencias, identidad de prefix, artefactos administrados, compatibilidad y diagnóstico                                                               |
| Objetivo                | Introducir seams tipados e incrementales que preserven el comportamiento validado y permitan agregar un backend gráfico sin modificar launcher, setup, tools y diagnóstico por separado |
| Decisión bloqueante     | Cerrada por `docs/adr/ADR-001-runtime-graphics-domain.md` y `docs/adr/ADR-002-runtime-prefix-identity.md`                                                                               |
| Primer consumidor nuevo | D7VK, sólo después de consolidar el seam actual y completar un spike de despliegue                                                                                                      |
| Fuera de esta tarea     | Implementar Fases 2–9, descargar D7VK o modificar runtimes, prefixes, clientes o anti-cheat                                                                                             |

## 1. Cómo leer este documento

Las afirmaciones de la auditoría usan estas categorías:

- **Confirmado:** observado directamente en el código, tests, manifests o documentación autoritativa
  del repositorio.
- **Inferencia fuerte:** consecuencia arquitectónica respaldada por más de un seam actual, pero aún
  no validada con una implementación completa.
- **Desconocido:** requiere una prueba controlada, un artefacto concreto o evidencia de runtime; no
  debe convertirse en un default.

Este documento es un roadmap ejecutable, no un compromiso con todos los tipos o nombres mostrados.
Cada fase debe volver a contrastar su diseño con el código vigente y puede ajustar nombres sin
cambiar las decisiones cerradas ni las invariantes.

Para Fases 0–2, los ADR-001/ADR-002, `RO_RUNTIME_PHASE_0_1_CONTRACT.md` y (Fase 2)
`RO_RUNTIME_PHASE_2_CONTRACT.md` son normativos y prevalecen sobre sketches anteriores de este
roadmap. En particular, simplifican Fase 1 y difieren
`ClientInspection`, `DependencyPlan`, `PrefixPlan` y compatibility productiva hasta que exista su
consumidor correspondiente.

## 2. Diagnóstico arquitectónico

RO-Launcher no necesita una reescritura. Ya tiene boundaries valiosos para runners, invocaciones,
prefixes, operaciones transaccionales, ownership de procesos y herramientas platform-neutral. El
problema es más estrecho: **la decisión de runtime está repartida y todavía no existe como un valor
único que los consumidores puedan compartir**.

Hoy una misma decisión gráfica se reconstruye de forma independiente en:

- `src-tauri/src/tools/deps/check.rs`, para readiness y diagnóstico;
- `src-tauri/src/commands/prefix.rs`, para derivar `RuntimeRequirements`;
- `src-tauri/src/tools/prefix/setup.rs`, para seleccionar y provisionar DXVK;
- `src-tauri/src/tools/launcher/session.rs`, para descubrir dgVoodoo, revisar el manifest y aplicar
  el environment;
- `src-tauri/src/tools/server_tools/session.rs`, para repetir una variante del mismo cálculo en
  OpenSetup, patcher y dgVoodoo CPL;
- `src-tauri/src/utils/wine.rs`, donde dos booleanos terminan convirtiéndose en
  `WINEDLLOVERRIDES` y variables DXVK.

Esto funciona para las combinaciones presentes, pero obliga a cada nuevo backend a conocer runner,
manifest, layout de DLLs, entorno, tools y diagnóstico. D7VK haría crecer esa multiplicación si se
añadiera como otro booleano.

La consolidación propuesta introduce una secuencia única:

```text
solicitud + inspección
        -> RuntimeProfile (intención cerrada)
        -> RuntimePlan (resolución concreta e inmutable)
        -> validación/materialización
        -> invocaciones derivadas del mismo plan
        -> evidencia del resultado
```

No se propone un grafo arbitrario de capas, un sistema de plugins ni un rewrite de
`ResolvedRunner`. Se propone un dominio cerrado con las combinaciones que el producto realmente
soporta y variantes nuevas sólo cuando existe un segundo consumidor real.

## 3. Baseline y alcance auditado

Baseline establecido el 2026-09-16 desde la raíz del repositorio:

```text
branch: main
HEAD: 5019b92 Harden ro-sessiond lifecycle and audit workflow
upstream: main == origin/main
working tree al iniciar la revisión ADR: únicamente este planning sin trackear; preservado
AGENTS.md adicionales: ninguno
git diff --check: limpio
```

Se contrastaron como mínimo:

- `AGENTS.md` y `README.md`;
- `docs/PTRACE_SESSION_SUPERVISOR_PLAN.md`;
- `docs/features/DISCORD_RICH_PRESENCE_PLAN.md`;
- runner, prefix y Wine en `src-tauri/src/utils/`;
- catálogo administrado en `src-tauri/src/tools/runners/`;
- setup/reset en `src-tauri/src/tools/prefix/`;
- lanzamiento y lifecycle en `src-tauri/src/tools/launcher/`;
- dgVoodoo, Gepard y análisis PE en `src-tauri/src/tools/server_tools/`;
- comandos Tauri, modelos serializados, stores y contracts frontend relacionados;
- `RunnerSessionRegistry`, `GameProcessHandle`, `ro-sessiond` y su protocolo;
- `crates/ro-tools-core` y `crates/ro-tools-linux`;
- scripts de build, AppImage y CI.

No se ejecutaron clientes ni se inspeccionaron datos locales, prefixes o binarios de juego para
este planning. Las conclusiones de runtime que requieren un cliente real quedan marcadas como
desconocidas.

## 4. Inventario del estado actual

### 4.1 Resolución de runner

**Confirmado.** `ServerConfig.runner` es un override opcional por servidor. La resolución backend
vive en `resolve_server_wine_context_with_runner` y aplica:

```text
server.runner no vacío
        ?? runner/default recibido por IPC
        ?? Proton-CachyOS administrado
```

`ResolvedRunner` en `src-tauri/src/utils/runner.rs` ya es una estrategia cerrada:

```rust
RunnerStrategy::Wine { wine_bin, wineserver_bin }
RunnerStrategy::Proton { proton_script, proton_dir, umu_bin }
```

El tipo es dueño de las invocaciones de juego, tools, builtins, creación de prefix, winetricks y
shutdown. `RunnerInvocation` separa programa, argumentos, cwd y delta de environment del acto de
spawn; esa separación permitió integrar `ro-sessiond` sin duplicar la resolución del runner.

Para Wine, `wine_sync_mode` sólo habilita ESYNC/FSYNC cuando `wine-tkg-config.txt` declara los
patches correspondientes. Proton se ejecuta mediante UMU y verbos explícitos. Esta lógica es un
boundary correcto y no debe absorber gráficos ni compatibilidad de cliente.

**Gap confirmado.** `is_wine_7_16` comprueba la versión reportada, pero la resolución no prueba que
esa distribución tenga layout legacy/old-WoW64. `discover.rs` usa la presencia de
`lib/wine/i386-unix` sólo para una etiqueta de UI. Por tanto, el código actual no demuestra toda la
restricción documentada para SakuraRO.

**Gap confirmado.** `AppSettings::default` llama a `default_system_wine`. El frontend migra a
Proton administrado sólo si la ruta guardada está vacía o ya no figura entre los runners. En una
instalación limpia que tenga Wine del sistema detectable, esa ruta puede conservarse. Esto no
coincide con el contrato de `AGENTS.md` que define Proton-CachyOS 11 mediante UMU como default
general. La corrección debe preservar selecciones explícitas existentes y contar con un test de
primera instalación.

### 4.2 Identidad y resolución de prefix

**Confirmado.** Para servidores, `ServerConfig::effective_prefix_mode` fuerza siempre
`PrefixMode::Isolated`. Los campos `prefixMode` y `winePrefix` legacy siguen deserializándose y
validándose, pero la UI y la resolución efectiva normalizan el servidor a un entorno aislado.

`resolve_server_prefix_with_runner` deriva actualmente:

```text
prefixes/<hash(server-id)>-<hash(canonical-runner-path)>
```

La combinación servidor+runner crea o reutiliza un prefix separado. La ruta no es la autoridad:
`PrefixManifest` schema 2 registra scope, server id, runner kind, runner path y una lista de strings
`components`. `runtime_prefix_blockers` exige que manifest, location y runner coincidan.

Las protecciones actuales son fuertes:

- un directorio administrado no vacío y sin manifest no se adopta;
- un custom prefix no se elimina automáticamente;
- symlinks y paths fuera de la raíz esperada se rechazan;
- un manifest legacy o con runner desconocido obliga a rearmar antes de lanzar;
- reset conserva el prefix anterior bajo un backup hasta completar el reemplazo;
- si el entorno nuevo no puede apagarse con seguridad, se conservan ambos estados;
- procesos activos se detectan antes de mover o reemplazar el prefix.

**Límite confirmado.** La identidad registrada contiene kind+path, no versión, hash del artefacto,
layout WoW64 ni identidad gráfica estructurada. `components` permite reconocer
`dxvk-2.6.2`, pero no expresa procedencia, arquitectura, hashes de archivos instalados ni receta.

### 4.3 Provisioning del runtime

**Confirmado.** `commands/prefix.rs` deriva un `RuntimeRequirements` con WebView2 y
`DxvkProvision`. `setup_resolved_prefix` ejecuta, bajo `RunnerOperation` y `OperationGuard`, la
secuencia:

```text
crear prefix -> Gecko -> graphics -> vcrun2019 -> d3dx9
-> WebView2 opcional -> corefonts -> font fallback -> audio -> manifest
```

`DxvkProvision::for_runner` decide:

| Runner    | Provision actual                     |
| --------- | ------------------------------------ |
| Proton    | DXVK perteneciente al runner         |
| Wine 7.16 | DXVK 2.6.2 administrado              |
| Otro Wine | verbo `dxvk` del winetricks efectivo |

El workaround WebView2 Windows 7/109 se limita correctamente a Wine 7.16 acelerado por ESYNC o
FSYNC y restaura Windows 10 en éxito y error.

**Límite confirmado.** La versión instalada por winetricks para Wine genérico no queda fijada ni
registrada con procedencia exacta. Puede conservarse como comportamiento legacy, pero una entrada
de compatibilidad futura no debe llamarla reproducible sin evidencia adicional.

### 4.4 DXVK y environment gráfico

**Confirmado.** El DXVK administrado para Wine 7.16 instala cinco DLLs x86/x64 según `#arch` del
prefix y crea estado privado en:

```text
<prefix>/.ro-launcher-dxvk/dxvk.conf
<prefix>/.ro-launcher-dxvk/logs/
<prefix>/.ro-launcher-dxvk/cache/
```

`apply_game_env` y `apply_tool_env` reciben dos booleanos, `use_dgvoodoo` y
`use_managed_dxvk`. Para DXVK administrado producen overrides nativos para D3D8/9/10/11/DXGI y,
si dgVoodoo está activo, también para `d3dimm`/`ddraw`. Proton deja que el runner provea DXVK; Wine
genérico depende de la instalación/registry de winetricks.

La decisión `use_managed_dxvk` se reconstruye tanto en `launcher/session.rs` como en
`server_tools/session.rs` leyendo versión de runner más el string del manifest.

### 4.5 dgVoodoo

**Confirmado.** dgVoodoo no vive exclusivamente dentro del prefix. Sus DLLs, configuración y CPL se
instalan en el directorio del juego desde recursos bundled. El manifest
`.ro-launcher-dgvoodoo.json` registra cada archivo instalado y cualquier original respaldado en
`.ro-launcher-dgvoodoo-backup/`.

La implementación actual:

- rechaza symlinks y colisiones case-insensitive;
- no reemplaza wrappers modificados;
- preserva `dgVoodoo.conf` editado por el CPL;
- restaura originales en uninstall;
- hace rollback de mutaciones parciales;
- verifica los wrappers contra los recursos bundled antes de forzar su carga;
- mantiene un `OperationGuard` compartido durante la vida del cliente y exclusivo durante
  install/uninstall.

Esta ownership no debe convertirse en un string dentro del manifest del prefix. Es un
**game-directory overlay** reversible con lifecycle, conflictos y persistencia propios.

OpenSetup y el CPL reciben overrides de dgVoodoo cuando la instalación está verificada. El juego —o
el patcher usado como estrategia de lanzamiento— recibe el environment de juego. El patcher abierto
manualmente desde Tools excluye deliberadamente overrides dgVoodoo mediante
`ToolKind::should_apply_dgvoodoo_overrides`, aunque sí recibe DXVK administrado si corresponde.
Esta diferencia debe convertirse en una política explícita por target y someterse a la invariante
de coherencia, no permanecer escondida en un booleano.

### 4.6 Perfiles Gepard

**Confirmado.** `server_tools/gepard.rs` contiene dos records exactos por SHA-256 de `gepard.dll`:

| Build       | Resultado codificado                 |
| ----------- | ------------------------------------ |
| `26.8.26.1` | `GepardRunnerProfile::ModernProton`  |
| `26.9.3.1`  | `GepardRunnerProfile::Wine716Legacy` |

Un hash desconocido sólo produce advertencia. La recomendación no cambia el runner automáticamente.
Esta política es correcta.

**Límite confirmado.** El check de compatibilidad considera compatible cualquier Proton para el
primer record y cualquier Wine que reporte 7.16 para el segundo. No comprueba el artefacto exacto
Proton-CachyOS, old-WoW64, DXVK 2.6.2 ni el hash del cliente. El label describe una combinación más
precisa de la que el predicado realmente valida.

En particular, `stack_label()` muestra `Proton-CachyOS 11 + DXVK 3.0.1` para el perfil moderno,
pero el record sólo persiste el enum `ModernProton` y el predicado acepta cualquier Proton. Esa
versión de display no es todavía un artifact receipt ni prueba de la DLL efectiva.

Los hashes de clientes SakuraRO/HoneyRO ya existen en los perfiles de presencia, pero esa evidencia
no está unificada con Gepard ni con la resolución de runtime. No se debe promover automáticamente
una recomendación Gepard-only a compatibilidad completa sin verificar la pareja exacta.

### 4.7 Orquestación de launch y tools

**Confirmado.** `launch_game` valida prefix y dependencias, resuelve argumentos efímeros, prepara
input, adquiere leases/guards, repara Gecko/audio, vuelve a descubrir gráficos, crea la invocation,
aplica environment y delega el spawn a `RunnerOperation`. Luego detecta el cliente real por
`ProcessIdentity`, registra memoria, transfiere ownership a `GameProcessHandle` y conserva los
guards hasta la salida/handoff.

El flujo protege multi-client y PID reuse. `stop_game` señala identidades estables y detener un
cliente no apaga el prefix mientras queden leases. Los launch values no se serializan ni persisten y
se incorporan a la redacción de logs.

**Seam confirmado.** `RunnerInvocation` es el contrato adecuado para recibir un environment gráfico
ya resuelto. `launcher/session.rs` no necesita conocer versiones de DXVK ni strings de componentes;
debería consumir un plan y pedir una invocation para un target.

### 4.8 Supervisión de sesión

**Confirmado.** `ro-sessiond` está activo por defecto y hay una instancia por prefix activo.
`RunnerSessionRegistry` se indexa por prefix canónico y ancla kind+path del runner. Operations,
requests y clientes tienen leases separados; el shutdown idle revalida contadores y generación.

El protocolo NDJSON versionado, la sanitización AppImage, el subreaper, el shutdown acotado y la
revalidación `(pid, start_time)` ya están cerrados en `PTRACE_SESSION_SUPERVISOR_PLAN.md`. Esta
arquitectura no debe reabrirse para introducir perfiles de runtime.

**Extensión necesaria.** Cuando dos planes puedan compartir path de prefix, el registry debe anclar
también la identidad efectiva de runtime. Un proceso no puede cambiar gráficos, DLL ownership o
prefix recipe mientras otro cliente usa la sesión. Esto es una validación en el launcher; no exige
enseñar gráficos a `ro-sessiond` ni cambiar el protocolo v1.

### 4.9 Artefactos administrados

**Confirmado.** `tools/runners/managed.rs` ya contiene una abstracción interna `Artifact` con URL,
tamaño, digest, archive kind, root y payload validator. Administra UMU, Proton-CachyOS y DXVK 2.6.2.
Descarga a nombre único, verifica tamaño/checksum, extrae en staging, activa por rename y restaura el
runtime anterior si falla.

No hace falta reemplazar este mecanismo por un package manager. Sí llega a su límite por:

- IDs y descriptors privados en un único archivo;
- `ArtifactPayload::{Executable, Dxvk}` especializado;
- marker schema 1 con sólo artifact id y digest;
- readiness que no rechaza todos los symlinks de payload de forma explícita;
- ausencia de arquitectura, versión semántica, origen y receipt tipado consumible por manifests;
- recursos bundled dgVoodoo fuera del catálogo, con otra forma de procedencia.

### 4.10 Persistencia, IPC y frontend

**Confirmado.** `servers.json` persiste runner por servidor y datos de launch, pero no un graphics
profile. `settings.json` persiste default runner. El loader JSON migra campos default de forma
atómica, conserva backup y recupera el primary sólo desde un backup válido.

`DependencyStatus` expone campos legacy DXVK más una lista de checks. El frontend protege contra
resultados stale mediante `runtimeStatusKey`, pero la clave es un JSON derivado de configuración,
no una identidad del plan resuelto ni de artefactos reales.

La UI ya muestra correctamente el runner efectivo por servidor. Cualquier futura selección de
graphics profile debe conservar la misma regla: un override por servidor no puede presentarse como
si el default global estuviera activo.

### 4.11 `ro-tools-core` y `ro-tools-linux`

**Confirmado.** `ro-tools-core` contiene dominio platform-neutral de memoria/input, perfiles y
reglas puras de dgVoodoo. `ro-tools-linux` posee `/proc`, identidad de proceso, memoria, Yama e
input Linux.

El nuevo dominio de runtime no debe moverse a `ro-tools-core` sólo por aspiración de reutilización.
Al inicio tiene un único consumidor Tauri/Linux y usa paths/capabilities de Wine. Los DTOs IPC
pertenecen a `src-tauri/src/models`; los tipos internos deben vivir cerca del resolver en
`src-tauri/src/tools/runtime/`. Sólo una parte realmente platform-neutral y compartida debe migrar a
un crate.

## 5. Lo que ya está bien y no se debe reescribir

- `ResolvedRunner` y `RunnerInvocation`: extender mediante identity/capabilities, no reemplazar su
  estrategia de comandos.
- Precedencia de runner por servidor sobre default global.
- UMU para Proton y Proton-CachyOS 11 como default general contractual.
- Prefix aislado y manifest como autoridad; la ruta por sí sola nunca prueba compatibilidad.
- Safety de custom/unknown prefixes y rebuild transaccional con backup/restore.
- `OperationGuard` para mutaciones de prefix y directorio del juego.
- Instalación reversible de dgVoodoo y preservación de archivos del usuario/servidor.
- Provisioning especial y acotado de DXVK 2.6.2 para Wine 7.16.
- Política de sync derivada del artefacto; no NTSync forzado en Wine 7.16.
- Workaround WebView2 109 limitado a Wine 7.16 acelerado y restauración de Windows 10.
- Hash exacto de Gepard y `unknown` sin runner forzado.
- `ro-sessiond`, leases, subreaper, sanitización AppImage y rollback temporal por variable de
  entorno.
- `ProcessIdentity`, memory sessions y lifecycle multi-client.
- Separación `ro-tools-core`/`ro-tools-linux` y command handlers delgados.
- Escritura JSON atómica, backups y recuperación conservadora.
- Gates completos de frontend/Rust, smoke desktop y validación AppImage para cambios de runtime.

La consolidación debe envolver y hacer explícitas estas capacidades, no volver a implementarlas.

## 6. Presiones y seams arquitectónicos

| Presión confirmada                           | Evidencia                                             | Consecuencia si se agrega otra feature local                          |
| -------------------------------------------- | ----------------------------------------------------- | --------------------------------------------------------------------- |
| Estado gráfico representado por booleanos    | `apply_game_env(use_dgvoodoo, use_managed_dxvk, ...)` | combinaciones inválidas y ramas crecientes                            |
| Decisión repetida                            | deps, prefix command, setup, launcher y tools         | drift entre readiness, setup y launch                                 |
| `DxvkProvision` mezcla backend y procedencia | runner/winetricks/managed                             | difícil representar versión, receipt o D7VK                           |
| Launcher conoce componentes gráficos         | manifest string y Wine 7.16 en `launcher/session.rs`  | cada backend modifica lifecycle crítico                               |
| Manifest usa strings                         | `components: Vec<String>`                             | no expresa arquitectura, digest, receta ni ownership                  |
| Prefix path sólo incluye runner path         | `isolated_prefix_path_for_runner`                     | cambio in-place de runner o graphics prefix-resident no queda aislado |
| Compatibilidad ligada a categoría de runner  | `GepardRunnerProfile`                                 | un label exacto se valida con un predicado demasiado amplio           |
| dgVoodoo vive fuera del prefix               | manifest/backup en game dir                           | no puede tratarse como DLL copiada al prefix                          |
| Artifact payload cerrado sobre DXVK          | `ArtifactPayload::Dxvk`                               | un cuarto artefacto agrega branches internas                          |
| Environment last-write-wins                  | `ProcessEnv::set_env` reemplaza keys                  | conflictos de DLL overrides pueden ocultarse                          |
| Frontend usa una key de configuración        | `runtimeStatusKey`                                    | no detecta que cambió un artefacto bajo el mismo path                 |

**Inferencia fuerte.** El seam natural no es un trait de “backend cualquiera”, sino un
`RuntimePlan` inmutable con subplanes cerrados. DXVK y dgVoodoo ya son dos topologías distintas; D7VK
sería el tercer caso que justifica extraer provisioning/environment común donde realmente coincida.

## 7. Invariantes

### 7.1 Producto

- Proton-CachyOS 11 administrado mediante UMU sigue siendo el default general.
- Un runner no vacío en el servidor prevalece sobre el default y la UI muestra el efectivo.
- SakuraRO validado conserva Wine 7.16 legacy/old-WoW64 + DXVK 2.6.2.
- HoneyRO validado conserva Proton-CachyOS 11.
- Cada fase deja `main` funcional, testeable y entregable; no hay migración big-bang.
- Compatibilidad tiene prioridad sobre rendimiento.

### 7.2 Runtime

- Resolver runner, derivar identidad, validar prefix, provisionar, lanzar y observar usan el mismo
  `RuntimePlan` o uno recalculado con la misma fingerprint.
- Game, OpenSetup y patcher derivan su environment del mismo `GraphicsPlan`. Las diferencias por
  target deben estar tipadas y testeadas, no ser branches incidentales.
- Direct3D 8/9/11 usa DXVK/Vulkan en perfiles actuales. dgVoodoo emite D3D11 y conserva DXVK como
  backend.
- Wine 7.16 sólo habilita FSYNC/ESYNC por capabilities declaradas y nunca NTSync.
- WebView2 temporal Win7/109 no se generaliza.
- Un prefix/sesión activo no puede cambiar runner, prefix recipe ni graphics plan.
- AppImage se sanea antes de Wine, Proton, UMU y sidecars.

### 7.3 Seguridad

- No modificar, hookear, desactivar, spoofear ni evadir Gepard/GameGuard, ejecutable, paquetes,
  memoria o validación del servidor.
- La compatibilidad procede exclusivamente de runtimes legítimos y configuración observable.
- Nunca escribir memoria del cliente.
- No imprimir argumentos sensibles, credenciales, memoria ni paths personales en evidencia
  exportable.
- Toda extracción y activación de artefactos rechaza traversal, symlinks/hardlinks inesperados y
  payloads fuera del descriptor.

### 7.4 Datos y persistencia

- Nunca adoptar ni eliminar automáticamente un directorio no vacío sin manifest válido.
- Nunca eliminar custom prefixes.
- Nunca mover un prefix existente entre runners o identities.
- Un rebuild administrado preserva el anterior hasta validar el nuevo y restaura en fallo.
- Manifests viejos se leen explícitamente; no se reinterpretan como evidencia que nunca guardaron.
- dgVoodoo y cualquier overlay del directorio del juego conservan ownership y rollback separado.
- No hay garbage collection automático de prefixes en las fases iniciales.

### 7.5 Compatibilidad y evidencia

- Un hash desconocido permanece `Unknown`.
- `Validated`, `Experimental` e `Incompatible` se aplican sólo a la clave exacta observada.
- Una recomendación nunca sustituye silenciosamente un runner/profile explícito.
- No se infiere old/new-WoW64 por nombre de directorio.
- Un resultado de un Gepard, cliente, GPU o driver no se generaliza a otro.

### 7.6 Procesos y concurrencia

- Identidad de proceso es siempre `(pid, start_time)` y se revalida antes de señal/lectura.
- Detener un cliente no termina otros clientes, input compartido ni prefix en uso.
- `ro-sessiond` sigue siendo uno por prefix activo; su protocolo no incorpora conocimiento gráfico.
- Locks no cruzan `.await` salvo los async diseñados para ello y tienen orden explícito.
- Cada background task conserva owner, cierre acotado y cleanup.

## 8. Decisiones arquitectónicas propuestas

### 8.1 Decisiones cerradas por este planning y precisadas por ADR

1. **Separar intención de resolución.** `RuntimeProfile` representa una intención conocida;
   `RuntimePlan` representa la resolución concreta para servidor, cliente y máquina.
2. **Usar un dominio gráfico cerrado.** No habrá grafo arbitrario ni combinación de flags. Las
   variantes soportadas codifican topologías completas y mutuamente exclusivas.
3. **Mantener `ResolvedRunner`.** `RunnerPlan` lo envuelve con identity, capabilities y receipt; no
   duplica generación de comandos.
4. **Distinguir dos fingerprints.** `RuntimeFingerprint` identifica toda la ejecución para
   diagnóstico/evidencia; `PrefixFingerprint` es sólo la proyección de estado material que exige
   aislamiento de prefix.
5. **No crear un prefix por toda diferencia gráfica.** Un overlay puramente game-dir/environment no
   cambia por sí solo `PrefixFingerprint`; una DLL/registry/arquitectura dentro del prefix sí.
6. **Conservar ownership separado de dgVoodoo.** Su manifest no se absorbe en el manifest de prefix.
7. **Evolucionar el artifact manager, no reemplazarlo.** Se extraen descriptors/receipts/validators
   tipados sobre el mecanismo actual.
8. **Centralizar environment como datos.** DLL overrides se combinan estructuralmente y se renderizan
   una vez por target; conflictos son errores, no last-write-wins silencioso.
9. **Compatibilidad basada en evidencia exacta.** La ausencia de record produce `Unknown`; nunca un
   fallback optimista.
10. **D7VK llega después del seam y de un spike.** Permanece experimental y opt-in hasta completar
    matriz real; no desplaza dgVoodoo ni se declara más rápido por diseño.
11. **Compatibility DB antes que D7VK productivo.** Primero se corrige el significado de
    “validated”; después se registra D7VK como experimental.
12. **Benchmark y AutoTune no participan en resolución temprana.** Se agregan sólo después de
    fingerprints, receipts, compatibilidad y observabilidad reproducible.

### 8.2 Decisiones pendientes

- Una identidad _completa_ para runners externos sigue diferida: ADR-002 fija
  `ExternalObserved` con roles/digests acotados, pero no lo eleva a receipt exacto ni propone hashear
  recursivamente una distribución.
- Deployment D7VK para RO: side-by-side en game dir o system path del prefix. El upstream documenta
  ambos y no son equivalentes para ownership/rollback.
- Release D7VK concreta, arquitecturas y checksums; se fijan durante el spike, no en este planning.
- Si el cliente RO objetivo usa D3D immediate-mode 3/5/6/7 suficiente para beneficiarse de D7VK o
  depende principalmente de DDraw/GDI no cubierto.
- UX final para seleccionar profiles. La primera migración conserva auto-detección actual; no se
  exponen combinaciones arbitrarias.
- Método de captura de frametime y visual correctness para benchmarks.
- Política futura de inventario/remoción manual de prefixes obsoletos.
- Viabilidad y alcance de `nndsk-wine-ro`, condicionados a evidencia posterior.

## 9. Arquitectura objetivo

```text
 ServerConfig + global selection + host capabilities
                      |
                      v
              ClientInspection
          (PE/hash/Gepard/requirements)
                      |
                      v
          CompatibilityResolver  <---- curated evidence catalog
                      |
                      v
              RuntimeResolver
          profile intent -> concrete plan
                      |
                      v
                 RuntimePlan
        +-------------+-------------+
        |             |             |
        v             v             v
   RunnerPlan    GraphicsPlan   DependencyPlan
        |             |             |
        +-------> PrefixPlan <-------+
                      |
          +-----------+-----------+
          |                       |
          v                       v
  Artifact/Prefix provision   Game-dir overlay
   receipts + transaction     own manifest/rollback
          |                       |
          +-----------+-----------+
                      |
                      v
          InvocationFactory(target)
  game | launch-patcher | tool-patcher | setup | CPL
                      |
                      v
              RunnerInvocation
                      |
                      v
             RunnerOperation lease
                      |
                      v
                 ro-sessiond
                      |
                      v
        ProcessIdentity + MemorySession
                      |
                      v
         Outcome / diagnostics / evidence
```

La frontera importante es que `RuntimePlan` se resuelve antes de los consumidores. Setup, deps,
launch y tools no vuelven a decidir el backend. El plan no contiene `Child`, locks, secretos ni
estado mutable; sólo decisiones, requirements e identities verificables.

## 10. Modelo de dominio propuesto

> **Precisión normativa para Fases 0–2:** ADR-001 reduce el aggregate inicial. Fase 1 implementa
> `RuntimeProfile`, `RunnerPlan`, `GraphicsPlan` y un `RuntimePlan` mínimo que reutiliza
> `PrefixLocation` y contiene sólo `webview2_required`. Los tipos más amplios de esta sección son la
> dirección de largo plazo, no entregables de Fase 1.

### 10.1 `ClientInspection`

**Representa:** hechos observados sobre ejecutable activo, patcher, imports PE, arquitectura, hash y
anti-cheat. Puede incluir `Unknown` por campo cuando la inspección falla.

**No representa:** permiso para modificar el cliente, compatibilidad asumida ni configuración
elegida.

**Creador:** un servicio de inspección que reutiliza `server_tools/pe.rs`, `gepard.rs` y hashing
existente de presence sin duplicar lectores.

**Consumidores:** compatibility resolver, runtime resolver, diagnóstico y evidencia.

**Ubicación inicial:** `src-tauri/src/tools/runtime/inspection.rs`. Los DTOs visibles se proyectan en
`src-tauri/src/models/`.

**Invariantes:** hash exacto o `Unknown`; no fallback por filename para declarar una build
validada; no guarda paths en evidencia exportable.

**Secuencia:** diferido hasta Fase 5. Fases 1–2 consumen únicamente facts legacy mínimos; el
`SubjectFingerprint` de cliente/anti-cheat permanece separado de `RuntimeFingerprint`.

### 10.2 `RuntimeProfile`

**Representa inicialmente:** intención efímera expresada mediante runner solicitado, fuente de
selección y graphics profile. Dependency policy y compatibility constraints se agregan sólo con un
segundo comportamiento real.

**No representa:** paths resueltos, readiness, archivos instalados, GPU actual, procesos ni
resultados de benchmark.

**Creador:** defaults de producto, selección explícita por servidor y recomendaciones exactas de
compatibilidad. En fases tempranas se deriva del comportamiento actual.

**Consumidores:** `RuntimeResolver` exclusivamente.

**Ubicación:** `src-tauri/src/tools/runtime/model.rs`.

**Invariantes:** no permite construir `dgVoodoo + D7VK` como dos flags; no contiene URLs ni comandos
arbitrarios; un profile desconocido falla de forma explícita.

Fases 0–2 no persisten el struct ni agregan un `RuntimeProfileId`. Un ID estable se evaluará cuando
exista UI/config que realmente seleccione profiles.

### 10.3 `RuntimePlan`

Es un aggregate inmutable, no un god object con I/O:

```rust
struct RuntimePlan {
    runner: RunnerPlan,
    graphics: GraphicsPlan,
    prefix: PrefixLocation,
    webview2_required: bool,
}
```

**Representa:** una decisión completamente resuelta para esta máquina y este servidor.

**No representa:** ejecución, descargas completadas, locks, children, credenciales o mutable status.

**Creador inicial:** `RuntimeResolver`, como función determinista sobre configuración, facts
legacy mínimos, capabilities y estado verificado de dgVoodoo.

**Consumidores iniciales:** sólo el comparador shadow. Los consumers operacionales se migran en
Fase 4.

**Invariantes:** sus subplanes no se contradicen; no contiene compatibility, inspección completa,
readiness mutable, procesos ni locks. Fase 2 añade identidad sin ampliar el aggregate con esos
dominios.

### 10.4 `RunnerPlan`

Envuelve, no reemplaza, `ResolvedRunner`:

```rust
struct RunnerPlan {
    resolved: ResolvedRunner,
    identity: RunnerIdentity,
    capabilities: RunnerCapabilities,
    sync: SyncPlan,
}
```

`RunnerIdentity` diferencia artefacto administrado y runner externo. `RunnerCapabilities` registra
hechos como kind, versión reportada, arquitecturas/layout WoW64 y patches sync. `SyncPlan` sólo puede
elegir modos soportados.

Para Proton administrado, el receipt del artefacto completo es la evidencia. Para Wine externo, una
ruta y un string de versión no bastan para estado `Validated`; el plan debe conservar la
provenance como `ExternalObserved` hasta que exista evidencia más fuerte.

### 10.5 `GraphicsProfile` y `GraphicsPlan`

El dominio inicial debe contener sólo comportamientos actuales:

```rust
enum GraphicsProfile {
    Dxvk,
    DgVoodooDxvk,
}
```

La variante D7VK se agrega únicamente en su vertical slice:

```rust
enum GraphicsProfile {
    Dxvk,
    DgVoodooDxvk,
    D7vkDxvkExperimental,
}
```

El plan resuelto conserva la procedencia concreta:

```rust
enum GraphicsPlan {
    Dxvk {
        modern: DxvkPlan,
    },
    DgVoodooDxvk {
        overlay: DgVoodooPlan,
        modern: DxvkPlan,
    },
    // Sólo después del spike.
    D7vkDxvkExperimental {
        legacy: D7vkPlan,
        modern: DxvkPlan,
    },
}

enum DxvkPlan {
    RunnerBundled { runner_receipt: ArtifactReceipt },
    Managed { artifact: ArtifactRequirement },
    WinetricksLegacy { recipe: RecipeId, provenance: LegacyProvenance },
}
```

La variante D7VK incluye también `modern` porque un cliente puede importar DirectDraw/D3D7 y D3D9
en la misma build. D7VK atiende la familia pre-D3D8; las llamadas D3D8/9/11 directas siguen
necesitando una política explícita. El upstream D7VK reutiliza internamente un backend D3D9 de
DXVK, pero sólo distribuye `ddraw.dll` para esas APIs; no debe confundirse con proveer el
`d3d9.dll` que necesita otra ruta directa del cliente.

**Invariantes codificadas:**

- no `dgVoodoo` y `D7VK` simultáneos para el mismo `ddraw.dll`;
- DXVK managed sólo con el artefacto/arquitecturas declarados;
- runner-bundled no copia DLLs administradas dentro de Proton;
- un plan D7VK no existe sin estrategia para acceder a una implementación DDraw real;
- un profile WineD3D no se agrega hasta que exista un flujo diagnóstico real.

### 10.6 Targets y environment

```rust
enum InvocationTarget {
    Game,
    LaunchPatcher,
    MaintenancePatcher,
    OpenSetup,
    GraphicsControlPanel,
}

struct GraphicsEnvironment {
    dll_overrides: DllOverrideSet,
    variables: EnvironmentDelta,
}
```

`DllOverrideSet` fusiona nombres y orden de carga con reglas explícitas. Renderiza
`WINEDLLOVERRIDES` una vez y rechaza dos políticas incompatibles para la misma DLL.
`EnvironmentDelta` conserva sets/unsets únicos como `RunnerInvocation`, pero detecta conflictos
entre runner y graphics antes de aplicar last-write-wins.

La coherencia no exige que todos los targets reciban bytes idénticos. Exige que cada diferencia
esté codificada en `GraphicsPlan::environment_for(target)` y probada. El patcher usado desde
**Jugar** resuelve `LaunchPatcher`; el abierto desde **Tools**, `MaintenancePatcher`. Dos call sites
con el mismo rol no pueden divergir por caller, y la diferencia entre ambos roles queda visible.

### 10.7 `DependencyPlan`

**Diferido.** No es un tipo de Fase 1. La receta actual es fija y `webview2_required` es la única
variación; sólo una segunda policy real justifica promover esta sección a dominio productivo.

Representa requirements tipados actuales: `vcrun2019`, `d3dx9`, `corefonts`, font fallback,
WebView2 y Gecko. Conserva la receta especial WebView2 como una variante cerrada dependiente de
`RunnerCapabilities`.

No todas las dependencias forman parte del path del prefix. El plan clasifica cada requirement:

- `IsolationCritical`: cambia arquitectura/ABI/ownership y entra a `PrefixFingerprint`;
- `Repairable`: se valida/provisiona transaccionalmente en el mismo prefix y queda como receipt;
- `External`: pertenece al runner o al host y sólo se comprueba.

La clasificación es cerrada y revisada por ADR; no es un booleano configurable por cada feature.

### 10.8 Artefactos y receipts

```rust
struct ArtifactDescriptor {
    id: ArtifactId,
    version: ArtifactVersion,
    source: ArtifactSource,
    expected_size: u64,
    digest: Digest,
    archive: ArchiveLayout,
    architectures: ArchitectureSet,
    payload: PayloadValidatorId,
}

struct ArtifactReceipt {
    schema_version: u32,
    artifact_id: ArtifactId,
    version: ArtifactVersion,
    source_digest: Digest,
    installed_architectures: ArchitectureSet,
    recipe_revision: u32,
}
```

`ArtifactRequirement` apunta a un descriptor conocido; nunca acepta URL/checksum arbitrario desde
`servers.json`. Los validators siguen siendo código cerrado. UMU, Proton y DXVK migran sin cambiar
URL, tamaño, digest ni paths activos.

Los recursos bundled usan `BundledResourceReceipt`; dgVoodoo no se finge descargado ni prefix-local.
Un segundo overlay real puede justificar extraer un motor común de file ownership durante la fase
D7VK, no antes.

### 10.9 Fingerprints

`RuntimeFingerprint` identifica el stack de runtime resuelto:

```text
fingerprint schema
+ runner identity/capabilities/sync
+ graphics profile y artifact receipts
+ dependency recipe/receipts relevantes
+ prefix architecture/recipe
```

Cliente/anti-cheat forman un `SubjectFingerprint` separado y se unen al runtime dentro del record de
compatibilidad/benchmark. El runtime fingerprint sirve para logs, evidence y staleness de la
resolución. No incluye paths personales, credenciales, PID, timestamps de ejecución ni flags de
diagnóstico efímeros.

`PrefixFingerprint` es una proyección más pequeña:

```text
prefix fingerprint schema
+ runner prefix-identity projection (sin sync/config-only)
+ prefix architecture
+ componentes graphics/dependency que escriben estado incompatible dentro del prefix
+ prefix recipe revision
```

No incluye por defecto dgVoodoo porque vive en game dir, ni GPU/driver, ni benchmark options. D7VK
entrará sólo si el deployment elegido modifica el prefix. Esta separación evita proliferación
absurda y mantiene evidencia exacta.

El path administrado futuro se deriva de `server_id + PrefixFingerprint`; el manifest guarda el
digest completo y el algoritmo. Un hash truncado de path nunca basta para adoptar un directorio:
si el manifest completo no coincide, se rechaza.

### 10.10 Compatibilidad

```rust
enum CompatibilityAssessment {
    Validated { evidence_id: EvidenceId },
    Experimental { evidence_id: EvidenceId },
    Incompatible { evidence_id: EvidenceId, reason: FailureClass },
    Unknown,
}
```

Un record de evidencia contiene sujeto exacto (cliente, arquitectura, Gepard/GameGuard conocido),
runtime/profile exacto, artifact receipts, resultado, fecha y alcance. La ausencia no se serializa
como wildcard positivo.

El catálogo shipped y las observaciones locales son cosas distintas:

- el catálogo curated puede recomendar/validar;
- un resultado local se registra como observación hasta ser revisado;
- AutoTune futuro consume sólo candidates `Validated` exactos;
- una incompatibilidad exacta bloquea sólo esa combinación, no todos los runners de una familia.

### 10.11 Ownership y ubicación

| Estado                           | Owner                 | Manifest/receipt                 | Lock                    |
| -------------------------------- | --------------------- | -------------------------------- | ----------------------- |
| Runner/UMU/DXVK descargado       | artifact store global | runtime marker/receipt           | `runtime`               |
| DLLs/deps dentro de prefix       | prefix provisioner    | prefix manifest                  | `prefix` exclusivo      |
| dgVoodoo en game dir             | dgVoodoo overlay      | `.ro-launcher-dgvoodoo.json`     | `dgvoodoo`/game dir     |
| D7VK side-by-side, si se aprueba | graphics overlay      | manifest propio o común v2       | game dir                |
| D7VK system path, si se aprueba  | prefix provisioner    | prefix receipt con backup        | `prefix` exclusivo      |
| Environment de proceso           | invocation factory    | runtime fingerprint/log redacted | operation/session lease |

No se permite que un mismo archivo tenga dos owners. Cualquier collision produce preflight error
antes de mutar.

### 10.12 Abstracciones que no se justifican todavía

- Un DAG libre de APIs y translation layers.
- Plugins dinámicos de graphics backend o artifact validators.
- Un DSL de scripts de instalación.
- Un package manager general.
- Un único manifest que mezcle runtime global, prefix y directorio del juego.
- Hash de todo el árbol de cada runner en cada launch.
- Persistir `RuntimePlan` completo en `servers.json`.
- Crear `WineD3D` sólo para completar un enum sin un flujo de producto.
- Benchmark score único.
- AutoTune antes de que existan candidates exactos y rollback.
- Mover runtime domain a `ro-tools-core` sin un segundo consumidor.

## 11. Persistencia, compatibilidad hacia atrás y migración

La migración debe separar lectura compatible, escritura nueva y selección de paths. Introducir tipos
nuevos no autoriza a reinterpretar metadata vieja como si contuviera evidencia que nunca registró.

### 11.1 Superficies persistidas

| Superficie                   | Estado actual                                                 | Evolución propuesta                                                                      | Regla de compatibilidad                                                                                 |
| ---------------------------- | ------------------------------------------------------------- | ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `servers.json`               | selección de runner, paths y opciones del servidor            | referencia opcional a un `RuntimeProfileId` estable cuando exista UI para elegirlo       | conservar campos actuales y derivar el profile legacy; no persistir paths ni el `RuntimePlan` resuelto  |
| `settings.json`              | runner global y settings operacionales                        | default/profile global opcional y versionado                                             | una selección explícita existente gana; corregir el default sólo para instalaciones sin elección previa |
| `.ro-launcher-prefix.json`   | schema 2, identidad server+runner y `components: Vec<String>` | schema 3 con `PrefixFingerprint`, `RunnerReceipt` y `ComponentReceipt` tipados           | mantener reader v2; no reescribir v2 por el mero hecho de abrirlo                                       |
| markers bajo `runtimes/`     | schema 1, `artifactId` y digest                               | receipt versionado con kind, versión, arquitectura, fuente y digest                      | aceptar markers v1 para artefactos actuales y elevarlos sólo tras validar el payload                    |
| `.ro-launcher-dgvoodoo.json` | schema 2 y backups del game dir                               | permanecer separado; ampliar únicamente si otro overlay comparte de verdad su maquinaria | nunca absorberlo en el manifest del prefix                                                              |
| cache/status frontend        | key derivada de config serializada                            | key derivada del `plan_id` y de la revisión de inputs                                    | durante la transición invalidar ambas keys cuando cambie runner/profile                                 |

Los manifests deben serializar enums con tags explícitos y campos aditivos. No deben usar nombres de
display como identidad. Cada cambio de schema necesita fixtures de lectura de todas las versiones
soportadas y un test de rechazo para versiones futuras desconocidas.

### 11.2 Estrategia de paths y prefixes existentes

La transición recomendada es **legacy alias, no migración física automática**:

1. Resolver `RuntimePlan` y sus fingerprints sin mutar disco.
2. Buscar primero un prefix v3 cuyo path e identidad completa coincidan.
3. Si no existe, buscar el path v2 server+runner que hoy resolvería el launcher.
4. Reutilizarlo sólo si su manifest v2 es válido y corresponde al mismo server, runner kind y path
   canónico requeridos. Marcar su procedencia en memoria como `LegacyV2RunnerMatched`.
5. No afirmar que su DXVK o dependencies coinciden con el plan hasta ejecutar los checks actuales.
   Si coinciden, puede continuar como alias legacy para el profile heredado; no se renombra.
6. Si el nuevo plan es prefix-incompatible con ese alias, crear un prefix v3 distinto mediante el
   rebuild transaccional existente. Mantener el v2 intacto.
7. Un directorio no vacío sin manifest válido sigue siendo desconocido: no se adopta, sobreescribe
   ni elimina.
8. Un custom prefix conserva su path y ownership. Si no se puede demostrar compatibilidad, se pide
   una acción explícita; nunca se lo clona o limpia en segundo plano.

No habrá garbage collection automática en estas fases. Una UI futura podrá listar ambientes no
activos y explicar su identidad, pero la eliminación siempre será explícita y protegida.

Para evitar proliferación, sólo `PrefixFingerprint` participa del nombre del directorio. Cambiar
logging, clasificación de evidencia o una opción que vive únicamente en el game dir no crea otro
prefix. Cambiar runner efectivo, arquitectura, layout WoW64 o un componente instalado dentro del
prefix sí lo hace.

### 11.3 Casos ancla de migración

**SakuraRO validado:** el prefix v2 Wine 7.16 existente permanece en su ubicación. La resolución
debe comprobar Wine 7.16, layout legacy/old-WoW64, componentes efectivos y managed DXVK 2.6.2 antes
de representarlo como el plan validado. Si falta una prueba, el plan es incompleto/unknown; no se
convierte la ausencia en compatibilidad. Un nuevo prefix v3 usa la identidad exacta, pero nunca
reemplaza el v2 hasta que el provisioning haya terminado con éxito.

**HoneyRO validado:** el prefix v2 de Proton-CachyOS 11/UMU continúa como alias cuando el runner
efectivo coincide. El nuevo modelo registra que la implementación de DXVK pertenece al runner; no
instala managed DXVK 2.6.2 encima. Un cambio a otro Proton o a otro graphics plan crea una identidad
nueva sólo si altera material prefix-owned.

**dgVoodoo:** su instalación existente se valida con el manifest del game dir y continúa siendo
reversible independientemente del prefix elegido. Cambiar entre dos prefixes no duplica ni reclama
sus DLLs. Un launch no puede comenzar mientras una operación de install/uninstall sobre ese mismo
game dir esté en curso.

### 11.4 IPC y frontend durante la transición

Los commands actuales deben seguir aceptando payloads legacy. La evolución es aditiva:

- exponer una vista serializable y redacted de `RuntimePlanSummary`, no los tipos internos completos;
- incluir `plan_id`, runner efectivo, graphics profile, versiones/owners y compatibility assessment;
- mantener temporalmente `runner` y flags actuales para clientes frontend de la misma release;
- aceptar opcionalmente `expected_plan_id` en operaciones largas y fallar como stale si el plan se
  volvió a resolver entre preview y ejecución;
- mostrar siempre la decisión efectiva y su razón: server override, default, compatibility
  recommendation o elección explícita;
- no guardar URLs, checksums internos, paths de staging ni secretos dentro de `servers.json`.

La remoción de campos legacy es una decisión posterior, condicionada a que ningún command ni
fixture siga consumiéndolos. No forma parte de la introducción inicial del dominio.

### 11.5 Rollback de la migración

Cada fase que escriba schema nuevo debe poder volver a la release anterior sin destruir datos:

- v3 vive en un path distinto cuando la release anterior no puede entenderlo;
- v2 no se reescribe de forma oportunista;
- los nuevos artifact receipts acompañan al payload pero no invalidan markers v1 comprobables;
- las nuevas preferencias son opcionales y la ausencia reproduce el comportamiento anterior;
- desactivar el nuevo resolver vuelve al adapter legacy siempre que no se haya elegido un profile
  que la versión anterior no pueda ejecutar;
- el rollback nunca elimina el prefix nuevo: sólo deja de seleccionarlo.

## 12. D7VK como prueba del seam, no como centro del diseño

### 12.1 Corrección del modelo conceptual

La abreviatura `DirectDraw/D3D7 -> D7VK -> Vulkan` es útil para producto, pero incompleta para
provisioning. La documentación upstream actual describe D7VK como implementación de las APIs
immediate-mode D3D 3/5/6/7 sobre el backend D3D9 de DXVK; no implementa DirectDraw completo y delega
esas operaciones a una implementación Wine/native disponible. Esto produce dos pipelines distintos:

```text
Perfil dgVoodoo + DXVK
  DDraw/D3D7 del cliente
        -> dgVoodoo en el game dir
        -> D3D11
        -> DXVK seleccionado
        -> Vulkan

Perfil D7VK experimental
  D3D3/5/6/7 immediate-mode vía ddraw.dll
        -> frontend/proxy D7VK
        -> backend D3D9 derivado de DXVK
        -> Vulkan
  Operaciones DirectDraw no implementadas
        -> Wine/native DirectDraw real
```

Por ello, `D7vk` no debe modelarse como un reemplazo universal de DirectDraw ni como un boolean.
Además, un cliente mixto que importe D3D9 directamente puede necesitar una política explícita para
esa ruta aunque D7VK ya contenga/use código de DXVK internamente. El spike debe resolver ownership,
versiones y conflicts antes de cerrar el tipo definitivo.

**Confirmado upstream al 2026-09-16.** El
[README de desarrollo de D7VK](https://github.com/WinterSnowfall/d7vk/blob/devel/README.md) describe
deployment Linux con `ddraw.dll` y override `ddraw=n,b`, una alternativa permanente que conserva
otra implementación DirectDraw para delegación, variables de log propias en releases recientes y
requirements Vulkan diferentes entre la línea principal y Sarek. Son constraints del artefacto,
no prueba de compatibilidad con RO. Como aún no se eligió un release, este planning no fija Vulkan,
variables ni layout a los valores cambiantes de `devel`; el descriptor de fase 6A debe fijarlos.

No se declara ganador entre ambos perfiles. Correctitud visual, compatibilidad con GDI, startup,
stabilidad y frametimes deben medirse con el mismo cliente y máquina antes de comparar rendimiento.

### 12.2 Contrato mínimo de un nuevo graphics backend

Agregar un backend después de la fase 4 debe exigir implementar sólo estos seams:

1. **Identidad:** `GraphicsProfileId`, versión de schema y parámetros canónicos permitidos.
2. **Resolución:** `GraphicsProfile -> GraphicsPlan` con constraints del runner, arquitectura del
   cliente, APIs importadas y capacidades Vulkan.
3. **Artefactos:** descriptors/receipts exactos, checksums, arquitectura, procedencia y policy de
   actualización.
4. **Ownership y layout:** lista de archivos por game dir, prefix o store global; collisions
   comprobadas antes de escribir.
5. **Provisioning:** preflight, staging, instalación, verificación, commit y rollback transaccional.
6. **DLL overrides:** `DllOverrideSet` estructurado, merge con detección de conflictos y target
   explícito.
7. **Config/environment:** variables permitidas, defaults, sanitización AppImage y redaction.
8. **Targets:** comportamiento coherente para game, patcher handoff, patcher manual, OpenSetup y
   panel de control; cualquier diferencia debe ser deliberada y testeada.
9. **Diagnóstico:** versión efectiva, owner de cada DLL, log path, capabilities y motivo de la
   selección sin leer memoria del juego ni tocar anti-cheat.
10. **Persistencia:** receipts en el boundary correcto y contribución explícita a ambos fingerprints.
11. **Uninstall/repair:** restauración de backups, preservación de archivos modificados por el
    usuario y conducta ante partial state.
12. **Concurrencia:** locks/leasing que impidan mutar un game dir o prefix usado por clientes vivos.
13. **Compatibilidad:** assessment exacto; default `Unknown`, nunca promoción por familia.
14. **Packaging:** disponibilidad idéntica en dev, bundle, AppImage instalada y fallback offline.

El launcher/session sólo recibe el `InvocationPlan` resultante. No importa ni conoce el adapter
D7VK, las rutas de DLL o las reglas de instalación.

### 12.3 Spike obligatorio y preguntas de go/no-go

Antes de ofrecer un toggle experimental se debe seleccionar un release exacto y contestar con una
prueba aislada:

- ¿qué arquitectura(s) de `ddraw.dll` requiere cada cliente objetivo?
- ¿la instalación side-by-side junto al ejecutable es suficiente bajo Wine 7.16 old-WoW64 y bajo
  Proton-CachyOS/UMU?
- si se requiere reemplazo dentro de `system32`/`syswow64`, ¿cómo se conserva y verifica el
  DirectDraw real al que D7VK delega sin alterar un prefix existente en uso?
- ¿cómo interactúan `ddraw=n,b`, otros overrides y el casing del filesystem?
- ¿qué Vulkan API/extensions requiere ese release y cómo se reporta un host no apto?
- ¿qué variables/log names corresponden exactamente al release seleccionado?
- ¿qué ocurre en clientes que mezclan DDraw/GDI o abren D3D9 directamente?
- ¿el artefacto upstream ofrece checksum/signature reproducible y un layout estable?
- ¿se puede desinstalar/restaurar después de interruption, crash y upgrade parcial?
- ¿patcher, OpenSetup y game observan el mismo plan donde corresponde?

Un resultado `no-go` no bloquea la arquitectura: demuestra que el seam permite rechazar un backend
por constraints explícitas. No se agregará un profile persistible hasta resolver estas preguntas.

### 12.4 Go/no-go mínimo

Se habilita la fase experimental sólo si:

- el artefacto exacto y su licencia/procedencia pueden fijarse y verificarse;
- existe una estrategia de deployment reversible para ambos anchors de runner o el profile declara
  claramente el subconjunto soportado;
- no se necesita modificar, hookear ni evadir Gepard/GameGuard;
- el resolver puede rechazar hosts, clientes y targets incompatibles antes de mutar;
- la restauración sobre interruption y el conflicto con dgVoodoo están cubiertos por tests;
- un runtime smoke test reproduce al menos startup, render correcto y cleanup.

Si una restricción limita D7VK a un solo runner/layout, eso se representa como constraint y no como
un `if server == ...` dentro de launcher.

## 13. Fases de trabajo

Las fases son mergeables por separado. Las flags de transición mencionadas aquí son internas y
temporales; no reemplazan tests ni deben convertirse en preferencias permanentes sin caso de uso.

| Orden | Fase                      | Cambio de autoridad                                    |
| ----- | ------------------------- | ------------------------------------------------------ |
| 0     | Characterization + ADRs   | ninguno; congela y prueba conducta actual              |
| 1     | Dominio y resolver shadow | nuevo plan sólo observa/compara                        |
| 2     | Fingerprints/prefix v3    | identidad nueva con alias v2 conservador               |
| 3     | Catálogo de artefactos    | mismo payload activo bajo descriptors/receipts tipados |
| 4     | DXVK/dgVoodoo vertical    | `GraphicsPlan` pasa a ser autoridad operacional        |
| 5     | Compatibilidad exacta     | assessments se ligan al runtime verificable            |
| 6A    | Spike D7VK                | no productivo; decide deployment y scope               |
| 6B    | D7VK experimental         | primer backend nuevo, sólo con go explícito            |
| 7     | Observabilidad            | outcomes locales unidos a fingerprint                  |
| 8     | Benchmark A/B             | comparación opt-in, sin selección automática           |
| 9     | AutoTune                  | candidato futuro, exacto y conservador                 |

El orden de integración es lineal. La implementación interna de fase 3 puede prepararse en paralelo
con fase 2, pero fase 4 no se integra hasta que ambas tengan salida verificable.

### Fase 0 — Characterization, decisiones y baseline corregible

**Objetivo.** Congelar el comportamiento real, cerrar los ADR bloqueantes y convertir discrepancias
con el contrato en issues reproducibles antes de mover ownership.

**Motivación.** No se puede demostrar una migración sin tests que describan la resolución actual.
Además, `AppSettings::default()` parte de `default_system_wine()` y el frontend puede conservar ese
valor cuando está disponible. Eso no demuestra en todos los flujos el default administrado
Proton-CachyOS exigido por `AGENTS.md`; es una discrepancia confirmada que debe resolverse sin
sobrescribir selecciones explícitas existentes.

**Cambios previstos.** Tests/fixtures en `utils/runner.rs`, `utils/prefix.rs`,
`tools/prefix/setup.rs`, `tools/launcher/session.rs`, `tools/server_tools/*`, modelos/settings y
frontend runner resolution. ADR-001 y ADR-002. Un pequeño inventario fixture de los dos anchors
Gepard, sin inventar hashes de cliente.

**Entregables.**

- [x] Test de precedencia server override > elección global > default administrado.
- [x] Test que conserva una elección explícita de system/custom Wine después de upgrade.
- [x] Test de decisión `Runner/Managed/Winetricks` de `DxvkProvision` vigente.
- [x] Fixtures de manifests prefix v2, runtime schema 1 y dgVoodoo schema 2.
- [x] Tests de env para game, patcher handoff, patcher manual y OpenSetup tal como funcionan hoy.
- [x] Matriz documentada de componentes efectivos de SakuraRO y HoneyRO.
- [x] ADR-001 y ADR-002 aceptados.

**No entra.** Tipos productivos nuevos, nuevos paths, schema writes, D7VK ni corrección simultánea de
cualquier otro default histórico.

**Compatibilidad/rollback.** Characterization es aditiva. Si se corrige el default, se aplica sólo
cuando no existe una selección explícita persistida; rollback restaura el resolver anterior sin
tocar prefixes.

**Tests y validación.** Tests Rust y frontend focalizados, suites completas, build y smoke
`tauri:dev` de default Proton y override Wine 7.16. Registrar runner/prefix/env efectivos, no sólo el
label UI.

**Adversarial review.** Intentar que un settings viejo sea interpretado como “sin elección”, que un
server override vacío gane, que una recomendación Gepard sobrescriba una elección explícita o que
el patcher manual reciba accidentalmente los overrides del handoff.

**Criterio de salida.** Cada rama de resolución actual tiene un test y los dos ADR bloqueantes
definen representaciones cerradas y migración legacy.

**Dependencias.** Ninguna.

**Estado de implementación (2026-09-17).** `SettingsDocumentOrigin` distingue `Missing`,
`Persisted` y `Recovered` bajo el lock de settings. Sólo `Missing` recibe
`managed_proton_path()` como default efectivo, sin escribir `settings.json`; los valores
persistidos o recuperados se conservan exactamente aunque discovery no los encuentre. La
precedencia server/global/product default, la policy legacy de DXVK, los manifests existentes,
los cinco targets y el comportamiento editable/seguro de dgVoodoo quedaron congelados por tests y
fixtures compartidos.

La validación manual de primera instalación, override Wine 7.16 y settings system/custom
persistidos continúa pendiente junto con la matriz de clientes reales indicada al final de Fase 1.

### Fase 1 — Dominio tipado y resolver en shadow mode

**Objetivo.** Introducir `RuntimeProfile`, `RunnerPlan`, `GraphicsPlan` y el `RuntimePlan` mínimo de
ADR-001 sin cambiar provisioning, paths ni environment efectivo.

**Motivación.** Crear un único lugar que explique la decisión antes de delegarle mutaciones. El
segundo consumidor real ya existe: `Dxvk` y `DgVoodooDxvk`.

**Cambios previstos.** Nuevo módulo `src-tauri/src/tools/runtime/`, adapters desde `Server`,
`ResolvedRunner`, prefix y status dgVoodoo ya calculado. `GraphicsProfile` sólo con los perfiles
activos. No hay IPC nuevo. Los consumers legacy siguen siendo la autoridad operativa.

**Entregables.**

- [x] Tipos cerrados, separados por intent y plan resuelto.
- [x] Resolver puro con razones y constraints estructuradas.
- [x] Adapter que calcula el plan en paralelo sin ejecutar acciones.
- [x] Comparador shadow entre decisiones legacy y plan nuevo.
- [x] Telemetría/log local redacted de divergencias, sin “autocorregirlas”.

**No entra.** Cambiar `apply_game_env`, prefix manifests, artifact manager, server config ni UI de
selección de profiles.

**Compatibilidad/rollback.** Una flag interna permite omitir por completo shadow resolution. Una
divergencia falla el test/diagnóstico, no el launch del usuario.

**Tests y validación.** Table tests de combinaciones runner/profile/target, merge de DLL/env y
comparación runtime con los dos anchors y con un Gepard desconocido. Golden hashes comienzan en
Fase 2.

**Adversarial review.** Intentar construir dgVoodoo y D7VK simultáneos (D7VK aún no existe), managed
DXVK sobre Proton, NTSync con Wine 7.16, graphics plan sin owner o un `Validated` sin evidence id.
Verificar que los constructores públicos no permitan esos estados.

**Criterio de salida.** Para el corpus actual, shadow y legacy producen runner, DXVK strategy,
dgVoodoo decision, targets y env equivalentes; toda divergencia conocida está clasificada.

**Dependencias.** Fase 0.

**Estado de implementación (2026-09-17).** El módulo `tools/runtime/` ya contiene el dominio
cerrado, probes `Known`/`Unknown`, resolución pura, environments estructurados para los cinco
targets y merge conflictivo de DLL/environment. Los adapters shadow observan launch, setup/reset,
deps y server tools; comparan contra las decisiones legacy, registran sólo categorías y tokens
redacted, y pueden deshabilitarse con `RO_LAUNCHER_RUNTIME_SHADOW=0`. Ningún valor del plan nuevo se
aplica a provisioning, paths, environment efectivo, spawn, IPC, schemas o persistencia.

Pasaron los gates completos de frontend y Rust: lint, format check, 109 tests frontend, build,
`cargo fmt`, clippy con warnings como error y 345 tests Rust (dos pruebas manuales/hardware
ignoradas). `npm run tauri:dev` arrancó correctamente con shadow habilitado y deshabilitado.

El criterio de salida queda **pendiente de validación runtime** con clientes reales: anchors Sakura
y Honey, primera instalación/default Proton, settings system/custom persistidos, override Wine
7.16, launch directo, patcher handoff/manual, OpenSetup, multi-client y AppImage instalada. El
arranque `tauri:dev` no sustituye esos casos.

### Fase 2 — Fingerprints e identidad de prefix compatible

**Objetivo.** Hacer explícitas identidad de runtime y de prefix sin mover ni reinterpretar prefixes
existentes.

**Motivación.** Server+runner no distingue layout, versión efectiva ni componentes prefix-owned;
usar el fingerprint completo para paths, en cambio, duplicaría ambientes por cambios irrelevantes.

**Cambios previstos.** `RuntimeFingerprint`, `PrefixFingerprint`, manifest v3, receipts tipados,
legacy alias resolver y `plan_id` en session registry/status. `utils/prefix.rs` conserva helpers de
seguridad y agrega path v3; `tools/prefix/` escribe v3 sólo para prefixes nuevos.

**Entregables.**

- [x] Canonical encoding versionado (goldens en `tools/runtime/encode.rs` y `fingerprint.rs`).
- [x] Collision check del digest truncado del path (`digests_share_v3_path_suffix`, binding `Incompatible`).
- [x] Reader v2/v3 con estados `V3Verified`, `LegacyV2RunnerMatched`, `Unknown` e `Incompatible`.
- [x] Algoritmo legacy alias según §11.2 (`resolve_prefix_binding` + flag `RO_LAUNCHER_PREFIX_V3`).
- [x] Nuevo path v3 sólo para material prefix-owned distinto (preserva v2).
- [x] Session anchor con `plan_id` en registry (rechazo bajo lease); digest de runtime **placeholder** hasta facts dgVoodoo.
- [x] `explain_identity_delta` en `fingerprint.rs` (diagnóstico de campos).

**No entra.** Migrar/renombrar v2, cleanup automático, elegir D7VK, mover dgVoodoo al prefix ni
rehacer un custom prefix.

**Compatibilidad/rollback.** Read old/write new. Los v2 permanecen seleccionables para el profile
heredado. Ante error de provisioning v3, se conserva el viejo y se restaura el backup transaccional.

**Tests y validación.** Golden tests de canonical encoding; fixtures v2/v3/future-version; path
collision; directorio no vacío desconocido; failed rebuild; custom prefix; dos graphics profiles que
sólo difieren en game-dir overlay comparten prefix; un componente prefix-owned distinto no comparte.
Smoke de upgrade real usando copias no sensibles de prefixes fixture.

**Adversarial review.** Truncar el digest para forzar collision, cambiar casing/canonical path,
simular manifest escrito a medias, runner reemplazado en el mismo path, active lease y rollback
interrumpido. Confirmar que ningún caso adopta o elimina datos.

**Criterio de salida.** Una release puede crear/usar v3, seguir usando v2 válido y rechazar estado
ambiguo sin pérdida ni migración automática.

**Dependencias.** Fase 1 y ADR-002.

**Estado de implementación (2026-09-17).** Commit local `aaf5836`: encoder y `PrefixFingerprint`
operacionales; `StoredPrefixManifest` v2/v3; path `*-v3-*`; writer v3 en setup; `WineContext` con
`identity`/`probe`; `commands/prefix` no rearmar por schema futuro; contrato en
[`RO_RUNTIME_PHASE_2_CONTRACT.md`](RO_RUNTIME_PHASE_2_CONTRACT.md). Shadow de gráficos/env sin
cambio de autoridad de spawn.

Pasaron: `npm run lint`, `npm test`, `npm run build`, `cargo fmt --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`cargo test --workspace --all-features`. No se ejecutó en esta pasada: `npm run format:check`,
`npm run tauri:dev`, matriz runtime Sakura/Honey/AppImage/multi-client.

Pendiente antes de declarar Fase 2 cerrada en producción: `compute_runtime_fingerprint` completo
post-scan dgVoodoo; smokes del plan §12; tests de conflicto `plan_id`; goldens de
`RuntimeFingerprint`. Un binario anterior ante prefixes schema 3 en disco los verá incompatibles
sin borrarlos (mismo commit que introduce reader v3).

### Fase 3 — Catálogo de artefactos y receipts tipados

**Objetivo.** Extraer la generalidad ya presente en `managed.rs` sin cambiar los artefactos activos.

**Motivación.** `Artifact` ya modela URL, tamaño, digest, archive root y payload, pero su identidad y
readiness son todavía especializadas. D7VK no debe producir un segundo downloader ad hoc.

**Cambios previstos.** Separar descriptor, fetch/verify, safe extraction, installer y receipt dentro
de `tools/runners/managed.rs` o `tools/artifacts/`. Adaptar UMU, Proton-CachyOS y DXVK 2.6.2; mantener
dgVoodoo bundled en su boundary.

**Entregables.**

- [ ] `ArtifactDescriptor`/`ArtifactReceipt` tipados por kind, versión y arquitectura.
- [ ] Validación común de URL allowlist, tamaño, checksum, archive traversal y symlinks.
- [ ] Staging/commit/restore común con ownership explícito.
- [ ] Adapters que conservan IDs, URLs, digests y paths actuales.
- [ ] Lectura compatible de runtime markers schema 1.
- [ ] Fault injection en download, extract, validation, rename y restore.

**No entra.** Actualizar versiones, incorporar DXVK 3, D7VK, mirrors dinámicos, firma remota, package
manager ni mover recursos bundled.

**Compatibilidad/rollback.** Los paths e IDs actuales no cambian. El adapter anterior puede
reinstalar un payload cuyo nuevo receipt no comprenda, pero no borra uno válido. Fallo de upgrade
restaura exactamente el directorio previo.

**Tests y validación.** Tests de digest/tamaño, traversal, symlinks, wrong root, partial marker,
concurrent install, cancellation y repeated cycles; smoke offline con cache válida y online en
build dev/AppImage cuando aplique.

**Adversarial review.** Usar dos artifacts con mismo display/version pero digest distinto, payload
válido con marker ajeno, extraction con link fuera de staging, error durante restore y AppImage env
contaminado. Verificar que “archivos presentes” nunca oculte status no cero.

**Criterio de salida.** Los tres artefactos actuales pasan byte-for-byte por el nuevo catálogo y un
fallo en cualquier frontera conserva el payload previamente válido.

**Dependencias.** Fase 1. Puede desarrollarse después o en paralelo técnico con fase 2, pero se
integra sin mezclar sus migraciones.

### Fase 4 — Migración vertical de DXVK y dgVoodoo al `GraphicsPlan`

**Objetivo.** Convertir el plan tipado en autoridad operacional para los perfiles existentes y sacar
la lógica gráfica de launcher/session.

**Motivación.** Ésta es la consolidación que D7VK debe consumir. Hasta aquí el nuevo modelo sólo
observa; aquí reemplaza booleans duplicados por una decisión resuelta.

**Cambios previstos.** Graphics adapters/provisioners, `InvocationPlan` por target, merge
estructurado de overrides y env. `prefix/setup.rs`, deps/check, launcher/session y server tools
consumen el mismo plan. `dgvoodoo.rs` conserva manifest/transaction/ownership. `apply_game_env` se
reduce a ejecutar un environment delta o queda como shim temporal.

**Entregables.**

- [ ] Adapter `DxvkPlan::{RunnerBundled, Managed, WinetricksLegacy}` con paridad actual.
- [ ] Adapter `DgVoodooDxvk` que compone overlay game-dir y DXVK sin mezclar manifests.
- [ ] `InvocationTarget` explícito para game, launch patcher, maintenance patcher, OpenSetup y
      control panel.
- [ ] Conflict detector de `WINEDLLOVERRIDES` y variables incompatibles.
- [ ] Dependency/status UI derivados del mismo `RuntimePlanSummary`.
- [ ] Eliminación de branches gráficos de `launcher/session` salvo invocar el adapter.
- [ ] Shim de config legacy con deprecation medible y fecha de retiro.

**No entra.** D7VK, nuevos versions de DXVK, cambiar dónde vive dgVoodoo, AutoTune ni rediseñar la
UI completa.

**Compatibilidad/rollback.** Feature flag de una release permite volver al env builder legacy. No se
reescriben manifests dgVoodoo. Los prefixes existentes usan las mismas DLLs/strategies y el mismo
orden de provisioning.

**Tests y validación.** Equivalence tests de env legacy/nuevo por target; conflicts; Wine 7.16 +
managed DXVK 2.6.2; Proton runner-owned; generic Wine winetricks; dgVoodoo install/repair/uninstall;
patcher handoff; manual patcher; OpenSetup. Full gates, `tauri:dev`, multi-client y build AppImage.

**Adversarial review.** Intentar doble owner de `d3d9.dll`, hacer que dgVoodoo quede activo pero DXVK
no, cambiar profile con dos clientes vivos, fallar después de backup en game dir, heredar
`WINEDLLOVERRIDES` conflictivo desde AppImage y lanzar setup con env distinto al game.

**Criterio de salida.** Los perfiles actuales se ejecutan únicamente desde un `RuntimePlan`; no hay
`use_dgvoodoo`/`use_managed_dxvk` fuera de adapters legacy y launcher/session no decide gráficos.

**Dependencias.** Fases 1 y 3; fase 2 para identidad durable antes de writes nuevos.

### Fase 5 — Catálogo de compatibilidad basado en evidencia exacta

**Objetivo.** Evolucionar las dos reglas Gepard actuales a assessments exactos sobre runtime plans,
sin ampliar las afirmaciones que hoy pueden demostrarse.

**Motivación.** “Wine 7.16” o “Proton” son categorías demasiado amplias para representar layout,
graphics y artefactos validados. Un hash desconocido debe seguir siendo `Unknown`.

**Cambios previstos.** Nuevo módulo de compatibility records, adapter desde `gepard.rs`, identifiers
de client/build opcionales hasta disponer de evidencia, y UI de razón/estado. Separar catálogo
shipped de observaciones locales.

**Entregables.**

- [ ] Schema versionado con sujeto, runtime fingerprint/profile, outcome, provenance y fecha.
- [ ] Migración conservadora de Sakura: hash Gepard exacto, Wine 7.16 old-WoW64 y DXVK 2.6.2.
- [ ] Migración conservadora de Honey: hash Gepard exacto y Proton-CachyOS 11 efectivo.
- [ ] `Unknown` por ausencia, campos incompletos o artifact/layout diferente.
- [ ] Recomendaciones separadas de restricciones; ninguna fuerza silenciosamente un runner.
- [ ] Observaciones locales no promovidas automáticamente a catálogo curated.

**No entra.** Crowdsourcing, backend remoto, compatibilidad por nombre de server, fuzzy matching de
hash, modificación de anti-cheat, benchmarks ni AutoTune.

**Compatibilidad/rollback.** El adapter legacy puede seguir mostrando las dos recomendaciones. Si el
nuevo catálogo no puede construir un assessment exacto, degrada a `Unknown` y conserva la elección
del usuario; nunca degrada seguridad para mantener un badge verde.

**Tests y validación.** Exact/mismatch tests para cada dimensión; missing client hash; unknown Gepard;
mismo Gepard con runner/layout distinto; serialized fixtures; runtime smoke de ambos anchors y
revisión manual de las versiones/hashes efectivos.

**Adversarial review.** Quitar un campo del record, cambiar sólo un digest, usar otro Wine 7.16,
otro Proton 11 o nuevo WoW64, falsificar el label UI manteniendo otro binario y observar si algo se
marca `Validated`. Debe resultar `Unknown` o `Incompatible` sólo cuando exista evidencia negativa.

**Criterio de salida.** Cada badge/recomendación puede explicar el record exacto que lo respalda y
ningún wildcard positivo amplía los dos hechos actualmente validados.

**Dependencias.** Fases 2 y 4; ADR-004.

### Fase 6A — Spike técnico D7VK y decisión de deployment

**Objetivo.** Responder las preguntas de §12.3 con artefacto y clientes controlados, sin exponer una
opción persistente al usuario.

**Motivación.** D7VK tiene constraints de DirectDraw, GDI, Vulkan, arquitectura y deployment que no
deben suponerse desde el nombre de la feature.

**Cambios previstos.** Harness descartable bajo tests/fixtures o scripts de diagnóstico no
instalados; descriptor provisional de un release exacto; informe/ADR-005 con evidencia. No se toca
el launcher productivo salvo instrumentation ya prevista.

**Entregables.**

- [ ] Release, source URL, license, digest, tamaños y arquitecturas registrados.
- [ ] A/B aislado contra dgVoodoo+DXVK con mismos cliente, runner y host.
- [ ] Resultados separados para Wine 7.16 old-WoW64 y Proton/UMU que sean técnicamente aplicables.
- [ ] Prueba de rutas DDraw/D3D7, GDI mixta y D3D9 directo si el cliente las usa.
- [ ] Prototipo de install/restore con interruption injection.
- [ ] Decisión side-by-side versus prefix-owned y contribución a `PrefixFingerprint`.
- [ ] ADR-005 go/no-go con limitaciones explícitas.

**No entra.** Toggle UI, catálogo curated `Validated`, descarga automática productiva, cambios por
server, optimización o anuncio de superioridad.

**Compatibilidad/rollback.** Usar game dir/prefixes de prueba separados. No tocar entornos del
usuario ni reutilizar un prefix entre runners. El harness debe restaurar sus propios fixtures.

**Tests y validación.** Startup/exit, render screenshots/manual visual correctness, logs, repeated
cycles, cancellation, uninstall y checks de archivos/manifest. Registrar GPU/vendor/driver/Vulkan,
runner hash/layout, client/Gepard hashes y profile exacto.

**Adversarial review.** Buscar éxito aparente con pantalla corrupta, fallback silencioso a WineD3D,
uso de una DLL del runner en vez de D7VK, GDI roto, stale DLL tras uninstall, y diferencias entre
dev y AppImage. Confirmar el pipeline con logs y archivos efectivos.

**Criterio de salida.** ADR-005 responde cada pregunta de §12.3 y decide go, scope reducido o no-go
con evidencia reproducible. Un no-go es salida válida.

**Dependencias.** Fases 3, 4 y 5.

### Fase 6B — D7VK experimental como vertical slice

**Objetivo.** Si ADR-005 es go, agregar D7VK como profile opt-in y demostrar que el seam no requiere
ramificar medio launcher.

**Motivación.** Primer consumidor nuevo de la arquitectura consolidada.

**Cambios previstos.** Un nuevo variant cerrado de graphics profile/plan, descriptor artifact,
provisioner/overlay según ADR, overrides, diagnostics y compatibility constraints. UI claramente
experimental. Launcher/session sólo observa un nuevo `plan_id`.

**Entregables.**

- [ ] Variant D7VK con parámetros limitados al release aprobado.
- [ ] Provisioning y rollback transaccional en el owner correcto.
- [ ] Delegación segura al DirectDraw real y conflict detection con dgVoodoo.
- [ ] Target environments y logs version-specific.
- [ ] Manifest/receipt, status, repair y uninstall.
- [ ] Opt-in con advertencia `Experimental`; jamás default por `Unknown`.
- [ ] Runtime/packaging matrix del scope aprobado.

**No entra.** Auto-selection, múltiples releases configurables, UI de graph composition, catálogo
remoto ni eliminación del perfil dgVoodoo.

**Compatibilidad/rollback.** D7VK y dgVoodoo son profiles mutuamente excluyentes. Volver al profile
anterior restaura el overlay/prefix exacto sin borrar user data. Feature flag retira el profile sin
invalidar los actuales.

**Tests y validación.** Contract tests de §12.2, fault injection, multi-client lock, DDraw delegation,
mixed D3D9 si aplica, visual correctness, AppImage instalada y comparación con baseline. Full Rust
y frontend gates más smoke real.

**Adversarial review.** Añadir el variant y revisar el diff: si requiere branches D7VK en session,
commands, prefix setup y cada tool, el seam falló y no se mergea. Probar corrupt receipt, profile
switch activo, unknown Gepard y host Vulkan insuficiente.

**Criterio de salida.** D7VK se agrega mediante un adapter gráfico, un artefacto y UI/diagnóstico
acotados; no modifica la semántica de DXVK/dgVoodoo ni se selecciona automáticamente.

**Dependencias.** Fase 6A con go explícito y todas las fases 1–5.

### Fase 7 — Observabilidad reproducible y registro de resultados

**Objetivo.** Registrar por qué se eligió un plan y cómo terminó una ejecución sin capturar datos
sensibles ni confundir observación con compatibilidad validada.

**Motivación.** Benchmarks y soporte necesitan unir el resultado al runtime exacto; hoy logs y
status no constituyen un evidence record reproducible.

**Cambios previstos.** `RuntimeObservation` local, redacted diagnostic bundle y outcome taxonomy.
Integración con session lifecycle y process identity; storage separado de config y catálogo curated.

**Entregables.**

- [ ] Record con client/build, server pseudonymous/local id, runner, graphics, receipts, GPU/driver,
      plan id, timestamps y outcome.
- [ ] Outcomes separados: startup, clean exit, crash confirmado, timeout, visual check pendiente y
      user stop.
- [ ] Redaction tests para args, tokens, paths sensibles, memory y environments.
- [ ] Retention/export/delete explícitos.
- [ ] Correlación con `(pid,start_time)` y ro-sessiond sin alterar su protocolo de memoria.

**No entra.** Upload automático, score, sampling de memoria del juego, benchmark harness ni promoción
automática a `Validated`.

**Compatibilidad/rollback.** Feature desactivable; fallo de storage nunca bloquea launch. Los records
son append-only/atomic y se pueden borrar sin tocar servers/prefixes.

**Tests y validación.** Crash versus error exit, PID reuse, two clients, launcher exit, ro-sessiond
disabled fallback, disk full/corrupt record y AppImage sanitation. Revisar manualmente un export.

**Adversarial review.** Intentar filtrar command-line credentials, asociar exit del PID reutilizado,
atribuir crash de otro client, convertir timeout en incompatibilidad o hacer que telemetry failure
termine el juego.

**Criterio de salida.** Un run puede explicarse con identidad exacta y outcome confiable; eliminar
observaciones no afecta la capacidad de lanzar.

**Dependencias.** Fases 2, 4 y 5.

### Fase 8 — Harness A/B de benchmarks controlados

**Objetivo.** Comparar profiles conocidos bajo un protocolo repetible, sin producir aún decisiones
automáticas.

**Motivación.** FPS promedio aislado oculta stutter, fallos de startup y errores visuales.

**Cambios previstos.** ADR-006, run specification, capture adapters permitidos y result schema.
Separar launcher benchmark de métricas in-game que requieran cooperación explícita/segura.

**Entregables.**

- [ ] Protocolo que fija cliente, mapa/escena, duración, warm-up, runner, profile, GPU/driver y carga.
- [ ] Métricas de frametime p50/p95/p99, 1%/0.1% lows, startup reliability y crashes.
- [ ] Visual correctness/compatibility como gates, no como número dentro de un score.
- [ ] Repeticiones, variance e invalidación de runs no comparables.
- [ ] Export de resultados vinculados a runtime fingerprint.

**No entra.** Ranking global, benchmark en background sin consentimiento, AutoTune ni afirmar
causalidad entre hosts distintos.

**Compatibilidad/rollback.** Herramienta opt-in y separada del launch normal. No muta profiles ni
compatibility catalog.

**Tests y validación.** Datasets sintéticos para percentiles/variance; missing samples; clock jumps;
crash parcial; A/B order bias; smoke real repetido. Validar que un run visualmente incorrecto queda
invalidado aunque tenga mejores FPS.

**Adversarial review.** Cambiar driver, thermal state, resolución o scene entre A/B y comprobar que
el harness rehúsa compararlos. Intentar premiar un profile con startup failures o frames omitidos.

**Criterio de salida.** Dos profiles pueden compararse con protocolo y provenance completos, y el
resultado no se reduce a un score opaco.

**Dependencias.** Fase 7 y ADR-006.

### Fase 9 — AutoTune conservador basado en candidatos conocidos

**Objetivo.** Seleccionar sólo entre planes exactos previamente validados y reversibles.

**Motivación.** Automatizar después, no antes, de disponer de constraints, evidencia y medición.

**Cambios previstos.** Candidate generator desde compatibility catalog, policy/ranking explicable,
preview/consent y rollback al último plan estable.

**Entregables.**

- [ ] Candidatos limitados a assessments `Validated` exactos para el sujeto/host.
- [ ] Compatibility y visual correctness como hard gates antes de rendimiento.
- [ ] Explicación de selección y evidencia usada.
- [ ] Opt-in, dry-run y pin manual que siempre gana.
- [ ] Rollback automático ante startup failure, sin borrar el runtime fallido.

**No entra.** Explorar combinaciones unknown en entornos del usuario, entrenamiento remoto, bypass de
anti-cheat, score universal ni cambio silencioso de runner.

**Compatibilidad/rollback.** Default desactivado. La selección manual o server override tiene
precedencia. Desactivar AutoTune restaura el profile fijado previamente.

**Tests y validación.** Empty/one/many candidate, stale evidence, host mismatch, benchmark variance,
startup failure, explicit pin, no network y rollback con clientes concurrentes.

**Adversarial review.** Alimentar evidencia incompleta, resultados de otro GPU, hashes casi iguales,
un profile más rápido pero incompatible y un candidate cuyo artifact ya no está disponible. Ninguno
debe seleccionarse silenciosamente.

**Criterio de salida.** Toda decisión automática puede reproducirse y explicarse; ante duda el
sistema conserva el plan conocido/manual, no experimenta.

**Dependencias.** Fases 5, 7 y 8; no tiene fecha hasta que el corpus de evidencia lo justifique.

### Línea futura — `nndsk-wine-ro`

Un runner mantenido para Ragnarok sigue siendo plausible si la evidencia muestra una brecha estable
entre legacy compatibility y mejoras modernas. No es una fase temprana ni un requisito de D7VK.
Primero debe poder expresarse como otro `RunnerArtifact` con capabilities, layout, provenance y
compatibility records normales. Su investigación necesita threat model, mantenimiento de patches,
CI de builds reproducibles y una matriz separada old/new WoW64. No se diseñará una distribución Wine
hasta que perfiles y observaciones demuestren qué problema exacto no resuelven los runners actuales.

## 14. Matrices mínimas de validación

Las matrices no reemplazan los casos de failure injection de cada fase. Cada fila de runtime debe
registrar paths/versiones/hashes efectivos, prefix y manifest, environment relevante, graphics
owner, resultado y cleanup.

### 14.1 Anchors de compatibilidad durante toda migración

| Sujeto                                         | Runner esperado                     | Graphics esperado                                           | Prefix/layout                        | Resultado obligatorio                                                          |
| ---------------------------------------------- | ----------------------------------- | ----------------------------------------------------------- | ------------------------------------ | ------------------------------------------------------------------------------ |
| SakuraRO + Gepard `26.9.3.1` validado por hash | portable Wine 7.16 exacto           | managed DXVK 2.6.2; dgVoodoo sólo si el server lo configura | aislado, legacy/old-WoW64 comprobado | sigue lanzando; FSYNC/ESYNC sólo si el artefacto declara patches; nunca NTSync |
| HoneyRO + Gepard `26.8.26.1` validado por hash | managed Proton-CachyOS 11 vía UMU   | DXVK runner-owned                                           | aislado por pairing/identity         | sigue lanzando sin superponer managed DXVK                                     |
| Gepard hash desconocido                        | selección explícita/default vigente | profile seleccionado con constraints                        | identidad correspondiente            | warning/`Unknown`; no fuerza runner/profile                                    |
| Custom prefix válido                           | runner elegido por usuario          | sólo operaciones explícitamente seguras                     | path user-owned                      | no rebuild/delete/adoption automática                                          |
| Prefix no vacío sin manifest                   | ninguno asumido                     | ninguno asumido                                             | unknown                              | rechazo seguro y diagnóstico                                                   |

### 14.2 Targets y entorno gráfico

| Target                         | Plan base                        | Expectativa                                                            |
| ------------------------------ | -------------------------------- | ---------------------------------------------------------------------- |
| Game directo                   | plan resuelto                    | runner, prefix, overrides, logs y artifacts del mismo `plan_id`        |
| Patcher como handoff de launch | plan resuelto                    | mismo environment capaz de llegar al game sin divergence               |
| Patcher abierto como tool      | policy explícita                 | no hereda accidentalmente overrides sólo por compartir executable/path |
| OpenSetup                      | plan resuelto para configuración | ve el backend que el game usará; no escribe config para otro pipeline  |
| dgVoodoo CPL                   | `DgVoodooDxvk`                   | accede al overlay correcto sin reclamar ownership del prefix           |

La policy exacta del patcher manual se conserva en fase 0 y se modifica sólo mediante requisito
explícito. “Coherente” no significa aplicar ciegamente todos los overrides a todos los procesos.

### 14.3 Lifecycle y packaging

Para cualquier fase que cambie provisioning, env o process ownership:

- direct launch y patcher handoff;
- stop durante resolución, download, prefix setup y launch;
- dos clientes en el mismo prefix y en prefixes diferentes;
- cierre del launcher con clientes vivos;
- profile switch mientras existe lease;
- ciclos repetidos sin zombies ni locks/dirs temporales abandonados;
- `ro-sessiond` activo por defecto y rollback con `RO_LAUNCHER_SESSION_SUPERVISOR=0`;
- `ptrace_scope=1` como acceptance environment para memory features;
- dev, bundle, AppImage e AppImage instalada con variables heredadas conflictivas;
- artifact cache presente, ausente, corrupta y offline.

## 15. Matriz de riesgos y failure modes

| Riesgo                                             | Prob.      | Impacto    | Prevención/detección                                      | Rollback/contención                                     |
| -------------------------------------------------- | ---------- | ---------- | --------------------------------------------------------- | ------------------------------------------------------- |
| Romper prefixes existentes                         | Media      | Crítico    | read-old/write-new, alias v2, fixtures y smoke de upgrade | seleccionar v2 anterior; nunca renombrarlo/eliminarlo   |
| Duplicar environments por cambios irrelevantes     | Media      | Alto       | separar runtime y prefix fingerprints; explanation diff   | reutilizar identidad prefix; cleanup sólo manual        |
| Interpretar manifest viejo como evidencia completa | Alta       | Alto       | estado `LegacyV2RunnerMatched`; revalidar componentes     | degradar a `Unknown`, conservar datos                   |
| Colisión de digest/path                            | Baja       | Alto       | full digest en manifest y comparación al abrir            | rechazar path; crear identidad no colisionante          |
| Proliferación de profiles/parameters               | Media      | Alto       | enum cerrado y versiones curadas                          | deprecar profile sin reinterpretar su ID                |
| Divergencia game/patcher/OpenSetup                 | Alta       | Alto       | `InvocationTarget` desde un mismo plan; equivalence tests | volver al adapter legacy y bloquear profile conflictivo |
| Conflictos de DLL overrides                        | Alta       | Alto       | merge estructurado, owner único y preflight               | no lanzar; restaurar último plan conocido               |
| Tratar dgVoodoo como prefix-only                   | Media      | Crítico    | manifest/lock del game dir separado                       | restaurar backups del overlay; prefix intacto           |
| Instalar managed DXVK sobre DXVK del runner        | Media      | Alto       | variant owner `RunnerBundled` mutuamente excluyente       | repair/rebuild transaccional del prefix afectado        |
| Winetricks DXVK no reproducible                    | Alta       | Medio/Alto | clasificarlo `LegacyUnpinned`, registrar receipt posible  | no elevarlo a validated; mantener conducta legacy       |
| Wine 7.16 sin old-WoW64 real                       | Media      | Crítico    | capability/layout inspection, no version string sola      | `Unknown`/rechazo para profile Sakura                   |
| Confundir new WoW64 y old-WoW64                    | Media      | Alto       | capability enum y fingerprint                             | prefix separado; no migración cruzada                   |
| Romper WebView2 workaround                         | Media      | Alto       | step tipado Wine716: win7 -> install -> finally win10     | finally/repair y transactional rebuild                  |
| Heredar variables AppImage                         | Media      | Alto       | sanitizer único antes de runner/sidecar/artifact process  | abortar launch y mostrar keys conflictivas redacted     |
| Mutar runtime con múltiples clientes               | Baja/Media | Crítico    | plan anchor, leases y locks por owner                     | postergar operación; no matar otros clients             |
| PID reuse en diagnóstico/cleanup                   | Baja       | Crítico    | `(pid,start_time)` en toda lectura/signal                 | no actuar si identity cambió                            |
| Regresión de `ro-sessiond`                         | Baja/Media | Crítico    | mantener protocol/lifecycle fuera del graphics domain     | env rollback documentado; fallback existente            |
| Gepard/client unknown promovido por similitud      | Media      | Crítico    | exact-match, ausencia=`Unknown`                           | conservar selección manual; retirar record erróneo      |
| Acoplar compatibility a evasión                    | Baja       | Crítico    | sólo selección de runtimes legítimos; review de seguridad | rechazar feature/record                                 |
| D7VK sin DirectDraw real delegable                 | Media      | Alto       | spike de deployment y test GDI/DDraw                      | no-go o scope reducido; no toggle productivo            |
| D7VK/dgVoodoo en el mismo game dir                 | Media      | Alto       | profiles excluyentes y owner/collision preflight          | restaurar manifest/backups del profile previo           |
| Artefacto comprometido/corrupto                    | Baja/Media | Crítico    | HTTPS conocida, tamaño, digest, safe extraction           | conservar instalación anterior; bloquear commit         |
| Sobrearquitectura sin consumidor                   | Media      | Alto       | fases verticales, lista §10.12, exit criteria             | retirar tipos shadow sin tocar persistencia             |
| Big-bang accidental                                | Media      | Crítico    | shims y flags temporales; una autoridad migra por fase    | revertir fase aislada, datos antiguos intactos          |
| Benchmark no reproducible                          | Alta       | Alto       | protocol, host/runtime fingerprints, repetitions          | marcar incomparable, no alimentar policy                |
| Métrica premia corrupción visual                   | Media      | Crítico    | visual/compatibility hard gates                           | invalidar run/profile candidate                         |
| AutoTune con evidencia insuficiente                | Media      | Crítico    | sólo exact `Validated`, opt-in y manual pin               | volver al último plan estable; desactivar policy        |
| Catálogo curated se confunde con observaciones     | Media      | Alto       | stores/schemas/provenance separados                       | quitar observación sin tocar curated catalog            |

## 16. ADRs propuestos

| ADR     | Decisión                                                                      | Momento                            | Debe cerrar                                                                 |
| ------- | ----------------------------------------------------------------------------- | ---------------------------------- | --------------------------------------------------------------------------- |
| ADR-001 | Dominio gráfico cerrado, `Profile` vs `Plan`, targets y composición permitida | Fase 0, antes de tipos productivos | variants iniciales, ownership y rechazo de DAG/plugin genérico              |
| ADR-002 | Runtime/prefix fingerprints e identidad/migración legacy                      | Fase 0, antes de manifest v3       | canonical encoding, campos prefix-affecting, path y alias v2                |
| ADR-003 | Catálogo de artefactos y receipts                                             | inicio de fase 3                   | descriptors, trust/source policy, transaction y schema evolution            |
| ADR-004 | Compatibility catalog basado en evidencia                                     | inicio de fase 5                   | exact matching, outcome taxonomy, curated vs local, unknown semantics       |
| ADR-005 | Deployment y support envelope de D7VK                                         | fin de fase 6A                     | release exacto, layout/owner, DirectDraw delegation, constraints y go/no-go |
| ADR-006 | Protocolo de benchmark reproducible                                           | antes de fase 8                    | workload, metrics, invalidation, privacy y comparability                    |

No se necesita un ADR para cada struct ni para `nndsk-wine-ro` ahora. Este último merece ADRs sólo
si comienza una investigación con patches/build pipeline concretos.

## 17. Revisión adversarial del planning

Esta sección registra el segundo pase exigido por `AGENTS.md`: intentar refutar el roadmap en lugar
de repetir su intención.

### 17.1 ¿Está sobre-diseñado?

- Se descartó un graph/DAG genérico. Las combinaciones válidas son variants cerrados.
- El artifact manager evoluciona el `Artifact` existente; no se propone package manager.
- `RuntimePlan` no se persiste completo y no se mueve a `ro-tools-core` sin segundo consumidor.
- Compatibility, observations y benchmarks permanecen stores separados.
- No se introduce `WineD3D` hasta que exista un flujo real que necesite seleccionarlo.
- D7VK no entra al enum persistido durante el spike.

La parte más abstracta sigue siendo `RuntimePlan`. Se justifica porque ya tiene al menos cuatro
consumidores reales —dependency check, prefix setup, launch y server tools— que hoy reconstruyen la
misma decisión. Si fase 1 no logra paridad sin adapters complejos, debe reducirse el plan, no ampliar
la jerarquía.

### 17.2 Feature futura que tensiona el modelo

Caso usado para falsarlo: un cliente importa DirectDraw/D3D7 y D3D9 directo, usa D7VK sólo para la
ruta legacy y DXVK administrado para D3D9. Un enum ingenuo `GraphicsProfile::D7vk` no expresa qué
owner/version atiende la ruta D3D9. El modelo propuesto sí debe poder hacerlo con un variant
específico que contenga ambos subplanes permitidos, o rechazarlo hasta que exista ese variant. No se
abre un DAG arbitrario para resolverlo.

Otro caso, un futuro DXVK 3 que sólo soporte ciertos runners/GPUs, se representa como un descriptor y
constraints nuevos dentro de `DxvkPlan`; no exige otro boolean global. Si modifica archivos del
prefix cambia `PrefixFingerprint`; si sólo cambia un artifact runner-owned, la decisión depende de
su owner y debe quedar explícita.

### 17.3 Combinaciones inválidas intentadas

El diseño debe hacer imposible o fallar antes de mutar para:

- dgVoodoo y D7VK reclamando `ddraw.dll` en el mismo target;
- managed DXVK instalado encima de Proton runner-owned;
- D7VK seleccionado sin una implementación DirectDraw delegable;
- Wine 7.16 con NTSync;
- perfil Sakura “validado” con Wine 7.16 de otro layout/hash o DXVK no verificable;
- profile switch mientras una session lease usa el prefix/game dir;
- un `RuntimePlan` sin receipts/owners para componentes que requieren provisioning;
- dos overrides diferentes para la misma DLL/target;
- `Validated` sin evidence record exacto.

Si los tipos de fase 1 permiten construir cualquiera de ellos mediante API normal, ADR-001 está
incompleto. Deserialización corrupta debe producir error, no un plan parcial.

### 17.4 Anchors actuales y migración segura

- Wine 7.16 + managed DXVK 2.6.2 conserva su prefix v2 como legacy alias y sólo crea v3 mediante
  provisioning transaccional. Layout old-WoW64 se comprueba en vez de inferirse del label.
- Proton-CachyOS 11 sigue siendo default general y mantiene DXVK runner-owned; una elección explícita
  previa no se sobrescribe.
- dgVoodoo conserva manifest/backups/lock en game dir y no se convierte en component receipt
  exclusivamente prefix-owned.
- game, patcher handoff y OpenSetup reciben `InvocationPlan`s derivados del mismo `plan_id`; patcher
  manual mantiene una policy de target explícita, no una excepción invisible.
- cada cambio de autoridad tiene un adapter/rollback; ninguna fase requiere convertir todos los
  manifests, paths, commands y UI en un solo merge.

### 17.5 Boundaries de proceso, empaquetado y seguridad

El plan no cambia la identidad `(pid,start_time)`, los leases multi-client ni el lifecycle de
`ro-sessiond`; sólo adjunta un plan id a su owner existente. Cualquier implementación que enseñe al
supervisor detalles de graphics excede el scope. Todos los procesos externos continúan pasando por
sanitización AppImage. Los compatibility records no contienen técnicas de modificación, hooking,
bypass, packet manipulation ni memory writes.

### 17.6 Desconocidos que permanecen abiertos

- El release/version/arquitectura de D7VK apropiado y su support envelope real.
- Si side-by-side basta para los clientes objetivo o se necesita una mutación prefix-owned.
- Cómo identificar de forma durable ciertos builds de cliente cuando no haya hash recogido.
- Qué provenance exacta puede obtenerse hoy del DXVK instalado por winetricks.
- El volumen de evidencia necesario para justificar benchmark y AutoTune.

Ninguno de estos desconocidos se rellena con un default optimista.

## 18. Definition of ready y definition of done por implementación

Una fase está **ready** cuando sus ADRs están cerrados, hay fixture/baseline previo, la superficie de
rollback está identificada y se sabe qué archivos/owners muta. Está **done** únicamente cuando:

- todos sus entregables y exit criterion se cumplen;
- se ejecutaron checks focalizados y gates completos aplicables;
- runtime/packaging smoke se hizo cuando cruza esos boundaries;
- se revisó el diff completo contra callers, callees, persistence y cleanup;
- el adversarial review de la fase intentó sus failure modes reales;
- la documentación distingue checks ejecutados de inferencias pendientes;
- `main` queda funcional sin necesitar la fase siguiente.

## 19. Referencias auditadas

Repositorio:

- `AGENTS.md` y `README.md`.
- `docs/PTRACE_SESSION_SUPERVISOR_PLAN.md`.
- `docs/features/DISCORD_RICH_PRESENCE_PLAN.md`.
- Los módulos y tests enumerados en §3, incluyendo modelos/frontend/commands relacionados.

Fuentes primarias externas para el spike futuro:

- D7VK upstream: <https://github.com/WinterSnowfall/d7vk>.
- D7VK README de desarrollo: <https://github.com/WinterSnowfall/d7vk/blob/devel/README.md>.
- DXVK upstream: <https://github.com/doitsujin/dxvk>.

Las fuentes externas informan constraints y experimentos; no convierten por sí solas una combinación
en compatible con un cliente, Gepard build o runner concreto.
