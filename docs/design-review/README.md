# Revisión de unslop-graphite

Build de producción, Chromium local, CSP real de Tauri e IPC simulado. Base:
`f3bcbad` (`rework/unslop-refine`). No se usaron datos ni clientes reales.
Dirección: grafito silencioso, superficies neutras, separadores entre grupos,
sin iconos decorativos ni nuevos efectos. Layout, textos y acciones se conservan.

| Escena | 1440×900 (defecto) | 1280×820 (mínimo) |
| --- | --- | --- |
| Preparación | [prep](prep-1440x900.png) | [prep](prep-1280x820.png) |
| En juego | [ingame](ingame-1440x900.png) | [ingame](ingame-1280x820.png) |
| Servidor | [server](server-1440x900.png) | [server](server-1280x820.png) |
| Escáner | [scanner](scanner-1440x900.png) | [scanner](scanner-1280x820.png) |
| Herramientas activas | [active](active-1440x900.png) | [active](active-1280x820.png) |
| Editores abiertos | [active-editors](active-editors-1440x900.png) | [active-editors](active-editors-1280x820.png) |
| Buffs con dos reglas | [buffs](buffs-1440x900.png) | [buffs](buffs-1280x820.png) |
| Preparación realista, grupo cerrado | [realistic](prep-realistic-1440x900.png) | [realistic](prep-realistic-1280x820.png) |
| Realista, scroll inferior y grupo cerrado | [scrolled](prep-realistic-scrolled-1440x900.png) | [scrolled](prep-realistic-scrolled-1280x820.png) |
| Realista, grupo abierto | [open](prep-realistic-open-1440x900.png) | [open](prep-realistic-open-1280x820.png) |
| Realista, grupo abierto y scroll inferior | [open-scrolled](prep-realistic-open-scrolled-1440x900.png) | [open-scrolled](prep-realistic-open-scrolled-1280x820.png) |
| Preparación sin entorno, runtime pendiente | [pending](prep-pending-1440x900.png) | [pending](prep-pending-1280x820.png) |
| Runtime pendiente, diagnósticos visibles | [pending-scrolled](prep-pending-scrolled-1440x900.png) | [pending-scrolled](prep-pending-scrolled-1280x820.png) |
| Sin servidor: controles deshabilitados | [empty](prep-empty-1440x900.png) | [empty](prep-empty-1280x820.png) |

28 capturas nuevas/actualizadas. Informes [1440×900](review-1440x900.json) y
[1280×820](review-1280x820.json): cero errores de consola, desbordes, texto
recortado y cajas interiores. Los scrolls previstos no cuentan como recortes:
conservan el contenido completo. Las capturas 1100×800 y su informe pertenecen
al polish anterior, antes de fijar el nuevo mínimo; se conservan como historial.

Revisión visual: rail fijo de 300px (64px en juego); contenido ocupa todo el ancho
disponible. Entre mínimo/defecto, Logs crece 176→196px y los cuerpos de herramientas
ganan 60px en preparación estándar/en juego, sin max-width ni centrado artificial.
La escena realista conserva cinco servidores, Setup.exe, HoneyRO Patcher.exe,
dgVoodoo conf OK, avisos anti-cheat/Gepard y cuatro líneas largas de logs.
La columna izquierda conserva un único scroll y acciones inferiores fijas.
La nueva escena incorpora runner nndsk-ro-proton 0.1.0-dev.2 y los hints exactos
solicitados; runner/compatibilidad/entorno/DXVK warn, audio/permisos/uinput ok.
El panel es warn. Un error explícito de runner se prueba sin captura adicional:
fila/panel bad aunque el hint siga diciendo pendiente. Se conserva la acción
Preparar entorno habilitada y no se ofrece Jugar hasta estar listo, igual que antes.

## Líneas e iconos

Elementos DOM con clases de ancho `border-t`/`border-b` y borde resuelto >0
dentro de paneles; no incluye su borde superior exterior ni bordes completos de
controles. Incluye filas del scroll, no solo el fragmento del viewport. Medidos
antes y después con el mismo arnés; ambos tamaños dan los mismos conteos:

| Escena | Antes → después |
| --- | --- |
| prep | 30 → 10 |
| server | 30 → 10 |
| ingame | 15 → 7 |
| scanner | 15 → 7 |
| active | 15 → 7 |
| active-editors | 18 → 7 |
| buffs | 7 → 3 |
| prep-realistic | 34 → 10 |
| prep-realistic-scrolled | 34 → 10 |
| prep-realistic-open | 34 → 10 |
| prep-realistic-open-scrolled | 34 → 10 |
| prep-pending | 35 → 10 |
| prep-pending-scrolled | 35 → 10 |
| prep-empty | 28 → 9 |

9 colocaciones de iconos retiradas (3 títulos, 2 pestañas, 2 disclosures y 2
etiquetas ATK/DEF). Adornos de título/pestañas/disclosure renderizados: combate
6→0, Buffs 3→0. Se conservan iconos funcionales y sus nombres accesibles.
La regla de pestañas de ancho completo desaparece; el subrayado de 2px no toca
los paneles. Avisos/selección de servidor conservan barras de 3px.

Avanzado comienza con el grupo Benchmarks A/B cerrado. Se verificaron apertura,
cierre, orden y disponibilidad de las 13 acciones de observaciones/benchmarks.
DarkSelect conserva rol combobox del selector sustituido; selección por puntero
y teclado, menú dentro del viewport y reposicionado al redimensionar.
Las fuentes locales cargan sus ocho caras con HTTP 200 y `document.fonts`;
familias computadas: Plex Sans para UI, Plex Mono tabular para datos, Barlow para marca.

## Neutralidad y contraste

Máximo RGB menos mínimo RGB, umbral ≤6:

| Token | Canales | Diferencia |
| --- | --- | --- |
| surface | 14 15 17 | 3 |
| panel | 21 22 24 | 3 |
| panel-raised | 28 29 32 | 4 |
| field | 10 11 12 | 2 |
| modal | 18 19 21 | 3 |
| line | 38 40 44 | 6 |
| line-strong | 52 55 58 | 6 |

Único ajuste: line-strong #34373C→#34373A (azul −2; diferencia 8→6).
Los siete colores de acción/estado no cambian. WCAG sRGB, sin redondeos intermedios:

| Texto | Surface | Panel |
| --- | --- | --- |
| muted | 5.906:1 | 5.576:1 |
| ink | 15.498:1 | 14.633:1 |
| warn | 8.706:1 | 8.220:1 |
| ok | 7.594:1 | 7.170:1 |
| bad | 5.426:1 | 5.123:1 |
| info | 6.478:1 | 6.117:1 |
| accent | 6.248:1 | 5.899:1 |

on-accent/accent: **6.126:1**. Todos ≥4.5:1.
Contraste compuesto, con opacidad efectiva 1:

| Par | Ratio |
| --- | --- |
| Idle: título, texto y borde superior / panel | 5.576:1 |
| Disabled: texto y perilla / field | 6.066:1 |
| Disabled: límite exterior / panel | 5.576:1 |
| Disabled: límite exterior / surface | 5.906:1 |
| Disabled: límite exterior / modal | 5.725:1 |
| Borde interno line / field (no es el límite accesible) | 1.334:1 |

Se mantiene el borde line solicitado y se añade outline muted de 1px, porque
line solo no alcanza 3:1. El límite visible supera 3:1 por ambos lados; el texto
supera 4.5:1. El arnés rechaza opacidad heredada y mide todos los controles disabled
renderizados, también con el grupo abierto y sin servidor. Switches habilitados
conservan ambos estados legibles. Reduced-motion sigue verificado.
Switch habilitado: borde ok/panel 7.170:1; perilla on-accent/ok 7.447:1.
Cuatro controles (input, tecla inactiva, secondary, select) se prueban por
reposo/hover/teclado: line→line-strong, foco accent de 1px, box-shadow none.
Scrollbar de 6px, pista transparente, pulgar line-strong.

Guardia: siguen cero paletas crudas, opacity-* y inputs/selects nativos en
features. Se añade neutralidad, contraste de ink y rechazo de Lucide/SVG
decorativo en Panel.leading. Fixtures inválidos de paleta e icono fallan con
código 1; un IconButton con nombre pasa.
[Auditoría de accent/warn](color-usage.md): 25→19 y 12→12 en app/features;
seis focos centralizados, sin perder accesibilidad.

Repetir:

```sh
npm run build
npm run preview -- --host 127.0.0.1 --port 5175
npm run review:design -- 5175 1440 900
npm run review:design -- 5175 1280 820
```

`RO_DESIGN_CHROMIUM` permite otro Chromium instalado y `RO_DESIGN_REVIEW_OUTPUT`
un destino temporal. Los volcados DOM detallados quedan en `/tmp`, no en el repo.

Pendiente manual: `npm run tauri:dev` con clientes reales; redimensionado de la
ventana fija sin decoraciones en Hyprland; reproducir el texto fantasma
en WebKit. Hay WebKitGTK 2.52.6, pero no Xvfb/xvfb-run. No se aplicaron parches
de compositing. Chromium/IPC simulado no valida esas condiciones de escritorio.
No se modificó la configuración de Hyprland ni el tamaño/resizable de la ventana.
