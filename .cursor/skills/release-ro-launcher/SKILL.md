---
name: release-ro-launcher
description: Choose the next RO-Launcher version from the release changes, synchronize version files, and prepare a signed GitHub Release draft for the Tauri updater. Use when the user asks for a release, a version bump, a vX.Y.Z tag, or a new AppImage update.
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

## Decide the next version

When the user authorizes a release without specifying its version, **the model must
choose major, minor or patch**, calculate the next X.Y.Z and briefly justify it.
Do not default to patch or ask the user to choose the bump merely because no
version was provided. An explicit user version/bump takes precedence.

Review the complete intended release delta against the latest published launcher
tag (not the latest runtime tag, a draft, or only the last commit). Include pending
changes intended for that release; do not absorb unrelated user-owned changes.
Read the actual behavior/configuration/migration impact, not just commit subjects
or Conventional Commit prefixes. Use the highest applicable impact:

- **Major:** incompatible public behavior, configuration/protocol/persistence
  contracts, or removal that breaks supported existing usage without a compatible
  path. State the broken contract and migration. Do not call a large diff or a new
  default major when existing selections/data remain compatible.
- **Minor:** new user-facing capability or supported workflow with compatible
  existing data/behavior (for example a managed runtime download). A catalogue
  change that preserves saved selections/prefixes is not by itself a forced
  migration. Do not imply universal game compatibility from a new runtime.
- **Patch:** fixes, hardening or internal/docs/test changes with no new public
  capability or incompatible contract. Large fixes can still be patch changes.

While the app is 0.x, treat an incompatible development-contract change as a minor
bump and document its migration explicitly; do not declare 1.0/stability merely
because a change is large. A first stable 1.0 requires that product milestone to
be authorized and supported by acceptance evidence.

Report `base tag → current version → bump → next version` with the decisive
changes, then verify using `node scripts/bump-version.mjs --dry-run <bump>`.
If only planning/preparing a release, stop at the dry-run: choosing a version does
not authorize writing version files, committing, tagging, pushing or publishing.
Preserve the existing authorization and draft-only boundaries below. If the
intended release scope/base cannot be established, stop and explain that specific
uncertainty rather than inventing a version.

## Later cuts

1. `git status --short --branch` is clean and `main` matches `origin/main`.
2. Quality CI on that commit is green (`ci.yml`, not `release.yml`). `ci.yml` must not run on version tags.
3. Apply the justified decision above with `npm run version:bump -- <bump>` (or the user's explicit X.Y.Z). Do not bump if show already equals the version they want to tag.
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
