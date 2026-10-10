# Design system

Dirección: **grafito suave**, según [el boceto aprobado](design-reference/refined.html).
La fuente de verdad de producción sigue siendo `src/index.css`: 60 variables,
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
| outline | #4A4E55 | Contorno de radio/checkbox sin seleccionar |
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
| text-micro / caption / detail | 11px (escala conservada) |
| text-label / xs / sm / base / lg / xl | 12 / 12 / 13 / 15 / 18 / 18px |
| text-panel-title / weight-panel-title / tracking-panel-title | 15px / 600 / 0 |
| text-data | 12.5px (campos) |
| text-brand / tracking-brand | 17px / -0.01em |
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
| Panel glass/idle; default/compact/hero | panel, borde 1px line-soft, radio 14; padding exterior 16/18; cabecera sin línea. tone conserva API, no colorea borde |
| Button secondary/outline; xs/sm/md/lg/dialog/dialog-sm | panel-raised, sin borde, radio 9, 13px/500 y 14/8 en sm/md; xs conserva tamaño compacto |
| Button primary | accent/on-accent/600; Jugar y Preparar entorno radio 11, padding 13 |
| Button ghost | transparente, muted, hover ink |
| Button danger/success y tone bad/ok/info/warn | mezcla tonal 10%, hover 15%, texto del tono; ver decisión de contraste abajo |
| Input modal/config/inline; DarkSelect default/keycap, sm/md | field, line/1px, radio 9, Mono 12.5; hover line-strong; roles/eventos intactos |
| ToggleSwitch | píldora 36×20, perilla circular 16; track/muted, encendido ok/surface; movimiento instantáneo |
| Checkbox / Input radio-checkbox | círculo 16, outline/1.5px; seleccionado: contorno y punto accent; tonos semánticos compatibles |
| StatusDot | círculo de 8px; mismos estados/pulse |
| Teclas | panel-raised, sin borde, radio 7, alto 28; elegidas accent/on-accent |
| Combate/Buffs | segmentado panel/line-soft, radio 11, padding 3; activo line/ink y radio 8; sin subrayado/iconos |
| Slider | pista track de 4px, fill track-fill, thumb ink circular 14 |
| HP/SP | pista panel-raised de 6px, redondeada; rellenos actuales bad/info |
| ModalShell | modal, line-soft/1px, radio 16, scrim sólido |
| Avisos | notice-warn: warn/9%, radio 9, punto warn (pseudo-elemento decorativo), texto muted; sin barra |
| Scrollbar | 6px, pista transparente, pulgar line-strong redondeado |

Foco: contorno de 2px accent al 50%, sin glow. Transiciones solo color ≤120ms.
Se conservan pulse-dot, fade de modal 120ms y reduced-motion. Sombras y efectos
decorativos siguen none. El slider usa una imagen CSS del mismo color en ambos
extremos para pintar un relleno plano, sin rampa; solo añade una variable de
pintura al Input controlado, nunca altera valores ni eventos.

Servidor: fila de altura mínima 40px, gap 10 y radio 9; selected panel-raised,
nombre 500, radio circular accent. Editar/quitar siempre visibles con sus nombres
y handlers originales. El mínimo permite crecer a nombres arbitrariamente largos,
en vez de truncarlos; las cinco filas de la fixture caben en 40px.

Cabecera 64px; rail 300px y gap 12px. No cambia la distribución de columnas,
scroll único, botones inferiores fijos, cuerpos flexibles ni altura de Logs.
Ventana permanece 1440×900, mínimo 1280×820, resizable false, decorations false;
src-tauri no se modifica.

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
Idle usa muted sin opacidad; disabled mantiene field/muted y contorno muted
visible de 1px, con disponibilidad nativa intacta.

## Contraste y decisiones

Texto sobre surface/panel/panel-raised ≥4.5:1; muted 5.906/5.576/5.190:1.
on-accent/accent 6.126:1. Aviso warn/9% sobre panel: muted 4.824:1.
Idle/panel 5.576:1. Disabled: texto/field 6.066:1; límite exterior ≥5.190:1.
Switch: perilla apagada/track 4.303:1; encendida/ok 7.594:1.
Slider: thumb/track 11.291:1.

Decisión de implementación: fondos de botones tonales son mezclas opacas al 10%
sobre panel, idénticas a la composición prevista en esa superficie y estables
también sobre raised/surface. En danger hover (15%) el texto pasa a ink:
bad sobre ese relleno daría 4.299:1; ink da 12.280:1.
Reposo bad/ok/info/warn: 4.595/6.157/5.342/6.979:1; otros hovers ≥4.925:1.
No cambian los colores base.

**Cierre pendiente de decisión de contraste:** el boceto exige contornos que
no llegan a 3:1: outline/panel 2.166:1, line/panel 1.226:1,
track/panel 1.296:1, anillo accent/50% sobre panel 2.326:1.
line-soft/panel 1.122:1 es decorativo, no un límite funcional.
Una interfaz fiel al boceto no puede satisfacer simultáneamente el umbral
estricto para todos esos bordes sin añadir contornos o cambiar colores.
El arnés devuelve código 1 por los límites de switches apagados y guarda
contrastFailures; no confunde outline-style:none con un contorno visible.
No se ha autorizado una excepción ni añadido contornos nuevos a controles activos.

## Guardias y revisión

`npm run check:design`: sin paletas crudas, radios legacy/arbitrarios, text px,
tracking arbitrario, opacidad, fuentes retiradas, bordes de tono, efectos ni
wrappers anidados delineados. Input/Select nativos encapsulados en shared/ui.
Panel.leading rechaza iconos decorativos. Se permite el segmentado delineado,
un control real; no un wrapper de contenido.

[Capturas, comparación y mediciones](design-review/README.md).
Tema: cambiar tokens de index.css y pocos mapas/primitivos compartidos;
no introducir colores ni radios crudos en features. El catálogo de migración
incluye los cuatro colores nuevos.

`npm run review:design -- 5175 1440 900` y `-- 5175 1280 820` usan el build
de producción con Chromium, CSP real e IPC simulado. El primero genera también
la comparación lado a lado. Pendiente manual: tauri:dev con clientes reales
y aspecto en WebKit con la configuración de Hyprland, incluido texto fantasma.
