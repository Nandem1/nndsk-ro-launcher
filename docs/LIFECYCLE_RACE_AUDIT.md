# Auditoría adversarial de ciclo de vida — 2026-10-02

## Alcance y baseline

Baseline: `main` en `86a3cbd`, coincidente con `origin/main`, versión 0.1.1. Se leyó
`AGENTS.md`, se comprobó que no hay instrucciones anidadas y se revisó el diff completo de los
13 archivos ya modificados. Esos cambios fueron preservados, revisados y ampliados; no se
consideraron una prueba de corrección por sí mismos. No hubo commits, pushes, releases, cambios
de versión, instalaciones ni modificaciones de juegos, anti-cheat o política del host.

Criterios: preparación sin lanzamiento/credenciales; diagnóstico ligado a la configuración
vigente y distinto de «inexistente»; respuestas/eventos antiguos sin autoridad sobre operaciones
nuevas; ownership hasta la salida del programa real; reintento después del cierre completo;
aislamiento de clientes/prefixes; identidad estable antes de leer memoria o señalar; rollback
sin mutar archivos usados por procesos vivos.

## Hallazgos confirmados y correcciones

P1 = riesgo alto de afectar procesos/datos o bloquear el ciclo de vida. P2 = estado incorrecto,
operación duplicada o fallo de primera ejecución. Las líneas corresponden al árbol auditado.

### P1 — ownership y procesos

1. **Apagado no equivalía a prefix libre.** `src-tauri/src/tools/runner_sessions/exec.rs:163`
   y `src-tauri/src/tools/prefix/setup.rs:91`. Un controlador retornaba 0 mientras persistían
   hijos; también se anunciaba «Listo» antes del cierre real. La preparación ahora espera
   quiescence antes de escribir el manifiesto/publicar 100%. El shutdown comprueba procesos,
   no sólo exit status. Prueba: `successful_shutdown_controller_is_not_proof_that_prefix_is_clear`.

2. **Una segunda limpieza podía usar shutdown directo y afectar otro lease.**
   `src-tauri/src/tools/runner_sessions/exec.rs:190`. Consumir el lease cambiaba la rama de una
   siguiente llamada. Ahora el modo supervisado es persistente y el control de contadores es
   atómico con adquisición/cierre bajo el lock del prefix. Prueba real:
   `repeated_quiesce_cannot_kill_another_operation`.

3. **Herramientas liberaban guards al salir UMU, no el programa Windows.**
   `src-tauri/src/tools/server_tools/session.rs:313` y `tools/runner_sessions/exec.rs:24`.
   Un handoff vivo permitía otra operación destructiva o el timer idle. Ahora conserva guards
   y lease hasta terminar los programas, incluyendo descendientes del subreaper que borran
   `WINEPREFIX`. Prueba real: `tool_descendant_keeps_its_prefix_guard_after_controller_exit`.

4. **Cierre, arranque y tareas antiguas competían por la entrada del registry.**
   `src-tauri/src/tools/runner_sessions/registry.rs:405`, `:593`, `:713`, `:723`.
   Un reintento encontraba Stopping; una limpieza antigua podía borrar su reemplazo;
   `shutdown_all` omitía arranques aún no insertados; cancelar al caller abandonaba el cierre.
   Ahora comparten lock, el cierre tiene tarea propia, la retirada compara la instancia y
   admisión cierra antes del censo. Hooks usan Weak para evitar ciclos. Fallbacks de cierre son
   paralelos y sólo señalan descendientes con identidad revalidada. Pruebas: reintento inmediato
   con caller abortado (tres ciclos), cierre concurrente, startup pendiente, sesión reemplazada,
   lease compartido y fallo de una sesión preservando otro prefix.

5. **Último cliente y nuevo cliente podían competir por herramientas compartidas.**
   `src-tauri/src/tools/launcher/session.rs:824`. La salida retiraba el último cliente y luego
   detenía herramientas sin el lock de admisión. Ahora finish/cleanup comparten `tool_lifecycle`
   con launch y comandos de herramientas. Los guards se liberan antes de `GAME_EXIT`.
   La revisión de locks y tests de registry/multicliente pasan; la secuencia completa con dos
   juegos Windows queda en la matriz manual pendiente.

6. **Cancelación dejaba reservas o un juego detectado sin registrar.**
   `src-tauri/src/state/game_process.rs:25`, `commands/launcher.rs:25`,
   `tools/launcher/session.rs:441`. La limpieza dependía de recibir un Result; matar sólo el
   controlador no terminaba necesariamente el juego detectado. Guards RAII cancelan únicamente
   la generación Launching y terminan la identidad detectada si no se transfiere al observador
   de salida. TERM escala a KILL sobre esa misma identidad. Pruebas con tareas abortadas,
   promoción a Running, proceso resistente a TERM y otro cliente que permanece vivo.

7. **Identidad podía perderse entre detección, IPC y lectura.**
   `crates/ro-tools-linux/src/wine_process.rs:155`, `proc_memory.rs:87`, `:483`,
   `crates/ro-sessiond/src/supervisor.rs:183`. Se capturaba PID tarde o se validaba sólo alrededor
   de un scan completo. Los candidatos conservan identidad desde antes de leer `/proc`;
   LaunchAccepted transmite `controllerStartTime` antes de reap; cada backend/chunk de lectura
   revalida. Pruebas con start_time obsoleto, proceso real y memoria de un huérfano bajo Yama=1.
   Esto no constituye una prueba de reutilización real del PID durante una syscall.

8. **Autoridad de limpieza de prefix provenía de un snapshot anterior al lock.**
   `src-tauri/src/tools/prefix/setup.rs:82`, `:89`, `:147`. Un arranque que esperaba podía retener
   el dato «directorio vacío» después de otra preparación. Se revalidan permisos/manifiesto y
   el carácter inicial del directorio bajo el guard. Una transacción tiene tarea propia
   (`tools/prefix/mod.rs:9`) para no abandonar limpieza al cancelar IPC. Pasan los tests de
   guards/manifiestos y cancelación del propietario de la transacción. No se reconstruyó un
   prefix del usuario para forzar rollback.

### P2 — UI, protocolo y entorno

9. **Preparación se confundía con lanzamiento; null con entorno ausente.**
   `src/features/launcher/useLaunchGame.ts:75`, `LaunchButton.tsx:95`,
   `src/features/settings/useSelectedRuntimeStatus.ts:16`. Se distingue prepared/ready y
   diagnóstico pendiente/fallido/negativo. El diagnóstico inicial espera settings y servidores,
   usa runner efectivo y no depende de cambiar de servidor. Preparar finaliza sin lanzar ni
   abrir credenciales. Tests para ambos órdenes de carga, reparación, error/reintento y modal.

10. **Resultados antiguos sobrescribían inicialización, selección o persistencia reciente.**
    `src/app/useAppInit.ts:21`, `src/features/settings/settings.store.ts:97`, `:119`, `:167`,
    `src/features/servers/servers.store.ts:56`. Generaciones, revisiones de escrituras y claves
    de configuración limitan la autoridad de cada respuesta; inicialización unifica solicitudes.
    La persistencia de settings es serial y registra el documento completo exitoso (`:50`).
    La prueba intercalada runner/presence falló con `true` frente al `false` persistido antes
    del último arreglo, y pasó después. Incluye respuestas invertidas y fallos antiguos.

11. **Doble clic, desmontaje y eventos tardíos abrían otra operación o resucitaban clientes.**
    `src/features/launcher/useLauncherTask.ts:15`, `launcher.store.ts:41`,
    `refreshGameClients.ts:6`, `useLauncherEvents.ts:31`, `:56`, `:66`,
    `src/shared/hooks/useTauriEvent.ts:19`. Reserva síncrona compartida sobrevive al desmontaje;
    callbacks verifican snapshot vigente; censo usa generación/revisión y tombstones;
    `GAME_EXIT` no cambia el estado de una operación ocupada. Progreso lleva `operationId`.
    El censo inicial espera suscripciones, y listeners desmontados ignoran eventos incluso
    antes de resolver unlisten. Tests: doble clic, desmontaje/remontaje, StrictMode, censo
    invertido, salida anterior al Result, progreso antiguo y modal de otro servidor.

12. **Protocolo podía perder un exit o concatenar frames al cancelar una escritura.**
    `src-tauri/src/tools/runner_sessions/protocol.rs:247`, `:270`, `:395`, `:587`.
    Hooks de contadores preceden a promises; un exit no consumido queda disponible; escritura
    NDJSON tiene propietario, lock, límite y deadline; una línea oversized falla sin esperar
    otro newline. Tests con frame de 128 KiB parcialmente bloqueado, cancelación, siguiente
    mensaje intacto, receiver cancelado, fatal y varios mensajes en una sola escritura.

13. **Bootstrap abandonaba comandos de apagado y ocultaba su resultado.**
    `src-tauri/src/tools/runner_sessions/bootstrap.rs:191`. Pipes sin reader podían llenarse;
    timeout/wait/status se descartaban. Ahora streams van a null, status no-cero es error,
    timeout mata/reapea al hijo propio y kill_on_drop protege cancelación. Tests: 512 KiB de
    salida, exit 17 y timeout de 200 ms con `/proc/pid` ausente después de cleanup.

14. **Sanitización AppImage pisaba PATH explícito del runner.**
    `src-tauri/src/utils/runner.rs:47`, `tools/runner_sessions/client.rs:98`.
    Sanitizar después de overrides hacía divergir ejecución directa/supervisada.
    Ahora sanea primero y aplica después el plan explícito. El sidecar elimina las variables
    de ownership heredadas para no contarse como usuario de otro prefix. Test de PATH y tests
    existentes de env UMU/Wine; sidecar extraído del AppImage ejercitado separadamente.

## Verificación ejecutada

- Frontend: lint, format:check, build y **171 tests en 35 archivos**, todos pasan.
- Rust: fmt --check, clippy workspace/all-targets/all-features con `-D warnings` y test
  workspace/all-features: **497 tests unitarios pasan, cuatro ignorados** por escenarios
  manuales, además de los ejecutables de integración Yama/orphan.
- Supervisor real: handshake, mensajes batched, EOF/protocolo, controlador que sale antes de
  cerrar su pipe heredado, hijos vivos/grace, 100 ciclos sin zombies, memoria del huérfano con
  `kernel.yama.ptrace_scope=1`, leases compartidos y prefixes independientes. Sólo fixtures
  propias `/bin/sh`, `sleep`, `true`, `cat` y orphan-worker; no procesos de partidas.
- `npm run tauri:dev -- --no-watch`: arranque desktop real y verificación visual de HoneyRO
  reconociendo el entorno existente/mostrando Jugar sin alternar servidor. Se reinicia en frío
  tras los edits para no tomar HMR transitorio como validación del producto.
- AppImage generado: ejecutado también en frío mediante `--appimage-extract-and-run`, con
  integración de escritorio desactivada; reconoce HoneyRO y muestra Jugar. No se instaló ni
  integró. El diálogo inicial de AppImageLauncher se canceló sin aceptar integración.
  Las instancias propias de smoke se detuvieron con SIGINT; esto no valida el cierre normal
  de la ventana con clientes vivos. El wrapper AppImage reportó exit 2 tras esa interrupción.
- `npm run tauri:build` y `npm run tauri:build:appimage`: generan bundles bajo
  `target/release/bundle/`, pero **exit 1 al firmar**, por falta de `TAURI_SIGNING_PRIVATE_KEY`.
  No se declararon builds firmados exitosos ni se instaló/publicó nada.
- Source/bundled ro-sessiond comparte Build ID `8b20fed49db28c75c4b8c6ea83a92cc46bc3a075`.
  La sección `.text` también coincide. Su SHA difiere; el bundle añade RUNPATH `$ORIGIN/../lib`. La integración se ejecutó
  contra el sidecar extraído usando `RO_SESSIOND_TEST_BINARY` y pasó, incluidos sus 100 ciclos.
- `git diff --check` y `check-release-invariants` pasan; no cambió ningún archivo de versión.

## Brechas: no equivalen a validación

1. **Matriz Windows/runner:** faltan launch directo, patcher handoff, stop durante launch,
   dos clientes/múltiples prefixes, cierre del launcher y repetición con HoneyRO/Proton-CachyOS 11
   y SakuraRO/Wine 7.16 TkG + DXVK 2.6.2. También abrir realmente OpenSetup/patcher/dgVoodoo
   desde el primer intento. Los fixtures prueban ownership, no comportamiento de esos binarios.
   Se preservó la AppImage existente y su árbol Wine/UMU, y no se lanzaron ni cerraron partidas.

2. **Rollback ensamblado:** forzar fallo y cancelación durante provisioning de un prefix
   desechable administrado, incluyendo salida total de la aplicación, comprobar restauración
   y reap. La prueba de tarea propia cubre cancelación IPC, no supervivencia a detener el runtime
   completo ni un fallo de almacenamiento real. No usar los prefixes existentes como experimento.

3. **Instalado y firma:** proporcionar la clave por el procedimiento de build establecido,
   completar firma y probar el nuevo AppImage instalado en sesión controlada. La inspección y
   ejecución del sidecar extraído no reemplazan esta prueba. Instalación no autorizada aquí.

4. **Límites extremos:** no se forzó D-state, reutilización real de PID ni bloqueo del kernel.
   Los timeouts tienen fase posterior de kill/reap y no prometen resolver procesos
   ininterrumpibles. Los cuatro tests ignorados incluyen acceso/latencia uinput; ejecutarlos
   en su entorno específico antes de afirmar cobertura de esos casos.

Por estas brechas, las correcciones están implementadas y comprobadas con el alcance indicado,
pero la aceptación end-to-end de todas las garantías Windows/distribución no se declara cerrada.

## Seguimiento al preparar la release

Con autorización posterior para commit/push y el bump, se integró por fast-forward el cambio
remoto de README (`cdfd8b0`) sin modificar los archivos auditados. Su CI había fallado con
Rust 1.99 por la deprecación de `AtomicU32::fetch_update`; el toolchain local es 1.98.1.
El decremento saturado de requests ahora usa compare-exchange con el mismo orden SeqCst,
sin silenciar warnings ni exigir una instalación/cambio de toolchain. La prueba
`active_requests_decrement_saturates_under_concurrent_exits` ejerce 16.000 decrementos en
ocho threads sobre 10.000 requests y comprueba saturación y reutilización del contador.
Se repitieron todos los gates locales: 171 tests frontend y 498 unitarios Rust pasan,
cuatro ignorados, además de la integración real del supervisor. Las brechas anteriores
continúan abiertas; preparar una release no las convierte en validación.

El primer CI del commit auditado (`1be2782`, run `37057381956`) pasó clippy con Rust
1.99, pero expuso interferencia entre fixtures unitarias de `ro-sessiond`: un supervisor
recogía mediante `waitpid(-1)` el hijo de otro thread antes de capturar su identidad.
Se reprodujo el mismo error localmente con ocho threads. El fixture ahora retiene ownership
exclusivo de la tabla de hijos y espera ECHILD/join antes de liberarlo; el código productivo
no cambió, pues cada supervisor real ya vive en un proceso separado. La nueva prueba comprueba
reap al destruir el fixture. Pasaron 100 ejecuciones con ocho threads, los gates frontend
completos y los gates Rust completos (499 unitarios, cuatro ignorados, más integración real).
