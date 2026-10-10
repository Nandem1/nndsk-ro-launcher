# Design system

La fuente de verdad es `src/index.css`: 50 variables, incluidas 17 de color.
Tailwind expone los canales RGB como `rgb(var(--c-name) / <alpha-value>)`.
La UI es plana, sin rampas, gradientes, blur, glows ni sombras decorativas.

## Color

| Token `--c-*` | Valor | Uso |
| --- | --- | --- |
| surface | #0F1412 | Fondo de la aplicación |
| panel | #161C19 | Paneles y filas |
| panel-raised | #1B2420 | Hover, selección y pista de barras |
| field | #0B100E | Campos, selects y controles |
| modal | #121815 | Superficie del diálogo |
| line | #2A332E | Separadores y filas |
| line-strong | #3A463F | Bordes de campos y borde superior neutral |
| ink | #E8EEE8 | Texto principal |
| muted | #8A968D | Texto secundario; no usar line para texto legible |
| accent | #FF5C39 | Acción principal, foco, selección y marca |
| on-accent | #14110F | Texto sobre acciones sólidas |
| warn | #E2A33A | Atención, advertencias, trabajo pendiente y revisión |
| ok | #7FB069 | Activo, disponible o correcto |
| bad | #E0604A | Error, HP y acciones destructivas |
| info | #5B9BD5 | Información y SP |
| special | #4FB3A9 | Estado de Shift y opciones especiales |
| overlay-dark | RGB 0 0 0 | Scrim negro sólido con alfa 0.60/0.70 según layer |

Los estados mantienen sus textos y controles existentes: el color los acompaña,
no los sustituye. Los puntos son decorativos y están ocultos a lectores de pantalla.
No reducir la opacidad del texto informativo; idle (60%) y disabled conservan
sus señales anteriores.

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
| ToggleSwitch | 34×18px, perilla cuadrada; encendido ok, posición instantánea; tone sigue aceptado por compatibilidad |
| Checkbox | Cuadrado de 16px; selección por defecto accent, tone explícito semántico |
| DarkSelect | default/keycap, sm/md; field/line-strong; selección por defecto accent, tone explícito semántico; aliases compact/keycap |
| Input | modal/config; field/line-strong, foco accent; props/ref nativos conservados |

Las features conservan dimensiones, grids, modos, jerarquía DOM y handlers.
Las filas reutilizan line como borde superior. Las descripciones de estado
permiten ajuste de texto en vez de recortar los hints de benchmarks.
No se añadió ningún primitivo ni se modificó el IPC.

## Auditoría accent / warn

Se revisaron 129 referencias originales de accent, incluyendo stops de progreso
y el test de estilo de logs. Quedan 49 referencias de acción, foco, selección,
progreso y marca; 37 referencias explícitas se reclasificaron de accent a warn
(contadas en los hunks de estilo correspondientes). Las restantes se neutralizaron,
se pasaron a ok para estado activo o desaparecieron junto con rampas/efectos.

Warn cubre inicio limitado, avisos de mantenimiento, actualizaciones pendientes,
runners fuera del catálogo/propios, diagnósticos y dependencias, lectura de memoria
no disponible, privacidad de argumentos, escáner ocupado, mensajes de logs y
variantes warn de primitivos. Buscar, comparar y continuar en los diálogos son
acciones accent, no advertencias. Activación de herramientas y switches usa ok.
Selección de servidor, tabs, teclas y foco conserva accent.

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
decorativo. Incluye el cálculo WCAG: los 13 pares exigidos superan 4.5:1;
mínimo bad/panel **4.893:1**, on-accent/accent **6.126:1**.
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
escáner y herramientas activas, fuentes locales, recortes, overflow y reduced-motion.
Pendiente manual: `npm run tauri:dev` en WebKit/Tauri con clientes reales.
