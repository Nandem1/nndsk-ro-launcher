# Fase 2 — Contrato de identidad de prefix

Autoridad normativa: [ADR-002](adr/ADR-002-runtime-prefix-identity.md) y [RO_RUNTIME_ARCHITECTURE_PLAN.md](RO_RUNTIME_ARCHITECTURE_PLAN.md) §11.2.

## Alcance operacional

- `PrefixFingerprint` y `resolve_prefix_binding` eligen path v3 o alias v2 para prefixes aislados administrados.
- Manifiesto schema 3 solo en prefixes nuevos o reparación v3 verificada; alias v2 sigue schema 2.
- `SessionAnchorV2` y `plan_id` anclan sesiones bajo `startup_lock` del registry.
- `RO_LAUNCHER_PREFIX_V3=0` deshabilita **write-new** v3; el lookup v3 existente sigue activo.

## Fuera de alcance (fases posteriores)

- Env operacional y spawn (Fase 4).
- Catálogo de artefactos extendido (Fase 3).
- `RuntimeFingerprint` completo con dgVoodoo y recetas: stub/placeholder hasta cerrar facts en orquestación.

## Goldens bloqueados

- Encoder record: `855bf92f11437f37d149d93f865ac9dff6354980f5f546a774ca88d95d03f791`
- Server token `fixture-server`: `d8e0b61318835001`
- Prefix managed Proton: `a23f2a940a9cb8e5110b0e454ad68a35a22af760c005bcd0ebdf47ab44330270`

## Flags

| Variable | Default | Efecto |
| --- | --- | --- |
| `RO_LAUNCHER_PREFIX_V3` | write-new habilitado | `0` → provisión nueva usa path FNV y schema 2 |
| `RO_LAUNCHER_RUNTIME_SHADOW` | sin cambio | Paridad gráfica/env observacional |
