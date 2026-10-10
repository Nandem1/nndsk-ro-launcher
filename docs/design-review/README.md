# Revisión del tema

Capturas del build de producción con Chromium local, la CSP de Tauri y el IPC
simulado del arnés. No se usaron datos de usuario ni se lanzaron clientes reales.

| Escena | Ventana Tauri 1280×820 | Comprobación adicional 1100×800 |
| --- | --- | --- |
| Preparación | [prep](prep-1280x820.png) | [prep](prep-1100x800.png) |
| En juego | [ingame](ingame-1280x820.png) | [ingame](ingame-1100x800.png) |
| Servidor | [server](server-1280x820.png) | [server](server-1100x800.png) |
| Escáner de memoria | [scanner](scanner-1280x820.png) | [scanner](scanner-1100x800.png) |
| Herramientas activas | [active](active-1280x820.png) | [active](active-1100x800.png) |

Los informes [1280×820](review-1280x820.json) y [1100×800](review-1100x800.json)
registran cero errores de consola, overflow y texto recortado. Las regiones de
scroll previstas siguen siendo regiones de scroll; el pequeño punto que sobresale
de la insignia del rail permanece dentro del viewport y no genera scroll ni recorte.
Los tags, roles y textos de las cinco escenas coinciden con la línea base original.
Se comprobaron expansión/cierre del rail sin transformación animada y reduced-motion.

Las ocho caras locales de IBM Plex Sans, IBM Plex Mono y Barlow Condensed cargan
con HTTP 200 y `document.fonts`; las familias computadas corresponden a UI/datos/marca.
El contraste mínimo exigido es 4.893:1 (bad/panel); on-accent/accent es 6.126:1.

Medidas de fuente frente a `e09b9a7`: 71→50 variables, 35→17 colores;
169→0 referencias de rampa; 57→0 clases de efectos/overlay/tracking retiradas
(26 efectos, 29 overlay, 2 tracking); 129 referencias accent auditadas, 37
reclasificadas explícitamente a warn, 49 referencias accent vigentes. Los conteos
de clases incluyen el test de estilo de logs; las declaraciones de tokens se
cuentan aparte. Los radios son 0, incluidos controles nativos y pill.

Repetir con `npm run build`, `npm run preview -- --host 127.0.0.1 --port 5175`,
`npm run review:design -- 5175` y `npm run review:design -- 5175 1100 800`.
`RO_DESIGN_CHROMIUM` permite seleccionar otro ejecutable Chromium instalado.

Pendiente manual: `npm run tauri:dev` en el WebKit real y comprobar ambos modos
y modales con clientes reales. Chromium con IPC simulado no valida esa integración.
