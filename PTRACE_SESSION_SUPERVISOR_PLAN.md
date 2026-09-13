# Plan de implementación: supervisión de sesiones Wine y acceso a memoria

Estado: listo para implementar
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

`value=None` significa eliminar la variable. No se aceptarán strings con NUL. Los paths y valores
no UTF-8 producirán un error antes del lanzamiento; los modelos actuales ya representan las rutas
configurables como `String`, por lo que no se hará conversión con pérdida.

`shutdown_spec` será exactamente la invocación producida por `ResolvedRunner::shutdown_invocation`
para el prefix poseído. Se validará como cualquier otro comando y sólo podrá enviarse una vez que la
sesión esté en estado `Stopping`.

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
sondeos consecutivos separados por 100 ms.

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

El sidecar heredará el entorno ya saneado de RO-Launcher. Cada `ProcessSpec` aplicará sólo el delta
de entorno recibido. Antes de ejecutar un comando comprobará:

- path absoluto y ejecutable de `program`;
- `cwd` absoluto, existente y directorio;
- `WINEPREFIX` presente y canónicamente equivalente al prefix poseído; si existe
  `STEAM_COMPAT_DATA_PATH`, también debe identificar ese mismo entorno;
- máximo 4.096 argumentos y máximo 1 MiB para el mensaje completo;
- `request_id` único dentro de la sesión.

El sidecar ejecutará el comando como hijo directo. Sus stdout y stderr se leerán en threads
separados y se reenviarán al stderr del sidecar con los prefijos `[runner:stdout]` y
`[runner:stderr]`. El sidecar jamás copiará esos streams a stdout.

El bucle supervisor combinará requests, `SIGCHLD`, `SIGTERM` y un tick de 100 ms. En cada
`SIGCHLD`/tick llamará repetidamente a `waitpid(-1, ..., WNOHANG)` hasta obtener `0` o `ECHILD`.
Debe recolectar también hijos adoptados y no dejar zombies.

Apagado:

1. Dejar de aceptar `Launch`.
2. Ejecutar el `ProcessSpec` de apagado entregado previamente por el launcher para ese runner.
3. Esperar hasta `grace_ms`, limitado al rango 1.000–10.000 ms.
4. Enviar `SIGTERM` a todos los descendientes restantes.
5. Esperar 2 segundos.
6. Enviar `SIGKILL` a los sobrevivientes.
7. Recolectar todos los hijos y emitir `Stopped`.

No se usará sólo un process group para el cierre: Wine/Proton puede crear sesiones o grupos nuevos.
Los descendientes se resolverán recorriendo PPID desde `/proc`. La descendencia desde el supervisor
será la autoridad para el cierre, porque esa instancia sólo acepta comandos del prefix poseído;
la coincidencia del entorno será un dato diagnóstico y no un filtro que pueda dejar procesos vivos.

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

`apply_prefix_env`, `apply_game_env` y `apply_tool_env`, actualmente en
`src-tauri/src/utils/wine.rs`, recibirán una interfaz común para modificar tanto una invocación como
un `Command`. Sus pruebas actuales deberán conservar exactamente los overrides de dgVoodoo,
DXVK administrado y el saneamiento de variables de AppImage.

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

`RunnerSessionRegistry` se guardará en `GameState` y contendrá:

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

API interna definitiva:

```rust
impl RunnerSessionRegistry {
    async fn ensure(
        &self,
        app: &AppHandle,
        ctx: &WineContext,
    ) -> Result<Arc<RunnerSession>, SessionError>;

    async fn launch(
        &self,
        app: &AppHandle,
        ctx: &WineContext,
        invocation: RunnerInvocation,
        redactions: &[String],
    ) -> Result<SupervisedProcess, SessionError>;

    async fn shutdown_prefix(
        &self,
        ctx: &WineContext,
    ) -> Result<(), SessionError>;

    async fn shutdown_all(&self) -> Vec<SessionError>;
}
```

`ensure` será idempotente y serializado por prefix. Si existe una sesión `Ready`, verificará que el
runner sea idéntico; si difiere, devolverá error y exigirá cerrar el prefix antes de cambiarlo.

`SupervisedProcess` reemplazará el uso directo de `tokio::process::Child` y expondrá:

```rust
pub fn controller_pid(&self) -> u32;
pub fn try_exit(&self) -> Option<ProcessExit>;
pub async fn wait(&mut self) -> Result<ProcessExit, SessionError>;
pub async fn terminate(&mut self) -> Result<(), SessionError>;
```

El registro incrementará el contador al aceptar un `Launch` y lo reducirá sólo al recibir
`ControllerExited`; soltar el handle antes de tiempo no cambiará el contador. Los flujos compuestos
como creación o reparación de un prefix adquirirán un `OperationLease` durante toda la secuencia.
Cuando se detecte un cliente, `launch_game` adquirirá un `ClientLease` antes de soltar el handle del
patcher y lo conservará hasta `game.finish()`.

Cuando no queden requests, `OperationLease` ni `ClientLease`, el registro esperará una gracia fija
de 2 segundos y enviará `Shutdown`. Una nueva operación dentro de esa gracia cancelará el cierre.
Esta regla impide apagar el wineserver entre dos pasos de setup y mantiene vivo el supervisor cuando
el patcher ya terminó pero el juego continúa.

### 4.3 Bootstrap y prefixes existentes

Antes de iniciar el primer supervisor de un prefix:

1. Adquirir el `OperationGuard` exclusivo del prefix.
2. Ejecutar `find_prefix_processes(prefix)`.
3. Si hay un juego activo no registrado por esta instancia, no tocarlo y devolver:
   `El prefix ya está activo fuera del supervisor; cierra el juego y reintenta`.
4. Si sólo queda un wineserver inactivo, ejecutar una vez el comando de apagado del runner de
   forma directa y esperar hasta 5 segundos.
5. Verificar que no quedan procesos.
6. Iniciar `ro-sessiond`.

No se borrará, migrará ni recreará el contenido de ningún prefix. La migración afecta sólo a la
propiedad del árbol de procesos en la siguiente sesión.

### 4.4 Integración con el ciclo de vida

En `src-tauri/src/tools/launcher/session.rs`:

- reemplazar `cmd.spawn()` por `RunnerSessionRegistry::launch()`;
- obtener `controller_pid` desde `SupervisedProcess`;
- conservar la detección de `ragexe.exe`, la identidad estable y el handoff actuales;
- reemplazar `Child::try_wait/kill/wait` por la API supervisada;
- no considerar la salida del patcher como salida del juego;
- mantener el supervisor hasta que no queden clientes ni comandos del prefix.

En `src-tauri/src/tools/server_tools/session.rs`, `src-tauri/src/tools/prefix/setup.rs`,
`src-tauri/src/utils/gecko.rs` y `src-tauri/src/utils/audio.rs`, todas las invocaciones del runner
se enviarán al registro. El apagado de la aplicación llamará primero a las herramientas de combate,
luego a `shutdown_all()` y finalmente a `InputGateway::shutdown()`.

## 5. Servicio único de memoria

Una vez preservada la descendencia, se eliminarán aperturas independientes y tardías de
`/proc/<pid>/mem`.

Se añadirá `MemorySessionRegistry`, almacenado en `GameState` y indexado por `ProcessIdentity`.
Cuando el launcher detecte el cliente, abrirá inmediatamente una `MemorySession` y ejecutará un
preflight. AutoPot, AutoBuff, Presence y los scanners recibirán clones de la misma sesión.

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
- abrir una sesión nueva en cada handoff y cerrar la anterior;
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

Antes de habilitar una herramienta de memoria se ejecutará un preflight de cuatro bytes sobre una
dirección conocida y mapeada. Estados posibles:

- `Disponible mediante process_vm_readv`;
- `Disponible mediante /proc/<pid>/mem`;
- `Dirección de perfil no mapeada`;
- `Proceso fuera del supervisor`;
- `Permiso denegado por Yama`;
- `Proceso terminado o PID reutilizado`.

La UI no marcará AutoPot o AutoBuff como funcional si el preflight falla. El mensaje deberá indicar
la acción real: cerrar las instancias antiguas y relanzar el juego desde RO-Launcher. No sugerirá
automáticamente `ptrace_scope=0`.

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

- agregar tests para el parseo de PPID, detección de descendencia y errores exactos de memoria;
- crear un fixture que genere `padre -> proceso intermedio -> proceso objetivo`, termine el
  intermedio y mantenga un `u32` en un mapping inferior a 4 GiB;
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
- compilar ambos sidecars en `tauri:dev`, `tauri:build` y `tauri:build:appimage`;
- actualizar CI para compilar y probar el sidecar;
- implementar `RunnerSessionRegistry` y el handshake.

Criterio de salida:

- desarrollo y AppImage encuentran el mismo sidecar;
- el AppImage no requiere sudo ni archivos en `/etc`;
- cerrar RO-Launcher termina el supervisor según la política definida.

### Fase 3 — Lanzamiento de juego supervisado

Cambios:

- introducir `RunnerInvocation`;
- enrutar `launch_game` por el supervisor detrás de
  `RO_LAUNCHER_SESSION_SUPERVISOR=1`;
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
