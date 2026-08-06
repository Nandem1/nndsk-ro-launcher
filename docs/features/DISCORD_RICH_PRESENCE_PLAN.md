# Discord Rich Presence

Planning de investigación e implementación para publicar el estado del cliente de Ragnarok Online en Discord.

| Campo | Valor |
| --- | --- |
| Estado | MVP implementado; formatter visual cerrado; validación Discord manual pendiente |
| Última revisión | 2026-08-05 |
| Alcance | Rich Presence de solo lectura para clientes iniciados por RO-Launcher |
| Decisión bloqueante | Configurar Application ID y validar Discord IPC en el entorno objetivo |

## Objetivo

Mostrar una actividad útil y no invasiva en el perfil de Discord mientras el cliente RO está en juego.

Propuesta visual:

```text
Jugando a SakuraRO
tiny yawn · Nv. 99/70
Port Malaya
01:23:45 transcurrido
```

Discord localiza la palabra `Jugando`/`Playing`. El launcher sólo envía los campos de la actividad.

Para varios clientes se conserva un título estable y se agregan los servidores:

```text
Jugando a Ragnarok Online
2 clientes en juego
SakuraRO · HoneyRO
```

La implementación usa el nombre del servidor como título cuando hay un solo cliente. Para varios clientes usa `Ragnarok Online`, evitando atribuir personaje o mapa a un proceso arbitrario.

## Alcance Inicial

Incluido:

- Nombre del servidor.
- Nombre del personaje.
- Nivel del personaje.
- Mapa o ubicación actual.
- Tiempo transcurrido de la sesión de juego.
- Artwork estático opcional, fuera del MVP beta.
- Limpieza de la actividad al cerrar el último cliente.
- Fallbacks cuando no se pueda leer memoria o Discord no esté disponible.
- Perfiles exactos para los builds confirmados de SakuraRO y HoneyRO.

Fuera del alcance inicial:

- Botones `Join` o `Spectate`.
- Join secrets o invitaciones de Discord.
- Party size inventado a partir de clientes locales.
- Lectura de cuenta, IP, credenciales o archivos del servidor.
- Escritura o inyección en el proceso del juego.
- Una actividad independiente por cada cliente abierto.

## Inventario Actual

### Datos ya disponibles

| Dato | Ubicación actual | Estado |
| --- | --- | --- |
| Servidor | `GameProcessHandle` conserva `server_id` y `server_name` | Disponible |
| Cliente | `GameProcessHandle` conserva `client_id` | Disponible |
| PID | `GameClientSnapshot` y `ProcessIdentity` | Disponible |
| Identidad estable del proceso | `ro-tools-linux::ProcessIdentity` | Disponible |
| Handoff de proceso | `replace_running` en el launcher | Disponible |
| HP/SP | `ClientProfile.hp_base` y layout `+0x474` | Disponible |
| Nombre | `ClientProfile.name_address` | Disponible para perfiles conocidos |
| Nivel | `presence_profiles.json` y `CharacterSnapshot` | Disponible para SakuraRO y HoneyRO |
| Mapa | `presence_profiles.json` y `CharacterSnapshot` | Disponible para SakuraRO y HoneyRO |
| Tiempo de juego | Debe calcularse en el launcher | No requiere memoria |

Referencias relevantes:

- `src-tauri/src/state/game_process.rs`: registro de clientes, identidades, handoff y cierre.
- `src-tauri/src/tools/launcher/session.rs`: detección del cliente, handoff y ciclo de vida.
- `crates/ro-tools-core/src/domain.rs`: `ClientProfile` y offsets derivados.
- `src-tauri/resources/client_profiles.json`: perfiles embebidos actuales.
- `src-tauri/resources/presence_profiles.json`: perfiles de Rich Presence asociados a hash.
- `crates/ro-tools-linux/src/proc_memory.rs`: lectura de memoria y escaneo.
- `src-tauri/src/tools/autopot/scanner.rs`: escaneo incremental de HP y búsqueda de strings.
- `crates/ro-tools-core/src/ports.rs`: abstracción `MemoryReader` reutilizable en tests.

Direcciones actualmente conocidas, que deben seguir validándose por versión de cliente:

| Perfil | HP base | Nombre |
| --- | --- | --- |
| `rathena-default` | `0x10DCE10` | `0x10DF5D8` |
| `infinity` | `0x0146F28C` | `0x01471CD8` |

Aparte, el perfil HoneyRO confirmado (ver sección de resultados) aporta nivel, job, coords y mapa activo:

Rich Presence usa perfiles separados de AutoPot y sólo acepta una coincidencia exacta de hash, tamaño y nombre de ejecutable:

| Perfil | Nombre | Nivel base | Job level | Mapa activo |
| --- | ---: | ---: | ---: | ---: |
| `sakura-ro-ragexe-2025-07-16` | `0x01602568` | `0x015FB9F0` | `0x015FB9F8` | `0x015FB9AC` |
| `honey-ro-ragexe-2018-06-21` | `0x010DF5D8` | `0x010D9400` | `0x010D9408` | `0x010D856C` |

Todas las direcciones de esta matriz son valores `u32` little-endian o strings terminadas en `null`, y están asociadas a los hashes SHA-256 registrados en los resultados. No se usa el buffer SakuraRO `0x0160258A`, el campo adyacente `0x015FB9F4` ni las tablas de recursos `.gat/.rsw` de HoneyRO.

## Decisiones Pendientes

- [x] Usar el nombre del servidor como título cuando hay un solo cliente.
- [x] Mantener `Ragnarok Online` como título agregado para varios clientes.
- [x] Usar una actividad agregada cuando hay varios clientes abiertos.
- [x] Mantener Rich Presence desactivado por defecto y hacerlo opt-in.
- [x] Usar un perfil de memoria independiente de AutoPot.
- [x] Asociar direcciones absolutas al hash y tamaño del build; no usar fallback universal.
- [x] Truncar y limpiar los textos antes de enviarlos a Discord.
- [x] Implementar IPC detrás de `PresenceTransport` para el MVP Linux.
- [x] Omitir artwork en el MVP beta para no depender de Discord Art Assets.
- [ ] Añadir artwork `ro-logo` en una versión posterior si aporta valor.
- [ ] Confirmar relanzamiento HoneyRO y estabilidad RVA como hardening de distribución.

## Investigación Discord

### Ruta oficial

La documentación actual indica que el Game SDK está archivado y recomienda el Discord Social SDK para integraciones nuevas.

El Social SDK permite Rich Presence sin autenticación en escritorio mediante un `Application ID` y un cliente Discord local ejecutándose. El SDK soporta Linux en modo experimental y el API standalone usa C++20.

Implicaciones para este repositorio:

- Crear una aplicación en Discord Developer Portal.
- Habilitar Discord Social SDK para la aplicación.
- Obtener y versionar el SDK usado para compilar.
- Integrar una librería C++ o un helper/sidecar que exponga una interfaz simple al backend Rust.
- Empaquetar la librería Linux junto con el bundle/AppImage.
- Mantener la integración Discord aislada para que un fallo no afecte al lanzamiento del juego.

### Ruta IPC para prototipo

El RPC IPC de Discord documenta sockets locales y el comando `SET_ACTIVITY`. Existe un crate comunitario Rust, `discord-rich-presence`, que implementa el handshake y los payloads habituales.

Esta ruta sirve para validar rápidamente el diseño visual, pero debe considerarse experimental:

- No es la ruta recomendada por la documentación actual para una integración nueva.
- La documentación RPC genérica contiene requisitos de autenticación que hacen menos clara la garantía futura del uso directo de `SET_ACTIVITY`.
- El comportamiento depende del cliente Discord local.
- Hay que revisar la unidad de timestamp: el Social SDK actual documenta segundos Unix, mientras que implementaciones IPC comunitarias pueden manejar milisegundos.
- Debe existir una abstracción de transporte para poder sustituirlo por el Social SDK sin tocar el agregador de presencia.

### Decisión propuesta

El MVP implementa el transporte IPC directamente sobre los sockets locales de Discord, aislado detrás de `PresenceTransport`. El Application ID se obtiene de `RO_LAUNCHER_DISCORD_APPLICATION_ID` o `DISCORD_APPLICATION_ID`, en runtime o durante la compilación, y no es un secreto. El Social SDK oficial queda como sustitución futura del transporte, no como dependencia del dominio ni del lector de memoria.

## Modelo De Actividad

| Campo Discord | Valor propuesto | Fuente | Fallback |
| --- | --- | --- | --- |
| `type` | `Playing` | Constante | No publicar si el transporte no acepta |
| `name` | `<servidor>` con un cliente; `Ragnarok Online` con varios | Servidor/agregador | `Ragnarok Online` |
| `details` | `<personaje> · Nv. <base>/<job>` | Memoria | `En juego` |
| `state` | Nombre amigable del mapa | Catálogo local + memoria | `Ubicación no disponible` |
| `timestamps.start` | Inicio de sesión de juego | Launcher | No enviar hasta detectar estado estable |
| `assets.largeImage` | Opcional, omitido en el MVP beta | Discord Developer Portal | Sin artwork |
| `assets.largeText` | Opcional | Producto | Vacío |
| `assets.smallImage` | Fuera del MVP | Memoria/perfil | Omitir |
| `assets.smallText` | Fuera del MVP | Memoria/perfil | Omitir |

El tiempo será un contador de Discord, no un valor actualizado cada segundo. Se debe conservar el mismo `start` cuando cambie el mapa; sólo se reinicia al cambiar de personaje o comenzar una nueva sesión. Los IDs de mapa se convierten mediante un catálogo local; los mapas desconocidos conservan su ID original.

Los textos deben ser cortos, de una sola línea, truncados de forma segura y sin datos sensibles.

El catálogo de mapas amigables vive en `src-tauri/resources/map_names.json` (~1180 entradas). Se construyó a partir de RateMyServer (nombres cortos de ciudades), Rune-Nifelheim (world map) y tablas del cliente (`navi_map` / `mapnametable`), con overrides curados como `malaya` → `Port Malaya`. Los IDs desconocidos conservan el ID original.

## Investigación De Memoria

### Campos requeridos

| Campo | Preguntas que deben resolverse |
| --- | --- |
| Nombre | ¿Es un buffer directo, un puntero o una estructura? ¿ASCII o UTF-8? |
| Nivel | ¿`u8`, `u16`, `u32` o string? ¿Dirección absoluta o cadena de punteros? |
| Mapa | ¿Nombre directo, puntero, nombre con extensión `.gat` o ID numérico? |
| Build | ¿Qué ejecutable y hash producen esas direcciones? ¿Hay ASLR o relocación? |
| Estado | ¿Cómo distinguir selección de personaje, loading, juego y desconexión? |

### Método de descubrimiento

1. Identificar ejecutable, arquitectura, hash, runner, WINEPREFIX y base del módulo.
2. Validar las direcciones conocidas de HP y nombre contra el cliente correcto.
3. Buscar el nivel con un valor conocido y refinar después de cambiarlo.
4. Validar candidatos con rangos de nivel razonables y cambios reales.
5. Buscar el mapa como string conocido y repetir después de cambiar de mapa.
6. Si el campo es indirecto, documentar cada salto de puntero y el tipo final.
7. Repetir la prueba en selección, carga, juego, cambio de mapa, relog, muerte y cierre.
8. Registrar únicamente candidatos reproducibles en el perfil.

### Matriz mínima de validación

- [ ] Selección de personaje: no mostrar un personaje anterior.
- [ ] Entrada al mapa: detectar nombre y mapa correctos.
- [ ] Cambio de mapa: actualizar sólo después de confirmar el nuevo valor.
- [ ] Subida de nivel: detectar el nuevo nivel sin falsos positivos.
- [ ] Muerte o respawn: mantener o recuperar ubicación correctamente.
- [ ] Desconexión: retirar datos obsoletos.
- [ ] Cierre y relanzamiento: evitar reutilizar un PID o snapshot viejo.
- [ ] Cliente actualizado: detectar perfil incompatible en lugar de mostrar datos falsos.

### Plantilla de evidencia

Cada campo descubierto debe documentarse con esta información:

```text
Perfil:
Ejecutable:
Hash o versión:
Arquitectura:
Runner/WINEPREFIX:
Campo:
Dirección o RVA:
Cadena de punteros:
Tipo de dato:
Encoding:
Longitud máxima:
Estado del cliente en la prueba:
Cómo se descubrió:
Cómo se validó:
Última fecha de verificación:
Confianza: baja / media / alta
```

## Resultados Confirmados

### SakuraRO: sesión de prueba

Estos valores fueron leídos directamente del proceso activo `ragexe.exe` de SakuraRO. No se guarda aquí la ruta local del ejecutable, el WINEPREFIX ni ningún parámetro de conexión.

Identidad del build usada para asociar estas direcciones:

| Metadato | Valor |
| --- | --- |
| Perfil lógico | `sakura-ro-ragexe-2025-07-16` |
| PE build timestamp | `2025-07-16 08:00:00` |
| Formato | PE32/i386 |
| Tamaño | `16,144,384` bytes |
| SHA-256 | `143a5413fd8bd213ccb9362d835971b7ab81d1bd9805d8cfe426508cfd12ec91` |
| Filesystem mtime | `2026-07-09` (metadato local, no equivale al build timestamp) |

El hash y el timestamp identifican el build sin guardar la ruta local del ejecutable.

| Campo | Dirección | Tipo observado | Resultado de prueba | Confianza actual |
| --- | --- | --- | --- | --- |
| Nivel base | `0x015FB9F0` | `u32` little-endian | `99` | Alta para este build/sesión |
| Campo adyacente | `0x015FB9F4` | `u32` little-endian | `3` -> `48` -> `64` | Pendiente de identificar |
| Nivel job | `0x015FB9F8` | `u32` little-endian | `70`; luego `1` -> `7` | Alta para este build/sesión |
| Coordenada X | `0x015E8184` | `u32` little-endian | `242` -> `239`; luego `161` -> `123`; luego `119` -> `57`; relanzado `232` | Alta para este build |
| Coordenada Y | `0x015E8188` | `u32` little-endian | `205` -> `204`; luego `178` -> `61`; luego `69` -> `193`; relanzado `194` | Alta para este build |
| Nombre | `0x01602568` | string terminada en `null` | Personaje A -> Personaje B | Alta para este build/sesión |
| Mapa activo | `0x015FB9AC` | string terminada en `null` | `prontera` -> `geffen` -> `gef_fild00` -> `malaya` | Alta para este build |
| Buffer secundario de recurso/mapa | `0x0160258A` | string terminada en `null` | `malaya.rsw` -> `1@gldh.rsw` -> `prontera.rsw` | No usar como mapa activo |

Observaciones:

- `0x015FB9AC` contiene el mapa activo (`prontera`, `geffen` y luego `gef_fild00`) y está próximo a la estructura de niveles.
- `0x0160258A` equivale a `nameAddress + 0x22`, pero contiene recursos distintos al mapa visible (`malaya.rsw` y luego `1@gldh.rsw`); se clasifica como buffer secundario.
- El mapa puede aparecer con o sin extensión `.rsw`; el lector debe normalizar ambas formas a un ID como `malaya` o `prontera`.
- `Port Malaya` y `Prontera, Capital of Rune Midgard` son nombres amigables que deben resolverse desde el ID interno, no necesariamente desde la memoria.
- Las coordenadas cambiaron al moverse y volvieron a cambiar al cambiar de mapa; el mismo par numérico puede existir en mapas diferentes.
- El bloque de nivel contiene `99`, `3` y `70` en el personaje de nivel alto, y `1`, `48` y `1` antes de progresar en el personaje nuevo.
- Después de progresar a `6/7`, el mismo bloque pasó a `6`, `64` y `7`; Base Level y Job Level quedan confirmados, pero el campo intermedio requiere una prueba independiente.
- El ejecutable observado es PE32/i386 y el módulo principal estaba cargado con base `0x00400000`.
- Después de cerrar y relanzar el cliente, con un PID diferente, las mismas direcciones volvieron a entregar `99`, `3`, `70`, el mapa activo `malaya` y coordenadas válidas.
- Tras el relanzamiento, el buffer secundario pasó a contener `prontera.rsw` mientras el mapa activo era `malaya`; esto confirma que ambos campos tienen propósitos distintos.

Evidencia adicional después de cambiar de personaje, sin cambiar el PID:

```text
Nombre: Personaje A -> Personaje B
Mapa activo: prontera -> geffen
Nivel base/job: 99/70 -> 99/70
Coordenadas: 161,178 -> 123,61
```

Esto confirma que nombre, mapa activo, coordenadas y HP/SP se actualizan en las mismas estructuras entre personajes.

Prueba de niveles con personaje nuevo:

```text
Antes: 0x015FB9F0=1, 0x015FB9F4=48, 0x015FB9F8=1
Después: 0x015FB9F0=6, 0x015FB9F4=64, 0x015FB9F8=7
```

### HoneyRO: sesión de prueba

Estos valores fueron leídos directamente del proceso activo `HoneyRO.exe`. No se guarda aquí la ruta local del ejecutable, el WINEPREFIX ni ningún parámetro de conexión.

Identidad del build usada para asociar estas direcciones:

| Metadato | Valor |
| --- | --- |
| Perfil lógico | `honey-ro-ragexe-2018-06-21` |
| PE build timestamp | `2018-06-21 06:43:21` |
| Formato | PE32/i386 |
| Tamaño | `21,012,480` bytes |
| SHA-256 | `b3b9d0311d74b097ec1a7a354d5a23f6f706e0170bb5e6826f1f335412e705cb` |
| Filesystem mtime | `2026-07-15` (metadato local, no equivale al build timestamp) |
| Módulo base | `0x00400000` |

El hash y el timestamp identifican el build sin guardar la ruta local del ejecutable.

| Campo | Dirección | Tipo observado | Resultado de prueba | Confianza actual |
| --- | --- | --- | --- | --- |
| Nombre | `0x010DF5D8` | string terminada en `null` | Personaje A -> Personaje B | Alta para este build/sesión |
| Nivel base | `0x010D9400` | `u32` little-endian | `8` -> `10` | Alta para este build/sesión |
| Job level | `0x010D9408` | `u32` little-endian | `5` -> `7` | Alta para este build/sesión |
| Coordenada X | `0x010C5CA4` | `u32` little-endian | `137` -> `141` -> `138` -> `145` | Alta para este build |
| Coordenada Y | `0x010C5CA8` | `u32` little-endian | `114` -> `110` -> `113` -> `130` | Alta para este build |
| Mapa activo | `0x010D856C` | string terminada en `null` | `prontera` -> `xmas` | Alta para este build |

Observaciones:

- `0x010DF5D8` (nombre) y las cadenas de mapa se encontraron comparando la memoria entre cambios de mapa y de personaje en una ciudad poco poblada.
- `0x010C5CA4`/`0x010C5CA8` son coordenadas del jugador; `0x010C5CA4` cambió `137` -> `141` -> `138` -> `145` al moverse, y `0x010C5CA8` `114` -> `110` -> `113` -> `130`.
- `0x010D9400`/`0x010D9408` son nivel base y job; con personaje nuevo pasaron de `8/5` a `10/7`. El bloque `0x010DF4AC` con varios `99`/`70` repetidos es un array estático y NO es el nivel del jugador.
- `0x010D856C` es el mapa activo como string (`prontera` -> `xmas`). Direcciones que apuntan a `.gat`/`.rsw` de `payon`/`aldebaran` son tablas de recursos de mapas cargados, no el mapa activo.
- Los valores de nivel se validaron con dos personajes distintos (uno nuevo `8/5` subiendo a `10/7`) y con el personaje de nivel alto.

### Pruebas todavía necesarias (HoneyRO)

- [ ] Cerrar y relanzar HoneyRO para confirmar estabilidad de las direcciones.
- [x] Cambiar de mapa y comprobar que cambia `0x010D856C`.
- [x] Comprobar que X/Y se actualizan al moverse y cambiar de mapa.
- [x] Seleccionar otro personaje y confirmar que nombre/nivel/mapa no conservan datos anteriores.
- [ ] Confirmar si la base `0x00400000` es estable o si el perfil debe expresarse como RVA.

### Pruebas todavía necesarias

- [x] Cerrar y relanzar SakuraRO para confirmar estabilidad de las direcciones.
- [x] Cambiar de mapa y comprobar que cambia `0x015FB9AC`.
- [x] Comprobar que `0x0160258A` es un buffer secundario y no el campo que debe leer el launcher.
- [x] Comprobar que X/Y se reinician o actualizan correctamente al cambiar de mapa.
- [x] Seleccionar otro personaje y confirmar que nombre/nivel/mapa no conservan datos anteriores.
- [ ] Confirmar si el campo `0x015FB9F4` es job ID, clase u otro dato.
- [x] Registrar hash/versionado del ejecutable para asociar el perfil al build correcto.
- [ ] Confirmar si la base `0x00400000` es estable o si el perfil debe expresarse como RVA.

## Perfil De Memoria Propuesto

No añadir direcciones desnudas al servicio de Discord. La implementación separa el perfil de AutoPot y carga `src-tauri/resources/presence_profiles.json` en un modelo tipado:

```text
PresenceMemoryProfile {
  id: String
  exe_names: Vec<String>
  executable_sha256: String
  pe_build_timestamp: String
  image_size: u64
  module_base: u32
  name_address: u32
  level_address: u32
  job_level_address: u32
  map_address: u32
}

CharacterSnapshot {
  character_name: Option<String>
  level: Option<u32>
  job_level: Option<u32>
  map_name: Option<String>
  state: CharacterState
  sampled_at: Instant
}
```

El resolver exige nombre de ejecutable, tamaño y SHA-256 exactos. Si el perfil no coincide, el snapshot queda vacío y la actividad usa el fallback del servidor. Las direcciones son absolutas para los builds confirmados; RVA y detección dinámica quedan para hardening posterior.

## Arquitectura Propuesta

```text
GameProcessHandle
        |
        v
PresenceHandle
        |
        +-- TrackedClient por client_id
        |       +-- server metadata
        |       +-- ProcessIdentity
        |       +-- MemoryProfile
        |       +-- CharacterSnapshot
        |
        +-- Aggregator de una única actividad Discord
        |
        +-- PresenceTransport
                +-- IPC experimental
                +-- Social SDK oficial
```

Ubicación implementada: `src-tauri/src/tools/presence/`, con el modelo de memoria en `crates/ro-tools-core/src/presence.rs`.

Responsabilidades:

- Un worker dedicado dueño de la conexión Discord y del agregado de clientes.
- Un monitor de memoria por cliente en un hilo separado del runtime async.
- Verificación de `ProcessIdentity` antes y después de leer.
- Reapertura del lector después de un handoff de proceso.
- Dos muestras estables antes de publicar datos dinámicos.
- Debounce, heartbeat y rate limiting de actualizaciones.
- Reconexión mediante reintentos del transporte IPC.
- Limpieza explícita al cerrar el último cliente o la aplicación.
- Errores Discord no fatales para el juego.

Puntos de integración previstos:

- Crear la presencia después de `mark_running`.
- Actualizarla después de `replace_running`.
- Retirar el cliente después de `game.finish`.
- Ejecutar `clear_activity` en el evento de salida de Tauri.
- Añadir `PresenceHandle` al `GameState`.
- Propagar `richPresenceEnabled` desde `AppSettings` al worker.

## Política Multi-Cliente

Discord ofrece una actividad principal por usuario, no una por proceso del mismo launcher.

Política recomendada:

- Un cliente: mostrar servidor, personaje, nivel, mapa y tiempo.
- Varios clientes: mostrar `N clientes en juego` y los servidores, sin atribuir personaje/mapa a uno arbitrario.
- La actividad agregada de varios clientes no publica un timestamp individual.
- Alternativa futura: seleccionar explícitamente un cliente representativo.
- No usar `sole_running_pid`, porque presencia debe funcionar aunque haya varios clientes.
- Usar `client_id` como clave lógica y `ProcessIdentity` para protegerse contra reutilización de PID.

## Fases De Trabajo

### Fase 0: Cerrar decisiones de producto

- [x] Confirmar formato final de las tres líneas.
- [x] Confirmar comportamiento multi-cliente.
- [x] Confirmar privacidad y valor por defecto del toggle.
- [x] Confirmar que el nombre superior será estable.
- [x] Confirmar que no habrá botones Join en la primera versión.

Criterio de salida: payload y política de fallback aprobados. Cumplido.

### Fase 1: Spike de Discord

- [ ] Crear aplicación y configurar el Application ID mediante variable de entorno.
- [x] Dejar artwork fuera del primer build beta.
- [x] Implementar transporte IPC aislado con payload estático/dinámico.
- [x] Mantener el dominio independiente del Social SDK oficial.
- [ ] Probar Discord cerrado, reinicio, invisible y actividad desactivada.
- [ ] Medir comportamiento real de reconexión y limpieza.

Criterio de salida: una actividad aparece y desaparece correctamente sin afectar al juego. Pendiente de Application ID y prueba manual.

### Fase 2: Perfiles de memoria confirmados

- [x] Registrar los perfiles SakuraRO y HoneyRO por hash y tamaño.
- [x] Confirmar encoding, tipos y campos usados por el MVP.
- [x] Separar perfiles de Rich Presence de AutoPot.
- [x] Marcar perfiles no confirmados como incompatibles, no como valores por defecto.
- [ ] Confirmar RVA/ASLR y relanzamiento HoneyRO como hardening de distribución.

Criterio de salida: ambos clientes objetivo entregan nombre, nivel y mapa de forma reproducible. Cumplido para los builds documentados.

### Fase 3: Modelo y lector de snapshots

- [x] Extraer el modelo de snapshot a `ro-tools-core`.
- [x] Añadir campos tipados al perfil de memoria.
- [x] Implementar decoders de `u32` y strings.
- [x] Implementar validación semántica de nivel y mapa.
- [x] Añadir mocks de `MemoryReader` y tests unitarios.
- [x] Evitar publicar snapshots hasta obtener dos muestras estables.

Criterio de salida: un snapshot válido se puede producir sin depender de AutoPot. Cumplido.

### Fase 4: Servicio de presencia

- [x] Crear `PresenceHandle` y canal de comandos.
- [x] Implementar agregación de clientes.
- [x] Implementar formatter de actividad.
- [x] Añadir timestamp de sesión.
- [x] Añadir heartbeat, reintentos y rate limiting.
- [x] Añadir manejo de errores no bloqueante.
- [x] Añadir limpieza en cierre, crash y último cliente.

Criterio de salida: el launcher publica datos reales y nunca impide iniciar o cerrar el juego. Cumplido en tests; falta prueba manual.

### Fase 5: Integración del ciclo de vida

- [x] Integrar `mark_running`.
- [x] Integrar handoff de PID.
- [x] Integrar `finish` y `EVENT_GAME_EXIT`.
- [x] Integrar salida de Tauri.
- [x] Verificar varios clientes y cierre individual mediante el agregador.
- [x] Evitar que snapshots viejos sobrevivan a una nueva identidad de proceso.

Criterio de salida: no hay presencia stale después de cierres o handoffs. El relanzamiento HoneyRO queda para validación manual.

### Fase 6: UI, empaquetado y distribución

- [x] Añadir toggle de Rich Presence.
- [x] No mostrar ni pedir tokens OAuth.
- [x] Mantener la integración IPC sin sidecar adicional.
- [ ] Mostrar estado opcional de conexión Discord.
- [x] No requerir artwork para distribución beta.
- [x] Probar `npm run tauri:build:appimage`.
- [ ] Probar una instalación limpia sin Discord.
- [ ] Documentar artwork, permisos y limitaciones Linux.

Criterio de salida: AppImage funcional con Discord instalado y degradación limpia sin Discord. Pendiente de validación de bundle.

## Manejo De Fallos

| Situación | Comportamiento |
| --- | --- |
| Discord no ejecutándose | El juego continúa; presencia queda pendiente |
| Application ID no configurado | Rich Presence queda desactivado; el juego continúa |
| Discord se cierra | Reintentar con backoff; no mostrar error bloqueante |
| Memoria no accesible | Fallback a servidor/estado genérico |
| Perfil incompatible | No publicar nivel/mapa incorrectos; registrar diagnóstico |
| PID desaparece | Esperar handoff antes de limpiar si el launcher lo detecta |
| Cliente termina | Retirar cliente y recalcular actividad agregada |
| Último cliente termina | Limpiar la actividad de Discord |
| Varios clientes | Aplicar política agregada definida en Fase 0 |

## Seguridad Y Compatibilidad

- Rich Presence es información pública del perfil de Discord.
- No publicar PID, rutas, WINEPREFIX, IP, cuenta ni credenciales.
- El Application ID no es un secreto; no incluir client secrets ni tokens.
- La lectura debe ser de solo lectura y respetar las políticas del servidor.
- Documentar que algunos anti-cheats pueden detectar lectura externa de memoria.
- No ejecutar escaneos completos de memoria en cada tick.
- Validar strings, límites, caracteres de control y truncado antes de enviarlos.
- Mantener el juego funcional aunque falle toda la integración Discord.

## Pruebas De Aceptación

- [ ] Cliente único real: servidor, personaje, nivel, mapa y tiempo correctos.
- [x] El contador continúa sin enviar actualizaciones por segundo.
- [x] Cambiar de mapa conserva el timestamp y actualiza la ubicación tras muestras estables.
- [ ] Subir de nivel actualiza sólo el nivel en cliente real.
- [x] Un cambio de personaje reinicia el timestamp del snapshot.
- [x] Una identidad de proceso nueva descarta snapshots anteriores.
- [ ] Entrar a selección no muestra el personaje anterior en los clientes reales.
- [x] Cerrar el último cliente solicita limpiar la actividad.
- [ ] Reiniciar Discord recupera la actividad en entorno real.
- [x] Abrir dos clientes no muestra datos del cliente equivocado en el agregador.
- [x] Handoff de PID reabre el contexto de presencia.
- [x] Un perfil incompatible produce fallback, no datos falsos.
- [x] Discord ausente no cambia el resultado de `launch_game`.
- [x] AppImage incluye todos los recursos necesarios.

## Fuentes

Documentación oficial consultada el 2026-08-04:

- [Discord Rich Presence](https://docs.discord.com/developers/platform/rich-presence.md)
- [Setting Rich Presence con Social SDK](https://docs.discord.com/developers/discord-social-sdk/development-guides/setting-rich-presence.md)
- [Platform Compatibility](https://docs.discord.com/developers/discord-social-sdk/core-concepts/platform-compatibility.md)
- [RPC over IPC](https://docs.discord.com/developers/topics/rpc.md)
- [Rich Presence Best Practices](https://docs.discord.com/developers/rich-presence/best-practices.md)
- [Social SDK Release Cadence and Support](https://docs.discord.com/developers/discord-social-sdk/core-concepts/release-cadence-and-support.md)
- [Game SDK archivado](https://docs.discord.com/developers/developer-tools/game-sdk)

Referencias externas:

- [4RTools: Client.cs](https://github.com/4RTools/4RTools/blob/main/Model/Client.cs)
- [discord-rich-presence en docs.rs](https://docs.rs/discord-rich-presence/latest/discord_rich_presence/)

## Registro De Investigación

| Fecha | Tema | Resultado | Acción siguiente |
| --- | --- | --- | --- |
| 2026-08-04 | Estado de Discord SDK/RPC | Social SDK recomendado; Game SDK archivado; Linux experimental | Confirmar estrategia de bridge/sidecar |
| 2026-08-04 | Repo | Servidor, proceso, HP y nombre disponibles; nivel/mapa pendientes | Investigar perfiles de memoria |
| 2026-08-04 | Diseño de payload | `name`, `details`, `state` y `timestamps.start` cubren el objetivo | Validar visualmente con datos reales |
| 2026-08-04 | Perfil HoneyRO | Nombre, nivel base, job, coords y mapa activo confirmados en `HoneyRO.exe` | Cerrar y relanzar para confirmar estabilidad |
| 2026-08-05 | Implementación MVP | Perfiles por hash, snapshots seguros, worker agregado, IPC y toggle persistente integrados | Configurar Application ID y ejecutar pruebas manuales |
| 2026-08-05 | Formatter visual | Un cliente usa servidor + `Nv. base/job` + mapa amigable; varios clientes usan `Ragnarok Online` agregado sin personaje/mapa | Validar visualmente en Discord con Application ID |

Cada nueva dirección o comportamiento confirmado debe añadirse al registro y a la sección de perfil de memoria antes de implementarse.
