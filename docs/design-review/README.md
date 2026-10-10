# Revisión final · grafito suave / fit

Rama rework/unslop-soft-fit, base aa02414. Referencia
[refined.html](../design-reference/refined.html) intacta. Build de producción,
Chromium local, CSP real e IPC simulado; ninguna operación con clientes reales.
Paleta, fuentes, radios y aspecto de los controles se conservan.

## Cierre local

Pasan tsc/build, lint, format:check, 201 tests en 38 archivos, check:design,
fixtures de clases/bordes y cinco fixtures negativas del detector de encaje.
Rust fmt, clippy (workspace/all-targets/all-features, warnings como errores),
tests workspace/all-features y build del launcher pasan; siete pruebas Rust
ignoradas por prerrequisitos existentes.

25 estados comprobados a 1440×900 y otros 25 a 1280×820: cero errores de consola,
desbordes, recortes no intencionales, escapes/solapes de panel y scrolls anidados.
Se conserva acceso íntegro al contenido de los scrolls; no se ocultan fallos
por quedar fuera del viewport. Avisos line-clamp-2 y elipsis con title son
las únicas truncaciones explícitamente aprobadas.

[Conformidad](conformance.md): 27 filas, cero desviaciones injustificadas,
incluida Juego/Tools derivada del segmentado: contenedor 28px, segmento 20px,
radio 7 y Plex Sans 12px/500. Hover, foco por teclado, sin anillos al cargar/clicar,
switches/radios, fuentes y disponibilidad de acciones siguen comprobados.

## Causas y espacio recuperado

AutoBuff tenía catálogo shrink-0, una lista con mínimo de 56px y ninguna
contención del panel: seis reglas y el catálogo excedían la altura disponible.
Boxes, + Manual y el selector de tecla llegaban a Logs; el detector anterior
no examinaba la salida de elementos de su panel y daba un falso verde.

Ahora estado/switch permanecen fijos, catálogo y reglas comparten un único
cuerpo min-h-0/overflow-auto y la cabecera de tabla es sticky. Se recorrieron
las seis filas mostrando cada una completa; catálogo abierto/cerrado y selector
abierto conservan todas sus opciones. Ocho reglas también pasan en juego y
durante Iniciando juego... (texto original).

En Combate, Logs antes era shrink-0 y los avisos completos consumían altura
antes de los controles. Ahora Logs cede hasta 120px y los grupos usan un solo
gap de 4px, sin padding vertical duplicado. No se redujeron campos ni teclas.
No hay scroll interior de AutoPot/Spammer en preparación a 1440×900.

| Región · fixture realista 1440×900 | Antes | Después | Ganancia |
| --- | --- | --- | --- |
| Herramientas (alto consumido) | 316.875px | 216.750px | 100.125px liberados |
| Runner (alto consumido) | 462.625px | 325px | 137.625px liberados |
| AutoPot / Spammer (cada panel) | 233.125px | 411.250px | 178.125px disponibles |
| AutoBuff en preparación | 233.125px | 333.250px | 100.125px disponibles |
| Logs en preparación Combate | 196px | 120px | cede 76px |

Avisos expandido→colapsado: Herramientas 118.875→60.750px (58.125px);
Runner 198.375→60.750px (137.625px). Todos los textos y la línea
“Propio del servidor; el predeterminado global no lo reemplaza.” se conservan.
Rutas/datos Mono 12px/break-all; el único scroll del rail permite alcanzar
Avanzado y conserva fijos los botones inferiores.

Al expandir cinco diagnósticos no caben junto a Combate completo: solo entonces
Herramientas usa su cuerpo desplazable con cabecera fija, sin empujar Logs fuera.
A 1280×820, viewport de estrés ajeno a la ventana fija, cada cuerpo de Combate
puede usar su único scroll de respaldo. También allí hay cero recortes permanentes.

El detector encontró otro defecto: un DarkSelect corto abierto hacia arriba
usaba maxHeight=192px para situar una lista real de 67px, dejando 129px de hueco.
Ahora se ancla por su altura pintada: separación 4px, dentro del viewport,
incluido tras redimensionar. No cambian selección, teclado ni IPC.

Títulos siempre ink: se retiró tanto la clase muted de idle como su sobreescritura
CSS; contenido/controles siguen muted sin opacidad. Runner y campos largos
conservan métrica/elipsis y exponen el valor completo por title.

## Contraste y capturas

Abrir (OpenSetup/Patcher) y Config habilitados en entorno listo: 13.621:1
(ink sobre panel-raised). Pasan sin modificar su estilo. Disabled antes de
preparar es intencional. Contrastes de texto/estado sin fallos fuera de las
excepciones decorativas aprobadas; aviso muted/warn9% 4.824:1.
Siete caras locales Plex cargadas/HTTP 200; sin CDN ni Barlow.

| Escena guardada · 1440×900 | Captura |
| --- | --- |
| Preparación vacía | [vacía](prep-empty-1440x900.png) |
| Preparación Combate realista | [Combate](prep-combat-fit-1440x900.png) |
| Preparación Buffs realista, seis reglas | [Buffs](prep-buffs-1440x900.png) |
| En juego Combate con editores | [editores](active-editors-1440x900.png) |
| En juego Buffs, ocho reglas | [Buffs en juego](ingame-buffs-1440x900.png) |
| Servidor | [servidor](server-1440x900.png) |
| Escáner | [escáner](scanner-1440x900.png) |
| Entorno listo | [listo](prep-ready-1440x900.png) |

[Referencia frente a aplicación](reference-vs-app-1440x900.png), 2880×900:
referencia a la izquierda, Combate realista a la derecha. Diferencias aprobadas:
contenido real, avisos expandibles, catálogo replegable, Logs flexible,
espaciado de encaje y pestañas reducidas de Logs; no se copia el layout de ejemplo.

La carpeta queda en 13 archivos: cuatro Markdown, ocho escenas y la comparación.
Se retiran 37 PNG/JSON antiguos, recuperables desde aa02414. Tamaño (du -sb):
4,922,369 bytes antes; 1,158,639 bytes después. Informes y volcados
detallados se guardan en /tmp, no se vuelven a versionar.

## Repetir la compuerta

```sh
npm run build
npm run check:design
npm run check:design:fixtures
npm run check:design:fit
npm run preview -- --host 127.0.0.1 --port 5175
npm run review:design -- 5175
npm run review:design -- 5175 --minimum
```

La última orden guarda capturas/informe 1280×820 en /tmp/ro-soft-fit-minimum.
La predeterminada genera solo las ocho escenas 1440×900, comparación y
conformance.md; JSON en /tmp/ro-soft-fit-review-1440x900.json.
RO_DESIGN_CHROMIUM y RO_DESIGN_REVIEW_OUTPUT permiten ejecutable/destino alternos.
Evidencia de esta sesión: /tmp/ro-soft-fit-final/review-1440x900.json y
/tmp/ro-soft-fit-minimum/review-1280x820.json.

La guardia compilada rechaza clases inexistentes y bordes sin color.
El detector nuevo falla con fixtures de contenido cortado, panel solapado,
contenido escapado, scroll anidado y lista desanclada. La conformance sigue
fallando ante desviaciones sin justificar (modo RO_DESIGN_CONFORMANCE_MUTATION=1).

[Auditoría de color](color-usage.md), [sistema congelado](../design-system.md),
[checklist manual antes del merge](premerge-checklist.md). Pendiente tauri:dev
con clientes reales, fuentes/aspecto en WebKit y Hyprland; incluye texto fantasma.
No se cambió src-tauri, stores, lógica de dominio, IPC, modos, textos ni la ventana.
