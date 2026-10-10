# Design system

La fuente de verdad es `src/index.css`: 50 variables, incluidas 17 de color.
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
No reducir la opacidad del texto informativo. En las features de herramientas,
idle atenúa la cabecera al 60%, no el cuerpo ni los switches. Disabled conserva
su estado nativo, textos y cursor; el switch no reduce su opacidad.

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
Datos numéricos, teclas, rutas, argumentos y logs utilizan Mono.

Solo se conservan `pulse-dot` y un fade de modal de 120ms. Las demás transiciones
son de color/borde, hasta 150ms; no hay animaciones de transformación ni stagger.
`prefers-reduced-motion` reduce ambas duraciones a 0.01ms.

## Primitivos (API conservada)

| Primitivo | Variantes y estilo |
| --- | --- |
| Button / IconButton / buttonClasses | primary sólido accent/on-accent/600; secondary y outline con borde line-strong; ghost; danger/success con borde al 50% y texto bad/ok; solid para acciones de diálogo. xs/sm/md/lg/dialog/dialog-sm. tone explícito ok/warn/bad/info/neutral; warn nunca usa accent |
| Panel | glass (alias de superficie sólida) / idle; default/compact/hero; borde superior de 2px del tono, neutral line-strong; sin borde lateral; título 15px/600 sin uppercase; aliases idle/success/warning/danger conservados |
| ModalShell / modalSurfaceClasses | layers server/launch/scanner; plain/glass sobre modal con borde line-strong; scrim sin blur |
| StatusDot | Cuadrado de 8px, tone y pulse; aliases status ok/warning/error/neutral |
| ToggleSwitch | 34×18px, perilla cuadrada; apagado field/borde muted/perilla muted; encendido ok/borde ok/perilla on-accent; posición instantánea; tone sigue aceptado por compatibilidad |
| Checkbox | Cuadrado de 16px; selección por defecto accent, tone explícito semántico |
| DarkSelect | default/keycap, sm/md; field/line-strong, texto ink incluso en keycap; selección de menú por defecto accent, tone explícito semántico; aliases compact/keycap |
| Input | modal/config; field/line-strong, foco accent; props/ref nativos conservados |

Las features conservan modos, orden de secciones, handlers y textos/roles
accesibles. Solo cambian clases y contenedores presentacionales; se retiró el
wrapper de HP/SP y los detalles largos se colorean aparte de su titular.
Las descripciones de estado permiten ajuste de texto en vez de recortar hints.
No se añadió ningún primitivo ni se modificó el IPC.

## Jerarquía accent / warn

Accent se reserva para Jugar y botones primarios, selección y foco. Un solo
tratamiento dominante por región: no repetirlo en títulos, detalles ni acciones
secundarias. Las selecciones múltiples de teclas comparten el mismo tratamiento.
Pestañas: texto ink + subrayado de 2px accent, sin caja. Servidor seleccionado:
barra izquierda de 3px accent, texto ink y radio cuadrado relleno. Tecla elegida:
fondo accent y texto on-accent. Sliders: pista line-strong y pulgar ink; solo el
foco usa accent. Wordmark, progreso de setup y hovers secundarios son neutros.

Warn solo en un titular corto o línea de estado de atención. Anti-cheat/Gepard,
privacidad y demás párrafos de detalle son muted. Los puntos y bordes de tono
acompañan el estado y sus textos existentes; no sustituyen la información.
Los logs conservan su clasificación semántica de severidad, sin cambiar su lógica.

En `src/app` + `src/features`, las referencias literales de utilidades bajaron
de **36→23 accent** y **29→11 warn**, excluyendo tests y estilos de primitivos.
[Conteos por archivo y justificación](design-review/color-usage.md).
El rework anterior (`9960370`) había eliminado las rampas y reclasificado 37
referencias accent a warn; esta etapa reduce la repetición de color en features.

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
decorativo, la utilidad nativa de accent y wrappers presentacionales delineados
en features (análisis JSX; controles y pozo de logs exceptuados).
Incluye el cálculo WCAG: los 13 pares de texto exigidos superan 4.5:1;
mínimo bad/panel **4.893:1**, on-accent/accent **6.126:1**.
Ocho pares adicionales validan límites/perillas de switch a 3:1. Chromium mide
ambos estados, incluidos disabled y preparación, y rechaza opacidad heredada:
borde apagado/panel **5.623:1**, perilla apagada/field **6.237:1**;
borde encendido/panel **6.849:1**, perilla encendida/ok **7.447:1**.
La guardia corre en el CI existente y acepta fixtures como argumento.

```sh
npm run check:design
npm run check:design -- /tmp/fixture-invalido.tsx # debe devolver código 1
npm run build
npm run preview -- --host 127.0.0.1 --port 5175
npm run review:design -- 5175
npm run review:design -- 5175 1100 800
```

El arnés usa Chromium local (`RO_DESIGN_CHROMIUM` permite cambiar el ejecutable),
IPC simulado y la CSP real sobre el build de producción. Capturas e informes en
[design-review/](design-review/README.md); cubre preparación, en juego, servidor,
escáner, herramientas activas, editores abiertos, Buffs y preparación realista
con cinco servidores, avisos largos y logs. Audita cajas interiores, recortes,
overflow, scroll único, acciones fijas, fuentes, switches y reduced-motion.
Pendiente manual: `npm run tauri:dev` con clientes reales. WebKitGTK 2.52.6 está
instalado, pero faltan `Xvfb`/`xvfb-run`; no se reprodujo el texto fantasma ni se
añadieron parches de compositing sin evidencia.
