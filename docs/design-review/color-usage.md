# Auditoría de accent y warn — grafito suave / fit

Base aa02414. Referencias literales text/bg/border/ring/outline/accent/from/via/to,
incluidas variantes/alfa; excluye on-accent, tests y declaraciones CSS.
Los estilos comunes y notice-warn se contabilizan aparte de sus consumidores.

| Archivo en src/ | Accent antes→después | Warn antes→después | Motivo |
| --- | --- | --- | --- |
| app/AppHeader.tsx | 1→1 | 0→0 | RO, excepción de marca |
| app/MaintenanceNotice.tsx | 0→0 | 2→2 | Aviso tonal y punto |
| app/StartupNotice.tsx | 0→0 | 2→2 | Aviso tonal y punto |
| app/ToolViewTabs.tsx | 0→0 | 0→0 | Segmentado neutro compartido |
| features/autopot/AutopotPanel.tsx | 1→1 | 0→0 | Foco Encontrar |
| features/logs/LogPanels.tsx | 2→0 | 0→0 | Juego/Tools pasa al segmentado neutro; foco CSS común |
| features/logs/logs.logic.ts | 0→0 | 1→1 | Clasificación existente, sin cambios |
| features/launcher/LaunchButton.tsx | 1→1 | 0→0 | Acción principal |
| features/servers/ServerConfigModal.tsx | 1→1 | 0→0 | Foco de ruta |
| features/servers/ServerList.tsx | 1→1 | 0→0 | Foco Reintentar; radio encapsulado |
| features/settings/AdvancedSettings.tsx | 1→1 | 0→0 | Foco disclosure |
| features/spammer/GearSwitchEditor.tsx | 2→2 | 0→0 | Teclas elegidas |
| features/spammer/SharpShootingEditor.tsx | 1→1 | 0→0 | Tecla elegida |
| features/spammer/SpammerKeyboard.tsx | 1→1 | 0→0 | Teclas elegidas |
| features/updater/UpdateBanner.tsx | 0→0 | 1→1 | Estado |
| **Total app/features** | **12→10** | **6→6** | Otros archivos 0/0 |

| Primitivo en src/shared/ui/ | Accent | Warn |
| --- | --- | --- |
| Button.tsx | 3 | 2 |
| Checkbox.tsx | 3 | 2 |
| DarkSelect.tsx | 1 | 1 |
| StatusDot.tsx | 0 | 1 |
| CollapsibleNotice / segmentedControl / Input / Panel / ModalShell / DataText | 0 | 0 |
| **Total** | **7** | **6** |

Cinco usos literales de notice-warn en features (AutoBuff, AutoPot, escáner,
servidor y updater), más tres consumidores de CollapsibleNotice (dos en
Herramientas y uno en Runner). El primitivo contiene una referencia notice-warn.
Las ocho ubicaciones de aviso se conservan: centralizar no elimina avisos.

Accent sigue en primarios, selección, foco por teclado y RO; ningún acento
nuevo en títulos o etiquetas. Los títulos de panel siempre son ink, incluidos
los idle; solo contenido/controles se atenúan a muted. Warn sigue marcando avisos
y estados pendientes. No cambian productores, readiness ni clasificación.

Paleta, contrastes, foco 2px sólido y excepción decorativa line-soft/line/track
sin cambios. En entorno listo, Abrir/Config habilitados dan 13.621:1 (ink/raised):
no requieren corrección visual. Aviso muted/warn9% 4.824:1; outline/panel y
raised 3.587/3.339:1. Sin opacidad, sombras ni efectos nuevos.

[Revisión y encaje](README.md), [conformidad](conformance.md),
[pendientes manuales](premerge-checklist.md).
