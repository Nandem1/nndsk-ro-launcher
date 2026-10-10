# Design system

La fuente de verdad es `src/index.css`: 51 variables, incluidas 17 de color.
Tailwind expone los canales RGB como `rgb(var(--c-name) / <alpha-value>)`.
La UI es plana, sin rampas, gradientes, blur, glows ni sombras decorativas.

## Color

| Token `--c-*` | Valor | Uso |
| --- | --- | --- |
| surface | #0F1412 | Fondo de la aplicación |
| panel | #161C19 | Paneles y filas |
| panel-raised | #1B2420 | Hover, selección y pista de HP/SP |
| field | #0B100E | Campos, selects y controles |
| modal | #121815 | Superficie del diálogo |
| line | #2A332E | Separadores y filas |
| line-strong | #3A463F | Bordes de campos, pista de sliders y borde superior neutral |
| ink | #E8EEE8 | Texto principal |
| muted | #8A968D | Texto secundario; no usar line para texto legible |
| accent | #FF5C39 | Acción principal, foco e indicador de selección |
| on-accent | #14110F | Texto sobre acciones sólidas |
| warn | #E2A33A | Atención, advertencias, trabajo pendiente y revisión |
| ok | #7FB069 | Activo, disponible o correcto |
| bad | #E0604A | Error, HP y acciones destructivas |
| info | #5B9BD5 | Información y SP |
| special | #4FB3A9 | Estado de Shift y opciones especiales |
| overlay-dark | RGB 0 0 0 | Scrim negro sólido con alfa 0.60/0.70 según layer |

Los estados mantienen sus textos y controles existentes: el color los acompaña,
no los sustituye. Los puntos son decorativos y están ocultos a lectores de pantalla.
No reducir la opacidad de paneles, texto ni controles. Idle usa título/texto y
borde superior muted. Disabled conserva su disponibilidad nativa: field,
texto muted y borde interno line, con outline muted de 1px como límite visible.
No cambia al hacer hover. Switch disabled conserva aria-checked y posición;
su pista es field y su perilla muted.

## Ventana y espacio

Ventana de **1440×900**, mínimo **1280×820**, redimensionable, sin decoraciones
y oculta hasta estar lista; fondo nativo surface **#0F1412**. Solo cambió el bloque
`app.windows` de `src-tauri/tauri.conf.json`.
Rail fijo de 300px (64px colapsado), contenido flexible sin max-width central.
El alto extra se reparte: Logs recibe el 25% del incremento desde el mínimo
(176→196px entre ambos tamaños; límite 320px); el resto va a los cuerpos de panel.
La búsqueda en frontend/Rust no encontró supuestos de 1280×820: DarkSelect mide
el viewport actual y reposiciona su menú al redimensionar o desplazar el rail.

## Gramática de filas

Panel = borde superior de 2px + cabecera con línea inferior fina. Dentro,
secciones y filas se separan con `border-t border-line`, sin bordes completos
ni fondos propios. Solo inputs, selects, teclas, botones y el pozo de logs
conservan caja. Los modales conservan su superficie exterior; dentro usan filas.

Herramientas es una fila de tres columnas separadas por `border-l border-line`;
Cliente y sus diagnósticos van debajo con una línea superior. HP/SP quedan
directamente en AutoPot. Teclado, Gear Switch, Sharp Shooting y reglas de Buffs
no tienen una caja envolvente. Los controles individuales conservan sus bordes.

La columna izquierda tiene un solo scroll; sus paneles no se comprimen y Avanzado
no tiene scroll interno. Jugar y Rearmar/Reparar entorno quedan fuera del scroll,
fijos abajo. Nombres de servidores y diagnósticos largos ajustan línea.
Avanzado muestra lo operativo; Observaciones y Benchmarks A/B comparten el
disclosure «Benchmarks A/B · N», cerrado por defecto, con estado local no
persistido. Todas sus acciones conservan orden, handlers y disponibilidad;
las acciones de texto usan Button ghost/xs y separación uniforme.

## Forma, texto y efectos

| Variables | Valor |
| --- | --- |
| radius-inline / control-compact / control / panel / modal / pill | 0 |
| text-micro / caption / detail | 11px |
| text-label / xs | 12px |
| text-sm | 13px |
| text-base / panel-title | 15px |
| text-lg / xl | 18px |
| weight-panel-title / tracking-panel-title | 600 / 0 |
| tracking-micro-label | 0.1em |
| leading-xs / sm / base / lg / xl | 1 / 1.25 / 1.5 / 1.75 / 1.75rem |
| font-sans | IBM Plex Sans, sans-serif |
| font-mono | IBM Plex Mono, monospace |
| font-wordmark | Barlow Condensed, sans-serif |
| shadow-panel / modal / control | none |
| blur-panel | 0px |
| background-panel / progress / body-illumination | none |

Fuentes Latin de `@fontsource`, empaquetadas por Vite, sin CDN: Sans 400/500/600/700,
Mono 400/500/600 y Barlow Condensed 700 únicamente para el wordmark.
Las licencias OFL están en `public/licenses/` y se copian al build.
`font-src` hereda `default-src 'self'` de la CSP de Tauri; no requiere cambios.
La utilidad `.micro-label` usa Mono/11px/mayúsculas/tracking 0.1em/muted para
TECLAS, LECTURA, PERFIL DE MEMORIA, DELAY, HP/SP y categorías similares.
Títulos de panel/campo usan Sans sin uppercase (los textos fuente no cambian).
Datos numéricos, teclas, versiones, rutas, argumentos y logs utilizan Mono
con `tabular-nums`; `.font-mono` también activa los números tabulares.
DataText decora solo números/unidades/versiones/rutas dentro de texto mixto,
sin transformar su contenido. Inputs numéricos lo aplican automáticamente.

Solo se conservan `pulse-dot` y un fade de modal de 120ms. Las demás transiciones
son de color/borde, hasta 150ms; no hay animaciones de transformación ni stagger.
`prefers-reduced-motion` reduce ambas duraciones a 0.01ms.

## Primitivos (API conservada)

| Primitivo | Variantes y estilo |
| --- | --- |
| Button / IconButton / buttonClasses | primary sólido accent/on-accent/600; secondary y outline con borde line-strong; ghost; danger/success con borde al 50% y texto bad/ok; solid para acciones de diálogo. xs/sm/md/lg/dialog/dialog-sm. tone explícito ok/warn/bad/info/neutral; warn nunca usa accent |
| Panel | glass (alias de superficie sólida) / idle; default/compact/hero; borde superior de 2px del tono, neutral line-strong e idle muted; sin borde lateral; título 15px/600 sin uppercase; aliases idle/success/warning/danger conservados |
| ModalShell / modalSurfaceClasses | layers server/launch/scanner; plain/glass sobre modal con borde line-strong; scrim sin blur |
| StatusDot | Cuadrado de 8px, tone y pulse; aliases status ok/warning/error/neutral |
| ToggleSwitch | 34×18px, perilla cuadrada; apagado field/borde muted/perilla muted; encendido ok/borde ok/perilla on-accent; posición instantánea; tone sigue aceptado por compatibilidad |
| Checkbox | Cuadrado de 16px; selección por defecto accent, tone explícito semántico |
| DarkSelect | default/keycap, sm/md; field/line-strong, texto ink incluso en keycap; selección de menú por defecto accent, tone explícito semántico; aliases compact/keycap |
| Input | modal/config/inline; field/line-strong, foco accent; inline conserva tamaños compactos; range/checkbox/radio conservan su elemento y roles nativos; props/ref/handlers conservados |
| DataText / micro-label | Tipografía de datos tabulares y microetiquetas, sin lógica de dominio |

Las features conservan modos, orden de secciones, handlers y textos/roles
accesibles. Solo cambian clases y contenedores presentacionales; se retiró el
wrapper de HP/SP y los detalles largos se colorean aparte de su titular.
Las descripciones de estado permiten ajuste de texto en vez de recortar hints.
No se modificó el IPC ni la lógica de dominio.

## Jerarquía accent / warn

Accent se reserva para Jugar y botones primarios, selección y foco. Un solo
tratamiento dominante por región: no repetirlo en títulos, detalles ni acciones
secundarias. Las selecciones múltiples de teclas comparten el mismo tratamiento.
Pestañas: texto ink + subrayado de 2px accent, sin caja. Servidor seleccionado:
barra izquierda de 3px accent, texto ink y radio cuadrado relleno. Tecla elegida:
fondo accent y texto on-accent. Sliders: pista line-strong y pulgar ink; solo el
foco usa accent. Excepción única de marca: **RO** del wordmark usa accent;
«-Launcher» sigue ink. Progreso de setup y hovers secundarios son neutros.

**Warn marca el contenedor de un aviso, no una etiqueta auxiliar**: barra
izquierda de 3px warn + padding izquierdo + texto muted (≥4.5:1). «Cliente…»
es siempre neutro. Anti-cheat/Gepard, privacidad y demás detalles son muted.
Los puntos y bordes de tono acompañan el estado y sus textos existentes;
no sustituyen la información.
Los logs conservan su clasificación semántica de severidad, sin cambiar su lógica.

En `src/app` + `src/features`, las referencias literales de utilidades pasaron
de **23→25 accent** y **11→12 warn** respecto a `e08b936`, excluyendo tests y
estilos de primitivos: una excepción de marca y un foco de disclosure; warn
se trasladó al contenedor y se añadió al bloque de incidencias dgVoodoo.
[Conteos por archivo y justificación](design-review/color-usage.md).
El rework anterior (`9960370`) eliminó las rampas y reclasificó 37 referencias
accent a warn; el polish (`e08b936`) redujo la repetición. Esta etapa precisa
la semántica de los contenedores, la marca y los estados idle/disabled.

Las rampas se fusionaron en sus roles base; la antigua superposición clara se
sustituyó por line/line-strong/panel-raised. El catálogo de migración contiene solo
nombres vigentes. Se retiró el adaptador de orden CSS: preservaba la cascada del
tema anterior, que este rework sustituye intencionadamente.

## Tema y verificación

Cambiar valores de `:root` (o sobrescribirlos bajo `:root[data-theme="..."]`) y
los mapas de variantes de `src/shared/ui/`. No añadir colores en las features.
El [baseline anterior](design-baseline.md) es evidencia histórica, no un contrato
de igualdad de colores para este tema.

`npm run check:design` prohíbe paletas crudas, rampas retiradas, overlays claros,
radios legacy, tamaños px arbitrarios, tracking arbitrario, efectos y movimiento
decorativo, la utilidad nativa de accent, clases de opacidad y wrappers
presentacionales delineados en features (análisis JSX; controles y pozo de
logs exceptuados). Prohíbe
`<input>`/`<select>` nativos en features: usar Input/DarkSelect; los elementos
nativos permanecen encapsulados en shared/ui para conservar semántica y eventos.
Incluye el cálculo WCAG: los 13 pares de texto exigidos superan 4.5:1;
mínimo bad/panel **4.893:1**, on-accent/accent **6.126:1**.
Ocho pares adicionales validan límites/perillas de switch a 3:1. Chromium mide
ambos estados, incluidos disabled y preparación, y rechaza opacidad heredada:
borde apagado/panel **5.623:1**, perilla apagada/field **6.237:1**;
borde encendido/panel **6.849:1**, perilla encendida/ok **7.447:1**.
Idle: texto y borde superior contra panel **5.623:1**. Disabled: texto y
perilla contra field **6.237:1**; límite exterior contra panel **5.623:1**,
contra surface **6.047:1**, contra modal **5.850:1**. Opacidad efectiva 1.
El borde interno line/field es **1.472:1**: no se presenta como frontera accesible;
el outline muted garantiza ≥3:1 por ambos lados sin alterar los tokens de color.
Los separadores estructurales line no representan un control ni texto.
La guardia corre en el CI existente y acepta fixtures como argumento.

```sh
npm run check:design
npm run check:design -- /tmp/fixture-invalido.tsx # debe devolver código 1
npm run build
npm run preview -- --host 127.0.0.1 --port 5175
npm run review:design -- 5175
npm run review:design -- 5175 1280 820
```

El arnés usa Chromium local (`RO_DESIGN_CHROMIUM` permite cambiar el ejecutable),
IPC simulado y la CSP real sobre el build de producción. Capturas e informes en
[design-review/](design-review/README.md); cubre preparación, en juego, servidor,
escáner, herramientas activas, editores abiertos, Buffs y preparación realista
con cinco servidores, avisos largos y logs, disclosure abierto/cerrado y una
escena sin servidor para controles disabled. Audita cajas interiores, recortes,
overflow, scroll único, acciones fijas, fuentes, contraste compuesto idle/disabled,
switches, selector al redimensionar y reduced-motion.
Pendiente manual: `npm run tauri:dev` con clientes reales, redimensionado real
con decorations false en el gestor de ventanas y texto fantasma en WebKit.
WebKitGTK 2.52.6 está instalado, pero faltan `Xvfb`/`xvfb-run`;
no se reprodujo el texto fantasma ni se
añadieron parches de compositing sin evidencia.
