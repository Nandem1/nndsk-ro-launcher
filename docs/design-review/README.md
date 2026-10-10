# Revisión del polish de features

Capturas del build de producción con Chromium local, la CSP de Tauri y el IPC
simulado del arnés. No se usaron datos de usuario ni se lanzaron clientes reales.
Línea base de esta etapa: `9960370` (`rework/unslop-theme`).

| Escena | 1280×820 | 1100×800 | Cajas interiores antes→después |
| --- | --- | --- | --- |
| Preparación | [prep](prep-1280x820.png) | [prep](prep-1100x800.png) | 2→0 |
| En juego | [ingame](ingame-1280x820.png) | [ingame](ingame-1100x800.png) | 3→0 |
| Servidor | [server](server-1280x820.png) | [server](server-1100x800.png) | 3→0 |
| Escáner de memoria | [scanner](scanner-1280x820.png) | [scanner](scanner-1100x800.png) | 3→0 |
| Herramientas activas | [active](active-1280x820.png) | [active](active-1100x800.png) | 3→0 |
| Editores abiertos | [active-editors](active-editors-1280x820.png) | [active-editors](active-editors-1100x800.png) | 5→0 |
| Buffs con dos reglas | [buffs](buffs-1280x820.png) | [buffs](buffs-1100x800.png) | 2→0 |
| Preparación realista | [prep-realistic](prep-realistic-1280x820.png) | [prep-realistic](prep-realistic-1100x800.png) | 6→0 |
| Preparación realista, scroll inferior | [prep-realistic-scrolled](prep-realistic-scrolled-1280x820.png) | [prep-realistic-scrolled](prep-realistic-scrolled-1100x800.png) | 6→0 |

Los conteos son iguales en ambos tamaños. Se cuentan contenedores DOM de dos o
más hijos con borde en los cuatro lados dentro de paneles/modales. Se excluyen
controles, superficie exterior de modal y pozo de logs. Las mismas cajas pueden
aparecer en distintas escenas; no sumar las filas como cajas únicas. La guardia
JSX verifica además las ramas condicionales que no se ven en estas capturas.

Los informes [1280×820](review-1280x820.json) y [1100×800](review-1100x800.json)
registran cero errores de consola, desbordes, texto recortado y cajas interiores.
Las regiones de scroll previstas conservan todo el contenido; sus hijos fuera
del viewport no se consideran recortes. Los textos, roles, etiquetas accesibles
y su orden coinciden con las capturas de la línea base bajo el mismo fixture.
Solo se eliminó un wrapper sin rol, alrededor de HP/SP.

La escena realista incluye cinco servidores (primero seleccionado), Setup.exe,
HoneyRO Patcher.exe, dgVoodoo conf OK, los dos avisos anti-cheat/Gepard solicitados,
diagnósticos con rutas/detalles largos y cuatro líneas largas de logs. La columna
izquierda tiene un solo scroll, sin scroll anidado; Jugar y Rearmar entorno
conservan posición y tamaño al desplazarla. Se comprobaron expansión/cierre
del rail sin transformación animada y reduced-motion.

Las ocho caras locales de IBM Plex Sans, IBM Plex Mono y Barlow Condensed cargan
con HTTP 200 y `document.fonts`; las familias computadas corresponden a UI/datos/marca.
Switches apagados/encendidos, también disabled y preparación: opacidad efectiva 1,
borde contra panel ≥5.623:1 y perilla contra pista ≥6.237:1; encendido 6.849:1 y
7.447:1 respectivamente. Los 13 pares de texto siguen superando 4.5:1.

[Referencias accent/warn por archivo](color-usage.md): app/features 36→23 y
29→11 respectivamente. Los 50 tokens (17 de color) y las APIs no cambiaron.

Repetir con `npm run build`, `npm run preview -- --host 127.0.0.1 --port 5175`,
`npm run review:design -- 5175` y `npm run review:design -- 5175 1100 800`.
`RO_DESIGN_CHROMIUM` permite seleccionar otro ejecutable Chromium instalado;
`RO_DESIGN_REVIEW_OUTPUT` permite guardar evidencia temporal fuera del repo.

Pendiente manual: `npm run tauri:dev` en WebKit con clientes reales y reproducir
el texto monoespaciado fantasma. WebKitGTK 2.52.6 está instalado, pero este entorno
no tiene `Xvfb` ni `xvfb-run`; no se añadió ningún parche de compositing.
Chromium con IPC simulado no valida esa integración.
