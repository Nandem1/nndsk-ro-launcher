# Repository Guidelines

## Project Structure & Module Organization

This repository is a Tauri v2 desktop launcher for Ragnarok Online on Linux. The React/TypeScript frontend is in `src/`: `app/` wires the shell UI, `features/` contains domains such as launcher, servers, settings, logs, autopot, and spammer, and `shared/` holds reusable hooks, stores, UI, APIs, and types. Frontend tests are colocated as `*.test.ts`.

Rust backend code lives in `src-tauri/src/`, organized into `commands/`, `models/`, `state/`, `tools/`, and `utils/` (including `utils/webview.rs` for Linux WebKit/Wayland). Workspace crates live under `crates/`, including `ro-inputd`, `ro-tools-core`, and `ro-tools-linux`. Runtime resources are in `src-tauri/resources/`; app icons in `src-tauri/icons/`.

## Build, Test, and Development Commands

- `npm install`: install JavaScript and Tauri CLI dependencies.
- `npm run dev`: run the Vite frontend only.
- `npm run tauri:dev`: build `ro-inputd` and run the full Tauri desktop app with Linux WebView environment fixes.
- `npm run build`: run `tsc` and build the frontend bundle.
- `npm test`: run Vitest tests once.
- `cargo test --workspace`: run Rust tests for the backend and local crates.
- `cargo fmt --all`: format Rust workspace code.
- `npm run tauri:build`: build `ro-inputd` in release mode and create all production Tauri bundles.
- `npm run tauri:build:appimage`: AppImage only; sets `NO_STRIP=true` (required on Arch/CachyOS for `linuxdeploy`).

Production artifacts land in `target/release/bundle/` at the repo root (not under `src-tauri/`).

## Coding Style & Naming Conventions

Use TypeScript, React function components, and named exports. Keep feature-specific state and logic inside `src/features/<domain>/`; move reusable code to `src/shared/`.

Frontend files use 2-space indentation, single quotes, and no semicolons. Component files use `PascalCase.tsx`; hooks use `useThing.ts`; stores use `<domain>.store.ts`; pure logic modules use `<domain>.logic.ts`. Rust code must follow `rustfmt`; use snake_case modules and functions, and keep command payloads in `src-tauri/src/models/`.

## Testing Guidelines

Use Vitest for frontend logic, especially pure functions and state transitions. Prefer focused colocated tests such as `launcher.logic.test.ts`. Run `npm test` before changing TypeScript behavior.

For Rust behavior, add unit tests in the relevant module or crate and run `cargo test --workspace`. Format Rust changes with `cargo fmt --all` before submitting.

## Commit & Pull Request Guidelines

Recent history uses short, informal summaries such as `fix white screen at launch` and `few ui changes for better ux`. Keep commits concise, imperative, and scoped to one change.

Pull requests should explain user-visible behavior, list test commands run, link related issues when available, and include screenshots or screen recordings for UI changes.

## Security & Configuration Tips

Do not commit user data from `~/.local/share/ro-launcher/`, local game client paths, WINE prefixes, generated build output, or server-specific executables. Keep bundled dgVoodoo files limited to `src-tauri/resources/dgvoodoo/`.
