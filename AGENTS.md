# Repository Operating Guide

This file is the working contract for any agent changing RO-Launcher. Read it before editing. The
goal is not merely to make a test pass: preserve the validated Wine/Gepard behavior, user data, and
the ability to explain every runtime decision from evidence.

## Product Direction and Non-Negotiable Invariants

RO-Launcher is a Linux-first Tauri v2 launcher for Ragnarok Online. It manages runners, isolated
Wine prefixes, graphics compatibility, patchers, input automation, memory readers, and Discord
Rich Presence. A fix is acceptable only if it keeps these contracts:

- Proton-CachyOS 11 through managed UMU is the general default. Do not globally downgrade it.
- Runner selection is per server. A non-empty `server.runner` overrides the global default and the
  UI must show the effective runner rather than implying the global one is active.
- SakuraRO's validated Gepard `26.9.3.1` hash uses portable Wine 7.16 legacy/old-WoW64 plus managed
  DXVK 2.6.2. This is a hash-specific compatibility profile, not a universal Gepard rule.
- HoneyRO's validated Gepard `26.8.26.1` hash works with Proton-CachyOS 11. Unknown hashes may warn,
  but must not silently force a runner.
- Never modify, hook, disable, spoof, or bypass Gepard/GameGuard, the game executable, packets, or
  server validation. Compatibility comes from selecting and configuring a legitimate runner.
- A server+runner pairing owns its own prefix. Never migrate one prefix between runners, adopt an
  unknown non-empty directory, or delete a custom prefix. Managed rebuilds are transactional: keep
  the old prefix until the replacement succeeds and restore it on failure.
- Prefix manifests are the authority for runner identity and installed components. A directory
  name, dropdown label, or process name is not proof of the effective runner.
- Direct3D 8/9/11 uses DXVK/Vulkan. DirectDraw may use dgVoodoo, which emits D3D11 and then DXVK.
  Game, OpenSetup, and patcher must receive consistent graphics overrides.
- Wine 7.16 enables FSYNC/ESYNC only when the portable artifact declares the corresponding TkG
  patches. Vanilla Wine stays on wineserver. Never force NTSync onto Wine 7.16.
- Accelerated Wine 7.16 installs WebView2 under temporary Windows 7 so Evergreen selects the
  compatible 109 runtime, then restores Windows 10 even on failure. Do not generalize this
  workaround to other runners.
- Process identity is `(pid, start_time)`, never PID alone. Revalidate it before memory access or a
  signal so PID reuse cannot target an unrelated process.
- Multi-client lifecycle is real: one client stopping must not tear down another client, shared
  input, or a prefix still in use.
- AppImage variables must be sanitized before starting external Wine, Proton, UMU, or sidecars.
- El supervisor `ro-sessiond` está activo por defecto. Restaura el acceso a memoria
  con `kernel.yama.ptrace_scope=1` sin sudo ni sysctl. Rollback de una release:
  `RO_LAUNCHER_SESSION_SUPERVISOR=0`. No debilitar la política del host ni dejar
  un supervisor a medias. El contrato detallado sigue en `docs/PTRACE_SESSION_SUPERVISOR_PLAN.md`.

## Repository Map and Ownership

- `src/app/`: React shell and top-level wiring.
- `src/features/<domain>/`: domain UI, stores, hooks, and colocated `*.test.ts` tests.
- `src/shared/`: reusable frontend APIs, types, stores, hooks, and UI primitives.
- `src-tauri/src/commands/`: thin Tauri IPC boundary. Validate payloads and delegate behavior.
- `src-tauri/src/models/`: serialized command/configuration models and validation.
- `src-tauri/src/state/`: application registries and lifecycle state.
- `src-tauri/src/tools/`: launcher, prefix, runner, graphics, memory-tool, and sidecar orchestration.
- `src-tauri/src/utils/`: reusable storage, runner, Wine, process, and Linux WebView infrastructure.
- `crates/ro-tools-core`: platform-neutral scanning/tool contracts.
- `crates/ro-tools-linux`: `/proc`, process identity, memory reading, and Linux implementations.
- `crates/ro-inputd`: unprivileged launcher-owned input sidecar.
- `src-tauri/resources/`: bundled runtime resources; dgVoodoo assets stay under `dgvoodoo/`.
- `scripts/install-nndsk.mjs`: local AppImage installation workflow.
- `target/release/bundle/`: generated production artifacts; never commit them.

Keep feature logic in its domain. Move code to `shared` or `utils` only when at least two domains
truly share the abstraction. Do not put business logic in React components or Tauri command
wrappers. Prefer small pure functions with focused tests over effects embedded in orchestration.

## Working Method

### Completion protocol: implement, then try to disprove it

For any change that crosses process, protocol, persistence, packaging, or lifecycle boundaries,
work in two explicit passes even when one agent performs both roles:

1. **Implementation pass:** derive acceptance criteria from the request and repository invariants,
   trace the caller and callee, and implement the smallest coherent change with focused tests.
2. **Adversarial review pass:** return to the acceptance criteria and inspect the complete diff as
   if it came from another author. Try to falsify completion; do not merely confirm the happy path
   or repeat the implementation rationale.

The adversarial pass must inspect every boundary touched by the change, as applicable:

- serialized producer/consumer field names, framing, buffering, batching, size limits, and EOF;
- inherited environment versus explicit overrides, including AppImage source, bundle, and installed
  runtime behavior;
- process identity from capture through every memory read and signal, including exit and PID reuse;
- leases, counters, locks, cancellation, timeouts, shutdown ordering, and concurrent clients;
- filesystem mutation and rollback while subprocesses or sidecars may still own the target;
- source binary versus bundled sidecar versus installed artifact;
- disabled features, fallback paths, error cleanup, and repeated cycles without leaks or zombies.

For each relevant seam, add or run a test that can fail for the suspected defect: multiple protocol
messages in one write, partial input, stale identity, inherited conflicting environment, cleanup
failure, cancellation, concurrent shutdown, or repeated launch/stop cycles. A green unit suite does
not replace a runtime or packaging check when the contract exists only in the assembled artifact.
If an acceptance condition cannot be exercised in the current environment, report it explicitly;
do not silently convert inference into validation.

### 1. Establish the baseline before editing

Run from the repository root:

```bash
pwd
git status --short --branch
git log --oneline --decorate -8
git diff --check
git diff --stat
rg --files -g 'AGENTS.md' -g '!target' -g '!node_modules'
```

Treat every pre-existing modification as user-owned. Read its full diff and integrate around it;
never discard, reset, or rewrite it merely to obtain a clean tree. Check for more specific
`AGENTS.md` files before touching a subtree.

Inspect with targeted commands instead of dumping the whole repository:

```bash
rg -n "symbol_or_error" src src-tauri crates
sed -n 'START,ENDp' path/to/file
git diff -- path/to/file
git log -p -- path/to/file
```

Read the caller, callee, state model, and existing tests before changing an interface. Search all
call sites before renaming or changing behavior.

### 2. Diagnose from evidence, not from the loudest log line

Classify the failing layer first: frontend/Tauri IPC, launcher orchestration, runner, prefix,
patcher, client, graphics, audio, input, memory/Yama, or anti-cheat compatibility. Reproduce the
smallest failing path and change one variable at a time.

Evidence priority:

1. Exact executable, runner path/version/hash, prefix manifest, and environment used by the process.
2. First causal error and exit status from the smallest reproduction.
3. Process tree, stable identity, `/proc` state, coredump, and kernel journal.
4. A/B test with one controlled variable and separate prefixes when the runner changes.
5. Upstream primary documentation or issue reports matching the exact build and architecture.

Wine emits many harmless `err:` lines. Do not call them the cause unless they correlate with the
exit. Conversely, never hide a non-zero installer status because the desired files happened to
appear. Check coredumps and OOM evidence before describing a normal error exit as a crash.

Useful read-only diagnostics:

```bash
# Capture a development run. Review/redact before sharing because launch args can contain secrets.
npm run tauri:dev 2>&1 | tee /tmp/ro-launcher-dev.log

# Process ancestry without command-line credentials.
pgrep -f 'ragexe|wineserver|umu|wine'
ps -o pid,ppid,pgid,sid,stat,lstart,comm -p <pid>
rg '^(Name|State|Pid|PPid|TracerPid|Uid|Gid|NSpid|Seccomp):' /proc/<pid>/status

# Print environment keys first; reveal selected values only when needed and safe.
tr '\0' '\n' < /proc/<pid>/environ | cut -d= -f1 | sort
tr '\0' '\n' < /proc/<pid>/environ | rg \
  '^(WINEPREFIX|WINE|WINESERVER|PROTONPATH|WINEESYNC|WINEFSYNC|WINEDLLOVERRIDES|DXVK_LOG_PATH)='

# Binary and prefix identity.
file /absolute/path/to/game.exe /absolute/path/to/gepard.dll
objdump -f /absolute/path/to/game.exe
sha256sum /absolute/path/to/runner /absolute/path/to/game.exe /absolute/path/to/gepard.dll
rg -m1 '^#arch=' /absolute/path/to/prefix/system.reg
rg -n 'runner_kind|runner_path|components|schema_version' \
  /absolute/path/to/prefix/.ro-launcher-prefix.json
/absolute/path/to/wine --version

# Kernel/crash/graphics evidence.
sysctl kernel.yama.ptrace_scope
coredumpctl --no-pager list --since today
coredumpctl --no-pager info <pid-or-exe>
journalctl --user -b --no-pager -n 200
vulkaninfo --summary
nvidia-smi
```

Do not paste unredacted command lines, credentials, tokens, memory contents, or complete process
environments into logs, issues, commits, or chat. Avoid global Wine `+relay` unless a targeted A/B
already proved it necessary; it changes timing and produces unusable logs.

### 3. Make the smallest coherent change

Fix the layer that owns the bug. Avoid unrelated refactors while behavior is under diagnosis. Add a
regression test for the decision logic, failure cleanup, or state transition whenever practical.
For runner/prefix changes, preserve these boundaries:

```text
resolve runner -> derive isolated prefix -> validate manifest/requirements
-> provision transactionally -> launch with one coherent environment -> observe stable process
```

Do not copy random native DLLs, combine multiple sync/graphics tweaks in one experiment, or infer
old/new WoW64 from a folder name. Verify runner layout, PE architecture, and actual process env.

Use `apply_patch` for deliberate source edits. Use project formatters only after reviewing the diff;
do not run broad mechanical rewrites over unrelated user changes. Never use destructive Git or
filesystem commands (`git reset --hard`, checkout-overwrite, recursive deletion) to clean up.

### 4. Test in proportion to the change

Fast focused checks come first, then the full gate. Frontend code uses TypeScript, React function
components, named exports, two spaces, single quotes, and no semicolons. Rust follows `rustfmt` and
keeps serialized payloads in `models`.

```bash
# Frontend focused/full
npm test -- path/to/file.test.ts
npm run lint
npm run format:check
npm test
npm run build

# Rust focused/full
cargo test -p <crate> test_name
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Before committing any source or runtime behavior change, run the complete frontend and Rust gates
above. A documentation-only change needs at minimum `git diff --check` plus validation that commands,
paths, links, and cross-references match the repository. Never claim a check that was not run.

Changes to runner selection, prefix setup, process lifecycle, graphics, WebView2, or sidecar
packaging also require a real `npm run tauri:dev` smoke test. Validate both the general Proton path
and any server-specific Wine path touched. Do not treat frontend-only `npm run dev` as a desktop
integration test.

For packaging/release-sensitive work:

```bash
npm run tauri:build
npm run tauri:build:appimage
RO_LAUNCHER_DISCORD_APPLICATION_ID=<id> npm run tauri:build:appimage
npm run install:nndsk
```

Artifacts must land under `target/release/bundle/`. Verify the installed AppImage separately from
development whenever environment sanitation, resources, sidecars, or paths changed.

### 5. Validate runtime changes as a matrix

Record the runner path/version/hash, prefix path/architecture, executable hashes, relevant env,
graphics backend, and exact outcome. Keep game files constant and use separate prefixes when
comparing runners. For process changes, test direct launch, patcher handoff, stop during launch,
multiple clients, multiple prefixes, launcher exit, and repeated cycles without zombies. For
memory work, the acceptance environment is `ptrace_scope=1`; `ptrace_scope=0` is diagnostic only.

SakuraRO with the validated Wine 7.16 profile and HoneyRO with default Proton-CachyOS are mandatory
regression anchors for runner/prefix architecture. A result from one Gepard build does not prove a
rule for another build.

### 6. Deliver a reviewable, clean tree

Before a requested commit:

```bash
git status --short --branch
git diff --check
git diff --stat
git diff
git status --ignored --short
```

Confirm that no local config, game executable, prefix, runner archive, generated artifact, secret,
or unrelated user change is staged. Use a short imperative commit subject scoped to one coherent
change. Commit or push only when the user explicitly requests it. For a requested main delivery:

```bash
git add <explicit-paths>
git diff --cached --check
git diff --cached --stat
git commit -m "concise imperative summary"
git push origin main
git status --short --branch
```

Do not report success until the push returns successfully and local `main` matches `origin/main`.
Summarize user-visible behavior, files changed, validation performed, and any remaining manual
runtime test. A clean tree is an outcome, never a reason to erase work.

## Debugging and Design Philosophy

- Prefer a falsifiable hypothesis over a broad workaround. Name the variable an experiment changes.
- Distinguish confirmed facts, strong inference, and unknowns. Preserve exact error codes and hashes.
- Make invalid states difficult to represent: typed runner strategies, stable process identities,
  explicit lifecycle states, RAII guards/leases, and closed status enums.
- Preserve rollback paths and original data until the replacement is proven healthy.
- Keep locks short, define acquisition order, never hold synchronous locks across `.await`, and test
  cancellation/error paths—not only the happy path.
- Every background task needs ownership, termination, and cleanup. Every PID needs reuse protection.
- Defaults should serve most servers; exceptional compatibility belongs to evidence-backed profiles.
- Improve observability at decision boundaries, but redact secrets and avoid flooding the UI with
  routine Wine noise.
- If external research is needed, prefer primary sources and record how the source maps to the exact
  runner/build. Research is evidence for an experiment, not permission to cargo-cult a tweak.
- Stop and ask before an action would expand scope, weaken security, destroy data, or alter external
  systems. Otherwise proceed autonomously through safe inspection, implementation, and verification.

## Security and Local Data

Never commit anything from `~/.local/share/ro-launcher/`, local game directories, Wine prefixes,
downloaded runners, generated bundles, crash cores, or server-specific executables. Do not request
`sudo` for normal launcher operation. If a system-level diagnostic genuinely needs privileges, give
the user one exact read-only command and explain what evidence it obtains; the user executes it.

The launcher may automate input and read the user's own game process, but it does not write game
memory or evade anti-cheat. Keep `docs/PTRACE_SESSION_SUPERVISOR_PLAN.md` aligned with that boundary.
