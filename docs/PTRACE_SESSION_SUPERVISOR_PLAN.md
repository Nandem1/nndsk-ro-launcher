# Supervisión de sesiones Wine y acceso a memoria

> El nombre del archivo se conserva por compatibilidad. La implementación quedó cerrada el
> 15 de septiembre de 2026; este documento describe su contrato operativo vigente.

| Campo               | Estado                                             |
| ------------------- | -------------------------------------------------- |
| Plataforma          | Linux; Wine, Proton y UMU                          |
| Activación          | Default ON                                         |
| Rollback de release | `RO_LAUNCHER_SESSION_SUPERVISOR=0`                 |
| Política de host    | `kernel.yama.ptrace_scope=1`, sin cambios globales |
| Sidecar             | `ro-sessiond`, uno por prefix canónico activo      |

## 1. Problema y solución

Wine puede daemonizar y reparentar `wineserver` y el cliente a systemd. Con Yama en
`ptrace_scope=1`, el launcher deja de ser ancestro y tanto `process_vm_readv` como `/proc/<pid>/mem`
pueden devolver `EPERM`, aunque el proceso sea del mismo usuario.

`ro-sessiond` se inicia como hijo del launcher, fija `PR_SET_CHILD_SUBREAPER` antes de cualquier
proceso del runner y adopta los descendientes huérfanos:

```text
ro-launcher
└── ro-sessiond --prefix <prefix-canónico> --parent-pid <launcher>
    ├── wineserver
    ├── patcher / setup / tools
    ├── ragexe.exe
    └── otros procesos del mismo prefix
```

Así el launcher sigue siendo ancestro del cliente y puede ofrecer lectura de memoria de sólo lectura
a AutoPot, AutoBuff y Discord Rich Presence sin debilitar el host.

## 2. Límites de seguridad

El supervisor:

- no usa `setuid`, capabilities, `sudo`, daemon root ni servicio global;
- no cambia `ptrace_scope`, `/proc/sys`, `/etc` ni la política del host;
- no usa `LD_PRELOAD`, attach de ptrace, inyección DLL ni parches al runner;
- no modifica Gepard/GameGuard, el ejecutable, paquetes, memoria ni tráfico del juego;
- no expone un socket público: stdin/stdout heredados son el único canal de control;
- sólo acepta comandos cuyo `WINEPREFIX` canónico coincide con el prefix que posee;
- revalida `(pid, start_time)` antes de señales y lecturas para evitar PID reuse.

La solución preserva la topología de procesos; no intenta ocultarla ni evadir el anti-cheat.

## 3. Componentes y ownership

| Componente                                                         | Responsabilidad                                                  |
| ------------------------------------------------------------------ | ---------------------------------------------------------------- |
| [`crates/ro-session-protocol`](../crates/ro-session-protocol/)     | Tipos Serde, límites y validaciones compartidas                  |
| [`crates/ro-sessiond`](../crates/ro-sessiond/)                     | Subreaper, spawn, reaping y shutdown de descendientes            |
| [`tools/runner_sessions`](../src-tauri/src/tools/runner_sessions/) | Cliente Tokio, registry, leases y empaquetado                    |
| [`tools/memory_sessions`](../src-tauri/src/tools/memory_sessions/) | Sesión de lectura compartida por `ProcessIdentity`               |
| [`ro-tools-linux`](../crates/ro-tools-linux/)                      | Identidad estable, `/proc`, `process_vm_readv` y señales seguras |

`ro-sessiond` es distinto de `ro-inputd`: no comparten privilegios, protocolo ni lifecycle. El
binario se empaqueta como `externalBin`; desarrollo y build lo compilan junto con `ro-inputd`.

## 4. Protocolo v1

El protocolo es NDJSON UTF-8 sobre stdin/stdout. Cada mensaje ocupa una línea; stdout está reservado
para eventos. Los streams del runner se reenvían por stderr y pasan por redacción en el launcher.

Límites cerrados:

| Límite                |        Valor |
| --------------------- | -----------: |
| `PROTOCOL_VERSION`    |            1 |
| Mensaje               |        1 MiB |
| Argumentos por launch |        4.096 |
| Launches simultáneos  |           32 |
| Handshake             |          5 s |
| Grace de shutdown     | clamp 1–10 s |

Requests:

- `Hello { protocolVersion }`, obligatorio como primer mensaje;
- `Launch { requestId, spec }`;
- `Shutdown { requestId, shutdownSpec, graceMs }`.

Eventos:

- `Ready`, `LaunchAccepted`, `ControllerExited`;
- `ShutdownAccepted`, `Idle`, `Stopped`;
- `Error { requestId?, stage, errno?, message }`.

El lector admite mensajes parciales y varios mensajes en una lectura, limita memoria y descarta una
línea sobredimensionada sin consumir la siguiente. Un único writer serializa eventos. EOF, JSON
inválido, segundo `Hello`, versión incompatible o cierre inesperado fallan la sesión y despiertan
todos los waiters; no dejan futures esperando indefinidamente.

### `ProcessSpec`

`program` y `cwd` deben ser absolutos y UTF-8. Environment es un delta explícito `Set/Unset`; no
admite keys vacías, `=`, NUL ni duplicados. Debe contener exactamente un `WINEPREFIX` no nulo y
canónicamente igual al prefix owned. `STEAM_COMPAT_DATA_PATH`, cuando existe, debe coincidir también.

El sidecar captura un environment base ya sanitizado al arrancar. Variables heredadas de ownership
no sustituyen el spec. Esto evita reintroducir `APPDIR`, paths de montaje AppImage u otro
`WINEPREFIX` al serializar una invocación más tarde.

## 5. Lifecycle de una sesión

```text
begin_operation(prefix, runner, plan_id)
  -> bootstrap y rechazo de procesos ajenos
  -> spawn + Hello/Ready
  -> OperationLease
  -> Launch request(s)
  -> ClientLease por juego detectado
  -> MemoryLease por (pid, start_time)
  -> soltar leases/requests
  -> idle shutdown o shutdown_all
  -> wineserver shutdown -> TERM -> KILL -> reap hasta ECHILD
```

El registry se indexa por prefix canónico. Una entrada fija runner kind/path y `plan_id`; un intento
de usar otro runner o plan mientras está activa se rechaza sin matar la sesión existente.

| Owner            | Qué protege                                             |
| ---------------- | ------------------------------------------------------- |
| `OperationLease` | setup, patcher, tool o launch en curso                  |
| request activo   | controller aceptado hasta `ControllerExited`            |
| `ClientLease`    | juego vivo, incluidos varios clientes en un prefix      |
| `MemoryLease`    | sesión de lectura registrada para una identidad estable |

El shutdown idle sólo se agenda con los tres contadores de sesión en cero. Usa una generación y
revalida estado/contadores inmediatamente antes de actuar, por lo que un lease nuevo invalida el
timer anterior. `shutdown_all` procesa prefixes en paralelo, tiene límites por sesión y globales, y
escala a kill verificado si una sesión no termina.

Al cerrar normalmente se ejecuta el shutdown propio del runner y se espera el grace configurado. Si
quedan descendientes se envía SIGTERM, dos segundos después SIGKILL, repitiendo scan/reap hasta
`ECHILD`. `Stopped` sólo se emite cuando ya no quedan hijos. `PDEATHSIG` lleva el mismo cleanup al
caso en que muere el launcher.

### Bootstrap de un prefix existente

Antes de crear el supervisor se inspeccionan procesos del prefix bajo el guard `prefix`:

- sin procesos: continuar;
- sólo leftovers identificables: pedir shutdown una vez y revalidar;
- proceso registrado como juego o proceso no clasificable: rechazar;
- leftover de otro runner: sólo usar el runner exacto registrado por el manifest para apagarlo;
- nunca matar un proceso activo por inferencia débil.

## 6. Memoria compartida

`MemorySessionRegistry` mantiene una sesión por `ProcessIdentity`, no por PID. El registro verifica
la identidad, abre una vez `ProcMemoryReader` y ejecuta preflight sobre una región RW:

| Estado                     | Significado                                         |
| -------------------------- | --------------------------------------------------- |
| `processVmReadv`           | backend primario usable                             |
| `procMem`                  | fallback `/proc/<pid>/mem` usable                   |
| `yamaDenied`               | ambos backends denegados pese a descendencia válida |
| `outsideSupervisor`        | proceso no desciende del ancestro esperado          |
| `noReadableWritableRegion` | no existe región apta para el probe                 |
| `processExitedOrReused`    | cambió `(pid, start_time)`                          |
| `backendError`             | error no clasificable como permiso                  |

AutoPot, AutoBuff y Presence comparten el mismo reader mediante leases. Cada lectura y scan valida la
identidad antes y después; una identidad stale invalida acceso. La generación del registry impide
que al caer un lease antiguo borre una sesión nueva registrada con la misma clave.

El diagnóstico diferencia permiso, mapping, proceso ajeno y backend. No muestra credenciales,
command lines ni paths crudos; los logs de prefix/runner usan tokens y las redacciones se acumulan
durante toda la sesión.

## 7. Integración obligatoria

Todas las operaciones externas que pueden crear o reutilizar Wine/Proton para un prefix pasan por el
registry: creación/repair, winetricks, Gecko/WebView2, audio, setup, patcher, game y shutdown. Las
descargas, hashes, extracción y otras tareas puramente nativas se ejecutan directamente.

Un handoff de patcher reemplaza la identidad del juego, registra una nueva sesión de memoria y suelta
la anterior sin alterar otros clientes. El stop de un cliente sólo libera sus leases; no apaga input,
supervisor ni prefix mientras queden owners.

El path directo se conserva exclusivamente como rollback de release con
`RO_LAUNCHER_SESSION_SUPERVISOR=0`. Desactivar el flag no migra ni borra prefixes.

## 8. Evidencia y regresión

El cierre funcional se validó con SakuraRO, Wine 7.16 y `ptrace_scope=1`: `wineserver` y
`ragexe.exe` quedaron bajo `ro-sessiond`; AutoPot, AutoBuff y Spammer permanecieron estables. El
AppImage instalado incluyó el sidecar y preservó la jerarquía, con variables AppImage/ownership
sanitizadas. La verificación final de empaquetado que usó `ptrace_scope=0` no sustituye la prueba
funcional anterior con `ptrace_scope=1`.

La regresión manual mínima para cualquier cambio de procesos, prefix, runner, gráficos, WebView2 o
sidecars incluye:

- SakuraRO/Wine 7.16 y HoneyRO/Proton-CachyOS 11;
- direct launch y handoff de patcher;
- OpenSetup, dgVoodoo y operaciones de mantenimiento;
- dos prefixes, dos clientes en un prefix y cambio de runner rechazado;
- stop durante launch, cierre del launcher y ciclos repetidos sin huérfanos/zombies;
- desarrollo y AppImage instalada, siempre con `ptrace_scope=1` para aceptación de memoria.

La suite debe cubrir framing parcial/batched, mensajes grandes, sidecar roto, identidad stale,
leases concurrentes, shutdown repetido, restauración fallida, environment heredado conflictivo y el
fixture real de reparenting.

## 9. Fuera de alcance

- Escribir memoria o interpretar/evadir Gepard/GameGuard.
- Modificar el cliente, paquetes, red o política del host.
- Convertir el sidecar en daemon privilegiado o socket multiusuario.
- Downgradear globalmente Wine o sustituir Proton-CachyOS como default.
- Usar `ptrace_scope=0` como solución o criterio de aceptación.
