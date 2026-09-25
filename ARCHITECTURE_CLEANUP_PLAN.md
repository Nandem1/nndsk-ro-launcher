# Plan: orden arquitectónico (Linux-first)

Documento de ejecución para un agente/modelo potente.  
**Alcance:** ordenar `nndsk-ro-launcher` sin portabilidad macOS/Windows.  
**Principio:** especialización Linux hacia adentro; fronteras limpias hacia arriba.

---

## 0. Contexto fijo (no reinterpretar)

### Objetivo del producto

Launcher dedicado RO en **Linux x86_64** con:

- runtime administrado Proton-CachyOS + UMU
- prefixes aislados + dgVoodoo + server tools
- combat tools: AutoPot / AutoBuff / Spammer vía `/proc` + uinput + `ro-inputd`

### Capas actuales (mantener)

```
crates/ro-tools-core/     dominio puro + ports (traits)
crates/ro-tools-linux/    adaptadores Linux (/proc, uinput, wine PID, evdev helpers)
crates/ro-inputd/         sidecar evdev grab + uinput passthrough
src-tauri/src/
  commands/               IPC delgado
  tools/                  orquestación por feature
  state/                  proceso, sesiones, repos
  models/                 DTOs IPC
  utils/                  infra compartida (wine/proton, paths, events, webview)
src/                      frontend FSD
```

### Problema a resolver

1. `src-tauri` importa tipos concretos de `ro-tools-linux` dentro de loops/services (la frontera de ports existe en core pero no se usa del todo).
2. Módulos demasiado grandes y con varios motivos de cambio.
3. Docs internas (`CLAUDE.md`) desfasadas respecto al runtime/input reales.
4. Mezcla conceptual Runtime vs Combat tools sin daño funcional aún, pero frágil.

### Fuera de alcance (explícito)

- macOS / CrossOver / Whisky / WineSkin
- reintroducir ydotool
- volver a Wine del sistema como runner de producto
- reescritura “Clean Architecture completa” con muchos crates nuevos
- cambiar comportamiento de usuario salvo bugs detectados al refactor

### Regla de dependencia (invariante)

```
commands → tools | state | models
tools   → ro-tools-core (+ composition con ro-tools-linux solo en factories/session)
ro-tools-linux → implementa ports de ro-tools-core; no conoce Tauri
ro-tools-core → sin OS / sin Tauri
utils → I/O compartido; sin reglas de feature
```

### Definition of Done global

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `npm test`
- `npx tsc --noEmit` (o `npm run build` si el repo lo usa como gate)
- sin cambios de UX/IPC salvo que una fase lo declare
- cada fase = 1 PR/commit atómico con mensaje claro

### Cómo ejecutar este plan

1. Leer esta sección 0 completa.
2. Ejecutar **una sola fase** por iteración.
3. Al terminar la fase: tests de la fase + checklist.
4. No adelantar fases siguientes “de paso”.
5. Si una fase descubre bug de comportamiento: documentarlo en la PR; no expandir alcance salvo hotfix mínimo para no romper main.

---

## Fase A — Alinear documentación interna

**Meta:** que un agente futuro no se guíe por docs obsoletas.

### A1. Auditar desfase docs ↔ código

- Comparar `CLAUDE.md` / `AGENTS.md` contra:
  - `README.md` (runtime Proton+UMU, uinput, prefixes)
  - `src-tauri/src/tools/runners/managed.rs`
  - `src-tauri/src/tools/input/`
  - `crates/ro-tools-linux/`
- Listar afirmaciones falsas (Wine sistema, ydotool, discovery Proton Steam, etc.).

**Done:** lista concreta en el commit message o en comentario breve al inicio del diff de docs (no hace falta issue tracker).

### A2. Reescribir `CLAUDE.md` al estado real

Actualizar como mínimo:

- target OS Linux-only
- runtime administrado (Proton-CachyOS + UMU), no Wine/Proton Steam como path de producto
- input: uinput + `ro-inputd` (sin ydotool)
- mapa `tools/` actual
- flujos AutoPot / AutoBuff / Spammer con módulos reales
- eventos IPC vigentes
- comandos de test/build vigentes

**No** inventar arquitectura futura aquí; documentar lo que hay.

### A3. Ajuste mínimo `AGENTS.md` si contradice A2

Solo coherencia; no duplicar el tratado de `CLAUDE.md`.

**Gates A:** solo markdown; `git diff` acotado a docs.

---

## Fase B — Inventario de fronteras (sin refactors de código)

**Meta:** mapa accionable para fases C–F. Entregable: `ARCHITECTURE_BOUNDARY_MAP.md` en la raíz (se puede borrar al cerrar el plan o dejarlo).

### B1. Inventario de imports `ro_tools_linux` desde `src-tauri`

Para cada uso, clasificar:

| Símbolo | Archivo | Rol | Acción propuesta |
|---------|---------|-----|------------------|
| `ProcMemoryReader` | … | concreto en loop | port en fase E |
| … | … | … | keep / move / abstract |

### B2. Inventario Runtime vs Combat

Etiquetar módulos `tools/*` y `utils/*` como:

- `runtime` (runners, prefix, launcher, deps checks de Proton, wine env)
- `combat` (autopot, autobuff, spammer, input, game_pid, memoria)
- `shared` (session controller, events, paths, models)

### B3. Hotspots por tamaño / complejidad

Confirmar y anotar LOC + responsabilidades mezcladas en al menos:

- `tools/input/uinput_worker.rs`
- `tools/spammer/loop_runner.rs`
- `tools/server_tools/dgvoodoo.rs`
- `tools/runners/managed.rs`
- `tools/deps/check.rs`
- `tools/autopot/scanner.rs`

**Gates B:** markdown only.

---

## Fase C — Partir monólitos sin cambiar comportamiento

**Meta:** un archivo ≈ un motivo de cambio.  
**Regla de oro:** refactor mecánico; mismos tests verdes; sin cambios de timing/IPC.

Orden obligatorio (menor riesgo → mayor):

### C1. `tools/runners/managed.rs`

Separar en módulos internos privados, por ejemplo:

- `download.rs` / `verify.rs` / `activate.rs` / `paths.rs` (nombres orientativos)
- `mod.rs` reexporta la API pública actual

**Prohibido:** cambiar URLs, checksums, layout de `~/.local/share/ro-launcher/runtime/`.

### C2. `tools/server_tools/dgvoodoo.rs`

Separar:

- scan/validate ownership
- install transaccional
- uninstall/rollback
- helpers de hash/backup

API pública de `server_tools` intacta.

### C3. `tools/spammer/loop_runner.rs`

Separar sin tocar semántica:

- lifecycle `ro-inputd` (spawn/stdin/stdout/shutdown)
- tick loop / status emit
- integración gear switch (ya existe `gear.rs`; mover glue sobrante)
- recovery de input

Preservar delays y contrato de status events.

### C4. `tools/input/uinput_worker.rs` (**más delicado**)

Partir solo estructura:

- device open/prepare/permissions messaging
- write path (key/click/spam_cycle)
- recovery/reconnect
- tipos compartidos (`InputSource`, timing)

**Prohibido en C4:**

- cambiar protocolo con combat tools
- cambiar latencia / batching / ordering de eventos
- “mejoras” de performance

Smoke mental obligatorio en la PR: AutoPot press, Spammer cycle, release_spam idempotente.

### C5. (Opcional) `deps/check.rs` y `autopot/scanner.rs`

Solo si tras C1–C4 aún estorban. Misma regla: split mecánico.

**Gates C (cada subfase):**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## Fase D — Clarificar contextos Runtime vs Combat (organización ligera)

**Meta:** navegación clara **sin** mover lógica a crates nuevos todavía.

### D1. Documentar contextos en `tools/mod.rs`

Comentario de módulo estable:

- Runtime: `runners`, `prefix`, `launcher`, parte de `deps`
- Combat: `autopot`, `autobuff`, `spammer`, `input`, `game_pid`
- Shared: `session`, superficies usadas por ambos

### D2. Evitar dependencias cruzadas indebidas

- Combat no debe importar detalles de download Proton.
- Runtime no debe abrir uinput salvo el prepare-before-Wine ya existente (respetar invariante actual documentándola).

Si hay import cruzado sucio: extraer al shared mínimo (`game_pid`, `session`, events), no crear god-module.

### D3. (Opcional) carpetas `tools/runtime/` y `tools/combat/`

**Solo** si el move es mecánico y los `mod` paths se actualizan sin drama.  
Si el diff se vuelve ruidoso: **saltar D3** y quedarse en D1–D2.

**Gates D:** tests workspace + smoke compile Tauri lib.

---

## Fase E — Composition root + uso real de ports

**Meta:** loops/engines dependen de `ro-tools-core::ports`, no de structs Linux.

### E1. Fijar policy de wiring

Composition root permitido:

- `tools/*/session.rs`
- `tools/*/service.rs` (constructors)
- eventualmente un `tools/combat/wiring.rs` si reduce duplicación

**No** permitido: `loop_runner` creando `ProcMemoryReader::open` ni conociento paths `/proc`.

### E2. AutoPot usa `dyn MemoryReader` + writers por trait

1. En session/service: `ProcMemoryReader::open` + wrap en `Arc<dyn MemoryReader>` (o generic `R: MemoryReader` si evita object-safety issues).
2. `loop_runner` / engine calls solo vía trait.
3. Añadir fake in-memory en tests de orquestación si aporta; si no, al menos tests de core siguen bastando.

Preferencia: **generics en loops** si `dyn` complica lifetimes; object-safety no es dogma.

### E3. AutoBuff igual que E2

Misma forma que AutoPot para no divergir APIs.

### E4. Spammer writers vía `SpamCycleWriter` / held keys

- Gateway/uinput implementan traits de core (si aún no están explícitos en el impl block público).
- Loop spammer no nombra tipos Linux salvo wiring.

### E5. PID / process identity

- Decidir: trait mínimo en core **o** keep `ro_tools_linux` solo en `tools/game_pid.rs` + session (aceptable).
- No filtrar `ProcessIdentity` a commands/models si se puede evitar.

**Gates E (cada subfase):**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Checklist manual (si hay entorno Linux gráfico; si no, declarar no ejecutado):

- start/stop AutoPot
- start/stop AutoBuff
- start/stop Spammer + update config replace serializado

---

## Fase F — Hardening de contratos y tests donde paga

**Meta:** proteger invariantes tras el orden.

### F1. Tests de engines/core gaps

Cubrir bordes faltantes en `ro-tools-core` (clamps delay, keys inválidas, gear rules, autobuff tick).

### F2. Test de no-solapamiento de sesiones

Si no existe: test unitario del `SessionController` / replace serialized (ya hay piezas en state).

### F3. Contrato IPC

- Verificar `models/` ↔ `src/shared/types.ts` camelCase
- No cambiar payloads; solo tests/guards si faltan (`contract-fixtures` si aplica)

### F4. Frontend logic tests

Solo si el refactor backend no tocó UI. Si no hubo cambios TS: skip.

**Gates F:** `cargo test --workspace` + `npm test`.

---

## Fase G — Cierre y limpieza del plan

### G1. Actualizar este plan marcando fases hechas

o mover resumen final a `CLAUDE.md` (sección Architecture) y eliminar docs temporales (`ARCHITECTURE_BOUNDARY_MAP.md`) si ya no aportan.

### G2. Checklist final de invariantes

- [ ] Sin ydotool
- [ ] Runner de producto = managed Proton+UMU
- [ ] `ro-tools-core` sin deps OS
- [ ] loops combat sin tipos Linux concretos (post-E)
- [ ] docs alineadas
- [ ] AppImage/build scripts intactos

---

## Orden de PRs recomendado

| PR | Fases | Riesgo | Ideal para LLM |
|----|-------|--------|----------------|
| 1 | A1–A3 | Bajo | Muy alto |
| 2 | B1–B3 | Bajo | Muy alto |
| 3 | C1 | Bajo | Alto |
| 4 | C2 | Bajo | Alto |
| 5 | C3 | Medio | Medio |
| 6 | C4 | Alto | Medio (con instrucciones estrictas) |
| 7 | D1–D2 (+D3 opcional) | Bajo/Medio | Alto |
| 8 | E2 AutoPot | Medio | Medio |
| 9 | E3 AutoBuff | Medio | Medio |
| 10 | E4 Spammer | Alto | Medio-bajo |
| 11 | E5 + F | Bajo/Medio | Alto |
| 12 | G | Bajo | Alto |

---

## Prompt sugerido por fase (plantilla)

```text
Trabaja SOLO la fase <ID> de ARCHITECTURE_CLEANUP_PLAN.md.
Respeta sección 0 (fuera de alcance + regla de dependencia).
No adelantes otras fases.
Al terminar: formatea, clippy -D warnings, tests workspace, resume en español qué cambió y qué no.
```

Para C4/E4 añadir:

```text
Prohibido cambiar timing, ordering de input, o protocolo ro-inputd.
Si dudas entre limpieza y preservar comportamiento: preservar comportamiento.
```

---

## Señales de que una fase se está yendo de scope

- aparecen menciones macOS / CrossOver / abstracción RuntimeProvider multi-OS
- se “aprovecha” para retuning de Spammer delay
- se reescribe frontend “de paso”
- se crean >2 crates nuevos
- se cambia formato de `servers.json` / settings sin migración

Si ocurre: parar, revertir lo extra, cerrar la fase mínima.

---

## Nota de producto

Este plan **refuerza** la especialización Linux; no la diluye.  
El éxito no es “hexagonal puro”, es: **código navegable, docs ciertas, ports usados, monólitos partidos, comportamiento idéntico.**
