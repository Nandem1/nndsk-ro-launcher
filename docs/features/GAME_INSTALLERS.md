# Instalación de clientes desde un .exe

El alta existente seleccionando el cliente descomprimido sigue disponible. En **Agregar
servidor → Tengo un instalador**, el usuario selecciona un instalador `.exe`, nombre
y uno de los dos runners administrados. Conserva el destino habitual que proponga el
instalador, como `C:\HoneyRO`, dentro de un entorno exclusivo. La selección del runner se fija para esa
instalación; cambiar el predeterminado global no la modifica.

## Contrato

- `InstallationDraft` se persiste separadamente en `installations.json`. No se guarda un
  `ServerConfig` incompleto ni se utiliza el instalador como ejecutable ficticio del juego.
- El UUID y el namespace `installed-<uuid>` se reservan antes de preparar el entorno. El
  resolver usa la misma identidad material y ruta aislada que utilizará el servidor final.
- El runner se descarga/verifica mediante el catálogo existente. El instalador se ejecuta
  desde su carpeta original, conservando el acceso a archivos acompañantes como `.bin`.
- No se requiere una carpeta externa ni cambiar el destino del instalador. En opciones
  avanzadas se puede vincular una carpeta como `R:\`, sin modificar automáticamente el
  destino ni garantizar que el selector de cada instalador enumere la unidad. El launcher
  no introduce argumentos silenciosos específicos de un instalador.
- `destinationPath` es opcional/nulo para nuevas instalaciones en el entorno administrado.
  Los drafts anteriores con una ruta externa conservan su valor, mapeo e identidad, sin
  migrar archivos ni adoptar otro prefijo.
- La operación pertenece al backend y continúa al cerrar el modal. Conserva el lease del
  supervisor y el bloqueo exclusivo del prefijo hasta que terminan los procesos de
  aplicación, incluidos hijos cuyo bootstrapper terminó y borró `WINEPREFIX`.
- La entrada IPC síncrona despacha la operación mediante `tauri::async_runtime::spawn`.
  No presupone un runtime Tokio activo en el hilo GTK/WebKit que recibe el comando.
- La cancelación durante preparación espera que termine el paso transaccional. Durante la
  ejecución, termina el controlador y apaga/recolecta los procesos de esa sesión. Otro
  prefijo y sus clientes continúan independientes.
- Cerrar el launcher o instalar una actualización cierra la admisión de instalaciones,
  solicita cancelación y utiliza el cierre existente de sesiones. Un reinicio recupera los
  estados activos persistidos como `interrupted`, conservando identidad y datos.
- Un código cero significa que el controlador salió sin reportar fallo; no demuestra que
  se haya instalado un cliente. El estado `awaitingClient` permite elegir el ejecutable
  **o volver a abrir el instalador** con el mismo UUID, runner y binding, también después
  de reiniciar el launcher. No registra automáticamente un servidor.
  Se registra el código del controlador lanzado, no se infiere el resultado de cada hijo del
  asistente. Un código distinto de cero se conserva y no se oculta aunque aparezcan archivos.
- La búsqueda está limitada a la carpeta elegida y `drive_c` del entorno: profundidad máxima
  8, 10.000 entradas y 100 candidatos; no sigue enlaces a directorios ni escanea `windows`.
  El usuario confirma el cliente y puede seleccionar un patcher opcional. El selector
  manual empieza en `drive_c` del binding verificado. Una búsqueda vacía no bloquea la
  reapertura ni demuestra un fallo; se mantiene la selección manual.
- Reabrir limpia candidatos y selecciones anteriores en la UI y retiene la protección del
  entorno. No se permiten ejecuciones duplicadas ni reaperturas de drafts `completed`.
  Reapertura y confirmación comparten admisión para que la validación asíncrona y los
  guardados de confirmación no compitan con otra ejecución del instalador.
- La confirmación verifica la identidad y el manifiesto del prefijo. Persiste primero el
  servidor y después el estado completado. El reintento reconoce la misma instalación y
  conserva cualquier edición del servidor existente, sin duplicarlo ni sobrescribir otro.
  El frontend serializa confirmación y guardados de servidores; si falla la escritura final
  del draft, recupera el servidor persistido antes de permitir guardados locales pendientes.

## Preservación y reparación

Antes de ejecutar el instalador, se escribe `.ro-launcher-installation.json` en el prefijo.
Su presencia, incluso dañada o como backup, bloquea una reconstrucción destructiva. Este
recibo protege datos; el manifiesto de prefijo sigue siendo la autoridad del runner y sus
componentes.

El servidor final conserva `installationId`, `prefixPath` y `prefixFingerprint` en su
binding `installation`, además del runner explícito. El resolver rechaza otro prefijo o
material de aislamiento distinto. La edición de servidor informa que otro runner requiere
una instalación nueva. El comando de reset también protege el binding, incluso si falta
el recibo local.

La reparación de componentes continúa por el setup existente. Una operación que necesitase
reemplazar todo el entorno se rechaza conservando sus datos. Guardar el juego fuera del
prefijo no permite asumir que su registro o dependencias sean descartables.

Cancelar no desinstala ni borra archivos parciales. No se promete rollback de escrituras
arbitrarias hechas por el instalador. Un prefijo ajeno, corrupto o con identidad incompatible
se conserva y se rechaza, también al reintentar; no se adopta automáticamente.

## Alcance inicial y validación

Se soporta el flujo interactivo de `.exe`; `.msi`, descargas de clientes y automatización de
asistentes quedan fuera de esta entrega. WebView2 permite una declaración manual y utiliza
la provisión existente, incluida la política específica de Wine 7.16. La instalación guiada
utiliza el supervisor obligatorio `ro-sessiond`, igual que el juego y las herramientas.
El seguimiento de hijos que borran su entorno depende del supervisor; si no puede iniciarse
o validar su handshake, la operación falla sin lanzar el runner directamente.

La revisión adversarial cubre serialización camelCase, recuperación persistente, admisión
duplicada, cancelación, entrega a hijos sin `WINEPREFIX`, exclusión de prefijo, ciclos
repetidos, otro prefijo activo, identidad material, recibos dañados/backups, mapeos existentes,
búsqueda acotada y confirmación explícita. Las pruebas de procesos utilizan el binario real
`ro-sessiond` y procesos sintéticos, sin escribir datos de juegos o prefijos personales.
La regresión de despacho entra desde un hilo sin Tokio, ejercita un temporizador real y
verifica devolución inmediata, cancelación, error persistido y cierre de admisión. Con el
despacho anterior mediante `tokio::spawn` falla con el mismo panic del flujo nativo.

La aceptación con un instalador real exige registrar su hash, runner, manifiesto, destino y
resultado; probar el destino habitual en `C:` y la opción externa en `R:`, cierre sin
instalación con código cero y reapertura, interrupción/reapertura, registro y lanzamiento
posterior, además de las regresiones SakuraRO/Wine 7.16 y HoneyRO/nndsk-ro-proton. Las pruebas
automáticas no constituyen evidencia de compatibilidad de un instalador o Gepard concreto.
