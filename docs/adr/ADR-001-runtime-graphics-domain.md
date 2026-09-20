# ADR-001: dominio cerrado de runtime y gráficos

| Campo                 | Valor                                                                                      |
| --------------------- | ------------------------------------------------------------------------------------------ |
| Estado                | Aceptado e implementado                                                                    |
| Fecha                 | 2026-09-16                                                                                 |
| Decisión              | Separar intención de plan resuelto y modelar sólo topologías gráficas admitidas            |
| Autoridad relacionada | [`AGENTS.md`](../../AGENTS.md), [arquitectura vigente](../RO_RUNTIME_ARCHITECTURE_PLAN.md) |

## Contexto

La implementación legacy elegía runner, prefix, DXVK, dgVoodoo y variables en puntos diferentes. Una
combinación podía representarse mediante strings aunque fuera incoherente, y tools distintos podían
recibir environments diferentes al juego.

El dominio necesitaba hacer explícita la selección efectiva sin reemplazar las abstracciones que ya
eran correctas (`ResolvedRunner`, `PrefixLocation`, provisioning y registry de sesiones).

## Decisión

### Intención y resolución

`RuntimeProfile` es intención efímera: runner solicitado, fuente de selección y perfil gráfico.
`RuntimePlan` es un snapshot resuelto e inmutable para una operación:

```text
RuntimePlan
  runner: RunnerPlan(identity, capabilities, sync)
  graphics: GraphicsPlan(provider, optional overlay)
  prefix: PrefixLocation + PrefixBinding
  webview2_required: bool
```

La fuente de runner es cerrada: override de servidor, setting global o default de producto. Un
override vacío no cuenta. El resolver valida que request, runner observado, provenance y capabilities
sean coherentes; `Unknown` no se convierte en evidencia conocida.

### Topologías gráficas

Los perfiles productivos son únicamente:

```rust
enum GraphicsProfile {
    Dxvk,
    DgVoodooDxvk,
}
```

El provider DXVK es exactamente uno:

- `RunnerOwned` para Proton;
- `ManagedPrefix` para DXVK administrado;
- `WinetricksPrefix` para la receta legacy no fijada.

`DgVoodooDxvk` exige wrappers verificados y conserva un provider DXVK explícito para la etapa D3D11.
No se admite un `Vec<Layer>`, plugin gráfico libre ni dos owners de la misma DLL. Una topología nueva
requiere una variante, evidencia y tests nuevos. ADR-005 mantiene D7VK fuera del enum productivo.

### Targets y environment

La política distingue cinco targets:

| Target                 | DXVK prefix-owned | dgVoodoo |
| ---------------------- | ----------------- | -------- |
| `Game`                 | Sí                | Sí       |
| `LaunchPatcher`        | Sí                | Sí       |
| `MaintenancePatcher`   | Sí                | No       |
| `OpenSetup`            | Sí                | Sí       |
| `GraphicsControlPanel` | Sí                | Sí       |

Todos derivan del mismo plan; coherencia no significa que reciban bytes idénticos. El maintenance
patcher, por ejemplo, no necesita el overlay game-dir.

`GraphicsEnvironment` separa claims DLL de cambios de variables. Un nombre DLL es un basename ASCII
normalizado. Los merges son deterministas:

1. un claim nuevo se inserta;
2. mismo owner + mismo valor es idempotente;
3. owner, load order, contributor o valor distintos producen conflicto;
4. nunca gana el último caller.

`WINEDLLOVERRIDES` se renderiza una sola vez desde el set estructurado. No se concatena con un valor
heredado opaco. La sanitización AppImage y el environment del runner conservan ownership separado.

### Ownership

| Dominio        | Ejemplos                  | Regla                                               |
| -------------- | ------------------------- | --------------------------------------------------- |
| Runner         | DXVK incluido en Proton   | el launcher no lo instala como componente de prefix |
| Prefix         | DXVK managed o winetricks | mutación sólo por provisioning/repair               |
| GameDirOverlay | dgVoodoo                  | manifest y restore propios                          |
| HostExternal   | Wine/Proton externos      | observar, nunca adoptar ni mutar                    |

Un archivo efectivo tiene un solo owner. Una colisión se detecta antes de descargar, instalar,
copiar, lanzar o borrar.

### Sync

El plan de sync se deriva de capabilities observadas:

- Proton queda `RunnerManaged`;
- Wine portable sólo habilita ESYNC/FSYNC si la metadata TkG declara los parches correspondientes;
- Wine sin evidencia queda en wineserver;
- Wine 7.16 nunca recibe NTSync.

No es una preferencia gráfica ni un toggle arbitrario del usuario.

## Alternativas rechazadas

- Reescribir `ResolvedRunner`, `PrefixLocation` o el registry existente.
- Un objeto que mezcle profile, procesos, readiness, locks y estado mutable.
- Un DAG/plugin system genérico para combinaciones gráficas todavía inexistentes.
- Resolver conflictos por orden de aplicación o concatenar `WINEDLLOVERRIDES`.
- Usar labels/versiones como prueba de identidad o compatibility.

## Consecuencias

- Estados inválidos no son construibles sin fallar la resolución.
- Juego y tools comparten una decisión auditable aunque sus políticas por target difieran.
- Añadir un backend exige declarar ownership, targets, provider final y conflictos.
- Los fingerprints, receipts y assessments permanecen dominios separados; el plan los consume sin
  absorber persistencia ni lifecycle.
