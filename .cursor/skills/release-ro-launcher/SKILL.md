---
name: release-ro-launcher
description: Bump the single app version, tag, and publish RO-Launcher GitHub Releases for the Tauri updater. Use when the user asks to release, cut a version, bump patch/minor/major, tag vX.Y.Z, publish v0.1.0, or generate a new AppImage update.
---

# Release RO-Launcher

App version is one X.Y.Z written to five files. Do not edit those files by hand.

```bash
npm run version:show
node scripts/bump-version.mjs --dry-run patch
npm run version:bump -- patch
npm run version:bump -- minor
npm run version:bump -- 0.2.0
```

`version:bump` updates `package.json`, `package-lock.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, and the `ro-launcher` stanza in `Cargo.lock`, then runs `scripts/check-release-invariants.mjs`.

Never `npm version`. Never tag a version that is not exactly `v` plus `npm run version:show`. Never push a tag unless the user asked. Never publish a GitHub Release (leave `release.yml` drafts for the user). Never bump sidecars or other crates. Never move a tag once GitHub has a release object for it, draft or published.

Installed AppImages check only this URL.

`https://github.com/Nandem1/nndsk-ro-launcher/releases/latest/download/latest.json`

That URL exists only after the user publishes the draft. The plugin verifies the AppImage with the pubkey baked into `src-tauri/tauri.conf.json`. SHA-256 evidence on the release is for humans. It is not the authenticity gate.

`v0.1.0` is already published. Do not retag it. The next cut is a bump.

## Later cuts

1. `git status --short --branch` is clean and `main` matches `origin/main`.
2. Quality CI on that commit is green (`ci.yml`, not `release.yml`). `ci.yml` must not run on version tags.
3. `npm run version:bump -- patch` (or `minor` / `major` / an explicit X.Y.Z). Do not bump if show already equals the version they want to tag.
4. Commit only the five version files (plus nothing else) with a message like `Bump version to 0.1.1`. Tree-coupled version tests read `npm run version:show`. They must not be edited on a bump.
5. Push `main` if the user asked. Wait for quality CI.
6. Confirm with the user, then annotated tag only.

```bash
node scripts/check-release-invariants.mjs --tag vX.Y.Z
git tag -a vX.Y.Z -m "RO-Launcher X.Y.Z"
git push origin vX.Y.Z
```

7. Watch `.github/workflows/release.yml` on that tag. Environment `release` must supply `RO_LAUNCHER_DISCORD_APPLICATION_ID` and `TAURI_SIGNING_PRIVATE_KEY`. Quality steps stay in the same job as the AppImage so `target/` stays warm.
8. Open the **draft** GitHub Release. Check AppImage, `.sig`, `latest.json`, sidecar layout, SHA-256 evidence. Fill user-facing notes before asking the user to publish. Empty `notes` in `latest.json` is what the in-app updater shows as detail.
9. Stop. The user publishes the draft.

If `release.yml` fails before GitHub creates a draft, delete the unused remote tag after the user agrees, fix the tree, and tag the same X.Y.Z again. If a draft or published release already exists for that tag, bump instead.

## Stop

- Missing GitHub Environment `release`, Discord variable, or signing secret.
- `requireSignedVersion` false, extra updater endpoints, or guest `updater:allow-*`.
- Dirty tree, version drift, or tag not matching show.
- A request to force-push a published tag or to upload an unsigned AppImage.
- Quality CI triggered by a `v*.*.*` tag (`ci.yml` must stay on `main` and pull requests).
- `app.windows[0].decorations` not `false` (GTK CSD would duplicate the in-app title).
