# Revisión de unslop-soft

Base `cf702f8`, desde unslop-graphite. Build de producción, Chromium local,
CSP real e IPC simulado. Referencia: [refined.html](../design-reference/refined.html),
abierta a 1440×900 y leída completa, sin modificar el archivo aportado.

**Estado: fidelidad computada y contraste cerrados con las decisiones aprobadas.**
Corrección sobre ee3521c, sin reescribir historia. [Tabla de conformidad](conformance.md):
colores ±1 canal, radios/tamaños ±1px, familia/peso exactos. El arnés falla si una
desviación no tiene justificación específica; incluye un modo de mutación negativa.
line-soft/line/track apagado se reportan como decorativos; outline y foco sí
cumplen 3:1. Render/layout e interacción pasan en ambos tamaños.

Causas verificadas antes de corregir:

- Tokens registrados y reglas emitidas: panel line-soft/1px/radio14;
  segmentado line-soft/1px/radio11 y texto12.5/500; runner Mono12.5/line/radio9.
  No se reprodujeron sus supuestos fallbacks claros, esquinas rectas o anillo
  rojo inicial en el build de ee3521c. No se atribuyen a un token inexistente
  sin evidencia; la nueva guardia previene ese caso.
- idle-control:disabled añadía outline muted/1px y border/field a todos los
  controles. Producía los bordes claros de Abrir/Config y ocultaba la pista
  original de switches deshabilitados. Retirado; surfaces y radios se conservan.
- Dos utilidades de radio coexistían en Button lg: rounded-control ganaba
  por orden del CSS, no por orden del className. Cada tamaño define ahora
  exactamente un radio; secundario 9, primario grande 11.
- Interlineado heredado inflaba campos/segmentos/botones y microetiquetas;
  Importar usaba xs en lugar de la métrica secundaria del boceto. Se miden y
  aplican interlineados normales, 13px/500/8×14 y teclas 11.5px/28px.
- Chromium considera focus-visible algunos campos clicados. Un marcador
  presentacional de modalidad evita el anillo de ratón/autofocus, sin mover
  foco ni interceptar eventos. Teclado: anillo sólido accent/2px.

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
  disabled, fondo raised sin borde y texto muted. La referencia ilustra activos.
- AutoPot contiene Encontrar, selección de perfil y controles HP/SP completos;
  Spammer conserva Shift/Gear Switch y distribución ergonómica de teclas.
- Se conservan el scroll único del rail, los scrolls de herramientas y las
  acciones inferiores fijas. El boceto simplifica esas cantidades de contenido.
- Se conserva el padding superior existente de main (12px), el tamaño flexible
  de cuerpos y Logs de 176/196px; el boceto ilustra Logs de 130px y otra cantidad
  de contenido. No se copió su layout ni se ocultaron secciones.
- Idle sigue muted sin opacidad. Disabled no añade límites visibles ni anillos
  permanentes. No cambia su disponibilidad.
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
Foco por teclado de 2px accent sólido, sin sombra; teclas/secundarios sin borde,
campos line→line-strong al hover. Disclosure abierto/cerrado, 13 acciones y
disponibilidad intactas. Selector por puntero/teclado y reposicionado al resize;
scroll único del rail, acciones fijas y reduced-motion verificados. Sin anillos
al cargar ni al clicar botón/campo/tecla/radio; hover de fila raised y foco en
su radio funcional. Switch y radio medidos en ambos estados.
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
| Outline / panel / panel-raised | 3.587 / 3.339:1 |
| Foco accent sólido / panel / panel-raised | 5.899 / 5.491:1 |
| Perilla muted / track | 4.303:1 |
| Switch encendido, superficie / ok | 7.594:1 |
| Thumb ink / track | 11.291:1 |
| Tonal bad / 10% / hover ink al 15% | 4.595 / 12.280:1 |
| Tonal ok / 10% / hover 15% | 6.157 / 5.616:1 |
| Tonal info / 10% / hover 15% | 5.342 / 4.925:1 |
| Tonal warn / 10% / hover 15% | 6.979 / 6.313:1 |

Texto informativo y botones tonales pasan ≥4.5:1; perillas y pulgares pasan ≥3:1.
Idle/disabled mantienen opacidad efectiva 1.

Los pares siguientes se reportan como **decorativos**, por decisión aprobada:

| Borde/pista | Surface | Panel | Panel-raised |
| --- | --- | --- | --- |
| line (campo) | 1.299 | 1.226 | 1.142 |
| track (switch apagado) | 1.373 | 1.296 | 1.206 |
| line-soft (decorativo, no control) | 1.188 | 1.122 | 1.044 |

El bloqueo anterior queda resuelto: outline ahora es #6B6F76 y foco es accent
sólido. La guardia y el arnés devuelven 0 con las excepciones decorativas aprobadas.
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
Volcados detallados quedan en /tmp. Las ejecuciones actuales devuelven 0,
reportando los pares decorativos sin fallos fuera de esas excepciones.

```sh
npm run check:design:fixtures
RO_DESIGN_CONFORMANCE_MUTATION=1 RO_DESIGN_REVIEW_OUTPUT=/tmp/ro-soft-invalid npm run review:design -- 5175 1440 900
```

La segunda orden es una prueba negativa: altera solo el borde del panel en el
DOM de prueba y debe devolver 1 por desviación de color sin justificar. No
modifica fuentes, archivo de referencia ni capturas de revisión del repo.

Pendiente manual: tauri:dev con clientes reales; apariencia en WebKit con
Hyprland y comprobación del texto fantasma. Chromium/IPC simulado no sustituye
esas pruebas. No se tocó Hyprland ni se añadieron parches de compositing.
