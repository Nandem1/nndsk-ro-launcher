# Revisión de unslop-soft

Base `cf702f8`, desde unslop-graphite. Build de producción, Chromium local,
CSP real e IPC simulado. Referencia: [refined.html](../design-reference/refined.html),
abierta a 1440×900 y leída completa, sin modificar el archivo aportado.

**Estado: implementación preparada; cierre bloqueado por contraste de contornos.**
El boceto pide límites que no alcanzan 3:1. No se ha rebajado ese requisito
ni añadido contornos activos fuera del boceto sin autorización.
Las comprobaciones de render/layout pasan; review:design devuelve 1 por
contrastFailures (switches, campos y radios), no por consola o recortes.

## Comparación

[Referencia y aplicación lado a lado](reference-vs-app-1440x900.png):
izquierda boceto, derecha preparación con runtime pendiente.
Cada mitad es 1440×900; el PNG combinado mide 2880×900.
[Referencia sola](reference-1440x900.png). Las fuentes de esa captura se sirven
desde los mismos paquetes locales de Plex, no desde el CDN del HTML.

Diferencias que quedan:

- Contenido real: cinco nombres largos de servidores, siete diagnósticos, avisos
  completos con hash/FileVersion, detalles de runner y acciones de dgVoodoo.
- Estados reales: Abrir/Config no disponibles antes de preparar; conservan
  disabled y su contorno accesible existente. La referencia ilustra botones activos.
- AutoPot contiene Encontrar, selección de perfil y controles HP/SP completos;
  Spammer conserva Shift/Gear Switch y distribución ergonómica de teclas.
- Se conservan el scroll único del rail, los scrolls de herramientas y las
  acciones inferiores fijas. El boceto simplifica esas cantidades de contenido.
- Se conserva el padding superior existente de main (12px), el tamaño flexible
  de cuerpos y Logs de 176/196px; el boceto ilustra Logs de 130px y otra cantidad
  de contenido. No se copió su layout ni se ocultaron secciones.
- Idle sigue muted sin opacidad. Los controles disabled conservan un límite
  muted, necesario para el contraste ya validado. No cambia su disponibilidad.
- El hover danger usa ink sobre el tono al 15% para alcanzar contraste;
  los tonos se mezclan contra panel, evitando variaciones por el contenedor.

## Capturas

| Escena | 1440×900 | 1280×820 |
| --- | --- | --- |
| Preparación | [prep](prep-1440x900.png) | [prep](prep-1280x820.png) |
| En juego | [ingame](ingame-1440x900.png) | [ingame](ingame-1280x820.png) |
| Servidor | [server](server-1440x900.png) | [server](server-1280x820.png) |
| Escáner | [scanner](scanner-1440x900.png) | [scanner](scanner-1280x820.png) |
| Herramientas activas | [active](active-1440x900.png) | [active](active-1280x820.png) |
| Editores abiertos | [editors](active-editors-1440x900.png) | [editors](active-editors-1280x820.png) |
| Buffs | [buffs](buffs-1440x900.png) | [buffs](buffs-1280x820.png) |
| Preparación realista | [realistic](prep-realistic-1440x900.png) | [realistic](prep-realistic-1280x820.png) |
| Realista, scroll inferior | [scrolled](prep-realistic-scrolled-1440x900.png) | [scrolled](prep-realistic-scrolled-1280x820.png) |
| Realista, grupo abierto | [open](prep-realistic-open-1440x900.png) | [open](prep-realistic-open-1280x820.png) |
| Realista, abierto/scroll | [open-scrolled](prep-realistic-open-scrolled-1440x900.png) | [open-scrolled](prep-realistic-open-scrolled-1280x820.png) |
| Runtime pendiente | [pending](prep-pending-1440x900.png) | [pending](prep-pending-1280x820.png) |
| Pendiente, diagnósticos visibles | [pending-scrolled](prep-pending-scrolled-1440x900.png) | [pending-scrolled](prep-pending-scrolled-1280x820.png) |
| Sin servidor | [empty](prep-empty-1440x900.png) | [empty](prep-empty-1280x820.png) |

28 capturas actualizadas y dos nuevas de referencia/comparación.
[Informe 1440×900](review-1440x900.json), [1280×820](review-1280x820.json):
14 escenas por tamaño, cero errores de consola, overflow, recortes permanentes
y cajas anidadas. El contenido desplazado conserva acceso completo mediante sus
scrolls previstos. Fotos/informe 1100×800 quedan como historia del polish.

Fuentes: siete caras locales de Plex, todas loaded/HTTP 200; Barlow no se carga.
Foco por teclado de 2px accent/50%, sin sombra; teclas/secundarios sin borde,
campos line→line-strong al hover. Disclosure abierto/cerrado, 13 acciones y
disponibilidad intactas. Selector por puntero/teclado y reposicionado al resize;
scroll único del rail, acciones fijas y reduced-motion verificados.
Runtime pendiente: cuatro puntos warn y tres ok; error de runner permanece bad,
mismo texto/hint y acciones. El borde de panel permanece line-soft/1px en ambos.

## Contraste compuesto

WCAG sRGB, alfa compuesto sobre el fondo real; sin redondeos intermedios.

| Par | Ratio |
| --- | --- |
| muted / surface / panel / panel-raised | 5.906 / 5.576 / 5.190:1 |
| ink / surface / panel / panel-raised | 15.498 / 14.633 / 13.621:1 |
| Mínimo de colores de estado / esas superficies (bad/raised) | 4.769:1 |
| on-accent / accent | 6.126:1 |
| muted / aviso warn9% sobre panel | 4.824:1 |
| Idle, mínimo medido (incluido control raised) | 5.190:1 |
| Disabled, texto / field | 6.066:1 |
| Disabled, límite exterior / panel | 5.576:1 |
| Perilla muted / track | 4.303:1 |
| Switch encendido, superficie / ok | 7.594:1 |
| Thumb ink / track | 11.291:1 |
| Tonal bad / 10% / hover ink al 15% | 4.595 / 12.280:1 |
| Tonal ok / 10% / hover 15% | 6.157 / 5.616:1 |
| Tonal info / 10% / hover 15% | 5.342 / 4.925:1 |
| Tonal warn / 10% / hover 15% | 6.979 / 6.313:1 |

Texto informativo y botones tonales pasan ≥4.5:1; perillas y pulgares pasan ≥3:1.
Idle/disabled mantienen opacidad efectiva 1.

Los bordes de referencia **no pasan 3:1**:

| Borde/pista | Surface | Panel | Panel-raised |
| --- | --- | --- | --- |
| line (campo) | 1.299 | 1.226 | 1.142 |
| outline (radio/checkbox) | 2.294 | 2.166 | 2.016 |
| track (switch apagado) | 1.373 | 1.296 | 1.206 |
| accent50% (foco) | 2.344 | 2.326 | 2.276 |
| line-soft (decorativo, no control) | 1.188 | 1.122 | 1.044 |

Este es el bloqueo real para el cierre: mantener exactamente el boceto/colores
y pedir 3:1 para todos esos límites son condiciones incompatibles.
La guardia estructural y de texto pasa, pero el arnés de contornos devuelve 1.
Se corrigió una falsa medición anterior: outline-width puede ser distinto de
cero con outline-style:none; ahora solo cuenta un contorno realmente dibujado.
CSS color(srgb) de los rellenos tonales se convierte correctamente a canales
0–255 antes de medir. Los JSON conservan todos los fallos, no los ocultan.

## Gramática y auditoría

Líneas interiores border-t/border-b, sin contar panel exterior/controles;
ambos tamaños dan los mismos conteos respecto a graphite:

| Escenas | Antes→después |
| --- | --- |
| prep, server, todas realistas/pending | 10→6 |
| ingame, scanner, active, active-editors | 7→4 |
| buffs | 3→3 |
| prep-empty | 9→6 |

Las cinco filas largas de servidor miden 40px en la fixture; nombres más largos
pueden crecer para conservar su texto. Cero bordes superiores de tono,
barras laterales de aviso/selección, etiquetas uppercase/tracking o radios 0.
[Acento y advertencias por archivo](color-usage.md).

Pasaron lint, format:check, 201 tests/38 archivos, tsc/build, check:design y
Rust fmt/clippy/tests/build. Siete pruebas Rust ignoradas por prerrequisitos
existentes. Fixtures de paleta, marca retirada, etiqueta uppercase, borde de
tono y radio arbitrario devuelven 1; segmentado válido devuelve 0.
No cambian src-tauri, stores, lógica de dominio, IPC, textos ni roles.

Repetir con el build de producción:

```sh
npm run build
npm run preview -- --host 127.0.0.1 --port 5175
npm run review:design -- 5175 1440 900
npm run review:design -- 5175 1280 820
```

RO_DESIGN_CHROMIUM y RO_DESIGN_REVIEW_OUTPUT permiten otro ejecutable/destino.
Volcados detallados quedan en /tmp. Las ejecuciones actuales devuelven 1 por
los límites de control arriba documentados.

Pendiente manual: tauri:dev con clientes reales; apariencia en WebKit con
Hyprland y comprobación del texto fantasma. Chromium/IPC simulado no sustituye
esas pruebas. No se tocó Hyprland ni se añadieron parches de compositing.
