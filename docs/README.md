# Documentación

La documentación se divide por autoridad, no por la fase en que se implementó:

- [`RO_RUNTIME_ARCHITECTURE_PLAN.md`](RO_RUNTIME_ARCHITECTURE_PLAN.md): arquitectura y operación
  vigente del runtime.
- [`PTRACE_SESSION_SUPERVISOR_PLAN.md`](PTRACE_SESSION_SUPERVISOR_PLAN.md): contrato detallado de
  `ro-sessiond`, memoria y lifecycle supervisado.
- [`features/DISCORD_RICH_PRESENCE_PLAN.md`](features/DISCORD_RICH_PRESENCE_PLAN.md): diseño de la
  integración Discord Rich Presence.
- [`adr/`](adr/): decisiones arquitectónicas y su justificación.

Los contratos de implementación de las fases 0–8 se retiraron después de completarse. Para una
investigación histórica deben consultarse mediante Git, no copiarse a documentos vigentes.

## ADR vigentes

| ADR                                                 | Decisión                                   |
| --------------------------------------------------- | ------------------------------------------ |
| [001](adr/ADR-001-runtime-graphics-domain.md)       | Dominio cerrado de runtime y gráficos      |
| [002](adr/ADR-002-runtime-prefix-identity.md)       | Fingerprints, prefix v3 y migración legacy |
| [003](adr/ADR-003-runtime-artifact-catalog.md)      | Catálogo y receipts de artefactos          |
| [004](adr/ADR-004-runtime-compatibility-catalog.md) | Compatibilidad exacta basada en evidencia  |
| [005](adr/ADR-005-runtime-d7vk-deployment.md)       | D7VK `no-go`                               |
| [006](adr/ADR-006-runtime-benchmark-protocol.md)    | Protocolo reproducible de benchmark A/B    |
