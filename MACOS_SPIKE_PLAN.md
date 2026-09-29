# Plan de prueba: laboratorio macOS, fase 1

| Campo | Valor |
| ----- | ----- |
| Estado | Propuesta. No es un ADR y no cambia el runtime vigente. |
| Fecha | 2026-09-23 |
| Alcance | Un laboratorio reutilizable en Apple Silicon para lanzar el mismo cliente por dos backends de Direct3D 11. |
| Fuera de este cambio | Código, pins con hash, integración en `GraphicsProfile`, catálogo de compatibilidad. |
| Revisión esperada | Cerrar las ambigüedades `A1`–`A6` con Codex antes de implementar. |

La autoridad del producto sigue siendo [`AGENTS.md`](AGENTS.md). La arquitectura vigente sigue en
[`docs/RO_RUNTIME_ARCHITECTURE_PLAN.md`](docs/RO_RUNTIME_ARCHITECTURE_PLAN.md). Este archivo sólo
describe la primera prueba de macOS.

## 1. Objetivo

Comprobar si un cliente de Ragnarok ya instalado llega a la pantalla de login en Apple Silicon
cuando DirectDraw sigue yendo por la instalación actual de dgVoodoo y Direct3D 11 lo consume un
backend intercambiable.

El laboratorio tiene que poder repetirse: mismo juego, mismo dgVoodoo, dos ranuras, y un cambio de
ranura que no borre la otra. La fase 1 no porta el launcher.

Una corrida que llega a login valida la hipótesis gráfica para ese hash y esa ranura. No crea un
record `Validated`, no añade una variante a `GraphicsProfile` y no autoriza a cambiar el runner de
Linux.

## 2. Fuera de alcance

- Autopot, autobuff, spammer, presencia, `ro-inputd` y `ro-sessiond`.
- Leer memoria, inyectar teclas o debilitar la política del host.
- Proton, UMU, el Wine 7.16 portable y cualquier prefix creado en Linux.
- Copiar el directorio del juego o reinstalar dgVoodoo.
- Añadir `D3D8.dll` o `D3D9.dll` de dgVoodoo. La plantilla actual no los trae.
- D7VK. ADR-005 lo cerró en `no-go`: otro `ddraw.dll` no sustituye al de dgVoodoo.
- D3DMetal, CrossOver y Game Porting Toolkit.
- Patcher, OpenSetup y WebView2. La primera serie lanza el ejecutable directo.
- Integrar la ranura ganadora en Tauri, en el catálogo de ADR-004 o en el enum de ADR-001.

## 3. Hechos que esta prueba no reabre

- ADR-001 sólo admite `Dxvk` y `DgVoodooDxvk`. Una topología nueva necesita variante, evidencia y
  tests. Este laboratorio acumula esa evidencia y no declara la variante.
- ADR-002 separa el prefix del overlay. dgVoodoo vive en el directorio del juego y no multiplica
  prefixes. El material gráfico que el prefix copia al nacer sí forma parte de su identidad.
- ADR-004 no usa el perfil gráfico para decidir compatibilidad. El sujeto de un record curado es el
  SHA-256 de `gepard.dll`. Una observación local no se promueve sola.
- ADR-005 mostró que Gepard `26.9.3.1` rechazó un `ddraw.dll` distinto y toleró el de dgVoodoo. La
  prueba reutiliza esos archivos. No los reemplaza, mueve ni disfraza.
- La plantilla empaquetada, en `crates/ro-tools-core/src/dgvoodoo.rs`, es `D3DImm.dll`,
  `DDraw.dll`, `dgVoodoo.conf` y `dgVoodooCpl.exe`. El instalador de
  `src-tauri/src/tools/server_tools/dgvoodoo.rs` escribe el manifest
  `.ro-launcher-dgvoodoo.json` en el directorio del juego.
- `src-tauri/resources/dgvoodoo/dgVoodoo.conf` deja `OutputAPI = bestavailable`. Ese valor puede
  elegir Direct3D 12. DXMT sólo consume Direct3D 11.
- El cliente es un PE de 32 bits. Las DLL que cargue el proceso tienen que ser PE32. El puente Unix
  de DXMT es de 64 bits y tiene que estar compilado contra el Wine de la ranura.
- No se modifica, engancha, desactiva, suplanta ni evade Gepard, el ejecutable, los paquetes ni la
  validación del servidor.

## 4. Laboratorio

El laboratorio es un directorio nuevo, fuera del repositorio y fuera de cualquier prefix de Linux.
Su raíz se indica con `RO_LAUNCHER_MACOS_SPIKE_ROOT`. No hay default: un default podría caer sobre
datos ya existentes.

```text
$RO_LAUNCHER_MACOS_SPIKE_ROOT/
  runner-pristine/              Wine de Gcenx verificado, sin backends aplicados
  slots/
    dxmt/
      wine/                     copia del pristine más los builtin de DXMT
      prefix/                   WINEPREFIX creado con ese wine
      slot.json
    dxvk-moltenvk/
      wine/                     copia del pristine más DXVK y MoltenVK
      prefix/
      slot.json
  active-slot                   una línea: dxmt o dxvk-moltenvk
  subject.json                  ruta del juego, ejecutable y hashes observados
  runs/
    <utc>-<slot>/
      evidence.json
      stderr.log
```

Ejemplo de raíz, no un default: `~/Library/Application Support/ro-launcher/macos-spike`.

`subject.json` apunta al directorio de juego que ya tiene dgVoodoo. El laboratorio no copia ese
directorio ni lo adopta como prefix.

## 5. Runner

Hay un solo pin de Wine y dos árboles derivados.

1. Comprobar Rosetta en el Mac. Sin Rosetta no se descarga nada.
2. Instalar el Wine de Gcenx en `runner-pristine/` y verificar tamaño y digest del pin. Si el
   digest no coincide, borrar el staging y conservar el pristine anterior si existía.
3. Para cada ranura, copiar el pristine a `slots/<id>/wine` y aplicar sólo el backend de esa
   ranura.
4. No lanzar el pristine. No apuntar una ranura a un `Wine.app` del sistema ni a un prefix ya
   poblado.

El pin concreto (versión, URL, SHA-256 y prueba de que el build incluye WoW64 i386) es `A1`. Sin
ese pin no hay instalación.

## 6. Prefix

Cada ranura tiene su propio prefix, creado con el `wineboot` de su propio `wine/`. El prefix nace
vacío de archivos de juego.

Hace falta uno por ranura porque el primer `wineboot` copia los builtin al prefix. DXMT y DXVK no
pueden ser dueños a la vez de `d3d11.dll`. Cambiar las DLL del Wine después de crear el prefix no
demuestra qué DLL quedó copiada dentro. Por eso el cambio de backend no es un parche in-place sobre
un único prefix: es elegir otra ranura ya materializada.

Reglas del prefix de prueba:

- No se adopta un directorio no vacío cuyo `slot.json` no coincida con el pin y el backend.
- No se reescribe un prefix de la otra ranura.
- Un rebuild de una ranura es transaccional: el prefix viejo de esa ranura se conserva hasta que el
  nuevo termina, y se restaura si falla.
- `slot.json` no es un manifest v3 de producción. El resolver del launcher no debe tratar este
  directorio como prefix administrado.
- El nombre del directorio no es la autoridad. La autoridad es `slot.json` más los digest del wine
  y de las DLL del backend.

Cambiar de ranura no borra la otra.

## 7. dgVoodoo que se reutiliza

Antes de crear ranuras, la prueba comprueba el directorio del juego y se detiene si algo falla:

- existe `.ro-launcher-dgvoodoo.json` legible. El instalador escribe schema 2 y el lector
  también acepta schema 1;
- están `D3DImm.dll`, `DDraw.dll` y `dgVoodoo.conf`;
- `DDraw.dll` y `D3DImm.dll` coinciden con el hash del manifest. `dgVoodoo.conf` puede diferir
  porque el panel lo edita; se lee su `OutputAPI` y no se exige el hash de la plantilla;
- el ejecutable importa `ddraw.dll` o `d3dimm.dll`;
- `OutputAPI` es un valor Direct3D 11 explícito. `bestavailable`, vacío, `disabled` y cualquier
  `d3d12_*` detienen la prueba.

No se llama al instalador. No se copia otra plantilla encima. No se toca `DDraw.dll`.

`A4` decide si el laboratorio puede pasar `OutputAPI` a `d3d11_fl11_0` con copia de respaldo y
restauración, o si sólo informa y espera el cambio manual. Hasta cerrar `A4`, el plan no escribe en
el directorio del juego.

Las dos ranuras comparten ese overlay. Una sola partida a la vez: las dos escribirían en el mismo
conf y en los mismos logs del cliente.

## 8. Ranuras y cambio

| Ranura | Entrada | Salida | Dónde viven sus DLL |
| ------ | ------- | ------ | ------------------- |
| `dxmt` | Direct3D 11 emitido por dgVoodoo | Metal, vía DXMT | Builtin dentro de `slots/dxmt/wine`, PE32 para `d3d11`/`dxgi` y `winemetal.so` de 64 bits |
| `dxvk-moltenvk` | El mismo Direct3D 11 | Vulkan de DXVK y luego MoltenVK | DLL de DXVK en ese wine o en su prefix, más MoltenVK en el árbol Unix de esa ranura |

Dueños, un archivo efectivo cada vez:

| Archivo | Dueño |
| ------- | ----- |
| `DDraw.dll`, `D3DImm.dll`, `dgVoodoo.conf` | Overlay ya instalado en el directorio del juego. Igual en las dos ranuras. |
| `d3d11.dll`, `dxgi.dll`, `d3d10core.dll` | Sólo la ranura activa. |
| `winemetal.so` | Sólo `dxmt`. |
| MoltenVK | Sólo `dxvk-moltenvk`. |

El cambio de ranura es un puntero, no una mutación:

1. Rechazar si hay un `wine` o `wineserver` vivo de cualquiera de las dos ranuras.
2. Escribir `active-slot` con `dxmt` o `dxvk-moltenvk`.
3. El lanzamiento siguiente usa exclusivamente el `wine` y el `WINEPREFIX` de esa ranura.

No se mueven DLL entre ranuras en el momento del cambio. Cada una quedó armada al crearla.

`WINEDLLOVERRIDES` se arma por ranura, una sola vez, sin heredar un valor opaco:

- las dos ranuras: `ddraw,d3dimm` nativo y luego builtin, para tomar el dgVoodoo que está junto al
  ejecutable;
- `dxmt`: no marcar `d3d11` ni `dxgi` como nativos si están instalados como builtin del wine de esa
  ranura;
- `dxvk-moltenvk`: `d3d11,dxgi,d3d10core` nativo y luego builtin, sólo si esas DLL son las de esa
  ranura.

Pins de DXMT y de DXVK-macOS/MoltenVK: `A2` y `A3`. Sin ellos la ranura correspondiente no se
materializa. No se inventan hashes en este plan.

## 9. Protocolo de una corrida

Misma `subject.json`, una ranura, un intento. La otra ranura queda intacta.

Cada corrida guarda en `runs/<utc>-<slot>/`:

- id de ranura y contenido de `slot.json`;
- versión de Wine y digest del binario usado;
- digest de `DDraw.dll`, `D3DImm.dll` y la línea `OutputAPI`;
- digest de las DLL Direct3D 11 efectivas y, si aplica, de `winemetal.so` o MoltenVK;
- SHA-256 de `gepard.dll` y del ejecutable;
- máquina PE del ejecutable y si importa `ddraw.dll`, `d3d8.dll`, `d3d9.dll` o `d3d11.dll`;
- ruta del prefix y la línea `#arch=` de su `system.reg`;
- el primer error fatal, si aparece;
- un veredicto de esta lista: `login`, `window`, `device-created`, `died-before-device`,
  `inconclusive`.

`login` en una ranura cierra la hipótesis para ese hash y ese backend. La otra ranura sigue siendo
necesaria para poder comparar: el laboratorio no está completo si sólo una ranura puede arrancar.
Que la segunda no llegue a login es un resultado, no un fallo del laboratorio.

Un fallo anterior a la creación del dispositivo no juzga DXMT ni DXVK. Juzga Wine, el prefix o
Gepard, y se anota como `died-before-device`.

## 10. Hecho cuando el laboratorio exista

Esto se implementa después de cerrar `A1`–`A6` y de escribir los pins. No forma parte del documento
de hoy.

- La raíz explícita se crea vacía y no adopta un directorio ajeno.
- `runner-pristine` verifica el pin `A1`.
- Existen las dos ranuras, cada una con su wine, su prefix y su `slot.json`.
- `active-slot` cambia de una a otra con un proceso de la otra ranura todavía vivo y el cambio se
  rechaza.
- El directorio del juego conserva los mismos hashes de dgVoodoo después de crear las ranuras y
  después de una corrida.
- Una corrida en cada ranura deja `evidence.json`, aunque el veredicto sea negativo.
- Nada de esto modifica `GraphicsProfile`, el catálogo curado ni un prefix de Linux.

## 11. Decisiones propuestas

Codex puede aceptar o sustituir cada una. Mientras sigan como propuesta, la implementación espera.

| Id | Propuesta |
| -- | --------- |
| D1 | El laboratorio vive en `RO_LAUNCHER_MACOS_SPIKE_ROOT`, obligatorio y fuera del repo. |
| D2 | Un Wine de Gcenx pinneado. Cada ranura recibe su propia copia. El pristine no se lanza. |
| D3 | Un prefix por ranura, creado con el wine de esa ranura. Cambiar de backend es cambiar de ranura. |
| D4 | dgVoodoo se reutiliza in situ. No se reinstala y no se añade envoltura de Direct3D 9. |
| D5 | La segunda ranura es `dxvk-moltenvk`: DXVK-macOS de Gcenx más MoltenVK, consumiendo el mismo Direct3D 11. |
| D6 | La entrada exige import de `ddraw.dll` o `d3dimm.dll`. Un cliente sólo Direct3D 9 queda fuera de esta fase. |
| D7 | El lanzamiento de la fase 1 es el ejecutable directo, sin patcher. |
| D8 | Una observación de login no modifica ADR-001 ni ADR-004. |

## 12. Ambigüedades para cerrar con Codex

| Id | Qué falta | Opciones | Recomendación | Bloquea |
| -- | --------- | -------- | ------------- | ------- |
| A1 | Pin del Wine de Gcenx con WoW64 i386: versión, URL, tamaño, SHA-256. | El build concreto de `macOS_Wine_builds`, o ningún build si ninguno publica i386. | No elegir aquí. Verificar el asset y anotar el pin en una revisión de este archivo. | Instalar el runner. |
| A2 | Pin de DXMT con `d3d11.dll` PE32, `dxgi.dll` PE32 y `winemetal.so` de 64 bits compilado contra el Wine de A1. | Artefacto upstream si publica i386; build documentado contra ese Wine si el release sigue siendo sólo de 64 bits. | No usar el zip de 64 bits como si fuera el de 32. | Materializar `dxmt`. |
| A3 | Pin de DXVK-macOS y de MoltenVK, y si MoltenVK ya viene dentro del Wine de A1. | Zip de Gcenx aparte, o el MoltenVK ya incluido en el pristine sin reinstalarlo. | Si el pristine ya trae MoltenVK, la ranura no copia otro. | Materializar `dxvk-moltenvk`. |
| A4 | `OutputAPI = bestavailable` no sirve para DXMT. | Abortar y dejar el conf al operador; o respaldo, escritura de `d3d11_fl11_0` y restauración al salir. | Abortar. El laboratorio no escribe en el directorio del juego en la primera versión. | La primera corrida, no la creación de las ranuras. |
| A5 | Feature level de esa línea Direct3D 11. | `d3d11_fl11_0` o `d3d11_fl10_0`. | Empezar por `d3d11_fl11_0`. Si el dispositivo falla por feature level, una segunda corrida en la misma ranura usa `d3d11_fl10_0` y queda anotada como otra run. | Sólo el valor escrito si A4 pasa a ser una edición. |
| A6 | Qué hace el laboratorio si `slots/<id>/` existe sin `slot.json` o con otro digest. | Adoptarlo; borrarlo; rechazarlo y conservar. | Rechazar y conservar. Misma regla que un prefix desconocido. | Crear o reconstruir una ranura. |

Cerrar una ambigüedad significa escribir en este archivo el valor elegido o el pin verificado, con
la fuente del artefacto. Un pin sin digest no cuenta como cerrado.

## 13. Orden posterior, cuando A1–A6 estén cerradas

1. Crear la raíz explícita.
2. Instalar y verificar `runner-pristine`.
3. Materializar `dxmt` y su prefix.
4. Materializar `dxvk-moltenvk` y su prefix.
5. Comprobar el sujeto y el dgVoodoo ya instalado.
6. Implementar `active-slot` con el rechazo si hay un wine vivo.
7. Lanzar el ejecutable directo y guardar `evidence.json`.

Ese orden es el de la implementación futura. Este cambio sólo deja el plan.
