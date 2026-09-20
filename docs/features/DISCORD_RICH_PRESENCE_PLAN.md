# Discord Rich Presence

> El nombre del archivo se conserva por compatibilidad. El MVP está implementado; este documento
> describe el comportamiento vigente y la validación que todavía debe repetirse en un cliente real.

| Campo            | Estado                                                                  |
| ---------------- | ----------------------------------------------------------------------- |
| Alcance          | Rich Presence de sólo lectura para clientes iniciados por RO-Launcher   |
| Activación       | Opt-in; `richPresenceEnabled` es `false` por defecto                    |
| Transporte       | Discord RPC sobre sockets IPC locales                                   |
| Configuración    | `RO_LAUNCHER_DISCORD_APPLICATION_ID` por environment de runtime o build |
| Código           | Implementado y cubierto por tests                                       |
| Pendiente manual | Confirmar visualmente publish/reconnect/clear con Discord real          |

## 1. Resultado de producto

Con un cliente, la actividad usa el servidor como título y publica los datos estables disponibles:

```text
SakuraRO
tiny yawn · Nv. 99/70
Port Malaya
01:23:45 transcurrido
```

Sin un snapshot confiable se degrada a `En juego` / `Ubicación no disponible`. Con varios clientes
no atribuye personaje ni mapa a uno arbitrario:

```text
Ragnarok Online
2 clientes en juego
HoneyRO · SakuraRO
```

La actividad agregada no publica un timestamp individual. Al cerrar el último cliente o desactivar
la feature, el worker solicita limpiar la actividad; al cerrar la aplicación termina también el
proceso al que Discord asocia la actividad.

Quedan fuera del MVP: Join/Spectate, secrets, invitaciones, party size inventado, una actividad por
proceso y artwork obligatorio.

## 2. Arquitectura

| Componente                                                                       | Responsabilidad                                 |
| -------------------------------------------------------------------------------- | ----------------------------------------------- |
| [`ro-tools-core/presence.rs`](../../crates/ro-tools-core/src/presence.rs)        | Perfil, snapshot, sanitización y lectura tipada |
| [`tools/presence/profiles.rs`](../../src-tauri/src/tools/presence/profiles.rs)   | Match del ejecutable y overrides                |
| [`tools/presence/service.rs`](../../src-tauri/src/tools/presence/service.rs)     | Worker, sampling, agregación y lifecycle        |
| [`tools/presence/transport.rs`](../../src-tauri/src/tools/presence/transport.rs) | Handshake y `SET_ACTIVITY` de Discord RPC       |
| [`presence_profiles.json`](../../src-tauri/resources/presence_profiles.json)     | Perfiles exactos embebidos                      |
| [`map_names.json`](../../src-tauri/resources/map_names.json)                     | Nombres públicos de mapas                       |

`PresenceHandle` vive en `GameState` y posee un worker dedicado. El worker recibe comandos para
enable, register, overrides, handoff, unregister y shutdown. Un error de Discord o memoria nunca
cambia el resultado del lanzamiento del juego.

La lectura reutiliza `MemorySessionRegistry`; no abre otro modelo de permisos ni escribe memoria.
El contrato del supervisor y Yama está en
[`PTRACE_SESSION_SUPERVISOR_PLAN.md`](../PTRACE_SESSION_SUPERVISOR_PLAN.md).

## 3. Perfiles de memoria

Los perfiles Rich Presence están separados de AutoPot. Un perfil embebido sólo hace match por:

- nombre de ejecutable permitido;
- tamaño exacto;
- SHA-256 exacto.

Los dos builds curated son:

| Perfil                        | Ejecutable    | SHA-256                                                            |     Tamaño |
| ----------------------------- | ------------- | ------------------------------------------------------------------ | ---------: |
| `sakura-ro-ragexe-2025-07-16` | `ragexe.exe`  | `143a5413fd8bd213ccb9362d835971b7ab81d1bd9805d8cfe426508cfd12ec91` | 16.144.384 |
| `honey-ro-ragexe-2018-06-21`  | `HoneyRO.exe` | `b3b9d0311d74b097ec1a7a354d5a23f6f706e0170bb5e6826f1f335412e705cb` | 21.012.480 |

Las direcciones exactas de nombre, base/job level y mapa son autoridad del JSON embebido; no se
duplican aquí. Los valores fueron confirmados para esos builds PE32 con módulo base `0x00400000`.
Otro hash no hereda sus direcciones.

Orden de resolución:

1. perfil exacto por ejecutable/tamaño/hash;
2. overrides explícitos completos de nombre, nivel y mapa;
3. familias derivadas conocidas como candidatas a partir de name/HP;
4. sin evidencia suficiente, fallback público sin datos de personaje.

Un perfil exacto no se reemplaza en caliente con overrides. Los candidatos derivados sólo quedan
fijados después de producir muestras válidas y estables.

### Lectura y validación

Cada snapshot puede contener nombre, base level, job level y mapa. Los strings se limitan, limpian y
normalizan; niveles fuera de `1..=300` se descartan. El mapa acepta el ID interno y elimina variantes
`.gat`/`.rsw`; el catálogo lo transforma a un label amigable y preserva IDs desconocidos para no
inventar ubicaciones.

El worker:

- muestrea cada 2 segundos;
- valida `ProcessIdentity` antes y después de leer;
- exige dos snapshots iguales antes de publicar datos dinámicos;
- descarta el snapshot tras tres muestras inválidas;
- reinicia el timestamp cuando cambia el personaje;
- limpia estado anterior ante un handoff de identidad;
- usa fallback si la sesión de memoria no está disponible o no es usable.

## 4. Transporte Discord IPC

El Application ID se busca en este orden:

1. `RO_LAUNCHER_DISCORD_APPLICATION_ID` del proceso;
2. `DISCORD_APPLICATION_ID` del proceso (alias);
3. los mismos nombres capturados en build mediante `option_env!`.

El ID no es un secret, pero no se aceptan client secrets, OAuth tokens ni credenciales. Sin ID, el
transporte queda unavailable y el juego continúa.

El transporte busca `discord-ipc-0`…`discord-ipc-9` bajo `XDG_RUNTIME_DIR`, `/run/user/<uid>` y
`/tmp`; realiza handshake v1 y envía `SET_ACTIVITY` con PID del launcher. Las respuestas están
limitadas a 1 MiB y tienen timeout de lectura/escritura. Un error cierra el stream para forzar una
reconexión posterior.

Política temporal:

- retry: 5 segundos;
- heartbeat: 30 segundos;
- no hay updates por segundo para el contador;
- payload sin cambios no se vuelve a publicar fuera de heartbeat/retry.

El payload usa `name`, `details`, `state` y `timestamps.start`. Artwork se omite mientras no haya un
asset registrado y una decisión de producto explícita.

## 5. Lifecycle

```text
settings load -> SetEnabled
cliente Running -> Register(identity, server, exe, overrides)
patcher handoff -> Handoff(new identity)
cambio de config -> ApplyOverrides(server)
cliente termina -> Unregister(client)
último cliente / disable -> clear_activity
cierre Tauri -> Shutdown
```

`client_id` es la clave lógica y `(pid, start_time)` protege el acceso al proceso. Handoff invalida
snapshots y vuelve a estabilizar la lectura. Cerrar un cliente recalcula la actividad agregada sin
afectar los demás.

El toggle se persiste con settings. Desactivarlo limpia la actividad; reactivarlo vuelve a agregar
los clientes registrados.

## 6. Privacidad y fallos

Sólo se publica servidor, personaje, nivel/job, mapa y timestamp de sesión. Se eliminan caracteres
de control y se truncan límites públicos antes de formar el payload.

Nunca se publica PID, client/server ID interno, path, prefix, runner, IP, cuenta, credenciales ni
contenido arbitrario de memoria. La integración no hace inyección, escritura, escaneo completo por
tick ni modificación del cliente o Gepard.

| Fallo                                 | Comportamiento                              |
| ------------------------------------- | ------------------------------------------- |
| Discord no está abierto o se reinicia | error no fatal; retry y reconnect           |
| Application ID ausente                | transporte unavailable; juego normal        |
| Memoria sin permiso                   | fallback de servidor/estado genérico        |
| Perfil incompatible                   | no publicar datos dinámicos                 |
| PID termina o se reutiliza            | invalidar snapshot; esperar handoff/cleanup |
| Cliente termina                       | retirar sólo ese cliente                    |
| Último cliente termina                | limpiar actividad                           |

## 7. Estado de validación

Confirmado por tests y código integrado:

- formato de uno y varios clientes;
- sanitización, truncado, niveles/mapas y dos muestras estables;
- identidad stale, handoff, cleanup del último cliente y shutdown del worker;
- heartbeat/retry y error no bloqueante;
- toggle persistido y recursos dentro del AppImage;
- perfiles exactos SakuraRO/HoneyRO y fallbacks conservadores.

No consta en el repositorio una sesión manual completa con Discord visible. Antes de declarar una
release validada para esta feature se debe comprobar con el AppImage instalado:

- publish y clear con un cliente real;
- cambio de mapa/nivel/personaje;
- cierre y reinicio de Discord;
- selección de personaje sin datos stale;
- dos clientes y cierre individual;
- ausencia de Discord sin impacto en launch.

El Application ID debe inyectarse al build o al proceso mediante environment, por ejemplo:

```bash
RO_LAUNCHER_DISCORD_APPLICATION_ID=<id> npm run tauri:build:appimage
```

No se hardcodea en settings ni se solicita al usuario final.
