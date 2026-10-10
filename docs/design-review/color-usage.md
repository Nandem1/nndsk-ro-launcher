# Auditoría de accent y warn

Referencias literales de utilidades text/bg/border/ring/outline/accent/from/via/to
con el rol base, incluidas variantes y alfa. Excluye on-accent, declaraciones,
tests y expansión de primitivos. Base: `f3bcbad` (`rework/unslop-refine`).

| Archivo en src/ | Accent antes→después | Warn antes→después | Por qué queda |
| --- | --- | --- | --- |
| app/AppHeader.tsx | 1→1 | 0→0 | RO en accent: única excepción de marca; el resto ink |
| app/MaintenanceNotice.tsx | 0→0 | 1→1 | Barra del contenedor; titular y detalles muted |
| app/StartupNotice.tsx | 0→0 | 1→1 | Barra del contenedor; titular y detalles muted |
| app/ToolViewTabs.tsx | 2→2 | 0→0 | Subrayado de selección y foco |
| features/autobuff/AutobuffPanel.tsx | 0→0 | 1→1 | Barra del aviso de memoria, no etiqueta auxiliar |
| features/autobuff/AutobuffRulesEditor.tsx | 3→0 | 0→0 | Foco centralizado de tres campos; Manual usa Button primary |
| features/autopot/AutopotPanel.tsx | 3→1 | 1→1 | Foco de Encontrar; porcentajes usan foco común; barra del aviso de acceso |
| features/autopot/MemoryScannerModal.tsx | 0→0 | 1→1 | Barra del estado ocupado/progreso; texto muted |
| features/logs/LogPanels.tsx | 2→2 | 0→0 | Subrayado de selección y foco |
| features/logs/logs.logic.ts | 0→0 | 1→1 | Clasificación de severidad existente; lógica intacta |
| features/servers/ServerConfigModal.tsx | 2→1 | 1→1 | Foco de ruta; argumentos usan foco común; barra del aviso de estrategia |
| features/servers/ServerList.tsx | 2→2 | 0→0 | Barra de selección y foco de Reintentar |
| features/servers/ServerToolsPanel.tsx | 0→0 | 2→2 | Barras de diagnósticos e incidencias dgVoodoo; Cliente siempre muted |
| features/servers/ToolRow.tsx | 1→1 | 0→0 | Foco de acción |
| features/settings/AdvancedSettings.tsx | 1→1 | 0→0 | Foco del disclosure; estado mediante Panel/StatusDot |
| features/settings/RunnerSelector.tsx | 0→0 | 1→1 | Barra del aviso de runner efectivo; texto muted y datos Mono |
| features/spammer/GearSwitchEditor.tsx | 4→4 | 0→0 | Dos categorías de teclas elegidas, relleno/borde |
| features/spammer/SharpShootingEditor.tsx | 2→2 | 0→0 | Relleno/borde de tecla elegida |
| features/spammer/SpammerKeyboard.tsx | 2→2 | 0→0 | Relleno/borde de tecla elegida |
| features/updater/UpdateBanner.tsx | 0→0 | 1→1 | Barra de estado del contenedor; texto muted |
| features/updater/UpdatePanel.tsx | 0→0 | 1→1 | Barra del aviso de clientes activos |
| **Total app/features** | **25→19** | **12→12** | Los demás archivos son 0/0 |

Las seis referencias accent retiradas eran focos de campos, ahora cubiertos por
la regla común de 1px en index.css. No se pierde foco de teclado. RO conserva la
excepción de marca. No se colorea una segunda etiqueta auxiliar.
Warn marca el contenedor del aviso con barra izquierda
de 3px y texto muted ≥4.5:1; Cliente nunca se colorea por sus advertencias.
Panel y StatusDot siguen resolviendo sus tonos semánticos sin duplicar estos conteos.

| Primitivo en src/shared/ui/ | Accent | Warn | Uso |
| --- | --- | --- | --- |
| Button.tsx | 5 | 4 | Variantes, selección y foco |
| Checkbox.tsx | 3 | 2 | Variantes, selección y foco |
| DarkSelect.tsx | 1 | 1 | Selección; foco común de 1px |
| Input.tsx | 0 | 0 | Foco común de 1px |
| Panel.tsx | 0 | 2 | Variantes, selección y foco |
| StatusDot.tsx | 0 | 1 | Variantes, selección y foco |
| **Total primitivos** | **9** | **10** | DataText y utilidades tipográficas: 0/0 |

Las teclas seleccionadas comparten el indicador accent. Disabled/idle no usan
opacidad ni añaden accent: texto/borde superior muted, controles field/line con
límite exterior muted. Los callbacks, datos, IPC y condiciones siguen intactos.

Avanzado consume `checks[].severity` únicamente para presentación:
pending/warning→warn, error→bad, listo→ok, conteos informativos→neutral.
El borde toma bad > warn > ok > neutral. La fixture de runtime pendiente tiene
cuatro filas warn (runner, compatibilidad, entorno, DXVK) y tres ok (audio,
permisos, uinput); una variante con error explícito de runner usa bad para fila
y panel, aunque conserva el mismo hint. Los flags readyToLaunch/canSetup no cambian.

Se retiraron **9 colocaciones de iconos decorativos**: títulos AutoPot/Spammer/
AutoBuff (3), Combate/Buffs (2), Sharp Shooting/Gear Switch (2), etiquetas ATK/DEF
(2). Se conservan ChevronDown, controles con nombre y StatusDot. En las escenas
de combate, los seis adornos de título/pestañas/disclosure pasan de 6→0; en Buffs,
3→0. La barra de selección de servidor y las barras de avisos permanecen en 3px.
