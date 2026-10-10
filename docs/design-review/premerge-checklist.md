# Checklist manual antes del merge

La compuerta local de Chromium/IPC simulado pasa, pero no sustituye estas pruebas
del escritorio y de los clientes reales. Registrar resultado y cliente/runtime.

- [ ] npm run tauri:dev con clientes reales: preparación y en juego; Buffs con
  tus reglas reales, catálogo abierto/cerrado, todas las filas alcanzables.
- [ ] Fuentes locales IBM Plex Sans/Mono cargadas en WebKit (sin fallback ni CDN).
- [ ] Entorno listo: Abrir de OpenSetup/Patcher y Config de dgVoodoo habilitados;
  comprobar también su estado deshabilitado antes de preparar.
- [ ] Select de runner con elipsis y tooltip completo, sin modificar selección.
- [ ] Avisos de Herramientas/Runner efectivo colapsados y expandidos; rutas largas
  legibles y Avanzado alcanzable con el único scroll del rail.
- [ ] Selector de tecla abierto en Buffs: menú junto al disparador, dentro del
  viewport; teclado, Escape y selección conservan el comportamiento.
- [ ] Aspecto de la ventana fija 1440×900 sobre Hyprland en pantalla completa;
  no cambiar decorations/resizable durante la revisión.
- [ ] Comprobar el texto fantasma en WebKit; sin evidencia no aplicar parches
  de compositing.
