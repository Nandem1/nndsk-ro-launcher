# Plan de implementación: supervisión de sesiones Wine y acceso a memoria

Estado: listo para implementar (decisiones de implementación bloqueadas en §13)
Alcance: Linux; Wine, Proton y UMU
Objetivo: permitir que AutoPot, AutoBuff y Discord Rich Presence lean el cliente RO con
`kernel.yama.ptrace_scope=1`, sin requerir `sudo`, capabilities, cambios globales de sysctl,
inyección en Wine ni modificaciones al cliente o a Gepard Shield.

## 1. Problema confirmado

El 11 de septiembre de 2026 se reprodujo el fallo con SakuraRO y Wine 7.16 legacy:

- RO-Launcher: PID `83202`.
- `wineserver`: PID `85985`, PPID `1111`.
- `SakuraRO Launcher.exe`: PID `86892`, PPID `1111`.
- `ragexe.exe`: PID `92901`, PPID `1111`.
- `kernel.yama.ptrace_scope = 1`.
- `ragexe.exe` pertenecía al mismo UID que RO-Launcher.
- `TracerPid=0`, `Seccomp=0` y ninguna capability efectiva en `ragexe.exe`.
- La dirección de HP configurada, `0x015FF908`, estaba dentro del mapping legible/escribible
  `0x01250000-0x01607000`.
- La apertura de `/proc/92901/mem` devolvía `EPERM` y `process_vm_readv` también fallaba.

La dirección no estaba obsoleta y Gepard no era quien emitía este error. Wine había reparentado
sus procesos a la instancia de systemd del usuario, PID `1111`. Para Yama, el cliente dejó de ser
descendiente de RO-Launcher y el kernel rechazó la lectura.

### Impacto actual

| Función                    | Dependencia de memoria          | Resultado sin permiso                                                                                 |
| -------------------------- | ------------------------------- | ----------------------------------------------------------------------------------------------------- |
| AutoPot                    | HP, SP y nombre                 | Se detiene en el primer tick fallido                                                                  |
| AutoBuff                   | buffer de estados               | Permanece activo, pero no puede evaluar ni aplicar reglas correctamente                               |
| Discord Rich Presence      | nombre, nivel, job y mapa       | La conexión a Discord sobrevive, pero publica sólo el fallback `En juego` / `Ubicación no disponible` |
| Spammer                    | ninguna para su bucle principal | No afectado                                                                                           |
| Launcher, setup y gráficos | ninguna                         | No afectados                                                                                          |

El workaround manual `kernel.yama.ptrace_scope=0` confirma la naturaleza del problema, pero no es
una solución de producto: reduce la política de aislamiento para todos los procesos del mismo
usuario y se pierde al reiniciar si no se modifica `/etc`.

## 2. Decisión de arquitectura

Se añadirá un sidecar Linux llamado `ro-sessiond`. Habrá exactamente una instancia por prefix
activo. RO-Launcher será padre de `ro-sessiond`; el sidecar se declarará _child subreaper_ antes de
iniciar cualquier proceso del runner.

```text
RO-Launcher
└── ro-sessiond --prefix <prefix canónico>
    ├── wineserver
    ├── patcher / setup
    ├── ragexe.exe
    └── otros procesos pertenecientes al mismo prefix
```

Cuando Wine daemonice un proceso o termine un patcher intermedio, el kernel reparentará los
descendientes huérfanos a `ro-sessiond`, no a systemd. RO-Launcher continuará siendo ancestro de
todos ellos y las reglas normales de Yama permitirán `process_vm_readv` y `/proc/<pid>/mem`.

### Decisiones cerradas

- `ro-sessiond` será un binario independiente. No se mezclará esta responsabilidad con
  `ro-inputd`.
- No tendrá `setuid`, file capabilities ni permisos especiales.
- No cambiará `/proc/sys/kernel/yama/ptrace_scope` ni escribirá en `/etc`.
- No usará `LD_PRELOAD`, `ptrace attach`, DLL injection ni parches al runner.
- No modificará Gepard, el ejecutable del juego, sus paquetes ni la comunicación con el servidor.
- El registro de sesiones usará el path canónico del prefix como clave.
- Un prefix sólo podrá estar asociado a un runner mientras su supervisor esté activo.
- Todos los comandos Wine/Proton de un prefix pasarán por el mismo supervisor: creación,
  winetricks, Gecko, configuración de audio, setup, patcher, juego y apagado.
- Las descargas, hashes y operaciones puramente nativas seguirán ejecutándose directamente.
- El protocolo será NDJSON versionado sobre stdin/stdout heredados; no habrá socket público.
- stdout quedará reservado para el protocolo. Los streams del runner serán reenviados por stderr
  con su origen y continuarán pasando por la redacción existente del launcher.
- Ante la muerte de RO-Launcher, `ro-sessiond` cerrará la sesión que posee; no dejará un prefix
  huérfano que una instancia posterior no pueda inspeccionar.
- La primera versión soportará múltiples juegos en prefixes diferentes y múltiples clientes en
  un mismo prefix mediante un único supervisor compartido.

## 3. Componentes nuevos

### 3.1 `crates/ro-session-protocol`

Crate sin dependencias de Tauri ni Linux. Contendrá los tipos Serde compartidos por el launcher y
el sidecar.

Archivos:

```text
crates/ro-session-protocol/
├── Cargo.toml
└── src/lib.rs
```

Constantes:

```rust
pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_MESSAGE_BYTES: usize = 1_048_576;
```

Solicitudes, una por línea:

```rust
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SessionRequest {
    Hello { protocol_version: u16 },
    Launch { request_id: String, spec: ProcessSpec },
    Shutdown {
        request_id: String,
        shutdown_spec: ProcessSpec,
        grace_ms: u64,
    },
}

pub struct ProcessSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub env: Vec<EnvironmentChange>,
}

pub struct EnvironmentChange {
    pub key: String,
    pub value: Option<String>,
}
```

`value=None` significa eliminar la variable. La clave debe ser no vacía y no puede contener NUL ni
`=`; clave o valor con NUL y claves duplicadas en un mismo `ProcessSpec` se rechazan. Los paths y
valores no UTF-8 producirán un error antes del lanzamiento; los modelos actuales ya representan
las rutas configurables como `String`, por lo que no se hará conversión con pérdida.

`shutdown_spec` será exactamente la invocación producida por `ResolvedRunner::shutdown_invocation`
para el prefix poseído. Recibir el primer `Shutdown` cambia atómicamente `Ready -> Stopping` y
después valida el spec; un segundo `Shutdown` se rechaza. Si la validación/ejecución falla, la sesión
permanece `Stopping` y continúa con TERM/KILL, no vuelve a `Ready`.

Respuestas/eventos, una por línea:

```rust
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SessionEvent {
    Ready {
        protocol_version: u16,
        supervisor_pid: u32,
        prefix: String,
        subreaper: bool,
    },
    LaunchAccepted {
        request_id: String,
        controller_pid: u32,
    },
    ShutdownAccepted {
        request_id: String,
    },
    ControllerExited {
        request_id: String,
        controller_pid: u32,
        exit_code: Option<i32>,
        signal: Option<i32>,
    },
    Idle,
    Error {
        request_id: Option<String>,
        stage: String,
        errno: Option<i32>,
        message: String,
    },
    Stopped,
}
```

El primer request debe ser `Hello`: produce `Ready` o `Error` y cierre. Cada `Launch` produce
exactamente un `LaunchAccepted` o `Error`; cada `Shutdown`, un `ShutdownAccepted` o `Error`.
`ControllerExited` informa sólo la salida del comando solicitado; no implica que el prefix esté
inactivo. `Idle` se emite únicamente cuando `waitpid(-1, ..., WNOHANG)` informa `ECHILD` durante dos
sondeos consecutivos separados por 100 ms. **El launcher no espera `Idle` para enviar `Shutdown`.**
Mientras viva `wineserver` u otro hijo adoptado, no habrá `Idle`; el cierre lo decide
`RunnerSessionRegistry` con leases y contadores de requests (§4.2, §13).

Reglas adicionales del protocolo v1 (detalle en §13.2):

- `request_id` lo genera el launcher (`Uuid::new_v4()`) y no puede repetirse entre ningún `Launch`
  o `Shutdown` de la sesión.
- `stage` de `Error` es uno de: `handshake`, `validation`, `launch`, `reap`, `shutdown`, `protocol`, `internal`.
- Línea vacía → ignorar. Segundo `Hello` → `Error` y cierre.
- `Launch` o `Shutdown` con `request_id` duplicado → `Error` (`validation`); la sesión continúa.
- Claves env vacías, con `=`/NUL o duplicadas, valores con NUL y mensajes >1 MiB → `Error`
  (`validation`/`protocol`) sin ejecutar nada.
- Máximo 32 `Launch` sin `ControllerExited` correspondiente; el 33.º → `Error`.
- **No hay request `Kill` en v1.** `ControllerExited` no incluye captura de stdout del runner.
- `Shutdown` se acepta aunque queden hijos adoptados o `Launch` sin `ControllerExited`; el sidecar
  deja de aceptar `Launch` y ejecuta la secuencia de apagado.

### 3.2 `crates/ro-sessiond`

Binario Linux responsable del árbol de procesos.

```text
crates/ro-sessiond/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── protocol.rs
    ├── supervisor.rs
    └── process.rs
```

Secuencia obligatoria de arranque:

1. Leer `--prefix` y `--parent-pid`.
2. Canonicalizar el prefix; si todavía no existe, canonicalizar su padre y reconstruir el path.
3. Verificar que `getppid()` coincide con `--parent-pid`.
4. Ejecutar `prctl(PR_SET_CHILD_SUBREAPER, 1)` y abortar si falla.
5. Ejecutar `prctl(PR_SET_PDEATHSIG, SIGTERM)`.
6. Volver a comprobar `getppid()` para cerrar la carrera entre los pasos 3 y 5.
7. Esperar como máximo 5 segundos el request `Hello`; cualquier otro mensaje es un error fatal.
8. Validar la versión y emitir `Ready` con `subreaper=true`; ante incompatibilidad, emitir `Error`
   y terminar.
9. Procesar requests y señales hasta recibir `Shutdown` o morir el padre.

RO-Launcher saneará explícitamente el `Command` que inicia el sidecar con
`sanitize_appimage_env`; no se asumirá que el proceso Tauri ya está saneado. El sidecar tomará una
instantánea de ese entorno al arrancar y cada `ProcessSpec` aplicará sólo el delta recibido. Antes
de ejecutar un comando comprobará:

- path absoluto y ejecutable de `program`;
- `cwd` absoluto, existente y directorio;
- exactamente un cambio `WINEPREFIX=...` presente (nunca heredado ni eliminado) y canónicamente
  equivalente al prefix poseído; si el spec incluye
  `STEAM_COMPAT_DATA_PATH`, también debe identificar ese mismo entorno (si no viene en el spec, no
  es error; Proton/UMU hoy no lo setean);
- máximo 4.096 argumentos y máximo 1 MiB para el mensaje completo;
- `request_id` único dentro de la sesión.

El sidecar ejecutará el comando como hijo directo mediante **`fork` + `execve`** (no
`std::process::Command::spawn` ni `tokio::process`). Antes del `fork`, el padre materializará
`argv`, `envp`, `cwd`, CStrings, `/dev/null` y pipes `O_CLOEXEC`. Después del `fork`, el hijo
redirigirá stdin a `/dev/null` —nunca al pipe de protocolo— y sólo podrá ejecutar syscalls
async-signal-safe (`dup2`/`close`/`chdir`, `prctl` vía libc, `getppid`, `execve`, `write` de un error
mínimo y `_exit`); quedan prohibidos allocations, locks, formatters, logging y destructores Rust en
el hijo. Esto es obligatorio porque launches anteriores ya pueden haber creado threads lectores.

Sus stdout y stderr se leerán en threads separados creados **después** del `fork` y se reenviarán
al stderr del sidecar con los prefijos exactos `[runner:stdout] ` y `[runner:stderr] ` (espacio tras
el corchete). Un writer de stderr compartido escribe cada línea completa para evitar interleaving;
líneas mayores a 64 KiB se trocean con una marca de continuación, sin allocation ilimitada. Cada
thread termina en EOF y se recolecta; no se acumulan threads detached. El sidecar jamás copiará
esos streams a stdout.

`crates/ro-sessiond` usa sólo std + `libc` + `serde_json` + `ro-session-protocol` (sin Tokio, Tauri
ni `ro-tools-linux`). Un único reaper: `waitpid(-1, WNOHANG)` en el loop principal; nunca
`Child::wait` en paralelo. Ver §13.1.

El lector NDJSON será acotado: no se usará `BufRead::lines()` sin límite; se leerá como máximo
`MAX_MESSAGE_BYTES + 1`, se rechazará la línea demasiado larga sin reservar memoria ilimitada y se
drenará hasta el siguiente `\n`. Un único writer serializará todos los `SessionEvent` para impedir
que dos JSON se intercalen en stdout.

El bucle supervisor combinará requests, `SIGCHLD`, `SIGTERM` y un tick de 100 ms. En cada
`SIGCHLD`/tick llamará repetidamente a `waitpid(-1, ..., WNOHANG)` hasta obtener `0` o `ECHILD`.
Debe recolectar también hijos adoptados y no dejar zombies.

Apagado:

1. Dejar de aceptar `Launch`.
2. Ejecutar el `ProcessSpec` de apagado incluido en `Shutdown`; `ShutdownAccepted` se emite después
   de crear ese controlador y el deadline comienza en ese instante. Si su creación falla, emitir
   `Error(stage=shutdown)` y continuar igualmente con el cierre forzado.
3. Esperar hasta `grace_ms`, limitado al rango 1.000–10.000 ms, mientras se sigue drenando streams
   y recolectando hijos.
4. Enviar `SIGTERM` a todos los descendientes restantes.
5. Esperar 2 segundos.
6. Enviar `SIGKILL` a los sobrevivientes.
7. Recolectar todos los hijos y emitir `Stopped` sólo después de `ECHILD`.

El controlador interno de `shutdown_spec` no incrementa el contador de `Launch` ni emite
`ControllerExited`; sólo participa en el reap general. `ShutdownAccepted` es su único ack.

No se usará sólo un process group para el cierre: Wine/Proton puede crear sesiones o grupos nuevos.
Los descendientes se resolverán recorriendo PPID desde `/proc`. El scan + señal se repetirá hasta
el deadline para cubrir procesos creados durante la carrera de apagado. La descendencia desde el
supervisor será la autoridad para el cierre, porque esa instancia sólo acepta comandos del prefix
poseído; la coincidencia del entorno será un dato diagnóstico y no un filtro que pueda dejar
procesos vivos. Si el cierre nace de `SIGTERM`, EOF de stdin o `PDEATHSIG`, no existe un
`shutdown_spec` confiable: se omite el paso 2 y se ejecuta directamente TERM/KILL con los mismos
límites. El sidecar nunca espera indefinidamente durante la salida del launcher.

## 4. Cambios en RO-Launcher

### 4.1 Representar comandos sin iniciarlos

`src-tauri/src/utils/runner.rs` dejará de ser el lugar que decide cómo se crea el proceso Unix.
`ResolvedRunner` continuará decidiendo programa, argumentos y entorno, pero devolverá un
`RunnerInvocation` en lugar de exponer únicamente un `tokio::process::Command`.

```rust
pub struct RunnerInvocation {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub env: Vec<(OsString, Option<OsString>)>,
}
```

Métodos afectados:

- `game_command` -> `game_invocation`;
- `tool_command` -> `tool_invocation`;
- `builtin_command` -> `builtin_invocation`;
- `create_prefix_command` -> `create_prefix_invocation`;
- `winetricks_command` -> `winetricks_invocation`;
- `shutdown_command` -> `shutdown_invocation`.

Durante la transición se permitirá `RunnerInvocation::into_command()` sólo para tests y para el
bootstrap que detiene una sesión Wine preexistente. Ningún flujo normal de producción podrá usarlo
después de la fase 4.

`apply_prefix_env`, `apply_game_env`, `apply_tool_env` y `sanitize_appimage_env`, en
`src-tauri/src/utils/wine.rs`, operarán sobre un trait común `ProcessEnv` (no sólo sobre
`tokio::process::Command`):

```rust
pub trait ProcessEnv {
    fn set_env(&mut self, key: impl AsRef<OsStr>, val: impl AsRef<OsStr>);
    fn unset_env(&mut self, key: impl AsRef<OsStr>);
}
```

Implementaciones: `Command` y `RunnerInvocation`. `RunnerInvocation.env` es el **delta** de
sets/unsets que aplican esas funciones, no un volcado del entorno completo; el sidecar hereda el
entorno ya saneado de RO-Launcher y aplica sólo ese delta. Los tests actuales conservan exactamente
los overrides de dgVoodoo, DXVK administrado y AppImage (vía `into_command()` en tests).

`ProcessEnv::set_env` y `unset_env` reemplazan cualquier cambio anterior para la misma clave; el
delta serializado contiene una sola entrada por key. `RunnerInvocation` resolverá `program` a path
absoluto antes de cruzar el protocolo. Nombres encontrados por `PATH` (por ejemplo el fallback
`winetricks`) se resuelven una vez en el launcher o fallan con un error accionable; el sidecar nunca
hace búsqueda implícita en `PATH`.

`cwd` en invocaciones: `game_invocation` / `tool_invocation` usan el work dir del exe. Builtins
(`wineboot`, `winetricks`, `reg`, `msiexec`, `winecfg`) usan por defecto el prefix canónico si
existe como directorio; si no, el directorio actual del launcher canonicalizado. El setup crea el
prefix antes del primer `Launch`.

Si algún campo de `RunnerInvocation` no es UTF-8 válido, el launcher **no envía** el `Launch` y
devuelve error al caller (el sidecar no intenta latin1).

### 4.2 Registro de supervisores

Se añadirá `src-tauri/src/tools/runner_sessions/`:

```text
runner_sessions/
├── mod.rs
├── registry.rs
├── client.rs
├── protocol.rs
└── diagnostics.rs
```

`GameProcessHandle` (`state/game_process.rs`) es un registro **multi-cliente** (`Launching` /
`Running` por `clientId`); no es un enum unitario `Idle | Launching | Running`. El `Child` del
controlador (wine/umu) no vive en `GameProcessHandle`; `SupervisedProcess` lo reemplaza sólo para
ese controlador. `ragexe.exe` se detecta por `/proc` como hoy.

`RunnerSessionRegistry` se guardará en `GameState` (junto a un stub o implementación de
`MemorySessionRegistry` según la fase) y contendrá:

```rust
HashMap<CanonicalPrefix, Arc<RunnerSession>>
```

`RunnerSession` tendrá:

- identidad del runner: kind, path canónico y hash/version ya registrado en el manifiesto;
- prefix canónico;
- PID e identidad estable del supervisor;
- canal de requests;
- mapa `request_id -> ProcessHandle`;
- contador de requests activos;
- leases explícitos para operaciones de varios comandos y clientes detectados;
- task de lectura de eventos;
- task de drenaje/redacción de stderr;
- estado `Starting | Ready | Stopping | Stopped | Failed`.

Los requests pendientes usarán canales one-shot por `request_id`. La task lectora de eventos será
la única que resuelva `LaunchAccepted`, `ControllerExited`, `Error` y muerte del sidecar; ninguna
espera hará polling de un mutex. No se mantendrá un `std::sync::Mutex` ni el mutex global del
registro a través de `.await` o de I/O. El mutex Tokio por prefix podrá serializar sólo el
arranque/publicación de una sesión `Starting`; el resto de esperas ocurrirá fuera del mapa.

API interna definitiva:

```rust
impl RunnerSessionRegistry {
    async fn begin_operation(
        &self,
        app: &AppHandle,
        ctx: &WineContext,
    ) -> Result<OperationLease, SessionError>;

    async fn launch(
        &self,
        app: &AppHandle,
        operation: &OperationLease,
        invocation: RunnerInvocation,
        redactions: &[String],
    ) -> Result<SupervisedProcess, SessionError>;

    fn attach_client(
        &self,
        operation: &OperationLease,
        client_id: &str,
    ) -> Result<ClientLease, SessionError>;

    async fn shutdown_prefix(
        &self,
        ctx: &WineContext,
    ) -> Result<(), SessionError>;

    async fn shutdown_all(&self) -> Vec<SessionError>;
}
```

`OperationLease` conserva un `Arc<RunnerSession>` y se obtiene en la misma transición que cancela
un shutdown pendiente. `launch` sólo acepta invocaciones cuyo prefix y runner coincidan con ese
lease; no vuelve a resolver una sesión. Para un comando aislado, el caller obtiene un lease, lanza,
espera y lo suelta. `ProcessState::Running` será dueño de un `ClientRuntimeGuard` con
`SessionOwnership::Supervised(ClientLease)` y, desde fase 5, `Option<MemoryLease>`; el path de
rollback usa `SessionOwnership::Direct`. Los leases no quedan como variables locales ni en mapas
paralelos y se liberan automáticamente al remover ese cliente en `game.finish()`. `ClientLease`,
`MemoryLease` y `ClientRuntimeGuard` no implementan `Clone`.

```rust
pub enum SessionOwnership {
    Direct,
    Supervised(ClientLease),
}

pub struct ClientRuntimeGuard {
    pub session: SessionOwnership,
    pub memory: Option<MemoryLease>,
}
```

El `ensure` privado usado por `begin_operation` será idempotente y serializado por prefix
(`tokio::Mutex` por clave canónica). Si existe
una sesión `Ready`, el runner anclado es `kind` + path canónico; si difiere, error determinista:
`El prefix ya está activo con otro runner; ciérralo antes de cambiarlo.` Hash/versión del manifiesto
es sólo telemetría, no clave de rechazo.

`OperationLease` / `ClientLease` son **distintos** de `OperationGuard` (`operation_lock.rs`): el
guard serializa setup/launch/tools en filesystem; los leases evitan que el registro envíe `Shutdown`
al sidecar. No unificarlos en un solo tipo.

`SupervisedProcess` reemplazará el uso directo de `tokio::process::Child` y expondrá:

```rust
pub fn controller_pid(&self) -> u32;
pub fn try_exit(&self) -> Option<ProcessExit>;
pub async fn wait(&mut self) -> Result<ProcessExit, SessionError>;
pub async fn terminate(&mut self) -> Result<(), SessionError>;
```

`terminate()` y la cancelación de launch envían **`SIGTERM` (y `SIGKILL` tras timeout corto) al
`controller_pid` desde el launcher** vía `libc::kill`, no un mensaje de protocolo al sidecar.
`SupervisedProcess` guardará `ProcessIdentity { pid, start_time }`, capturada inmediatamente tras
`LaunchAccepted`, y volverá a validarla antes de **cada** señal. Si el proceso ya terminó o el PID
fue reutilizado, no enviará la señal y esperará/resolverá el `ControllerExited`; queda prohibido
matar basándose sólo en un `u32`. El sidecar reaps y emite `ControllerExited`. `stop_game` mantiene
la misma regla de identidad estable sobre cliente y controlador y **no** llama `shutdown_prefix` si
quedan otros `ClientLease` o requests.

El registro incrementará el contador al aceptar un `LaunchAccepted` y lo reducirá sólo al recibir
`ControllerExited`; soltar el handle antes de tiempo no cambiará el contador. Los flujos compuestos
como creación o reparación de un prefix adquirirán un `OperationLease` durante toda la secuencia.
`launch_game` también lo adquirirá **antes del primer comando del runner** y lo conservará durante
patcher, detección y handoff inicial. Con la identidad detectada, preparará primero la
`MemorySession`/`MemoryLease` (no-op en fases 3–4), obtendrá el `ClientLease`, construirá el
`ClientRuntimeGuard` y lo transferirá a `mark_running` en una sola mutación del estado. Sólo entonces
soltará el `OperationLease`; desde fase 5, un snapshot `Running` nunca puede observarse sin memoria
y ownership ya listos. Si cualquiera de esos pasos falla, se descartan los leases provisionales,
se cancela la reserva, se termina el controlador por identidad estable y se suelta la operación.
El `ClientLease` se conservará hasta `game.finish()` de ese cliente. Varios clientes en el mismo
prefix = varios `ClientLease`, un supervisor.

Cuando no queden requests, `OperationLease` ni `ClientLease`, el registro esperará una gracia fija
de 2 segundos y enviará `Shutdown`. El timer llevará una generación/cancellation token; antes de
enviar el request volverá a validar bajo lock que siguen en cero los tres contadores y que la sesión
continúa `Ready`. Una nueva operación dentro de esa gracia invalida la generación y cancela el
cierre.
Esta regla impide apagar el wineserver entre dos pasos de setup y mantiene vivo el supervisor cuando
el patcher ya terminó pero el juego continúa.

### 4.3 Bootstrap y prefixes existentes

Antes de iniciar el primer supervisor de un prefix:

1. Adquirir el `OperationGuard` exclusivo del prefix.
2. Ejecutar `find_prefix_processes(prefix)`.
3. «Juego activo» = cualquier proceso del prefix cuyo `comm`/cmdline **no** esté en la allowlist de
   leftover: `wineserver`, `winedevice.exe`, `plugplay.exe`, `services.exe`, `rpcss.exe`,
   `svchost.exe`, `explorer.exe`, `wineboot.exe`, `start.exe`. Si hay juego activo y
   `GameProcessHandle` no lo registra, no tocarlo y devolver:
   `El prefix ya está activo fuera del supervisor; cierra el juego y reintenta`.
4. Si sólo queda leftover, ejecutar **una vez** `shutdown_invocation().into_command()` (bootstrap
   permitido sin supervisor) y esperar hasta 5 segundos; si siguen procesos, mismo error que (3), sin
   `SIGKILL` a ciegas.
5. Verificar que no quedan procesos.
6. Iniciar `ro-sessiond`.

No se borrará, migrará ni recreará el contenido de ningún prefix. La migración afecta sólo a la
propiedad del árbol de procesos en la siguiente sesión.

Orden de adquisición obligatorio para evitar inversión de locks:

1. El flujo llama `begin_operation`; el slot de startup por prefix elige un único bootstrap.
2. Sólo ese bootstrap adquiere y libera el `OperationGuard` exclusivo antes de publicar la sesión.
3. `begin_operation` publica `Ready` y crea el `OperationLease` en una transición indivisible.
4. Ya con el lease, el caller adquiere el `OperationGuard` shared/exclusivo que corresponda y
   ejecuta su secuencia. El registro nunca intenta adquirir otro `OperationGuard` en esta etapa.

Ningún command handler debe adquirir `OperationGuard` antes de `begin_operation`. En una
reconstrucción, el apagado del wineserver actual se ejecuta como un `Launch` regular dentro del
mismo `OperationLease`; después se verifica que no quedan hijos y recién entonces se renombra el
prefix. `shutdown_prefix` queda reservado al cierre idle/de aplicación y rechaza ejecutarse si hay
leases. Así el sidecar puede permanecer vivo mientras la ruta se reemplaza de forma transaccional,
sin un segundo bootstrap ni una ventana accesible a otro command.

### 4.4 Integración con el ciclo de vida

En `src-tauri/src/tools/launcher/session.rs`:

- enrutar el spawn del runner con un helper `session_supervisor_enabled()` (§13.5): si está
  desactivado, `spawn_runner_direct(invocation)` (código actual extraído); **no** duplicar
  `launch_game` entero;
- reemplazar `cmd.spawn()` por `RunnerSessionRegistry::launch()` cuando el supervisor esté activo;
- obtener `controller_pid` desde `SupervisedProcess`;
- conservar la detección de `ragexe.exe`, la identidad estable y el handoff actuales;
- reemplazar `Child::try_wait/kill/wait` por la API supervisada;
- no considerar la salida del patcher como salida del juego;
- mantener el supervisor hasta que no queden clientes ni comandos del prefix;
- transferir `OperationLease -> ClientLease` sin hueco y liberar ese lease en todas las rutas que
  hoy llaman `game.finish()` o cancelan una reserva.

En `src-tauri/src/tools/server_tools/session.rs`, `src-tauri/src/tools/prefix/setup.rs`,
`src-tauri/src/utils/gecko.rs` y `src-tauri/src/utils/audio.rs`, todas las invocaciones del runner
se enviarán al registro. `run_logged_command` / `run_logged_command_ok` dejarán de recibir
`Command` directo del runner; usarán `registry.launch` + `wait` + drenaje de stderr del sidecar.

Audio: no ampliar el protocolo v1 con captura de stdout. `read_current_driver` usará
`user.reg`; `set_audio_driver` lanzará `reg` vía supervisor, validará exit code y confirmará
leyendo `user.reg` (stderr del runner por `[runner:stderr]`).

Ejecución directa **permanente** (sin supervisor): `reported_version`, `curl`/Gecko download, copias
DXVK, I/O nativo, `kill` desde el launcher, bootstrap leftover (§4.3).

El apagado de la aplicación (`RunEvent::Exit` en `lib.rs`): presence → autopot/autobuff/spammer →
`sessions.shutdown_all()` → `InputGateway::shutdown()`. `shutdown_all()` toma un snapshot del mapa,
cierra sesiones en paralelo y aplica timeout por sesión y global; el callback de Tauri puede hacer
`block_on` sólo sobre esa future acotada. Un sidecar defectuoso se marca `Failed` y se mata por su
`ProcessIdentity`, sin bloquear indefinidamente la salida de la UI.

## 5. Servicio único de memoria

Una vez preservada la descendencia, se eliminarán aperturas independientes y tardías de
`/proc/<pid>/mem`.

Se añadirá `MemorySessionRegistry`, almacenado en `GameState` y indexado por `ProcessIdentity`.
Cuando el launcher detecte el cliente y en cada handoff, abrirá inmediatamente una `MemorySession`,
ejecutará preflight y la registrará. `register` devuelve un `MemoryLease` no clonable; su `Drop`
elimina la entrada sólo si continúa siendo la misma generación, para no borrar un reemplazo más
nuevo. Ese lease vive en `ClientRuntimeGuard` y se reemplaza junto a la identidad durante
`replace_running`. AutoPot, AutoBuff, Presence y los scanners recibirán `Arc<MemorySession>` vía
`MemorySessionRegistry::get(identity)`; **no** llamarán `ProcMemoryReader::open` por su cuenta. En
handoff, AutoPot y AutoBuff siguen la identidad nueva en el siguiente tick (como Presence hoy con
`handoff`); no reiniciar el loop si la sesión nueva es válida.

`is_descendant_of` / `read_ppid` en `crates/ro-tools-linux/src/wine_process.rs` pasan a `pub` (fase 0).

```rust
pub struct MemorySession {
    identity: ProcessIdentity,
    reader: Arc<ProcMemoryReader>,
    backend: MemoryBackend,
}

pub enum MemoryBackend {
    ProcessVmReadv,
    ProcMem,
    ProcessVmReadvWithProcMemFallback,
}
```

Reglas:

- validar `start_time` antes y después de cada lectura compuesta;
- invalidar la sesión inmediatamente si el PID fue reutilizado;
- en handoff, crear primero el nuevo `MemoryLease` y pasarlo a
  `replace_running(expected, replacement, lease)`; la mutación intercambia identidad+lease y recién
  después cae el lease anterior. Si el CAS falla, cae sólo el lease nuevo;
- no conservar un error de permisos como caché permanente;
- serializar sólo las lecturas mediante `/proc/mem`; `process_vm_readv` puede ejecutarse en
  paralelo;
- limitar todas las direcciones RO al rango `u32` existente;
- nunca escribir memoria.

`ProcMemoryReader::open` dejará de descartar el error de apertura. Guardará el errno de
`/proc/<pid>/mem`, y cada fallo de lectura conservará también el errno de `process_vm_readv`.
El fallback usará `FileExt::read_at` en lugar de `seek + read`, evitando compartir un cursor entre
consumidores.

El error público incluirá:

- PID y `ProcessIdentity`;
- dirección y tamaño solicitado;
- errno y texto de cada backend;
- `ptrace_scope` observado;
- UID del launcher y del objetivo;
- PPID del objetivo;
- si el objetivo es descendiente del launcher/supervisor;
- si la dirección aparece en `/proc/<pid>/maps`.

No se mostrarán valores leídos ni datos privados en los logs.

## 6. UX y observabilidad

Antes de habilitar una herramienta de memoria se ejecutará un preflight de **cuatro bytes** al
inicio de la **primera región readable+writable (`rw`)** de `/proc/<pid>/maps` dentro del rango
`u32`, comprobando que los cuatro bytes caben en el mapping (no usar `hp_base` del perfil para el
test de permiso Yama). Estados posibles (campo IPC `memoryAccess` en status/snapshot, enum Serde
cerrado; la UI traduce a estos labels):

```rust
#[serde(rename_all = "camelCase")]
pub enum MemoryAccess {
    ProcessVmReadv,
    ProcMem,
    NoReadableWritableRegion,
    OutsideSupervisor,
    YamaDenied,
    ProcessExitedOrReused,
    BackendError,
}

#[serde(rename_all = "camelCase")]
pub enum ProfileMemory {
    NotConfigured,
    AddressUnmapped,
    InvalidRead,
    Valid,
}
```

- `Disponible mediante process_vm_readv`;
- `Disponible mediante /proc/<pid>/mem`;
- `Sin región legible/escribible para preflight`;
- `Proceso fuera del supervisor`;
- `Permiso denegado por Yama`;
- `Proceso terminado o PID reutilizado`;
- `Backend de memoria no disponible`.

La validez del perfil se evalúa **después** del permiso y en un campo separado (`profileMemory`):
`No configurado | Dirección no mapeada | Lectura inválida | Válido`. Así una dirección `hp_base`
obsoleta nunca se etiqueta como fallo de Yama ni altera el backend elegido por el preflight.

Clasificación del preflight: éxito de `process_vm_readv` gana; si falla y `FileExt::read_at`
funciona, usar `ProcMem`; `EPERM`/`EACCES` de ambos con identidad viva y mapping presente es
`YamaDenied`; mismatch de `start_time` o desaparición de `/proc/<pid>` es
`ProcessExitedOrReused`; no pertenecer al árbol de la `RunnerSession` registrada es
`OutsideSupervisor`. Otros errno conservan su diagnóstico interno y usan `BackendError`; nunca se
reclasifican automáticamente como Yama.

La UI no marcará AutoPot o AutoBuff como funcional si el preflight no es
`Disponible mediante process_vm_readv` o `Disponible mediante /proc/<pid>/mem`. El mensaje deberá
indicar la acción real: cerrar las instancias antiguas y relanzar el juego desde RO-Launcher. No
sugerirá automáticamente `ptrace_scope=0`.

Logs mínimos por sesión:

```text
[Session] prefix=<redacted/hash> supervisor=<pid> subreaper=true protocol=1
[Session] runner=<kind/version/hash> controller=<pid> request=<id>
[Launch] client=<pid> ppid=<pid> supervised=true handoff=<old->new|none>
[Memory] client=<pid/start_time> backend=process_vm_readv preflight=ok
[Session] prefix idle; supervisor stopped cleanly; zombies=0
```

No se imprimirán credenciales, argumentos sensibles, contenido de memoria ni paths personales en
telemetría exportable. Los valores de redacción actuales deben aplicarse antes de emitir logs a la
UI.

## 7. Fases de implementación

### Fase 0 — Baseline y pruebas de regresión

Cambios:

- agregar tests para el parseo de PPID, detección de descendencia (`is_descendant_of` público) y
  errores exactos de memoria;
- crear un fixture Linux (`#[cfg(target_os = "linux")]`) `padre → intermedio → objetivo` con un
  `u32` en mapping &lt; 4 GiB; el intermedio termina; sin subreaper el lector no ancestro ve
  `EPERM` con `ptrace_scope=1`; con `ro-sessiond` subreaper el PPID del objetivo es el sidecar;
- registrar el baseline manual de SakuraRO bajo `ptrace_scope=1`.

Criterio de salida:

- el fixture sin subreaper reproduce `EPERM` cuando el objetivo queda fuera del árbol permitido;
- los tests existentes continúan pasando.

### Fase 1 — Protocolo y sidecar aislado

Cambios:

- crear `ro-session-protocol` y `ro-sessiond`;
- implementar handshake, validación, launch, reaping, idle y shutdown;
- añadir unit tests del protocolo y la máquina de estados;
- añadir integración Linux con el fixture huérfano.

Criterio de salida:

- tras morir el intermediario, el PPID del fixture es `ro-sessiond`;
- el proceso de test, ancestro del sidecar, puede leer el valor con `ptrace_scope=1`;
- 100 ciclos de launch/exit terminan con cero zombies;
- un protocolo incompatible falla antes de lanzar procesos.

### Fase 2 — Empaquetado y cliente del supervisor

Cambios:

- añadir ambos crates al workspace raíz;
- copiar `ro-sessiond-<target>` desde `src-tauri/build.rs` igual que `ro-inputd`;
- agregar `binaries/ro-sessiond` a `bundle.externalBin`;
- compilar ambos sidecars en `tauri:dev`, `tauri:build` y `tauri:build:appimage`
  (`cargo build -p ro-inputd -p ro-sessiond`);
- actualizar CI para compilar ambos sidecars;
- implementar `RunnerSessionRegistry`, `find_ro_sessiond()` (mismo patrón que `find_ro_inputd()`),
  drenaje de stderr del sidecar con prefijos `[runner:stdout|stderr] `.

Criterio de salida:

- desarrollo y AppImage encuentran el mismo sidecar;
- el AppImage no requiere sudo ni archivos en `/etc`;
- cerrar RO-Launcher termina el supervisor según la política definida.

### Fase 3 — Lanzamiento de juego supervisado

Cambios:

- introducir `RunnerInvocation`;
- enrutar `launch_game` por el supervisor cuando `session_supervisor_enabled()` sea true (fases 3–5:
  sólo con `RO_LAUNCHER_SESSION_SUPERVISOR=1`; fase 6: default true, `=0` rollback);
- adaptar la espera, cancelación, salida y handoff;
- implementar bootstrap seguro para wineservers antiguos.

Criterio de salida:

- SakuraRO abre launcher, selecciona cuenta, cierra el patcher y entra al mapa;
- `wineserver`, patcher y `ragexe.exe` permanecen bajo `ro-sessiond`;
- `process_vm_readv` funciona durante al menos 30 minutos con `ptrace_scope=1`;
- Gepard continúa aceptando Wine 7.16 y no reaparece `3::110::12`;
- Proton-CachyOS sigue siendo el runner predeterminado para perfiles no específicos.

### Fase 4 — Todas las operaciones del runner supervisadas

Cambios:

- enrutar setup, patcher manual, OpenSetup, dgVoodoo CPL, Gecko, audio, wineboot, winetricks y
  shutdown;
- eliminar los `.spawn()`/`.status()` directos de comandos Wine/Proton fuera del bootstrap;
- garantizar una sesión por prefix y compatibilidad con múltiples clientes.

Criterio de salida:

- `rg` no encuentra ejecuciones directas de invocaciones del runner fuera del módulo permitido;
- abrir y cerrar herramientas no deja `wineserver` con PPID de systemd;
- cambiar runner mientras el prefix está activo se rechaza con un mensaje determinista;
- dos prefixes pueden ejecutarse simultáneamente sin mezclar procesos ni logs.

### Fase 5 — Memoria compartida y diagnóstico

Cambios:

- implementar `MemorySessionRegistry`;
- migrar AutoPot, AutoBuff, Presence y scanners;
- conservar errno y metadatos de relación de procesos;
- añadir preflight y estados de UI.

Criterio de salida:

- los tres consumidores comparten una sesión por cliente;
- AutoPot y AutoBuff funcionan después del handoff del patcher;
- Presence actualiza personaje, nivel y mapa;
- un perfil con dirección incorrecta se distingue de un permiso denegado;
- desactivar una herramienta no cierra la sesión usada por las demás.

### Fase 6 — Activación predeterminada y limpieza

Cambios:

- ejecutar la matriz manual completa;
- activar el supervisor por defecto;
- conservar la variable de entorno sólo como rollback durante una release;
- eliminar el path directo en la release siguiente;
- documentar arquitectura y troubleshooting breve en README.

Criterio de salida:

- no se necesita `ptrace_scope=0` en una instalación limpia;
- no existen cambios persistentes del sistema;
- no hay regresiones de rendimiento o compatibilidad entre runners;
- CI, build AppImage e instalación local pasan.

## 8. Matriz de validación obligatoria

| Caso                         | Runner            | Estrategia     | Prefix             | Resultado requerido                                  |
| ---------------------------- | ----------------- | -------------- | ------------------ | ---------------------------------------------------- |
| SakuraRO                     | Wine 7.16 legacy  | Patcher        | aislado            | Gepard acepta; memoria y tres consumidores funcionan |
| HoneyRO                      | Proton-CachyOS 11 | Direct         | aislado            | juego y memoria funcionan                            |
| Cliente con patcher genérico | Proton-CachyOS 11 | Patcher        | aislado            | handoff preserva descendencia                        |
| OpenSetup/dgVoodoo           | ambos             | Tool           | mismo prefix       | GPU/overrides sin regresión; wineserver supervisado  |
| Dos servidores               | runners distintos | Direct/Patcher | prefixes distintos | dos supervisores independientes                      |
| Dos clientes                 | mismo runner      | Direct         | mismo prefix       | un supervisor, dos identidades de juego              |
| Runner cambiado              | cualquiera        | cualquiera     | prefix activo      | operación rechazada sin matar procesos               |
| Launcher cerrado             | cualquiera        | cualquiera     | activo             | shutdown acotado, sin huérfanos ni zombies           |

Cada caso se probará en desarrollo y en el AppImage instalado con:

```text
kernel.yama.ptrace_scope = 1
```

La validación debe registrar PID/PPID, versión y hash del runner, prefix, backend de memoria,
resultado de cada consumidor y cantidad de zombies. No se aceptará como prueba una ejecución hecha
con `ptrace_scope=0`.

## 9. Comandos de calidad y entrega

Antes de cada commit de una fase:

```bash
npm test
npm run build
npm run lint
npm run format:check
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Antes de cerrar las fases 2, 3 y 6:

```bash
RO_LAUNCHER_DISCORD_APPLICATION_ID=<id> npm run tauri:build:appimage
```

Se verificará que el AppImage contenga ambos sidecars con el triple de target correcto y que el
árbol instalado se comporte igual que `npm run tauri:dev`.

## 10. Riesgos y mitigaciones

| Riesgo                              | Mitigación obligatoria                                                                         |
| ----------------------------------- | ---------------------------------------------------------------------------------------------- |
| Zombies adoptados                   | drenar `waitpid(-1, WNOHANG)` hasta `ECHILD`; test de 100 ciclos                               |
| Sidecar muere antes que el launcher | marcar sesión `Failed`, desactivar consumidores y cerrar/reintentar el prefix explícitamente   |
| Launcher muere                      | `PDEATHSIG`; shutdown con límite y escalamiento TERM/KILL                                      |
| Wineserver previo fuera del árbol   | bootstrap detecta y rechaza juego activo; sólo apaga un servidor inactivo                      |
| Dos runners en un prefix            | identidad de runner fijada en `RunnerSession`; rechazo determinista                            |
| Runner contamina el protocolo       | stdout exclusivo del sidecar; streams del runner sólo por stderr                               |
| Argumentos contienen credenciales   | redacción previa a UI/log; protocolo local heredado, sin socket público                        |
| Dirección de memoria inválida       | preflight diferencia mapping/`EFAULT` de `EPERM`                                               |
| Cambio observable para Gepard       | no hay inyección ni modificación de Wine/cliente; prueba explícita de SakuraRO en cada release |
| Diferencia entre dev y AppImage     | sidecar empaquetado como `externalBin`; matriz en ambos entornos                               |

## 11. Fuera de alcance

- Interpretar o desactivar comprobaciones internas de Gepard.
- Escribir memoria del cliente.
- Ocultar procesos, módulos o automatización al anti-cheat.
- Modificar paquetes o tráfico del juego.
- Instalar un daemon root o un servicio global.
- Hacer downgrade global de Wine.
- Cambiar el runner específico validado para SakuraRO.
- Sustituir Proton-CachyOS como runner predeterminado general.

## 12. Definición final de terminado

La implementación estará terminada únicamente cuando, con `ptrace_scope=1` y sin privilegios:

1. Todos los procesos Wine/Proton de un prefix iniciado por la aplicación permanezcan bajo su
   `ro-sessiond` hasta terminar.
2. Ninguna operación normal del runner evite el supervisor.
3. AutoPot, AutoBuff y Presence compartan una sesión de memoria válida y sobrevivan al handoff.
4. SakuraRO funcione con Wine 7.16 + DXVK administrado/dgVoodoo sin el error Gepard `3::110::12`.
5. Los demás perfiles conserven Proton-CachyOS 11 como default.
6. No queden procesos huérfanos, zombies, prefixes bloqueados ni cambios globales de seguridad.
7. La suite completa, la matriz manual, el AppImage y su instalación hayan sido verificados.

## 13. Decisiones de implementación bloqueadas

Esta sección cierra ambiguedades para implementación automatizada. **No reinterpretar** durante las
fases 0–6 salvo cambio explícito de producto.

### 13.1 Sidecar: proceso POSIX, no Tokio

- Crate `ro-sessiond`: dependencias `libc`, `serde_json`, `ro-session-protocol`, `signal-hook`
  (patrón `ro-inputd`). Sin Tokio, Tauri, `ro-tools-linux`.
- Cada `Launch`:
  1. En el padre, validar y construir CStrings completas (`argv`, `envp`, `cwd`), abrir `/dev/null`
     y crear `pipe2(O_CLOEXEC)` para stdout/stderr.
  2. `fork`; en el hijo usar sólo syscalls async-signal-safe hasta `execve`.
  3. Hijo pre-exec: `dup2(/dev/null, stdin)`, `dup2` de streams, `close`/`chdir`,
     `prctl(PR_SET_PDEATHSIG, SIGTERM)`, comprobar `getppid()` y `execve`; ante fallo,
     `write(2, ...)` mínimo + `_exit(127)`.
  4. Padre, sólo después de `fork`: threads leyendo pipes → stderr del sidecar con prefijos `[runner:stdout] ` /
     `[runner:stderr] `.
  5. Reap único: `waitpid(-1, WNOHANG)` en loop 100 ms / `SIGCHLD`. **Prohibido**
     `std::process::Child::wait` o `tokio::process` para procesos Wine.
- Handler `SIGCHLD`: no usar `SA_NOCLDWAIT`.
- Handlers de señal sólo cambian flags atómicos de `signal-hook`; no hacen I/O, allocation ni
  `waitpid`.
- stdin se procesa con lectura NDJSON acotada a `MAX_MESSAGE_BYTES`; stdout tiene un writer único.
- Canonicalizar `--prefix`: si no existe, `canonicalize(parent)` + nombre (igual que
  `OperationGuard`); abortar si el padre tampoco existe.

### 13.2 Protocolo v1 (ejemplos canónicos)

```json
{"type":"hello","protocolVersion":1}
{"type":"ready","protocolVersion":1,"supervisorPid":1234,"prefix":"/abs/prefix","subreaper":true}
{"type":"launch","requestId":"<uuid-v4>","spec":{"program":"/usr/bin/wine","args":["a.exe"],"cwd":"/game","env":[{"key":"WINEPREFIX","value":"/abs/prefix"}]}}
```

- Dependencia `uuid` en `src-tauri` para `request_id`.
- Primer mensaje debe ser `Hello`; JSON inválido o tipo desconocido como primer mensaje → `Error` +
  exit.
- Línea vacía en stdin → ignorar. Segundo `Hello` → `Error` y cierre.
- `Launch` o `Shutdown` con `request_id` duplicado → `Error` (`validation`); la sesión continúa.
- Claves env vacías, con `=`/NUL o duplicadas, valores con NUL y mensajes >1 MiB → `Error`
  (`validation`/`protocol`) sin ejecutar nada.
- Máximo 32 `Launch` sin `ControllerExited` correspondiente; el 33.º → `Error`.
- No hay request `Kill` en v1; `ControllerExited` no incluye stdout del runner.
- `Idle` sólo diagnóstico; el launcher no espera `Idle` para `Shutdown`.
- `Shutdown` aceptado aunque queden hijos o launches pendientes; sidecar deja de aceptar `Launch`.
- `grace_ms` en `Shutdown`: clamp 1000–10000. Los 2 s post-`SIGTERM` del sidecar y los 2 s de gracia
  idle del launcher son constantes distintas.

### 13.3 Empaquetado

- Workspace members: `crates/ro-session-protocol`, `crates/ro-sessiond`.
- `src-tauri/build.rs`: copiar `ro-sessiond-{TARGET}` como `ro-inputd`.
- `tauri.conf.json`: `"externalBin": ["binaries/ro-inputd", "binaries/ro-sessiond"]`.
- Spawn sidecar: `Command::new(find_ro_sessiond()).args(["--prefix", canonical, "--parent-pid", pid])`,
  aplicar `sanitize_appimage_env`, stdin/stdout piped, stderr piped; `kill_on_drop(true)` **solo**
  para el proceso sidecar.
- Drenaje stderr launcher: líneas con prefijo runner → quitar prefijo → `should_log_line` +
  redacción; resto → log `[Session]`.

### 13.4 `GameState`

```rust
pub struct GameState {
    // game, tool_lifecycle, autopot, autobuff, spammer, input, presence (existentes)
    pub sessions: RunnerSessionRegistry,
    pub memory: MemorySessionRegistry, // stub vacío fases 1–4; completo fase 5
}
```

Ambos registries internamente `Arc`; inicializar en `lib.rs` `manage`.

### 13.5 Flag de activación

```rust
fn session_supervisor_enabled() -> bool
```

- Fases 3–5: `true` iff `RO_LAUNCHER_SESSION_SUPERVISOR=1`.
- Fase 6: default `true`; `RO_LAUNCHER_SESSION_SUPERVISOR=0` rollback.
- Ignorado por: `reported_version`, bootstrap leftover §4.3.

### 13.6 Memoria (fase 5)

- `ProcMemoryReader::open`: conservar errno de apertura de `/proc/pid/mem` (no `.ok()`).
- Fallback: `FileExt::read_at`; mutex sólo en fallback `/proc/mem`.
- Preflight permiso: primera región `rw` con cuatro bytes disponibles en maps (u32), no `hp_base`;
  estado del perfil separado de `memoryAccess`.
- Cerrar sesión en `game.finish()`; abrir nueva en `mark_running` y handoff.

### 13.7 Fixture fase 0/1

- 100 ciclos launch/exit: cero zombies.
- Tests Linux no omitirse cuando `ptrace_scope=1` (CI Ubuntu 24.04).

### 13.8 Referencia cruzada

| Tema                                             | Sección principal |
| ------------------------------------------------ | ----------------- |
| Leases vs `OperationGuard`                       | §4.2, §13.9       |
| `Idle` ≠ trigger shutdown                        | §3.1, §4.2        |
| `ProcessEnv` / delta env                         | §4.1              |
| Audio / `user.reg`                               | §4.4              |
| Bootstrap leftover allowlist                     | §4.3              |
| `SupervisedProcess::terminate` = `kill` launcher | §4.2              |
| Handoff memoria + tools                          | §5, §6            |

### 13.9 Leases y ciclo de shutdown del registro

- `OperationGuard`: lock de filesystem (`prefix`, `dgvoodoo`, `runtime`); no sustituye leases.
- `OperationLease`: setup/reset/reparación y lanzamiento hasta handoff; evita `Shutdown` entre
  pasos.
- `ClientLease`: uno por cliente en `mark_running` … `game.finish()`; varios clientes ⇒ varios
  leases, un `ro-sessiond`.
- Contador de requests: +1 en `LaunchAccepted`, −1 en `ControllerExited`; drop de
  `SupervisedProcess` no decrementa.
- Sin requests, sin leases: esperar 2 s; si no hay actividad nueva → enviar `Shutdown` al sidecar.
  Nueva operación en la gracia cancela el timer.

### 13.10 Ejecución directa permitida (siempre)

Sin pasar por `ro-sessiond`: `reported_version`, descargas Gecko/`curl`, copias DXVK, hashes, I/O
nativo, `kill` al juego/controlador **validado con `ProcessIdentity`** desde el launcher, bootstrap
leftover §4.3 (`into_command()` una vez). Tras fase 4, `rg` no debe encontrar
`.spawn()`/`.status()`/`.output()` sobre invocaciones del runner fuera de `runner_sessions/`,
bootstrap y `reported_version`.

### 13.11 Concurrencia, locks y fallos

- Orden canónico: `begin_operation`/slot de startup → publicación de sesión + `OperationLease` →
  `OperationGuard` del flujo → launches. No adquirir `OperationGuard` antes de
  `begin_operation`; el bootstrap es la única excepción interna y la ejecuta un único owner.
- Nunca mantener el mapa global, un `std::sync::Mutex` ni un lock de estado a través de `.await`,
  lectura de pipe, espera de proceso o adquisición de `OperationGuard`.
- Handoff: conservar `OperationLease` hasta que `mark_running`, `MemorySession` y `ClientLease`
  hayan quedado registrados. En error, rollback completo; jamás un cliente vivo sin lease.
- PID: supervisor, controlador y cliente usan `ProcessIdentity`; validar `start_time` antes de toda
  señal o lectura. Un mismatch significa proceso terminado/reutilizado, nunca permiso para matar.
- Shutdown idle: timer con generación; revalidar requests y leases justo antes de enviar
  `Shutdown`. `shutdown_all` es concurrente y acotado.
- Protocolo: lectura acotada, writer único y un solo reaper. EOF/stdin cerrado equivale a cierre de
  emergencia TERM/KILL, no a espera infinita.
- El sidecar repite scan de descendencia + señal hasta el deadline y sólo emite `Stopped` tras
  `ECHILD`; cualquier hijo no recolectado convierte la sesión en `Failed`.
