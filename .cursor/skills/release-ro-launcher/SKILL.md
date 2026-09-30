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

Never `npm version`. Never tag a version that is not exactly `v` plus `npm run version:show`. Never push a tag unless the user asked. Never publish a GitHub Release (leave `release.yml` drafts for the user). Never bump sidecars or other crates.

## First public cut (version already 0.1.0)

Do **not** bump. Show must print `0.1.0`.

1. `git status --short --branch` is clean and `main` matches `origin/main`.
2. Quality CI on that commit is green (`ci.yml`, not `release.yml`).
3. Confirm with the user, then annotated tag only:

```bash
node scripts/check-release-invariants.mjs --tag v0.1.0
git tag -a v0.1.0 -m "RO-Launcher 0.1.0"
git push origin v0.1.0
```

4. Watch `.github/workflows/release.yml` on that tag. Environment `release` must supply `RO_LAUNCHER_DISCORD_APPLICATION_ID` and `TAURI_SIGNING_PRIVATE_KEY`.
5. Open the **draft** GitHub Release. Check AppImage, `.sig`, `latest.json`, sidecar layout, SHA-256 evidence.
6. Stop. The user publishes the draft. Publishing is what makes `.../releases/latest/download/latest.json` visible to installed AppImages.

## Later cuts

1. Clean `main` matching `origin/main`. Quality CI green.
2. `npm run version:bump -- patch` (or `minor` / `major` / an explicit X.Y.Z). Do not bump if show already equals the version they want to tag.
3. Commit only the five version files (plus nothing else) with a message like `Bump version to 0.1.1`.
4. Push `main` if the user asked. Wait for quality CI.
5. Tag `vX.Y.Z`, push the tag if the user asked, watch `release.yml`, inspect the draft, let the user publish.

## Stop

- Missing GitHub Environment `release`, Discord variable, or signing secret.
- `requireSignedVersion` false, extra updater endpoints, or guest `updater:allow-*`.
- Dirty tree, version drift, or tag not matching show.
- A request to force-push the tag or to upload an unsigned AppImage.
