# Arquitectura vigente del runtime de Ragnarok Online

> El nombre del archivo se conserva para no romper enlaces históricos. Este documento ya no es un
> roadmap: describe el runtime implementado después de las fases 0–8.

| Campo        | Estado                                                                                                     |
| ------------ | ---------------------------------------------------------------------------------------------------------- |
| Vigencia     | 2026-09-20                                                                                                 |
| Implementado | Dominio tipado, prefix v3, artefactos, gráficos, catálogo de compatibilidad, observaciones y benchmark A/B |
| Cerrado      | Spike D7VK con veredicto `no-go`; no existe integración productiva                                         |
| Futuro       | AutoTune conservador (requiere una decisión y alcance nuevos)                                              |

## 1. Cómo usar la documentación

La autoridad se lee en este orden:

1. [`AGENTS.md`](../AGENTS.md) fija los invariantes de producto y seguridad.
2. Este documento explica el comportamiento operativo vigente y dónde vive.
3. Los [ADR](adr/) conservan por qué se eligieron las decisiones irreversibles o costosas.
4. El código y sus tests fijan los detalles ejecutables.

Los contratos de las fases 0–8 se retiraron al quedar implementados. Su detalle sigue disponible en
Git, pero no debe usarse como documentación vigente. El supervisor de sesiones conserva un contrato
propio en [`PTRACE_SESSION_SUPERVISOR_PLAN.md`](PTRACE_SESSION_SUPERVISOR_PLAN.md).

## 2. Invariantes

### Producto y compatibilidad

- Proton-CachyOS 11 administrado mediante UMU es el default general.
- Un `server.runner` no vacío gana sobre el default global. La UI muestra el runner efectivo.
- SakuraRO con el hash validado de Gepard `26.9.3.1` usa Wine portable 7.16 old-WoW64 y DXVK
  administrado 2.6.2.
- HoneyRO con el hash validado de Gepard `26.8.26.1` usa Proton-CachyOS 11 administrado.
- Un hash desconocido puede producir `Unknown` y una recomendación; nunca cambia el runner en
  silencio ni bloquea el lanzamiento por sí solo.
- No se modifica, engancha, desactiva, suplanta ni evade Gepard/GameGuard, el cliente, sus paquetes
  o la validación del servidor.

### Prefix, procesos y datos

- La unidad de aislamiento es servidor + identidad material del runner. El manifest es la autoridad,
  no el nombre del directorio ni una etiqueta de UI.
- Un directorio desconocido y no vacío no se adopta. Un prefix custom no se mueve, reescribe ni
  elimina automáticamente.
- Un rebuild administrado es transaccional: el anterior se conserva hasta que el reemplazo termina
  y se restaura si falla.
- La identidad de proceso es `(pid, start_time)`. Se revalida antes de leer memoria o enviar señales.
- El lifecycle admite varios clientes: cerrar uno no termina los demás ni libera recursos aún usados.
- Las variables AppImage se sanitizan antes de lanzar Wine, Proton, UMU o sidecars.
- `ro-sessiond` está activo por defecto y mantiene `ptrace_scope=1`; su rollback de release es
  `RO_LAUNCHER_SESSION_SUPERVISOR=0`.

### Gráficos y runners

- D3D8/9/11 termina en DXVK/Vulkan. DirectDraw puede entrar por dgVoodoo, que emite D3D11 y luego
  usa DXVK.
- Game, patchers, OpenSetup y el panel gráfico derivan su environment del mismo `RuntimePlan`.
- Wine 7.16 sólo habilita ESYNC/FSYNC cuando el artefacto declara los parches TkG necesarios. Nunca
  recibe NTSync.
- WebView2 bajo Wine 7.16 acelerado se instala temporalmente como Windows 7 para obtener Evergreen
  109 y siempre restaura Windows 10. La excepción no se generaliza.

## 3. Flujo de ejecución

```text
seleccionar runner
  -> inspeccionar identidad y capabilities
  -> derivar prefix aislado y validar manifest/fingerprint
  -> provisionar o reparar transaccionalmente
  -> resolver RuntimeProfile + RuntimePlan
  -> evaluar compatibilidad exacta
  -> construir environment por InvocationTarget
  -> adquirir leases/supervisor
  -> lanzar y observar (pid, start_time)
  -> persistir outcome y limpiar por ownership
```

La selección del runner conserva tres fuentes exhaustivas:

1. `server.runner` no vacío (`ServerOverride`);
2. setting global persistido (`GlobalSetting`);
3. Proton-CachyOS administrado (`ProductDefault`).

Un valor vacío no enmascara el siguiente nivel. La resolución y el provisioning usan el mismo
runner efectivo; no se vuelve a decidir durante el spawn.

## 4. Modelo de dominio

El dominio vive en [`src-tauri/src/tools/runtime/`](../src-tauri/src/tools/runtime/):

- `RuntimeProfile` expresa intención: runner solicitado, fuente de selección y perfil gráfico.
- `RunnerPlan` contiene runner resuelto, identidad observada, capabilities y política de sync.
- `RuntimePlan` agrega runner, gráficos, ubicación/binding del prefix y requirement de WebView2.
- `GraphicsProfile` sólo admite `Dxvk` y `DgVoodooDxvk`.
- `DxvkProvider` distingue DXVK incluido en el runner, administrado en el prefix o legacy por
  winetricks.
- `InvocationTarget` distingue `Game`, `LaunchPatcher`, `MaintenancePatcher`, `OpenSetup` y
  `GraphicsControlPanel`.

No existe un grafo genérico de layers. Cada topología permitida es una variante cerrada y testeable.
Los overrides DLL y variables tienen contributor/owner; un conflicto se reporta antes de mutar o
lanzar y nunca se resuelve por orden incidental.

### Política por target

| Target               | DXVK prefix-owned | Overlay dgVoodoo       |
| -------------------- | ----------------- | ---------------------- |
| Game                 | Sí                | Sí, si está verificado |
| LaunchPatcher        | Sí                | Sí, si está verificado |
| MaintenancePatcher   | Sí                | No                     |
| OpenSetup            | Sí                | Sí, si está verificado |
| GraphicsControlPanel | Sí                | Sí, si está verificado |

DXVK incluido en Proton conserva el environment del runner: el launcher no simula componentes
prefix-owned. `WINEDLLOVERRIDES` se construye una vez desde claims estructurados y deterministas.

## 5. Identidad de prefix y runtime

Se mantienen dos fingerprints distintos:

- `PrefixFingerprint` contiene sólo material que no se puede compartir ni reparar con seguridad
  dentro del mismo prefix: identidad/locator del runner, layout WoW64, policy de arquitectura y
  componentes gráficos prefix-owned.
- `RuntimeFingerprint` describe la ejecución completa: runner/capabilities, sync, gráficos y overlay,
  binding del prefix, recetas, WebView2 y revisiones de invocación/environment.

No entran hashes de cliente/Gepard, GPU, PID, timestamps, credenciales, paths personales, resultados
de benchmark ni labels de UI. Esos datos pertenecen a evidencia separada.

Los fingerprints usan SHA-256 sobre un encoding canónico versionado. Para un prefix administrado v3:

```text
prefixes/<server-token-16>-v3-<primeros-24-hex-del-prefix-fingerprint>
```

El sufijo corto sólo localiza. El manifest v3 guarda server id y digest completos, kind/path del
runner y componentes instalados; el digest compromete la identidad material calculada por el
resolver. Una colisión o mismatch es un hard error: no se adopta ni reescribe el directorio.

La migración es `read old / write new`:

- un v3 se reutiliza sólo con identidad completa y health válidos;
- un v2 se conserva como alias únicamente si coincide exactamente con server, runner y estructura
  legacy (`LegacyV2RunnerMatched`);
- una creación nueva escribe v3;
- un v2 rechazado se preserva;
- un custom prefix queda `Indeterminate` y bajo ownership del usuario.

El registry usa el locator canónico del prefix como clave y `SessionAnchorV2` fija el
`RuntimeFingerprint`/`plan_id`. Mientras haya leases activos, un plan distinto no puede mutar ese
entorno.

La decisión y el encoding están en [ADR-002](adr/ADR-002-runtime-prefix-identity.md).

## 6. Provisioning y artefactos

El catálogo común en [`src-tauri/src/tools/artifacts/`](../src-tauri/src/tools/artifacts/) administra
UMU, Proton-CachyOS y DXVK 2.6.2 mediante:

```text
descriptor -> URL HTTPS allowlisted -> size/digest -> extracción segura
           -> validación de payload -> activación transaccional -> receipt v2
```

La extracción rechaza paths absolutos, traversal y symlinks que escapen de staging; admite symlinks
relativos internos requeridos por Proton. Los payloads requeridos deben ser archivos regulares. Los
receipts schema 1 se leen y se elevan bajo lock; schema 2 es la escritura vigente; un schema futuro
no se borra ni se interpreta.

Los namespaces de operación (`runtime`, `prefix`, `dgvoodoo`) impiden mutaciones concurrentes. Los
locks se mantienen cortos, se respeta su orden y no se conserva un lock síncrono a través de
`.await`. Después de esperar por un lock se revalida el estado en disco.

Véase [ADR-003](adr/ADR-003-runtime-artifact-catalog.md).

## 7. Gráficos

| Runner / perfil      | Provider efectivo                                                 |
| -------------------- | ----------------------------------------------------------------- |
| Proton administrado  | DXVK incluido y propiedad del runner                              |
| Wine 7.16 compatible | DXVK 2.6.2 administrado en el prefix                              |
| Otro Wine            | receta legacy winetricks, marcada como no fijada                  |
| Perfil dgVoodoo      | wrappers verificados en game dir → D3D11 → provider DXVK anterior |

dgVoodoo es un overlay del directorio del juego, con manifest y backups propios; no forma parte del
manifest del prefix. Sus wrappers deben estar verificados antes de entrar al plan.

D7VK no es un tercer perfil. El spike v2.2 cargó con Wine 7.16, pero Gepard `26.9.3.1` rechazó
`ddraw.dll` como archivo ilegal. El veredicto es `no-go`: no se reubica, renombra, disfraza ni integra
el mismo payload. El harness permanece compilado sólo en tests como evidencia reproducible. Véase
[ADR-005](adr/ADR-005-runtime-d7vk-deployment.md).

## 8. Compatibilidad basada en evidencia

El catálogo productivo compara el SHA-256 exacto de `gepard.dll` con una
`CompatibilityRuntimeSpec` observada:

| Ancla                        | Runtime validado                                                    |
| ---------------------------- | ------------------------------------------------------------------- |
| SakuraRO / Gepard `26.9.3.1` | Wine 7.16 conocido + old-WoW64 conocido + DXVK managed `dxvk-2.6.2` |
| HoneyRO / Gepard `26.8.26.1` | Proton administrado `ro-proton-cachyos-11.0-20260702-slr`           |

El catálogo no hace wildcard por nombre de servidor, familia de runner o versión textual. Un
mismatch produce `Unknown`; una recomendación asociada al hash no cambia runner, prefix, environment
ni readiness. Las observaciones locales nunca ascienden automáticamente a evidencia curated.

Véase [ADR-004](adr/ADR-004-runtime-compatibility-catalog.md).

## 9. Sesiones, memoria y lifecycle

[`ro-sessiond`](PTRACE_SESSION_SUPERVISOR_PLAN.md) es el owner por defecto del proceso Wine/Proton.
El launcher obtiene handles supervisados sin relajar Yama ni usar `sudo`/`sysctl`. Toda lectura de
memoria y señal revalida `(pid, start_time)` para impedir reutilización de PID.

El registry separa leases de operación, cliente, memoria y request. El último owner efectúa cleanup;
un stop parcial no mata procesos, input compartido o prefixes usados por otros clientes. Cancelación,
handoff de patcher, cierre del launcher y errores de cleanup conservan ownership explícito.

## 10. Observaciones y benchmarks

### Observaciones

El store `~/.local/share/ro-launcher/observations/` registra inicio y fin por UUID, plan/fingerprints,
identidades inicial/final del juego, host gráfico y outcome clasificado. Conserva como máximo 200
registros o 90 días. La exportación tokeniza identificadores y redacta paths sensibles.

Las observaciones son diagnóstico; no cambian el catálogo de compatibilidad ni seleccionan perfiles.

### Benchmark A/B

El harness se adjunta a un cliente ya `Running`; nunca crea un segundo juego. Los facts del plan y
la identidad de proceso se congelan al adjuntar y se vuelven a verificar durante la captura. El store
`~/.local/share/ro-launcher/benchmarks/` conserva hasta 100 runs o 90 días.

El adaptador v1 importa CSV con uno de estos headers exactos:

```text
frametimeMs
frametimeMs,monotonicNs
```

Calcula p50/p95/p99 y 1%/0.1% lows, sin score único ni ganador automático. Una comparación exige
spec, servidor tokenizado, sujeto Gepard, runner, prefix fingerprint, GPU conocida, flags, adaptador
y condiciones térmicas compatibles. Además aplica gates de startup, samples y corrección visual.

Los IDs son UUID validados; paths arbitrarios, capturas solapadas, duplicados, identidad stale y
records corruptos se rechazan o invalidan. La exportación sale de `app_data_dir` y omite IDs locales.

Véase [ADR-006](adr/ADR-006-runtime-benchmark-protocol.md).

## 11. Flags de rollback

Todos estos flags están activos por defecto y sólo el valor exacto `0` desactiva su path:

| Variable                         | Efecto con `=0`                                                   |
| -------------------------------- | ----------------------------------------------------------------- |
| `RO_LAUNCHER_RUNTIME_SHADOW`     | omite comparación diagnóstica legacy/nuevo                        |
| `RO_LAUNCHER_RUNTIME_GRAPHICS`   | vuelve al builder gráfico legacy                                  |
| `RO_LAUNCHER_PREFIX_V3`          | evita nuevas escrituras v3 y conserva resolución legacy           |
| `RO_LAUNCHER_RUNTIME_COMPAT`     | usa el check Gepard legacy y omite assessment estructurado        |
| `RO_LAUNCHER_RUNTIME_OBSERVE`    | deja de escribir nuevas observaciones                             |
| `RO_LAUNCHER_RUNTIME_BENCHMARK`  | bloquea mutaciones del harness; lectura/export siguen disponibles |
| `RO_LAUNCHER_SESSION_SUPERVISOR` | usa temporalmente el spawn directo de rollback                    |

Ningún flag autoriza reinterpretar, mover o borrar datos existentes.

## 12. Persistencia y ownership

| Superficie                       | Autoridad                   | Regla de mutación                                        |
| -------------------------------- | --------------------------- | -------------------------------------------------------- |
| `settings.json` / `servers.json` | Configuración del usuario   | escrituras atómicas y compatibilidad hacia atrás         |
| `.ro-launcher-prefix.json`       | Identidad del prefix        | sólo provisioning/repair bajo guard; custom no se adopta |
| `.ro-launcher-runtime.json`      | Receipt de artefacto        | instalación/elevación bajo lock `runtime`                |
| `.ro-launcher-dgvoodoo.json`     | Overlay game-dir            | install/restore transaccional bajo guard                 |
| `observations/`                  | Evidencia local diagnóstica | retención, redacción y borrado explícito                 |
| `benchmarks/`                    | Evidencia local A/B         | estado monotónico, retención y borrado explícito         |

No se persisten credenciales, command lines completas, contenido de memoria, paths personales en
exports ni configuración del host.

## 13. Validación requerida

Los cambios del runtime deben probar primero la seam afectada y luego las gates completas frontend y
Rust definidas por `AGENTS.md`. Los cambios de runner, prefix, gráficos, WebView2, procesos o sidecars
también requieren smoke real de Tauri y, si afectan distribución, AppImage instalada.

La matriz manual mínima conserva ambos anchors:

- SakuraRO con Wine 7.16 old-WoW64 + DXVK 2.6.2;
- HoneyRO con Proton-CachyOS 11 + UMU;
- juego directo, handoff de patcher, OpenSetup y maintenance patcher;
- stop durante launch, varios clientes/prefixes y ciclos repetidos sin zombies;
- supervisor activo y rollback explícito;
- AppImage con environment sanitizado.

Una prueba automatizada verde no sustituye una validación live cuando el contrato sólo existe en el
artefacto ensamblado. Un caso no ejecutado se reporta como tal.

## 14. Trabajo futuro

AutoTune no forma parte del runtime actual. Antes de abrirlo debe existir un ADR que defina candidatos
permitidos, umbrales conservadores, opt-in, rollback y cómo consume `usable_for_autotune` sin convertir
observaciones locales en evidencia curated. D7VK no es un candidato pendiente: sólo puede revisarse
si cambia explícitamente la autorización del servidor o existe un cliente controlado sin esa política.
