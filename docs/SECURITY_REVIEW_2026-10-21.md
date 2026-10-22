# Revisión de dependencias de seguridad — 2026-10-21

Fecha local: 21 de octubre de 2026, America/Santiago. La ejecución comenzó el
22 de octubre a las 00:11 UTC. Ventana de avisos recientes: 8–21 de octubre;
también se contrastaron avisos anteriores con las versiones resueltas.
Base: `main` en `c5d1dd6b5aac8c770ea384630e6324b29f23a7bb`.

## Alcance y stack comprobado

- npm, `package-lock.json` v3 y Cargo, `Cargo.lock` v4. No hay lockfile Bun,
  pnpm o Yarn. Se usó Node 22.23.4 con su npm 10.9.9 y Rust 1.98.1, conservando
  `engines.node >=22`. `npm audit --help` confirma soporte de `--omit=dev`.
- El shell del host seleccionaba Node 26.8.1/npm 11.19.0 y Rust 1.99.1. Se
  seleccionaron explícitamente los toolchains del repositorio para validar.
  No se actualizaron instalaciones globales del host: para desarrollar hay que
  activar el Node fijado; el Node 26.8.1 global queda fuera del conjunto parchado.
- React/React DOM 18.3.1, Vite, Tailwind 4.3.3 y TypeScript 5.9.3 generan una
  SPA estática en `dist`. Vitest ejecuta las pruebas; Playwright es herramienta
  de revisión. No hay Astro, Next, SSR, adaptadores de servidor ni paquetes
  `react-server-dom-*`, tampoco copias internas de Next/RSC.
- Producción es una aplicación Linux Tauri v2/Rust con WebKitGTK y los sidecars
  `ro-inputd`/`ro-sessiond`. No ejecuta Node ni Bun. `.nvmrc` gobierna desarrollo,
  build, CI, auditorías y firma. Los runtimes internos de las acciones de GitHub
  los administra el runner de GitHub, independientemente de `.nvmrc`.
- WebKitGTK local: 2.54.1, GTK: 3.24.52, Python del host: 3.14.8. UMU 1.4.4
  es un zipapp que usa Python del host. Se conservan nndsk-ro-proton
  0.1.0-dev.3, su base Proton-CachyOS 11, Wine 7.16 y DXVK 2.6.2.
  Las versiones de las bibliotecas del host y de los clientes instalados en
  otras máquinas no quedan demostradas por los pins del repositorio.
- Se trabajó en un worktree de `main`. La rama
  `hardening/pr10-wine716-pe-gate` y sus tres documentos sin seguimiento se
  conservaron. No se cambiaron versiones de la aplicación, runners, prefijos,
  datos del usuario, políticas Yama ni reglas de compatibilidad Gepard.

## Versiones antes y después

| Componente resuelto o selector | Antes | Después |
| --- | --- | --- |
| Node de `.nvmrc` | 22.23.3 | 22.23.4 |
| npm incluido en ese Node | 10.9.9 | 10.9.9 |
| rustup de la imagen Ubuntu 24.04 documentada | 1.29.1 | actualización explícita antes de instalar Rust; 1.29.2 comprobado localmente |
| Rust | 1.98.1 | 1.98.1 |
| Vite | 8.1.4 | 8.1.6 |
| Vitest y sus paquetes `@vitest/*` | 4.1.11 | 4.1.12 |
| `@tauri-apps/cli` | 2.12.0 | 2.12.4 |
| `@tauri-apps/api` | 2.11.0 | 2.12.4 |
| `tauri` | 2.11.2 | 2.12.5 |
| `tauri-build` / codegen / macros | 2.6.2 | 2.7.4 |
| `tauri-runtime` / runtime-wry | 2.11.2 / 2.11.2 | 2.12.2 / 2.12.3 |
| `tauri-utils` / `wry` | 2.9.2 / 0.55.1 | 2.10.2 / 0.57.0 |
| `tauri-plugin-fs` transitivo | 2.5.1 | 2.7.1 |
| `tauri-plugin-updater` | 2.12.0 | 2.14.0 |
| `rustls-webpki` | 0.103.15 | 0.103.16 |
| `webpki-roots` / `webpki-root-certs` | 1.0.9 | 1.0.10 |

Los cambios transitivos de Tauri son los exigidos por sus versiones compatibles;
no se hizo una actualización global de Cargo. Rustls 0.23.46 y Tokio 1.52.3 se
conservaron. Los schemas regenerados añaden definiciones de permisos upstream,
sin ampliar los permisos por defecto ni las capabilities de la aplicación.

## Avisos y exposición

Se consultaron los textos completos y metadatos oficiales de advisories,
releases y changelogs, además de las auditorías. Las fechas de actualización de
GitHub son relevantes: varios avisos publicados el 6 de octubre se actualizaron
el 20 de octubre al asignar sus CVE.

| Aviso y fuente oficial | Condiciones y decisión |
| --- | --- |
| [Node: release de seguridad del 21/10](https://nodejs.org/en/blog/vulnerability/october-2026-security-releases), [v22.23.4](https://github.com/nodejs/node/releases/tag/v22.23.4) | Se actualiza la rama 22 sin migrar de major. Incluye CVE-2026-105481, -105482, -106069, -106072, -106073, -106074 y -111497: HTTP keep-alive, nombres DNS/IP, TLS, permisos, snapshots/SEA y Brotli. También actualiza c-ares, certificados, OpenSSL, SQLite y Undici. Afecta las herramientas de desarrollo/build/CI; no hay Node en la AppImage. No se constató uso de SEA, snapshots o `--permission`. Los avisos exclusivos de Node 24/26 no se atribuyen a la rama 22. |
| [CVE-2026-111943 / RUSTSEC-2026-0382](https://rustsec.org/advisories/RUSTSEC-2026-0382.html), [0381](https://rustsec.org/advisories/RUSTSEC-2026-0381.html), [0383](https://rustsec.org/advisories/RUSTSEC-2026-0383.html) | Certificados intermedios emSign comprometidos: el cliente HTTPS puede aceptar una cadena con claves comprometidas si sus anclas lo permiten. Se corrigen el verificador y ambas listas de raíces; el launcher descarga artefactos y consulta actualizaciones por HTTPS. `webpki-root-certs` solo aparece para otros targets, pero también se actualiza en el lockfile. |
| [rustup 1.29.2](https://blog.rust-lang.org/2026/10/20/Rustup-1.29.2/) | Corrige el mismo problema de certificados en la herramienta que instala Rust. CI ejecuta `rustup self update` antes del comando que conserva el pin 1.98.1; una prueba de política rechaza omitir esa actualización. La versión de rustup resultante no es un pin inmutable: depende de la release estable disponible al ejecutar el workflow. |
| [GHSA-w28w-mhc8-qvjv](https://github.com/tauri-apps/tauri/security/advisories/GHSA-w28w-mhc8-qvjv) | Tauri <=2.11.5 permite recuperar respuestas IPC grandes de otro WebView mediante IDs predecibles. Requiere un WebView atacante con acceso a invoke. La aplicación configura una ventana local y CSP restrictiva; no se demostró explotación. Se aplica el parche de aislamiento por WebView, ausente en el resultado inicial de RustSec. |
| [GHSA-9g54-6x48-9vpw](https://github.com/tauri-apps/plugins-workspace/security/advisories/GHSA-9g54-6x48-9vpw) | `tauri-plugin-fs` <2.5.2 falla al proteger datos internos de WebView si se concede acceso frontend a `$APPLOCALDATA`. No se conceden permisos `fs` en las capabilities revisadas. Se actualiza igualmente la dependencia transitiva de dialog. |
| [CVE-2026-112946 / GHSA-rq7h-c2jc-7f22](https://github.com/vitejs/vite/security/advisories/GHSA-rq7h-c2jc-7f22) | Lectura fuera de `server.fs.allow` en sistemas no Windows si hay imports que coinciden con rutas absolutas del host y se expone el dev server. Vite usa loopback por defecto, pero `TAURI_DEV_HOST` puede cambiarlo. Vite 8.1.6 corrige la rama instalada. |
| [CVE-2026-112945 / GHSA-9jrq-w75r-8gcw](https://github.com/vitejs/vite/security/advisories/GHSA-9jrq-w75r-8gcw) | Ejecución de scripts de otro origen al usar middleware/custom HTML con una URL sin normalizar e inline modules. Esta SPA usa el manejo HTML normal de Vite. La actualización incluye el parche. |
| [CVE-2026-112942](https://github.com/vitejs/launch-editor/security/advisories/GHSA-587h-7wmj-h5pm), [112943](https://github.com/vitejs/launch-editor/security/advisories/GHSA-698v-4r3x-3vcv), [112944](https://github.com/vitejs/launch-editor/security/advisories/GHSA-r825-w8mm-5pxw) | Inyección de comandos/argumentos en launch-editor. Requieren Windows o, para el caso de flags a través de Vite, WSL y archivos/control de argumentos específicos. No se demostró exposición en Linux nativo. El parche de Vite actualiza la copia integrada; no existe un paquete separado en este lockfile. |
| [CVE-2026-112949 / GHSA-qxg6-89mg-rqv8](https://github.com/vitest-dev/vitest/security/advisories/GHSA-qxg6-89mg-rqv8), [112950 / GHSA-xp26-3454-mjgp](https://github.com/vitest-dev/vitest/security/advisories/GHSA-xp26-3454-mjgp) | Registro remoto de mocks sin autenticación e inyección de JS; traversal mediante backslash en Windows. Requieren el plugin interceptor/mocker expuesto. El proyecto usa `vitest run`, sin UI, Browser Mode o API de red. Se actualiza toda la familia a 4.1.12. |
| [Tauri 2.12.4](https://github.com/tauri-apps/tauri/releases/tag/tauri-v2.12.4), [2.12.5](https://github.com/tauri-apps/tauri/releases/tag/tauri-v2.12.5) | Correcciones sin GHSA: panic ante una URI IPC/assets de path vacío; restricciones de downgrade NSIS en Windows. Se incluye la corrección IPC; el instalador Windows no es el artefacto distribuido por este proyecto. |
| [Updater 2.14.0](https://github.com/tauri-apps/plugins-workspace/releases/tag/updater-v2.14.0) | Endurecimiento sin identificador: verifica bytes antes de `Update::install`, rechaza payloads inválidos antes de sustituir una AppImage y mejora fallback/rollback. El launcher usa descarga verificada y su propio `activate_appimage`, por lo que no se atribuye a este proyecto todo fallo del instalador upstream. Se conservan firma, versión firmada y activación transaccional. |
| [CLI 2.12.4](https://github.com/tauri-apps/tauri/releases/tag/tauri-cli-v2.12.4) | La CLI nativa contiene dependencias que no audita npm. Su Cargo.lock oficial ahora resuelve zip 9.0.5, rustls-webpki 0.103.16 y ambas raíces 1.0.10. Corrige el ZIP vulnerable de la herramienta de empaquetado. |

Avisos recientes contrastados que no requieren más cambios:

- [Vite GHSA-8g5g-mxqh-grgj](https://github.com/vitejs/vite/security/advisories/GHSA-8g5g-mxqh-grgj)
  afecta solo 6.4.4. [CVE-2026-112947](https://github.com/vitejs/vite/security/advisories/GHSA-vfpm-58rq-9qcg)
  afecta ramas 8.2/8.3, no 8.1.4 ni 8.1.6. No se saltó a una rama afectada.
- [Vitest CVE-2026-112948](https://github.com/vitest-dev/vitest/security/advisories/GHSA-p458-7748-mcp3)
  afecta Vitest 5 con servidor HTTP habilitado, no la rama 4 usada aquí.
- [Rustls: secretos en Debug](https://github.com/rustls/rustls/security/advisories/GHSA-c8m5-c952-6cm4),
  [Brotli](https://github.com/rustls/rustls/security/advisories/GHSA-h2v8-394m-p7vv),
  [key exchange](https://github.com/rustls/rustls/security/advisories/GHSA-w44r-93j7-qv28)
  y [ECH](https://github.com/rustls/rustls/security/advisories/GHSA-vp88-798v-h67f):
  el lockfile ya tenía 0.23.46, fuera de los rangos afectados.
- [Tokio GHSA-5wjr-9356-4wc8](https://github.com/tokio-rs/tokio/security/advisories/GHSA-5wjr-9356-4wc8)
  afecta exactamente 1.54.0. Se conserva 1.52.3.
- Se revisaron advisories de React y su changelog: los avisos RSC requieren
  paquetes servidor ausentes aquí. No se justifica migrar React 18 a React 19.
- [WebKitGTK WSA-2026-0006](https://webkitgtk.org/security/WSA-2026-0006.html)
  corrige vulnerabilidades en 2.54.0. La biblioteca local 2.54.1 es posterior;
  esto no acredita la versión incluida en una AppImage publicada anteriormente.
- Se consultaron advisories y releases de Tailwind, esbuild, Wry, Hyper,
  Zustand, UMU, DXVK, nndsk-ro-proton y las acciones usadas en CI; no se encontró
  otro aviso reciente aplicable que justificara alterar sus versiones.
  Rust 1.99.1 corrige regresiones de compilación; no exige abandonar 1.98.1.

## Pendientes sin parche compatible aplicado

1. `zip` 2.4.2, dependencia exclusiva de pruebas del spike D7VK, y `zip` 4.6.1,
   dependencia del updater exclusiva de Windows:
   [RUSTSEC-2026-0367 / GHSA-vv5m-jxxh-4627](https://github.com/zip-rs/zip2/security/advisories/GHSA-vv5m-jxxh-4627),
   [0368 / GHSA-4v9h-p7m9-rh26](https://github.com/zip-rs/zip2/security/advisories/GHSA-4v9h-p7m9-rh26),
   [0369 / GHSA-4684-h2q9-2vcp](https://github.com/zip-rs/zip2/security/advisories/GHSA-4684-h2q9-2vcp)
   y [GHSA-m665-v7pm-7grj](https://github.com/zip-rs/zip2/security/advisories/GHSA-m665-v7pm-7grj).
   Los primeros afectan permisos SUID/SGID y extracción mediante symlinks;
   el último, inconsistencias entre cabeceras ZIP, se publicó el 17 y actualizó
   el 21. Los fixes requieren la rama 9; para cubrir el conjunto se propone
   9.0.5. Se solicitó autorización antes de migrar la dependencia directa de
   pruebas. El spike rechaza symlinks y copia archivos regulares sin llamar a
   `ZipArchive::extract`; no ejecuta como root. Esto reduce exposición, pero no
   elimina los avisos ni equivale a un parche. El ZIP del updater no aparece en
   `cargo tree -i zip@4.6.1` para Linux. Upstream 2.14.0 sigue declarando zip 4
   para Windows; no se forzó una major por debajo de su contrato.
2. [RUSTSEC-2024-0429 / GHSA-wrw7-89jp-8q8g](https://rustsec.org/advisories/RUSTSEC-2024-0429.html):
   `glib` 0.18.5 conserva unsoundness de `VariantStrIter`. El fix es >=0.20;
   GTK3/Tauri siguen usando 0.18. No se parcheó ni sustituyó ese ecosistema de
   forma local. [RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370.html)
   avisa que `proc-macro-error` 1.0.4 está sin mantenimiento. Se mantienen visibles.
3. La herramienta [cargo-audit 0.22.2](https://github.com/rustsec/rustsec/releases/tag/cargo-audit/v0.22.2),
   última release disponible en crates.io al consultar, no forma parte del
   `Cargo.lock` del launcher. Su paquete oficial incluye un Cargo.lock separado
   con Rustls 0.23.40, rustls-webpki 0.103.13 y webpki-root-certs 1.0.7. Quedan
   dentro de rangos afectados por los avisos TLS citados arriba; el impacto
   requiere las funciones/configuraciones afectadas o una cadena confiable con
   los certificados comprometidos. No se demostró explotación de la herramienta.
   El `cargo install --locked --version 0.22.2` de CI conserva ese grafo upstream;
   no existe aún una release corregida que fijar. No se retiró `--locked`, no se
   añadieron forks ni se fabricó otro lockfile. Este pendiente de la herramienta
   no está incluido en los seis hallazgos ZIP del proyecto.

RustSec inicial: 9 hallazgos de vulnerabilidad (3 TLS y 6 ZIP), seis avisos de
mantenimiento y uno de unsoundness. Después: quedan seis hallazgos ZIP, un aviso
de mantenimiento y uno de unsoundness. La actualización de Tauri elimina los
cinco crates UNIC sin mantenimiento. El aviso nuevo de ZIP cloaking aún no
figura en esta base RustSec y por eso también se registra manualmente.
No se agregaron ignores, filtros ni excepciones a la política de auditoría.
El gate RustSec de release permanece bloqueado por los ZIP pendientes.

## Validación

- `npm ci` con el Node fijado y su npm, sin flags para omitir peers o auditorías;
  `npm ls --all` sin dependencias inválidas. Se conservaron metadatos `libc` de
  paquetes opcionales sin cambios que npm 10 eliminaba al escribir el lockfile.
  Para resolver un fallo de Arborist (`edgesOut`) al actualizar Vitest se usó
  temporalmente una resolución única de Vite; el manifiesto final conserva sus
  rangos compatibles, sin overrides ni cambio de gestor.
- `npm audit` y `npm audit --omit=dev`: antes 3 paquetes afectados en la auditoría
  completa y 0 en producción; después 0 y 0. Política npm: pasa.
- `cargo audit` 0.22.2 y política del repositorio, con consulta online: hallazgos
  pendientes detallados arriba; no se presenta como una auditoría limpia.
- Frontend: 46 archivos y 324 pruebas pasan; lint, formato, TypeScript/build,
  guardas de diseño e invariantes de toolchains/release pasan.
- Rust: fmt y clippy workspace/all-targets/all-features con `-D warnings` pasan;
  suite completa: 734 pruebas pasan y 9 quedan ignoradas según su configuración.
  También pasa `orphan_reparent` con Yama=1, incluido su ciclo de 100 sesiones.
- Instalación aislada del rustup oficial 1.29.2, SHA-256 contrastado con su origen;
  el comando exacto de CI actualiza rustup e instala Rust 1.98.1 con rustfmt/clippy.
  Node 22.23.4 también se descargó y verificó contra el SHA-256 oficial.
- Smoke de `npm run tauri:dev` con un HOME absoluto desechable: ventana nativa
  renderizada e información de entorno recibida mediante IPC. Se acompasó la API
  JavaScript a Tauri 2.12 tras detectar el desfase en el primer arranque. El
  segundo arranque no reportó desfase. El botón de cierre de la UI funcionó y
  terminó el proceso con estado 0.
- `npm run tauri:build:appimage -- --config
  '{"bundle":{"createUpdaterArtifacts":false}}'`: construye la AppImage sin firma
  bajo `target/release/bundle/appimage/`. SHA-256 del artefacto local probado:
  `cf447231e7c4cc922a705a616927da8fbc8bcf97d7b326e04b2372db99893348`.
  `scripts/smoke-release-artifact.sh` pasa sobre esos bytes: AppRun, protocolo
  de ro-inputd y suite orphan_reparent contra el ro-sessiond extraído, Yama=1.
  La biblioteca WebKitGTK empaquetada devuelve 2.54.1 mediante su API de versión.
  Se arrancó ese AppImage normalmente y se verificó su UI con otro HOME
  desechable; el cierre desde la UI también terminó con estado 0. No se reemplazó
  la AppImage instalada del usuario ni se probó una actualización firmada real.
- Segunda pasada adversarial: revisión completa del diff, semántica de los
  schemas/ACL, paridad API JS/backend, dependencias nativas integradas de la CLI,
  metadatos de arquitectura/libc, activación/firma y las rutas de auditoría.
  Ambos lockfiles conservan sus formatos; ninguna versión de paquete existente
  baja. Las capabilities y permisos por defecto permanecen iguales. Se repitió
  `npm ci`, el árbol completo y ambas auditorías tras conservar los metadatos
  opcionales; pasan. Los hallazgos Cargo pendientes no se ocultaron.
- No se ejecutaron clientes SakuraRO/HoneyRO ni se revalidó la matriz de juegos:
  no se modificaron runners ni perfiles. Ese límite también impide afirmar que
  se verificó el comportamiento en las máquinas de los usuarios finales.

## Entrega y producción

`main` activa CI y la construcción del cache de release. No publica ni despliega
una AppImage automáticamente. `release.yml` exige un tag de versión y CI verde,
y solo crea un draft. La release pública observada es `v0.3.3`, publicada el
13 de octubre de 2026. Esta revisión no autoriza una nueva publicación ni cambia
esa instalación. El estado del push y de CI se informa con sus enlaces al entregar.

El [CI del parche ffd0f48](https://github.com/Nandem1/nndsk-ro-launcher/actions/runs/39699672269)
terminó en verde en ambos jobs, quality y release-cache. Confirma 324 pruebas
frontend y 734 Rust, con los mismos 9 casos Rust ignorados. Sus logs prueban
Node 22.23.4/npm 10.9.9 y que rustup se
actualizó efectivamente de 1.29.1 a 1.29.2, además de los pins configurados.

## Suplemento: migración ZIP autorizada y preparación de release

Revisión adicional el mismo 21 de octubre local (22 de octubre UTC), sobre
`12a3a3a`. Este suplemento conserva los resultados iniciales anteriores y
resuelve el pendiente ZIP después de la autorización de migración del usuario.

- La dependencia de pruebas pasa de `zip` 2.4.2 a
  [9.0.5](https://github.com/zip-rs/zip2/releases/tag/v9.0.5), conservando solo
  Deflate y el soporte Stored implícito. El archivo D7VK fijado usa esos dos
  métodos; su tamaño, SHA-256, layout y DLL x86 pasan la prueba con el lector
  nuevo. D7VK sigue siendo exclusivamente un spike de pruebas, fuera del catálogo.
- Se cubren RUSTSEC-2026-0367/0368/0369 y
  [GHSA-m665-v7pm-7grj](https://github.com/zip-rs/zip2/security/advisories/GHSA-m665-v7pm-7grj).
  La prueba adversarial de cabeceras contradictorias falló inicialmente:
  `ZipArchive::by_index` no activa por defecto la nueva comprobación. El harness
  ahora llama a `validate_header_consistency` antes de extraer cualquier entrada;
  la misma prueba pasa. Se mantienen los rechazos de traversal y symlinks.
- El plugin updater conserva TLS Rustls, proxies y `Update::download`, que
  autentica los bytes y su versión antes de devolverlos. Se deshabilita su
  feature opcional `zip`, usada por sus instaladores de archivos comprimidos,
  porque este producto activa únicamente AppImages sin comprimir mediante su
  propio reemplazo atómico. Así desaparece `zip` 4.6.1 del lockfile sin imponer
  una major bajo el contrato del plugin. Un ZIP real con una AppImage dentro
  se rechaza sin modificar el destino ni un temporal preexistente.
- `cargo tree --locked --target all --edges normal,build -i zip` no muestra
  dependencias. El grafo con dependencias de desarrollo muestra únicamente
  `zip` 9.0.5. No hay filtros de plataforma en la auditoría, overrides, ignores
  ni downgrades; ambos lockfiles conservan su formato.
- Auditorías repetidas: `npm audit` y `npm audit --omit=dev`, cero hallazgos;
  política Cargo/RustSec, cero vulnerabilidades bloqueantes. Siguen reportados
  GLib 0.18.5 (unsoundness) y proc-macro-error 1.0.4 (sin mantenimiento), además
  del pendiente separado de la herramienta cargo-audit descrito arriba.
- Pasan lint, formato frontend, 324 pruebas frontend, TypeScript/build, fmt,
  clippy completo con `-D warnings` y 737 pruebas Rust; las mismas 9 pruebas
  Rust permanecen ignoradas en la suite normal. Se ejecutó además la prueba
  ignorada del ZIP D7VK y pasó. Pasan invariantes de release/toolchains y diseño.
- `npm run tauri:dev` con HOME desechable renderiza el launcher y su información
  de entorno por IPC; el cierre desde la UI termina con estado 0.
- La AppImage local reconstruida pasa `smoke-release-artifact.sh`: AppRun,
  protocolo ro-inputd y orphan_reparent contra ro-sessiond extraído, con Yama=1
  y 100 ciclos. SHA-256 de estos bytes sin firmar:
  `84274619126dbedbe4ece1a5833db04fee1db9088840c7d3fc31ad5e9bb1728c`.
  También renderiza la UI con otro HOME desechable y cierra con estado 0.
- Segunda pasada adversarial: revisión completa del diff y de la resolución
  Cargo, compresión del artefacto real, prevalidación de cabeceras, conservación
  de contenido/permisos fuera de staging, rechazo ZIP sin mutación del updater,
  TLS/proxies y verificación de versión firmada sin feature de extracción.
  No se sustituyó la AppImage instalada ni se ejecutaron clientes de juego;
  no se afirma una actualización firmada N-1 → N ni validación de otros hosts.

La release elegida es `v0.3.3 → 0.3.3 → minor → 0.4.0`, considerando todo el
delta de main desde el último tag publicado. Además de los parches de seguridad,
incluye activación atómica y coordinación del updater, recuperación transaccional
de prefixes/artefactos, instancia única y correcciones del contrato IPC. El cambio
decisivo de contrato en 0.x es retirar `RO_LAUNCHER_SESSION_SUPERVISOR=0` y su ruta
directa: `ro-sessiond` es obligatorio. La migración consiste en usar el supervisor
empaquetado; si no funciona, reinstalar la AppImage anterior. Los datos y
selecciones de runners existentes no requieren migración. El dry-run de minor
confirma 0.4.0. Se preparará un draft firmado; publicarlo queda a cargo del usuario.
