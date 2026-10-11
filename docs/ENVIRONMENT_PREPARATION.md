# Diagnóstico y preparación del entorno

La preparación evoluciona por entregas sobre los contratos de
[runtime](RO_RUNTIME_ARCHITECTURE_PLAN.md), [prefix](adr/ADR-002-runtime-prefix-identity.md)
y [sesiones](PTRACE_SESSION_SUPERVISOR_PLAN.md). El diagnóstico no cambia identidades,
selecciones guardadas ni ownership.

## Entrega 1: implementada

- `models/environment.rs`: contrato IPC camelCase versionado (`schemaVersion: 1`).
- `tools/environment/host.rs`: comprobaciones de capacidades con el PATH base sanitizado
  del host que reciben los procesos externos. No cuenta herramientas de directorios adicionales
  que Winetricks no podría encontrar. El script efectivo usa el mismo resolver que la ejecución.
- `tools/environment/assessment.rs`: proyección pura del estado y las observaciones a
  problemas, operaciones afectadas, disponibilidad y acciones tipadas.
- `tools/environment/guidance.rs`: guía de paquetes por `ID`/`ID_LIKE` de os-release;
  inicialmente Arch y derivados. Una distribución desconocida conserva instrucciones
  generales; el nombre de la distro no determina capacidades.
- `features/environment/`: diálogo accesible con todos los problemas, grupos,
  alcance, detalles técnicos expandibles y comprobación explícita. Conserva los
  tokens y primitivos del diseño grafito. Un único cuerpo tiene scroll.

`check_dependencies` conserva la respuesta legacy y añade `environment`. Los consumidores
anteriores siguen funcionando; la UI nueva tolera la ausencia del campo. El frontend
no interpreta textos de error para decidir qué acción ejecutar.

Las ausencias de cabextract, unzip, downloader y Winetricks bloquean preparación y
reconstrucción, pero no invalidan el runner ni impiden jugar con un prefix preparado.
Las capacidades de ejecución, como el loader ELF32, también bloquean el lanzamiento.
Los warnings de compatibilidad y funciones opcionales no se promueven a bloqueos.

La disponibilidad del runner/prefix se conserva independientemente del host. El backend
recomprueba los requisitos de preparación en setup/reset, antes de descargar artefactos
o modificar un prefix: invocar IPC directamente no omite el control. Los guards,
leases, supervisor y rollback existentes siguen siendo la autoridad de ejecución.
Los fallos de host conservan todos los bloqueos en la respuesta de error.

Los proveedores actuales son `protonTools` y `wineHostTools`; Wine puede usar el script
incluido en el Proton ya instalado, como antes. La detección de herramientas comprueba
presencia ejecutable; no afirma todavía integridad o ejecución funcional de cada binario.
Una comprobación de loader no demuestra que el dispositivo/driver Vulkan funcione.

Las acciones son cerradas: `recheck`, `prepare` y `hostGuide`. La guía transporta nombres
de paquetes curados, nunca comandos ejecutables ni texto procedente de os-release.
La UI permite copiar esos nombres e indica cómo continuar con el administrador de
paquetes. No instala paquetes privilegiados ni modifica políticas del host.

El diagnóstico está asociado a la configuración efectiva del servidor/runner. Cambiarla
cierra la ayuda anterior. Las respuestas tardías siguen el control de request/key de
settings; volver a comprobar no inicia una instalación ni abre un cliente.

## Entregas siguientes

1. **Planificación y ejecución:** `EnvironmentRequest`, observaciones con estado
   available/missing/broken/unknown y `PreparationPlan` con pasos cerrados, condiciones
   verificables, ownership y políticas de cancelación/reintento. Un registro backend
   permitirá consultar el snapshot de una operación al volver a abrir la UI.
2. **Herramientas administradas:** artefacto independiente para Wine 7.16 con digest,
   plataforma, receta y receipt. Auditar primero shell, extractores, downloader,
   certificados, bibliotecas y dependencias transitivas de la receta real. Reutilizar
   `tools/artifacts`; validar en un host sin herramientas preinstaladas. Conservar
   los runners/prefixes existentes y la receta WebView2 específica de Wine 7.16.
3. **Recuperación persistente:** journal mínimo de recursos propios y estados
   confirmados, exclusión entre instancias y recuperación por inspección. Un checkpoint
   no autoriza saltarse un instalador interrumpido. No adoptar ni borrar estado ajeno.

Cada entrega tiene una implementación y una revisión adversarial. No introducir un
gestor general de paquetes, scripts libres, otro catálogo de runners o cambios de
política de seguridad para resolver dependencias.

## Verificación

```sh
cargo test -p ro-launcher --all-features tools::environment -- --nocapture
npm test -- src/features/environment/EnvironmentHelpDialog.test.tsx src/features/launcher/useLaunchGame.test.tsx
npm run check:design
npm run check:design:fixtures
npm run check:design:fit
npm run build
npm run preview -- --host 127.0.0.1 --port 5175
node scripts/review-environment.mjs 5175
RO_DESIGN_REVIEW_OUTPUT=/tmp/ro-environment-design npm run review:design -- 5175
RO_DESIGN_REVIEW_OUTPUT=/tmp/ro-environment-design-minimum npm run review:design -- 5175 --minimum
```

Los tests del host ejecutan procesos de prueba con HOME/XDG temporales y PATH controlado,
sin alterar variables globales de tests concurrentes. Cubren herramientas ausentes
simultáneamente, presencia accidental dentro de AppImage o fuera de PATH y comprobación
posterior con herramientas accesibles. El contrato serializado y la conservación de
launch/runner/prefix ante bloqueos exclusivos de preparación tienen regresiones propias.

El arnés visual usa el frontend de producción, CSP real e IPC sintético a 1440×900 y
1280×820; comprueba requisitos, scroll con detalles expandidos, recheck y preparación
sin arranque implícito. Sus imágenes y JSON quedan en `/tmp`.

La aceptación completa sigue requiriendo `npm run tauri:dev` y AppImage con Proton y
Wine, además de Arch limpio. Los tests sintéticos no prueban juegos, drivers, downloads
reales ni integración WebKit. SakuraRO/Wine 7.16 y HoneyRO/nndsk-ro-proton son los
anchors de regresión. Registrar limitaciones y evidencia sin credenciales ni datos de memoria.

## Evidencia de esta entrega (2026-10-10)

- Frontend: lint, format:check, build y 212 tests en 39 archivos, todos pasan.
- Rust: fmt --check, clippy workspace/all-targets/all-features con `-D warnings` y
  tests workspace/all-features pasan. Los tests que requieren juegos o permisos
  específicos siguen ignorados; no se cuentan como aceptación ejecutada.
- Diseño: guards de tokens, fixtures negativas y fit pasan. El review de producción
  cubre 25 escenas en cada tamaño (1440×900 y 1280×820), sin fallos de fit/contraste
  ni errores de consola. El arnés del diagnóstico cubre ambos tamaños por separado.
- `npm run tauri:dev -- --no-watch`: arranque frío con HOME/XDG temporales, selección
  Wine 7.16 legacy por servidor y apertura del diagnóstico en WebKit. Detecta los
  verbos ausentes del Winetricks efectivo sin modificar el runner ni el prefix.
- AppImage generado: arranque frío por `--appimage-extract-and-run`, integración de
  escritorio desactivada y otro HOME/XDG temporal. Comprueba Python/SSL/Vulkan del
  host y muestra el runtime Proton administrado pendiente, sin descarga implícita.
- `npm run tauri:build` y `npm run tauri:build:appimage` generan bundles bajo
  `target/release/bundle/`, pero terminan con exit 1 al firmar porque falta
  `TAURI_SIGNING_PRIVATE_KEY`. No se afirma un build firmado exitoso.
- `check-release-invariants` y `git diff --check` pasan. Sin bump, commit, instalación
  ni publicación. Las instancias propias de smoke se detienen con SIGINT; esto no
  demuestra el cierre con clientes vivos.

Pendientes: instalación del AppImage, host Arch realmente limpio, preparación real
de componentes y partidas de SakuraRO/Wine 7.16 y HoneyRO/nndsk-ro-proton. El arranque
del launcher y los tests con PATH controlado no sustituyen esos anchors.
