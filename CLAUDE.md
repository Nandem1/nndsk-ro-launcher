# CLAUDE.md

Compatibility entry point for Claude Code and other agents that automatically read this filename.

## Authority

Read [`AGENTS.md`](AGENTS.md) completely before inspecting or changing the repository. It is the
single authoritative operating contract for product invariants, safety, workflow, validation, and
delivery. If this file and `AGENTS.md` ever disagree, follow `AGENTS.md` and correct this file rather
than inventing a compromise.

The final design and audit record for Wine session supervision lives in
[`docs/PTRACE_SESSION_SUPERVISOR_PLAN.md`](docs/PTRACE_SESSION_SUPERVISOR_PLAN.md).

## Project orientation

RO-Launcher is a Linux-first Tauri v2 application for Ragnarok Online:

- React 18, TypeScript, Tailwind, Zustand, and Vite in `src/`;
- Tauri/Rust commands, state, orchestration, and infrastructure in `src-tauri/src/`;
- platform-neutral contracts in `crates/ro-tools-core`;
- Linux process, memory, evdev, and uinput adapters in `crates/ro-tools-linux`;
- input sidecar in `crates/ro-inputd`;
- Wine session protocol in `crates/ro-session-protocol`;
- POSIX subreaper sidecar in `crates/ro-sessiond`.

Keep Tauri commands thin, feature behavior in `tools/` or `src/features/<domain>/`, serialized IPC
models in `models/`, and genuinely shared infrastructure in `utils/` or `src/shared/`.

## Required completion workflow

Do not treat implementation and review as the same pass. For every substantial change:

1. establish the clean baseline and read the full user-owned diff;
2. turn the request and `AGENTS.md` invariants into concrete acceptance criteria;
3. implement the smallest coherent change and focused regression tests;
4. restart from the complete diff and try to disprove that the work is finished;
5. inspect cross-system seams: serialization/framing, inherited environment, stable process
   identity, async buffering, locks/leases, failure rollback, source-to-bundle packaging, and the
   installed runtime;
6. run the focused checks, full gates, and required runtime matrix from `AGENTS.md`;
7. state any acceptance condition that could not actually be exercised.

Passing unit tests is evidence, not permission to skip an AppImage or live-process check when the
behavior only exists after packaging. Prefer a regression that triggers the former failure: batched
messages, partial reads, stale PIDs, conflicting inherited variables, cleanup errors, concurrent
shutdown, and repeated cycles without zombies.

## Commands

```bash
# Desktop development smoke; builds both sidecars first
npm run tauri:dev

# Frontend gate
npm run lint
npm run format:check
npm test
npm run build

# Rust gate, including integration fixtures hidden behind features
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features

# Production artifacts
npm run tauri:build
npm run tauri:build:appimage
RO_LAUNCHER_DISCORD_APPLICATION_ID=<id> npm run tauri:build:appimage
npm run install:nndsk
```

Generated bundles belong under `target/release/bundle/` and must never be committed. For sidecar or
environment changes, verify that `ro-inputd` and `ro-sessiond` are embedded, install through the
repository script, and inspect the installed process tree separately from development.

## Non-negotiable reminders

- Never bypass, modify, hook, spoof, or disable Gepard/GameGuard or server validation.
- Runner choice is per server; manifests, not labels or directory names, prove runner identity.
- A server+runner owns its prefix. Managed rebuilds are transactional and custom prefixes are never
  deleted or adopted.
- Process identity is `(pid, start_time)` and must be revalidated before memory access or signals.
- Multi-client leases are real; one client must not tear down another client or its prefix.
- Sanitize AppImage variables before external Wine, Proton, UMU, or sidecar processes.
- `ro-sessiond` is on by default; `RO_LAUNCHER_SESSION_SUPERVISOR=0` is release rollback only.
- Do not weaken Yama or require `sudo`; memory acceptance is performed with `ptrace_scope=1`.
- Preserve user data, unrelated worktree changes, exact errors, and evidence needed to explain every
  runtime decision.
