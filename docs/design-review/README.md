# Revisión de unslop-refine

Build de producción, Chromium local, CSP real de Tauri e IPC simulado. Base:
`e08b936` (`rework/unslop-polish`). No se usaron datos ni clientes reales.

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
| Sin servidor: controles deshabilitados | [empty](prep-empty-1440x900.png) | [empty](prep-empty-1280x820.png) |

24 capturas nuevas/actualizadas. Informes [1440×900](review-1440x900.json) y
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

Avanzado comienza con el grupo Benchmarks A/B cerrado. Se verificaron apertura,
cierre, orden y disponibilidad de las 13 acciones de observaciones/benchmarks.
DarkSelect conserva rol combobox del selector sustituido; selección por puntero
y teclado, menú dentro del viewport y reposicionado al redimensionar.
Las fuentes locales cargan sus ocho caras con HTTP 200 y `document.fonts`;
familias computadas: Plex Sans para UI, Plex Mono tabular para datos, Barlow para marca.

Contraste compuesto, con opacidad efectiva 1:

| Par | Ratio |
| --- | --- |
| Idle: título, texto y borde superior / panel | 5.623:1 |
| Disabled: texto y perilla / field | 6.237:1 |
| Disabled: límite exterior / panel | 5.623:1 |
| Disabled: límite exterior / surface | 6.047:1 |
| Disabled: límite exterior / modal | 5.850:1 |
| Borde interno line / field (no es el límite accesible) | 1.472:1 |

Se mantiene el borde line solicitado y se añade outline muted de 1px, porque
line solo no alcanza 3:1. El límite visible supera 3:1 por ambos lados; el texto
supera 4.5:1. El arnés rechaza opacidad heredada y mide todos los controles disabled
renderizados, también con el grupo abierto y sin servidor. Switches habilitados
conservan ambos estados legibles. Reduced-motion sigue verificado.

Guardia: 47 referencias opacity-*→0 en src; 20 inputs y un select nativos→0
en features, encapsulados ahora en Input/DarkSelect con los handlers originales.
Fixtures de paleta, opacidad y controles nativos fallan con código 1; uno válido
pasa. [Auditoría de accent/warn](color-usage.md): 23→25 y 11→12 en app/features.

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
ventana sin decoraciones en el gestor de ventanas; reproducir el texto fantasma
en WebKit. Hay WebKitGTK 2.52.6, pero no Xvfb/xvfb-run. No se aplicaron parches
de compositing. Chromium/IPC simulado no valida esas condiciones de escritorio.
