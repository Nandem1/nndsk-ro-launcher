# Auditoría de accent y warn

Referencias literales de utilidades `text/bg/border/ring/outline/accent/from/via/to`
con el rol base, incluidas variantes hover/focus y opacidades; no contar on-accent,
declaraciones de tokens, tests ni clases expandidas por un primitivo. Comparación
con `9960370`. Los números no son un límite artificial de selecciones múltiples.

| Archivo en src/ | Accent antes→después | Warn antes→después | Por qué queda |
| --- | --- | --- | --- |
| app/AppHeader.tsx | 1→0 | 0→0 | Wordmark neutro |
| app/LoadingScreen.tsx | 1→0 | 0→0 | Indicador de carga neutro |
| app/MaintenanceNotice.tsx | 0→0 | 6→1 | Solo titular corto; detalle muted |
| app/StartupNotice.tsx | 0→0 | 4→1 | Solo titular corto; detalle muted |
| app/ToolViewTabs.tsx | 2→2 | 0→0 | Subrayado de selección y foco |
| features/autobuff/AutobuffPanel.tsx | 0→0 | 1→1 | Línea de estado de acceso a memoria |
| features/autobuff/AutobuffRulesEditor.tsx | 3→3 | 0→0 | Foco de los tres campos; Manual usa Button primary |
| features/autopot/AutopotPanel.tsx | 5→3 | 2→1 | Foco de Encontrar/porcentajes; estado de acceso, no detalle |
| features/autopot/MemoryScannerModal.tsx | 0→0 | 1→1 | Línea breve de progreso/ocupado |
| features/launcher/ActiveClients.tsx | 0→0 | 1→0 | Detalle de varios clientes muted |
| features/launcher/LaunchButton.tsx | 1→0 | 0→0 | Progreso neutro; Jugar usa Button primary |
| features/launcher/LaunchFieldsModal.tsx | 0→0 | 1→0 | Explicación de privacidad muted |
| features/logs/LogPanels.tsx | 2→2 | 0→0 | Subrayado de selección y foco |
| features/logs/logs.logic.ts | 0→0 | 1→1 | Clasificación existente de severidad; sin cambios de lógica |
| features/servers/ServerConfigModal.tsx | 4→2 | 1→1 | Foco del selector de ruta y textarea; aviso corto de estrategia |
| features/servers/ServerList.tsx | 5→2 | 0→0 | Barra de selección y foco de Reintentar; nombre ink |
| features/servers/ServerToolsPanel.tsx | 0→0 | 3→1 | Línea Cliente ante advertencias; anti-cheat/Gepard muted |
| features/servers/ToolRow.tsx | 2→1 | 1→0 | Foco de acción; detalle muted |
| features/settings/AdvancedSettings.tsx | 0→0 | 1→0 | Filas neutras; gravedad vía tone del Panel/StatusDot |
| features/settings/RunnerSelector.tsx | 0→0 | 3→1 | Línea Runner efectivo; detalle y selección legacy muted |
| features/spammer/GearSwitchEditor.tsx | 7→4 | 0→0 | Relleno/borde de las dos categorías de teclas elegidas |
| features/spammer/SharpShootingEditor.tsx | 0→2 | 0→0 | Relleno/borde de tecla elegida; selección uniforme |
| features/spammer/SpammerDelayControl.tsx | 1→0 | 0→0 | Slider neutro |
| features/spammer/SpammerKeyboard.tsx | 2→2 | 0→0 | Relleno/borde de tecla elegida, no texto accent |
| features/updater/UpdateBanner.tsx | 0→0 | 2→1 | Titular de estado; detalle muted |
| features/updater/UpdatePanel.tsx | 0→0 | 1→1 | Línea de error/atención |
| **Total app/features** | **36→23** | **29→11** | Los demás archivos son 0/0 |

Primitivos vigentes, contados aparte (no son color suelto de una feature):

| Archivo en src/shared/ui/ | Accent | Warn | Uso |
| --- | --- | --- | --- |
| Button.tsx | 5 | 4 | Primary/solid, focos y variantes de tono |
| Checkbox.tsx | 3 | 2 | Selección/foco y tono explícito |
| DarkSelect.tsx | 2 | 1 | Selección del menú/foco y tono explícito; keycap neutral |
| Input.tsx | 2 | 0 | Foco de las dos variantes |
| Panel.tsx | 0 | 2 | Borde superior de tono warn y alias warning |
| StatusDot.tsx | 0 | 1 | Indicador que acompaña el texto de estado |
| **Total primitivos** | **12** | **10** | Los demás son 0/0 |

Los callbacks, datos y condiciones de estado se mantienen. Las referencias de
tono (`tone="warn"`, alias warning) no se suman otra vez: se resuelven en los
mapas de primitivos. Las teclas seleccionadas comparten el indicador accent;
sus nombres, triggers y estados no añaden una segunda jerarquía de accent.
