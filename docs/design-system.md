# Design system

La fuente de verdad es `src/index.css`: 51 variables, incluidas 17 de color.
Tailwind expone los canales RGB como `rgb(var(--c-name) / <alpha-value>)`.
Dirección **grafito silencioso**: superficies neutras, baja carga de líneas e
iconos, color reservado a acciones y estados. La UI es plana, sin rampas,
gradientes, blur, glows ni sombras decorativas. No cambian layout ni comportamiento.

## Color

| Token `--c-*` | Valor | Uso |
| --- | --- | --- |
| surface | #0E0F11 | Fondo de la aplicación |
| panel | #151618 | Paneles y filas |
| panel-raised | #1C1D20 | Hover, selección y pista de HP/SP |
| field | #0A0B0C | Campos, selects y controles |
| modal | #121315 | Superficie del diálogo |
| line | #26282C | Separadores entre grupos y bordes de controles inactivos |
| line-strong | #34373A | Hover de controles, pista de sliders, scrollbar y borde superior neutral |
| ink | #E6E7E9 | Texto principal |
| muted | #8B8F96 | Texto secundario; no usar line para texto legible |
| accent | #FF5C39 | Acción principal, foco e indicador de selección |
| on-accent | #14110F | Texto sobre acciones sólidas |
| warn | #E2A33A | Atención, advertencias, trabajo pendiente y revisión |
| ok | #7FB069 | Activo, disponible o correcto |
| bad | #E0604A | Error, HP y acciones destructivas |
| info | #5B9BD5 | Información y SP |
| special | #4FB3A9 | Estado de Shift y opciones especiales |
| overlay-dark | RGB 0 0 0 | Scrim negro sólido con alfa 0.60/0.70 según layer |

Los siete tokens de superficie/línea tienen diferencia máxima entre canales
RGB ≤6: surface 3, panel 3, panel-raised 4, field 2, modal 3, line 6,
line-strong 6. Único ajuste respecto a la propuesta: #34373C→#34373A,
reduciendo azul en 2 para cumplir neutralidad (la propuesta daba 8).
Accent/on-accent/warn/ok/bad/info/special conservan sus valores anteriores.

Los estados mantienen sus textos y controles existentes: el color los acompaña,
no los sustituye. Los puntos son decorativos y están ocultos a lectores de pantalla.
No reducir la opacidad de paneles, texto ni controles. Idle usa título/texto y
borde superior muted. Disabled conserva su disponibilidad nativa: field,
texto muted y borde interno line, con outline muted de 1px como límite visible.
No cambia al hacer hover. Switch disabled conserva aria-checked y posición;
su pista es field y su perilla muted.

## Ventana y espacio

Ventana de **1440×900**, mínimo configurado **1280×820**, `resizable: false`,
sin decoraciones y oculta hasta estar lista; fondo nativo surface **#0E0F11**.
Graphite cambia únicamente backgroundColor del bloque `app.windows`.
Rail fijo de 300px (64px colapsado), contenido flexible sin max-width central.
El alto extra se reparte: Logs recibe el 25% del incremento desde el mínimo
(176→196px entre ambos tamaños; límite 320px); el resto va a los cuerpos de panel.
La búsqueda en frontend/Rust no encontró supuestos de 1280×820: DarkSelect mide
el viewport actual y reposiciona su menú al redimensionar o desplazar el rail.

## Gramática de filas

Panel = borde superior neutral de 1px en line-strong, o 2px para ok/warn/bad/info.
Idle conserva borde muted de 1px para mantener el contraste de estado.
La cabecera no lleva línea inferior. Dentro, líneas finas solo **entre grupos**;
entre filas relacionadas, espacio de 4/8px. No hay bordes completos ni fondos
propios. Solo inputs, selects, teclas, botones y el pozo de logs conservan caja.
Los modales conservan su superficie exterior; dentro usan grupos de contenido.

Herramientas es una fila de tres columnas separadas por `border-l border-line`;
Cliente y sus diagnósticos van debajo con una línea superior. HP/SP quedan
directamente en AutoPot, sin línea entre HP y SP. Modo proactivo y lectura forman
un grupo; perfil y controles HP/SP, otro. En Spammer, teclado/delay y cada editor
son grupos; no hay líneas entre etiquetas, controles y filas de reglas.
Servidor, clientes y diagnósticos de Avanzado usan espacio entre sus filas.
Filas de slider/control: mínimo 24px; línea de diagnóstico: mínimo 16px y altura
flexible para hints largos. Los controles individuales conservan sus bordes.
Combate/Buffs solo muestra el subrayado activo de 2px, separado de los paneles
por el gap existente; no hay línea de ancho completo.

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
| tracking-micro-label | 0.06em |
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
La utilidad `.micro-label` usa Mono/11px/mayúsculas/tracking 0.06em/muted para
TECLAS, LECTURA, PERFIL DE MEMORIA, DELAY, HP/SP y categorías similares.
Títulos de panel/campo usan Sans sin uppercase (los textos fuente no cambian).
Datos numéricos, teclas, versiones, rutas, argumentos y logs utilizan Mono
con `tabular-nums`; `.font-mono` también activa los números tabulares.
DataText decora solo números/unidades/versiones/rutas dentro de texto mixto,
sin transformar su contenido. Inputs numéricos lo aplican automáticamente.

Solo se conservan `pulse-dot` y un fade de modal de 120ms. Las demás transiciones
son de color/borde, hasta 150ms; no hay animaciones de transformación ni stagger.
`prefers-reduced-motion` reduce ambas duraciones a 0.01ms.
Scrollbar de 6px, pista transparente, pulgar line-strong.
Controles inactivos: borde line; hover line-strong; foco visible con outline
accent de 1px y separación de 2px, sin sombra. Disabled mantiene su outline muted.
Los títulos, Combate/Buffs y los editores no llevan iconos decorativos.
Se conservan iconos funcionales (Jugar, preparar, editar, cerrar, refrescar,
buscar, desplegar) y sus nombres accesibles. Panel.leading admite controles
con nombre o indicadores de estado, no adornos Lucide sin función.
Versión: texto muted Mono, sin recuadro. En juego conserva punto y texto,
también sin recuadro. Ningún texto de interfaz cambia.

## Primitivos (API conservada)

| Primitivo | Variantes y estilo |
| --- | --- |
| Button / IconButton / buttonClasses | primary sólido accent/on-accent/600; secondary y outline con borde line y hover line-strong; ghost; danger/success con borde al 50% y texto bad/ok; solid para acciones de diálogo. xs/sm/md/lg/dialog/dialog-sm. tone explícito ok/warn/bad/info/neutral; warn nunca usa accent |
| Panel | glass (alias de superficie sólida) / idle; default/compact/hero; borde superior neutral line-strong/1px, idle muted/1px, estados 2px; sin línea de cabecera ni borde lateral; título 15px/600 sin uppercase; aliases idle/success/warning/danger conservados |
| ModalShell / modalSurfaceClasses | layers server/launch/scanner; plain/glass sobre modal con borde line-strong; scrim sin blur |
| StatusDot | Cuadrado de 8px, tone y pulse; aliases status ok/warning/error/neutral |
| ToggleSwitch | 34×18px, perilla cuadrada; apagado field/borde muted/perilla muted; encendido ok/borde ok/perilla on-accent; posición instantánea; tone sigue aceptado por compatibilidad |
| Checkbox | Cuadrado de 16px; selección por defecto accent, tone explícito semántico |
| DarkSelect | default/keycap, sm/md; field/line, hover line-strong, texto ink incluso en keycap; selección de menú por defecto accent, tone explícito semántico; aliases compact/keycap |
| Input | modal/config/inline; field/line, hover line-strong, foco accent de 1px; inline conserva tamaños compactos; range/checkbox/radio conservan su elemento y roles nativos; props/ref/handlers conservados |
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
de **25→19 accent** y **12→12 warn** respecto a `f3bcbad`, excluyendo tests y
estilos de primitivos. Se centralizaron seis focos de campos en la regla común;
no se retiró foco visible. Las barras de avisos y selección siguen en 3px.
[Conteos por archivo y justificación](design-review/color-usage.md).
El rework anterior (`9960370`) eliminó las rampas y reclasificó 37 referencias
accent a warn; el polish (`e08b936`) redujo la repetición. Esta etapa precisa
la semántica de los contenedores, la marca y los estados idle/disabled.

## Semántica de Avanzado

bad = falló o es inutilizable; warn = pendiente o requiere acción; ok = listo;
neutral = informativo (conteos de Observaciones/Benchmarks). El borde superior
toma la mayor gravedad de sus filas: bad > warn > ok > neutral.
`advanced.presentation.ts` consume las severidades estructuradas de `checks`:
pending/warning son warn, error permanece bad incluso si el hint dice pendiente.
Una compatibilidad incompatible es bad; desconocida, warn. No se clasifican
estados por palabras del hint. Sin check explícito se conserva el fallback
existente de los booleanos. No se modifican readiness, acciones, stores,
advanced.logic ni productores IPC/Rust; un error real no se rebaja a pendiente.

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
También rechaza iconos Lucide/SVG decorativos en Panel.leading (análisis JSX).
Incluye neutralidad RGB y cálculo WCAG: los 15 pares de texto exigidos superan
4.5:1; mínimo bad/panel **5.123:1**, on-accent/accent **6.126:1**.
Ocho pares adicionales validan límites/perillas de switch a 3:1. Chromium mide
ambos estados, incluidos disabled y preparación, y rechaza opacidad heredada:
borde apagado/panel **5.576:1**, perilla apagada/field **6.066:1**;
borde encendido/panel **7.170:1**, perilla encendida/ok **7.447:1**.
Idle: texto y borde superior contra panel **5.576:1**. Disabled: texto y
perilla contra field **6.066:1**; límite exterior contra panel **5.576:1**,
contra surface **5.906:1**, contra modal **5.725:1**. Opacidad efectiva 1.
El borde interno line/field es **1.334:1**: no se presenta como frontera accesible;
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
escena sin servidor para controles disabled y preparación con runtime pendiente.
Audita líneas e iconos, cajas interiores, recortes,
overflow, scroll único, acciones fijas, fuentes, contraste compuesto idle/disabled,
switches, selector al redimensionar y reduced-motion.
Pendiente manual: `npm run tauri:dev` con clientes reales y texto fantasma en
WebKit. La ventana permanece no redimensionable, como en `f3bcbad`.
WebKitGTK 2.52.6 está instalado, pero faltan `Xvfb`/`xvfb-run`;
no se reprodujo el texto fantasma ni se
añadieron parches de compositing sin evidencia.
