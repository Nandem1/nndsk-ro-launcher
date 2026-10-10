# Design system

Dirección: **grafito suave**, según [el boceto aprobado](design-reference/refined.html).
La fuente de verdad de producción sigue siendo `src/index.css`: 63 variables,
21 de color (antes 51/17). Los 17 colores anteriores se conservan exactamente.
El HTML de referencia se conserva intacto: sus valores se traducen a tokens y
primitivos, no se copia su CSS ni se introducen fuentes de CDN en producción.

## Color

| Token --c-* | Valor | Uso |
| --- | --- | --- |
| surface | #0E0F11 | Fondo |
| panel | #151618 | Tarjetas |
| panel-raised | #1C1D20 | Secundarios, teclas y selección de servidor |
| field | #0A0B0C | Campos y selects |
| modal | #121315 | Diálogos |
| line-soft | #1F2124 | Borde decorativo de panel/segmentado |
| line | #26282C | Campos, segmento activo y separadores escasos |
| line-strong | #34373A | Hover de campo y scrollbar |
| track | #2A2C31 | Pistas |
| track-fill | #6B6F76 | Relleno sólido de slider |
| outline | #6B6F76 | Contorno de radio/checkbox sin seleccionar, ≥3:1 |
| ink | #E6E7E9 | Texto principal |
| muted | #8B8F96 | Texto secundario/idle |
| accent | #FF5C39 | Acción principal, selección y foco; RO de la marca |
| on-accent | #14110F | Texto sobre accent |
| warn | #E2A33A | Advertencia o trabajo pendiente |
| ok | #7FB069 | Listo/activo |
| bad | #E0604A | Falló/inutilizable/destructivo; HP |
| info | #5B9BD5 | Información; SP |
| special | #4FB3A9 | Shift |
| overlay-dark | RGB 0 0 0 | Scrim 60/70% |

## Forma y tipografía

| Token | Valor |
| --- | --- |
| radius-inline | 6px |
| radius-control-compact | 7px |
| radius-control | 9px |
| radius-panel / modal | 14 / 16px |
| radius-pill | 9999px |
| radius-action / segmented / segment | 11 / 11 / 8px |
| radius-notice | 11px (valor computado del boceto) |
| text-micro / caption / detail | 11px (escala conservada) |
| text-label / xs / sm / base / lg / xl | 12 / 12 / 13 / 15 / 18 / 18px |
| text-panel-title / weight-panel-title / tracking-panel-title | 15px / 600 / 0 |
| text-data | 12.5px (campos) |
| text-brand / tracking-brand | 17px / -0.01em |
| text-key / text-action | 11.5 / 14px (valores computados del boceto) |
| tracking-micro-label | 0 |

`.micro-label` conserva nombre/API: Plex Sans 12px/500/muted, normal-case,
sin tracking; no transforma los textos fuente. Sharp Shooting y Gear Switch
heredan este estilo. Plex Mono solo para datos, versiones, rutas, teclas y logs,
con tabular-nums. La marca usa Plex Sans 600: RO accent, -Launcher ink.
Barlow Condensed, su dependencia, import, token y licencia se retiraron
(recuperables en Git). Sans 400/500/600/700 y Mono 400/500/600 siguen locales,
con licencias OFL en public/licenses. CSP sin cambios; font-src hereda self.

## Componentes: misma API

| Primitivo/patrón | Estilo |
| --- | --- |
| Panel glass/idle; default/compact/hero | panel, borde 1px line-soft, radio 14; padding exterior 16/18; título siempre ink; cabecera sin línea. tone conserva API, no colorea borde; scrollBody designa el único cuerpo desplazable |
| Button secondary/outline; xs/sm/md/lg/dialog/dialog-sm | panel-raised, sin borde, radio 9, 13px/500 y 14/8 en sm/md; xs conserva tamaño compacto |
| Button primary | accent/on-accent/600; Jugar y Preparar entorno radio 11, 14px, padding 13; disabled raised/muted |
| Button ghost | transparente, muted, hover ink |
| Button danger/success y tone bad/ok/info/warn | mezcla tonal 10%, hover 15%, texto del tono; ver decisión de contraste abajo |
| Input modal/config/inline; DarkSelect default/keycap, sm/md | field, line/1px, radio 9, Mono 12.5; hover line-strong; roles/eventos intactos |
| ToggleSwitch | píldora 36×20, perilla circular 16; track/muted, encendido ok/surface; movimiento instantáneo |
| Checkbox / Input radio-checkbox | círculo 16, outline/1.5px; seleccionado: contorno y punto accent; tonos semánticos compatibles |
| StatusDot | círculo de 8px; mismos estados/pulse, neutral muted (≥3:1, no line-strong) |
| Teclas | panel-raised, sin borde, radio 7, alto 28; elegidas accent/on-accent |
| Combate/Buffs | segmentado panel/line-soft, radio 11, padding 3; activo line/ink y radio 8; sin subrayado/iconos |
| Juego/Tools | mismo segmentado a escala reducida: contenedor 28px, segmento 20px/radio 7, Sans 12px/500; sin subrayado |
| Slider | pista track de 4px, fill track-fill, thumb ink circular 14 |
| HP/SP | pista panel-raised de 6px, redondeada; rellenos actuales bad/info |
| ModalShell | modal, line-soft/1px, radio 16, scrim sólido |
| Avisos | notice-warn: warn/9%, radio 11, padding 11/14, punto warn; texto Sans 12.5px muted, sin barra. CollapsibleNotice: dos líneas, chevron Mostrar más/menos y estado local; datos Mono 12px/break-all |
| Scrollbar | 6px, pista transparente, pulgar line-strong redondeado |

Foco: contorno de 2px accent sólido, solo en :focus-visible al navegar con
teclado. Nunca un contorno inicial, de disabled o al clicar con el ratón.
`focus-modality.ts` marca únicamente la modalidad para CSS: Chromium considera
focus-visible también los inputs editables clicados. No intercepta eventos,
mueve foco, modifica valores ni persiste estado; incluye limpieza para HMR.
Un outline transparente de Tailwind outline-none no es un anillo pintado.
Transiciones solo color ≤120ms.
Se conservan pulse-dot, fade de modal 120ms y reduced-motion. Sombras y efectos
decorativos siguen none. El slider usa una imagen CSS del mismo color en ambos
extremos para pintar un relleno plano, sin rampa; solo añade una variable de
pintura al Input controlado, nunca altera valores ni eventos.

Servidor: fila de altura mínima 40px, gap 10 y radio 9; selected panel-raised,
nombre 500, radio circular accent. Editar/quitar siempre visibles con sus nombres
y handlers originales. El mínimo permite crecer a nombres arbitrariamente largos,
en vez de truncarlos; las cinco filas de la fixture caben en 40px.

Cabecera 64px; rail 300px y gap 12px. Se conserva la distribución de columnas,
scroll único y botones inferiores fijos; cuerpos y Logs ceden altura según Encaje.
Ventana permanece 1440×900, mínimo 1280×820, resizable false, decorations false;
su configuración vive en src-tauri/tauri.conf.json.

## Jerarquía y estados

Rellenos tonales en lugar de bordes internos completos. Líneas finas solo entre
grupos, nunca entre filas relacionadas; desaparecen divisores de Herramientas,
HP/SP y teclado. No iconos decorativos en títulos o tabs; se conservan todos los
iconos funcionales y nombres accesibles.

Accent: primarios, teclas/radios elegidos y foco; excepción de marca RO.
Warn: aviso tonal o punto de estado, nunca Cliente ni un párrafo largo coloreado.
Avanzado conserva exactamente el mapeo de presentación de graphite:
pending/warning→warn, error→bad, listo→ok, informativo→neutral.
El estado ya no colorea la tarjeta; aparece en sus puntos/textos existentes.
El título de todo Panel permanece ink incluso idle; solo contenido/controles
se atenúan a muted sin opacidad. Disabled mantiene la superficie/radio del control
y texto muted; las acciones primary/solid pasan a raised/muted. No se añade
un borde ni contorno permanente. Switches mantienen pista y perilla de su estado;
teclas elegidas conservan accent/on-accent también disabled. Disponibilidad intacta.

## Contraste y decisiones

Texto sobre surface/panel/panel-raised ≥4.5:1; muted 5.906/5.576/5.190:1.
on-accent/accent 6.126:1. Aviso warn/9% sobre panel: muted 4.824:1.
Idle/panel 5.576:1. Disabled: texto/field 6.066:1, texto/raised 5.190:1.
Switch: perilla apagada/track 4.303:1; encendida/ok 7.594:1.
Slider: thumb/track 11.291:1.

Decisión de implementación: fondos de botones tonales son mezclas opacas al 10%
sobre panel, idénticas a la composición prevista en esa superficie y estables
también sobre raised/surface. En danger hover (15%) el texto pasa a ink:
bad sobre ese relleno daría 4.299:1; ink da 12.280:1.
Reposo bad/ok/info/warn: 4.595/6.157/5.342/6.979:1; otros hovers ≥4.925:1.
No cambian los colores base.

Decisiones aprobadas para cerrar el bloqueo: line-soft, line y la pista apagada
track son **decorativos**; se reportan, no se someten a 3:1. Sobre panel dan
1.122/1.226/1.296:1 respectivamente. Outline sube a #6B6F76 y sí pasa el umbral:
3.587:1 sobre panel y 3.339:1 sobre panel-raised. El foco accent sólido da
5.899/5.491:1 sobre esas superficies. Perillas, selección, acciones principales
y puntos de estado siguen sujetos a ≥3:1; texto informativo a ≥4.5:1.
El arnés distingue decorativo/estado y falla ante cualquier incumplimiento no
exceptuado. No se modifica refined.html para hacer pasar la comparación.

## Guardias y revisión

`npm run check:design`: sin paletas crudas, radios legacy/arbitrarios, text px,
tracking arbitrario, opacidad, fuentes retiradas, bordes de tono, efectos ni
wrappers anidados delineados. Input/Select nativos encapsulados en shared/ui.
Panel.leading rechaza iconos decorativos. Se permite el segmentado delineado,
un control real; no un wrapper de contenido.

La guardia compila el CSS real de Tailwind, comprueba que cada candidata de
bg/text/border/ring/rounded/shadow/fill/stroke/divide/outline/placeholder/from/to/via
emita regla (incluidas variantes/alfa), y rechaza border/divide sin color explícito
o un primitivo CSS que lo defina. No acepta un token solo porque su nombre parezca
correcto. `npm run check:design:fixtures` prueba 14 prefijos desconocidos, borde
sin color, divisor sin color y un control válido; los negativos deben devolver 1.

[Conformidad computada](design-review/conformance.md): colores ±1 canal,
radios/tamaños ±1px, familia/peso exactos. Se miden las cajas y fuentes pintadas;
los insets exteriores de Panel siguen en su cabecera/cuerpo existentes. El arnés
falla por cualquier desviación sin una justificación específica de contenido,
control funcional o propiedad que no se pinta (p. ej. border-color con borde 0).
La sección de detalles del HTML es estática, sin reglas hover/focus; la revisión
de interacción valida su forma y nuestros estados de hover/foco documentados.

[Capturas, comparación y mediciones](design-review/README.md).
Tema: cambiar tokens de index.css y pocos mapas/primitivos compartidos;
no introducir colores ni radios crudos en features. El catálogo de migración
incluye los cuatro colores nuevos.

`npm run review:design -- 5175` y `-- 5175 --minimum` usan el build
de producción con Chromium, CSP real e IPC simulado. El primero genera también
la comparación lado a lado. Pendiente manual: tauri:dev con clientes reales
y aspecto en WebKit con la configuración de Hyprland, incluido texto fantasma.

## Encaje

En preparación, Logs cede altura desde su máximo existente (196px a 900px de
ventana) hasta 120px antes de reducir los cuerpos de Combate. Se elimina padding
vertical duplicado: una separación de 4px por grupo, sin reducir teclas/campos.
AutoPot y Spammer muestran todos los controles sin scroll interior a 1440×900.

AutoBuff: min-h-0 y recorte exterior; estado/switch fijos; catálogo y reglas
comparten un único cuerpo overflow-y-auto. La cabecera de tabla es sticky y
todas las filas pueden verse completas al desplazarse. Catálogo abierto sin
reglas y cerrado con reglas al montar; chevron con nombre accesible y estado
local, no persistido. Ninguna opción ni acción se retira.

Los avisos de Herramientas y Runner efectivo comienzan con line-clamp-2.
Expandir conserva literalmente todo el contenido. El aviso técnico usa Mono
12px/break-all; campos/selects conservan elipsis y title completo. El rail tiene
un solo scroll, tarjetas sin encogerse y acciones inferiores fijas; Avanzado es
alcanzable aunque se expanda una ruta larga.

Expandir los cinco diagnósticos de Herramientas no cabe junto a Combate completo:
solo entonces su cuerpo usa scroll, con cabecera fija y un presupuesto de altura
max(180px, 100dvh − 680px). Sin expansión no hay scroll activo. La ventana real
sigue fija a 1440×900; la prueba opcional 1280×820 permite un único scroll de
respaldo en cada cuerpo de Combate, porque el espacio de esa prueba no basta.
Sin scroll horizontal ni scroll anidado. La barra existente sigue siendo 6px.

## Congelado

[refined.html](design-reference/refined.html) es la referencia **inmutable**.
[conformance.md](design-review/conformance.md) y el detector de recortes/solapes
son la compuerta local antes de cualquier cambio visual. El detector comprueba
contenedores visibles, salida/solape entre paneles, scroll único y anclaje de
DarkSelect al disparador dentro del viewport. Los únicos recortes intencionales
son los avisos expandibles y la elipsis con tooltip; contenido fuera del área de
scroll debe seguir siendo alcanzable íntegramente. Las pruebas negativas de
recorte, escape, solape, scroll anidado y menú desanclado impiden falsos verdes.

Para cambiar un token:

1. Editar su valor en src/index.css; si se añade un rol/nombre, registrar también
   scripts/design-token-map.mjs y tailwind.config.js. No editar la referencia.
2. Actualizar esta tabla y el cálculo de contraste en scripts/check-design-contrast.mjs
   cuando corresponda. Una divergencia visual de la referencia requiere aprobación,
   no una excepción genérica para hacer pasar el arnés.
3. Ejecutar npm run check:design, npm run check:design:fixtures,
   npm run check:design:fit, npm run lint, npm run format:check, npm test y npm run build.
4. Servir producción con npm run preview -- --host 127.0.0.1 --port 5175;
   ejecutar npm run review:design -- 5175. Genera solo las ocho escenas 1440×900,
   comparación y conformidad. npm run review:design -- 5175 --minimum prueba
   1280×820 y guarda sus artefactos fuera del repo.
5. Revisar diff/capturas y completar [la checklist manual](design-review/premerge-checklist.md).
   Los informes JSON y volcados DOM quedan en /tmp, no en design-review.
