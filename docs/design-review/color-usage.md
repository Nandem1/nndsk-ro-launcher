# Auditoría de accent y warn

Referencias literales de utilidades text/bg/border/ring/outline/accent/from/via/to
con el rol base, incluidas variantes y alfa. Excluye on-accent, declaraciones,
tests y expansión de primitivos. Base: `e08b936` (`rework/unslop-polish`).

| Archivo en src/ | Accent antes→después | Warn antes→después | Por qué queda |
| --- | --- | --- | --- |
| app/AppHeader.tsx | 0→1 | 0→0 | RO en accent: única excepción de marca; el resto ink |
| app/MaintenanceNotice.tsx | 0→0 | 1→1 | Barra del contenedor; titular y detalles muted |
| app/StartupNotice.tsx | 0→0 | 1→1 | Barra del contenedor; titular y detalles muted |
| app/ToolViewTabs.tsx | 2→2 | 0→0 | Subrayado de selección y foco |
| features/autobuff/AutobuffPanel.tsx | 0→0 | 1→1 | Barra del aviso de memoria, no etiqueta auxiliar |
| features/autobuff/AutobuffRulesEditor.tsx | 3→3 | 0→0 | Foco de tres campos; Manual usa Button primary |
| features/autopot/AutopotPanel.tsx | 3→3 | 1→1 | Focos de Encontrar/porcentajes; barra del aviso de acceso |
| features/autopot/MemoryScannerModal.tsx | 0→0 | 1→1 | Barra del estado ocupado/progreso; texto muted |
| features/logs/LogPanels.tsx | 2→2 | 0→0 | Subrayado de selección y foco |
| features/logs/logs.logic.ts | 0→0 | 1→1 | Clasificación de severidad existente; lógica intacta |
| features/servers/ServerConfigModal.tsx | 2→2 | 1→1 | Foco de ruta/argumentos; barra del aviso de estrategia |
| features/servers/ServerList.tsx | 2→2 | 0→0 | Barra de selección y foco de Reintentar |
| features/servers/ServerToolsPanel.tsx | 0→0 | 1→2 | Barras de diagnósticos e incidencias dgVoodoo; Cliente siempre muted |
| features/servers/ToolRow.tsx | 1→1 | 0→0 | Foco de acción |
| features/settings/AdvancedSettings.tsx | 0→1 | 0→0 | Foco del disclosure; estado mediante Panel/StatusDot |
| features/settings/RunnerSelector.tsx | 0→0 | 1→1 | Barra del aviso de runner efectivo; texto muted y datos Mono |
| features/spammer/GearSwitchEditor.tsx | 4→4 | 0→0 | Dos categorías de teclas elegidas, relleno/borde |
| features/spammer/SharpShootingEditor.tsx | 2→2 | 0→0 | Relleno/borde de tecla elegida |
| features/spammer/SpammerKeyboard.tsx | 2→2 | 0→0 | Relleno/borde de tecla elegida |
| features/updater/UpdateBanner.tsx | 0→0 | 1→1 | Barra de estado del contenedor; texto muted |
| features/updater/UpdatePanel.tsx | 0→0 | 1→1 | Barra del aviso de clientes activos |
| **Total app/features** | **23→25** | **11→12** | Los demás archivos son 0/0 |

El incremento de accent es RO y el foco del disclosure. No se colorea una
segunda etiqueta auxiliar. Warn marca el contenedor del aviso con barra izquierda
de 3px y texto muted ≥4.5:1; Cliente nunca se colorea por sus advertencias.
Panel y StatusDot siguen resolviendo sus tonos semánticos sin duplicar estos conteos.

| Primitivo en src/shared/ui/ | Accent | Warn | Uso |
| --- | --- | --- | --- |
| Button.tsx | 5 | 4 | Variantes, selección y foco |
| Checkbox.tsx | 3 | 2 | Variantes, selección y foco |
| DarkSelect.tsx | 2 | 1 | Variantes, selección y foco |
| Input.tsx | 3 | 0 | Variantes, selección y foco |
| Panel.tsx | 0 | 2 | Variantes, selección y foco |
| StatusDot.tsx | 0 | 1 | Variantes, selección y foco |
| **Total primitivos** | **13** | **10** | DataText y utilidades tipográficas: 0/0 |

Las teclas seleccionadas comparten el indicador accent. Disabled/idle no usan
opacidad ni añaden accent: texto/borde superior muted, controles field/line con
límite exterior muted. Los callbacks, datos, IPC y condiciones siguen intactos.
