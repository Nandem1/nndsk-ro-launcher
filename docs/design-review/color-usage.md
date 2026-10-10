# Auditoría de accent y warn — grafito suave

Base `cf702f8` (unslop-graphite). Referencias literales de utilidades
text/bg/border/ring/outline/accent/from/via/to, incluidas variantes y alfa;
excluye on-accent, tests, declaraciones CSS y expansión de primitivos.
Los patrones semánticos `notice-warn` se cuentan aparte: centralizar no elimina
advertencias. Lógica de logs y estados intacta.

| Archivo en src/ | Accent antes→después | Warn antes→después | Justificación |
| --- | --- | --- | --- |
| app/AppHeader.tsx | 1→1 | 0→0 | RO, excepción de marca |
| app/MaintenanceNotice.tsx | 0→0 | 1→2 | Fondo warn/9% y punto redondo |
| app/StartupNotice.tsx | 0→0 | 1→2 | Fondo warn/9% y punto redondo |
| app/ToolViewTabs.tsx | 2→0 | 0→0 | Segmentado neutro; foco común |
| features/autobuff/AutobuffPanel.tsx | 0→0 | 1→0 | notice-warn: 1 |
| features/autopot/AutopotPanel.tsx | 1→1 | 1→0 | Foco Encontrar; notice-warn: 1 |
| features/autopot/MemoryScannerModal.tsx | 0→0 | 1→0 | notice-warn: 1; rol/texto conservados |
| features/logs/LogPanels.tsx | 2→2 | 0→0 | Selección/foco de logs existentes |
| features/logs/logs.logic.ts | 0→0 | 1→1 | Clasificación existente, archivo sin cambios |
| features/launcher/LaunchButton.tsx | 0→1 | 0→0 | Preparar entorno sólido accent como referencia; disponibilidad intacta |
| features/servers/ServerConfigModal.tsx | 1→1 | 1→0 | Foco ruta; notice-warn: 1 |
| features/servers/ServerList.tsx | 2→1 | 0→0 | Foco Reintentar; radio accent encapsulado; selección tonal sin barra |
| features/servers/ServerToolsPanel.tsx | 0→0 | 2→0 | notice-warn: 2, Cliente muted |
| features/servers/ToolRow.tsx | 1→0 | 0→0 | Acción usa Button/variantes |
| features/settings/AdvancedSettings.tsx | 1→1 | 0→0 | Foco disclosure; estados por puntos, no borde |
| features/settings/RunnerSelector.tsx | 0→0 | 1→0 | notice-warn: 1 |
| features/spammer/GearSwitchEditor.tsx | 4→2 | 0→0 | Solo relleno de teclas elegidas |
| features/spammer/SharpShootingEditor.tsx | 2→1 | 0→0 | Solo relleno de tecla elegida |
| features/spammer/SpammerKeyboard.tsx | 2→1 | 0→0 | Solo relleno de teclas elegidas |
| features/updater/UpdateBanner.tsx | 0→0 | 1→1 | Estado tonal, sin barra |
| features/updater/UpdatePanel.tsx | 0→0 | 1→0 | notice-warn: 1 |
| **Total app/features** | **19→12** | **12→6** | Otros archivos 0/0; 8 usos notice-warn aparte |

| Primitivo en src/shared/ui/ | Accent | Warn |
| --- | --- | --- |
| Button.tsx | 3 | 2 |
| Checkbox.tsx | 3 | 2 |
| DarkSelect.tsx | 1 | 1 |
| StatusDot.tsx | 0 | 1 |
| Input/Panel/ModalShell | 0 | 0 |
| **Total** | **7** | **6** |

Button encapsula fondos tonales y foco. index.css encapsula puntos/avisos,
radios nativos, foco 2px accent sólido y mezcla de hover. Los colores de estado
se mantienen; outline sube a #6B6F76 por decisión aprobada.
La guardia impide border-t de tono: todos los paneles usan line-soft/1px.
Avanzado mantiene pending/warning→warn, error→bad, listo→ok e informativo→neutral;
no cambian readiness, acciones, productores ni el helper de presentación.

No reaparecen iconos decorativos en títulos/pestañas/disclosures. Estados,
checkboxes y switches son redondos. El mapeo semántico permanece;
la retirada de bordes no significa eliminar avisos o disponibilidad.

[Revisión visual y contraste](README.md), [conformidad computada](conformance.md).

Corrección de fidelidad sobre ee3521c: conteos accent/warn sin cambios. El foco
no es un borde inicial ni de disabled; solo aparece con teclado en focus-visible.
line-soft/line/track apagado son decorativos y se reportan sin umbral 3:1.
Outline/panel 3.587:1 y outline/raised 3.339:1; accent sólido/foco 5.899/5.491:1.
Los puntos y selecciones siguen sujetos a contraste de estado. La pista no se
sustituye por field al deshabilitar un switch, y una tecla elegida conserva su
accent/on-accent; disabled no aplica opacidad ni oculta la selección.
Los puntos neutrales pasan de line-strong a muted: son estado, no un borde
decorativo, y ahora también cumplen ≥3:1. No cambia el mapeo de estados.
