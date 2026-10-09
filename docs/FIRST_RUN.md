# First-run contract and acceptance

This change is based on `baf3272aa7db850d924aa3b3348311a218fd129b` (`v0.2.0`).
The default remains nndsk-ro-proton `0.1.0-dev.2`; Wine 7.16 Staging/TkG amd64 is
the separate per-server fallback. Runtime/client patches and graphics configuration
are unchanged. The original investigation made no release/tag/version change.
The fixes are prepared for the separately authorized `v0.2.1` patch release;
clean-host acceptance is still pending and is not implied by cutting a release.

## Confirmed defects and fixes

### Missing prefix parents (A)

The actual ordering is `setup_runtime_prefix -> RunnerOperation::begin -> registry
canonicalization -> bootstrap guard -> spawn_supervisor -> sidecar canonicalization`,
before the provisioning code creates the prefix. Both canonicalizers previously
required at least the immediate parent to exist. A missing `prefixes/` therefore
failed even when the runtime was already installed.

Session identity now resolves the nearest existing directory ancestor and appends
the missing **normal** components. The protocol crate supplies the same resolver
to the launcher, supervisor and operation lock. It performs no mkdir, adoption or
deletion. Inaccessible ancestors, files, dangling symlinks and ambiguous missing
`..` paths are rejected. Identity remains stable after provisioning and through a
legitimate HOME alias. Managed path/manifest validation and the exclusive prefix
guard still authorize creation. Custom prefixes never bootstrap missing external
parents. Setup/reset retain their ownership checks, leases, backups and rollback.
Launcher and server-tool callers use this same session identity; they still require
a configured prefix, rather than creating one merely by starting a session.

The new real-supervisor regression failed with the original canonicalization error
before the fix. It now runs in a subprocess with a temporary HOME/XDG and proves
that neither app data nor `prefixes/` is created by admission, and that cleanup/retry
works. It also exercises an installer exiting zero with a diagnostic and no prefix.

### UMU cold bootstrap (B)

**Confirmed here, not yet confirmed on the user's separate Arch host:** UMU 1.4.0
downloaded a complete Steam Runtime archive from mutable `latest-public-beta`, but
checked it against a different version's digest. Expected digest was
`985e2728bbb65fff7ae5ea9e3389762beceaf8319878f542a0222bba241b5217`; the downloaded
165046124-byte archive hashed to
`4f48ea7fb5e4bc170d9f9e1f16cf22e15af4c5b9ebef7f3ad5b87563e11a4f5a`, exactly matching
the immutable `4.0.20260928.262390/SHA256SUMS`. Empty cache, complete download and
independent versioned checksum distinguish this from local corruption/truncation.

UMU 1.4.4 resolves `latest-public-beta.txt` and downloads from the immutable version
directory. The same empty-target control then passed download/SHA256/mtree and
created a prefix, exposing an independent error: Wine ShellExecute of the empty
target exited 1. Keeping `waitforexitandrun` but using `wineboot -i` passed with exit
0 and a complete prefix. `runinprefix` was tested and rejected: this Proton base
skips initial prefix setup for that verb.

Sources: [UMU 1.4.0 runtime source](https://github.com/Open-Wine-Components/umu-launcher/blob/1.4.0/umu/umu_runtime.py),
[UMU 1.4.4 runtime source](https://github.com/Open-Wine-Components/umu-launcher/blob/1.4.4/umu/umu_runtime.py),
[Steam CDN version-skew report](https://github.com/Open-Wine-Components/umu-launcher/issues/651),
[version-file fix #703](https://github.com/Open-Wine-Components/umu-launcher/pull/703).
Initial `CRITICAL: ... validation failed / platform missing` is also emitted during
a successful cold install. It is not by itself the download failure.

Hypotheses separated by evidence, not by guessing the external host:

| B hypothesis | Local verdict | Separate Arch host |
| --- | --- | --- |
| Stale cache or truncated archive | Rejected for control: empty HOME/cache; exact complete immutable-version digest/size | Not excluded without its original error |
| Python/zipapp incompatibility | Not the observed cause: both zipapps executed the download logic under Python 3.14.7 | Host interpreter/modules checked by preflight; still needs acceptance |
| CA/TLS/network unavailable | Not the observed cause: HTTPS completed and 1.4.4 integrity passed | Possible until its primary error is captured |
| Supervisor timeout/early teardown | Rejected for this control: UMU emitted its exception and exited before cleanup request | Not asserted absent in unobserved external logs |
| Version skew from mutable CDN URLs | Confirmed locally by hash/size plus source, corrected A/B with versioned 1.4.4 | Strongly consistent with symptom, not yet confirmed |
| Unmarked partial Steam Runtime permanently poisons retry | Rejected for tested candidate: seeded partial recovered; same-HOME retry passed | Marked-corrupt state remains a separate limitation |
| Empty target after fixing download | Confirmed independent failure; actual builtin target fixes it | Fix included; full provisioning still needs external acceptance |

The pinned catalog, URL, archive metadata, digest and receipt fixture now identify:

| Field | Value |
| --- | --- |
| ID/version | `umu-launcher-1.4.4` / `1.4.4` |
| Archive | `umu-launcher-1.4.4-zipapp.tar`, root `umu`, 430080 bytes |
| URL | `https://github.com/Open-Wine-Components/umu-launcher/releases/download/1.4.4/umu-launcher-1.4.4-zipapp.tar` |
| SHA256 | `eb590691841f7fad3fc3ad8fd5db4ccb87849fe7948e62b28ece7a4ee48cc851` |
| Extracted umu-run SHA256 | `d0005a58602041229cc467dab03dc0c0b9e8cce09a8145b16b7683244cf17804` |

The older artifact remains untouched. Managed Proton prefix-fingerprint golden
tests still pass; the changed helper identity is not used to relabel user prefixes.

### Additional first-run findings

| Priority | Finding | Disposition |
| --- | --- | --- |
| P0 | No newly confirmed security/data-loss defect | Existing boundary/rollback/identity tests retained |
| P1 | Prefix session assumed existing parents | Fixed and regression tested |
| P1 | UMU 1.4.0 mutable-endpoint version skew | Verified A/B; pin 1.4.4; external incident still needs acceptance |
| P1 | Empty-target Proton initialization | Real `wineboot -i` target; original verb retained |
| P1 | Zero exit without usable prefix masked bootstrap failure | Check exit and structure before provisioning; preserve bounded redacted runner output |
| P1 | Immediate disk check falsely rejects Wine 7.16 | After successful wineboot, shut down its wineserver under exclusive ownership to flush registry before inspection |
| P1 | Missing ELF32 interpreter misreported as a broken Wine artifact | Preflight the known managed Wine 7.16 interpreter `/lib/ld-linux.so.2`; report `host-elf32` with multilib remediation, not an integrity error |
| P1 | uinput permissions accidentally mandatory for launching any game | Preparation remains before Wine, but failure warns and only disables automation; feature starts still reject unavailable input |
| P2 | Successful Steam Runtime marker can conceal subsequent corruption when updates are disabled | Unmarked partial-state repair/retry tested; marked-corrupt state not promised repaired. No deletion of shared UMU state or automatic update of a live container |
| P2 | Vulkan ICD/physical device, 32-bit driver, audio and COW kernel capabilities incompletely preflighted | Loader/host checks below are deliberately not proof of these; external acceptance required |
| P2 | Audio inspection uses Arch-style `/usr/lib32` paths | Existing warning is non-blocking; portable/container-aware audio detection remains pending |
| P2 | ro-sessiond assumes reopenable `/dev/stderr` | Socket-backed stderr caused a harness panic; normal launcher uses pipes. Harness now uses a regular fd; production transport hardening deferred |
| P2 | Forced X11 `tauri:dev` window did not become visible on this workstation | Backend and Vite started; same debug binary/frontend with Wayland and packaged AppImage displayed correctly. No desktop/config change made |
| P3 | App data uses legacy `$HOME/.local/share/ro-launcher`, not XDG_DATA_HOME | Documented; no silent storage migration in this fix |
| P3 | Pinned Gecko HTTP helper lacks the main artifact downloader's timeouts | Deferred; installer failures are not hidden |

## Primary failure contract

`checkDependencies -> setupPrefix -> ensure selected runtime -> resolve context ->
operation/session -> UMU/Proton/Wine -> initialize -> provision -> final check` remains
the workflow. Host blockers take precedence over derived prefix issues. Artifact
download, integrity and installer errors propagate rather than becoming registry
file warnings. A non-zero initialization status cannot be accepted because files
exist. Zero with an unusable structure is a bootstrap failure, not success.

On setup/repair failure, owned processes are quiesced before incomplete managed
data is removed or a reset backup restored. Removal still requires the original
empty-managed-prefix authority; custom or unknown non-empty roots are preserved.
Cleanup failure is appended to the primary error, not substituted for it.

With the default supervisor, setup diagnostics retain the first explicit runner
`ERROR:` candidate plus eight
recent redacted lines, bounded to 1024 characters each. They are labeled diagnostic
output, not asserted to be causal Wine errors. They survive exit/stderr ordering
and quiescence; a new operation starts with a fresh window. The frontend stops on
the setup rejection and does not run a final check that overwrites its cause.
The `RO_LAUNCHER_SESSION_SUPERVISOR=0` rollback retains its existing logging;
the new bounded capture is not claimed for that disabled-supervisor path.

## Host capabilities and providers

Detect capabilities before suggesting packages. Suggestions never execute privileged
installation or change host security. A=managed/bundled, B=host, C=accidental dependency,
D=optional feature. Passing a preflight is not equivalent to playing a game.

| Capability | Required / class / provider | Detection and missing behavior |
| --- | --- | --- |
| Launcher UI and sidecars | Required, A AppImage/Tauri bundle | Assembled AppImage smoke; sidecars use bundle paths, never PATH fallback |
| AppImage mounting / host ABI | Required, B host libc/FUSE or extraction | AppImage runtime fails before Tauri if unavailable; `--appimage-extract-and-run` is the unprivileged fallback, not automatic package installation |
| Absolute valid HOME and writable data disk | Required, B user/session/filesystem | HOME preflight; actual mkdir/download write errors remain causal; no custom-parent creation or global cache deletion |
| nndsk-ro-proton and PE32/PE64 DLLs | Required for primary, A pinned catalog | Size/hash/receipt/manifest/module checks, transactional install; reject corrupt/unknown roots |
| UMU zipapp and Python modules packaged in it | Required for Proton, A pinned artifact | Versioned artifact above; no dependency on system UMU or pip |
| Python >=3.10 and stdlib ssl/lzma/bz2/zlib/socket/ctypes/fcntl | Required for UMU, B host interpreter | Sanitized absolute Python probe, 5-second deadline; actionable `host-python` blocker |
| HTTPS CA and network | Required for cold downloads, B host trust/network | Python CA probe with actual Proton CA override; bounded network errors retain stage. Loader preflight does not test network reachability. No certificates installed in Wine |
| Steam Runtime 4 | Required for this Proton, A UMU-managed under XDG_DATA_HOME or HOME | UMU performs SHA256/mtree/bootstrap; real cold and unmarked-partial/retry smokes. Do not assume directory existence means readiness |
| pressure-vessel / bubblewrap container execution | Required for Proton, A Steam Runtime + B kernel/user namespaces/host mount policy | Real UMU smoke and its stderr; no unshare/setuid/security-policy modification. Not exhaustively inferred from distro/package names |
| Vulkan x86_64 loader and GPU ICD | Required for managed graphics, B host driver exposed to container | Python CDLL checks loader only; missing loader blocks. ICD/device misconfiguration is a separate `vulkaninfo --summary`/real rendering acceptance |
| Vulkan/graphics ELF32 and i386 libraries | Required where old-WoW64 uses them, B host; PE32 DXVK is A | Managed Wine 7.16 ELF interpreter preflight and real loader/init smoke; driver/32-bit rendering acceptance remains external. Do not infer that every new-WoW64 PE32 process needs an ELF32 host driver |
| Pulse/ALSA libraries and desktop audio service | Required for sound, B host + A container libraries | Existing 32-bit library/socket checks warn, not block game launch; real 32-bit audio remains external, especially fallback |
| Wine 7.16 old-WoW64 + DXVK 2.6.2 | Optional fallback, A portable catalog + B multilib host | Actual artifact/--version/architecture and isolated init/retry tested; no UMU requirement or NTSync added |
| Winetricks script, cabextract, unzip, curl/wget | Required for provisioning, A Proton/protonfixes/container; B fallback host tools | Wine provisioning checks tools; host script or already-present primary bundle supplies winetricks. Fallback does not secretly download Proton. Already prepared Wine does not require provisioning tools to launch |
| X11/XWayland DISPLAY | Required by current Wine invocation, B desktop | Missing DISPLAY is actionable; non-empty DISPLAY is not proof of a working/authenticated server. WAYLAND_DISPLAY treatment/graphics overrides unchanged |
| COW kernel facilities / 4096-byte pages | Required by this runtime's accepted COW behavior, B kernel | UFFD WP_ASYNC/WP_UNPOPULATED, PAGEMAP_SCAN, accessible pagemap per runtime risk report; no minimum kernel falsely invented, no sysctl weakening; full capability preflight pending |
| /dev/uinput and /dev/input/event* | D automation, B device permissions; previously C universal launch prerequisite | Actual permission checks warn; failed preparation no longer aborts the game. AutoPot/AutoBuff/Spammer still fail explicitly; no sudo or automatic group changes |
| ptrace/Yama/process memory | D memory features, A default ro-sessiond + B kernel policy | Stable pid/start_time leases; supervisor tests at ptrace_scope=1. No host security weakening; unrelated clients retained |
| Other executables searched by PATH | B for explicitly selected fallback provisioning tools | Host-facing PATH strips AppImage paths. System Wine candidates and `/usr/bin` fallbacks remain legacy discovery only; no new offered runners |

## Reproducible local tests

Deterministic CI remains network-free. Build the real sidecar before its integration
tests; new tests use subprocess HOME/XDG, synthetic scripts or exclusively owned
temporary directories. No real prefix, UMU state or winetricks cache is consumed.

```sh
cargo build -p ro-sessiond
cargo test -p ro-launcher --all-features first_run_supervisor_accepts_missing_data -- --nocapture
cargo test -p ro-launcher --all-features initialization_requires_both_zero_exit -- --nocapture
npm test -- src/features/launcher/useLaunchGame.test.tsx
cargo test --workspace --all-features
```

Existing artifact regressions cover partial/corrupt files, receipts, retries, staging
collision, symlinks, failed activation/restore, cancellation and preserved unknown
destinations. Session regressions cover overlapping clients, failed cleanup, repeated
quiesce, cancellation and stable identity. These are not a substitute for a network
or assembled-package test.

Explicit real-runner initialization smoke (no games, no component provisioning):

```sh
RO_FIRST_RUN_RUNNER=/absolute/cache/nndsk-ro-proton/proton \
RO_FIRST_RUN_UMU=/absolute/cache/umu-1.4.4/umu-run \
  cargo test -p ro-launcher --all-features real_runner_clean_home_initialization_and_retry -- --ignored --nocapture

RO_FIRST_RUN_RUNNER=/absolute/cache/wine-7.16/bin/wine \
  cargo test -p ro-launcher --all-features real_runner_clean_home_initialization_and_retry -- --ignored --nocapture
```

The cache supplies identified artifacts only. Observed HOME/XDG and prefix are new.
The harness records an ownership-only manifest with **no installed components** to
exercise a second initialization safely; it never pretends a full setup occurred.
It verifies no prefix processes remain after each cycle and preserves the temp HOME.

Explicit UMU A/B/partial-state smoke, never run by normal CI:

```sh
mkdir -p .audit/first-run/manual-run
node scripts/smoke-first-run.mjs \
  --umu=/absolute/cache/umu-run --proton=/absolute/cache/proton \
  --sessiond=/absolute/workspace/target/debug/ro-sessiond \
  --output=/absolute/workspace/.audit/first-run/manual-run
```

`--target=empty` reproduces the old initialization invocation; `--seed-partial=1`
seeds an incomplete unmarked Steam Runtime in the fresh owned HOME.
`--resume-root=/tmp/ro-first-run-smoke-...` retries only a HOME with this harness's
ownership marker and the same Proton. Use a new empty output directory per run.
Logs/result.json include selected non-secret environment, executable hashes,
protocol events, actual exit, prefix checks and supervisor cleanup. Retained logs
from this investigation are under `.audit/first-run-20261008/` (ignored, not committed).

## External Arch acceptance — still required

For release acceptance, use **RO-Launcher_0.2.1_amd64.AppImage** from the
[v0.2.1 release](https://github.com/Nandem1/nndsk-ro-launcher/releases/tag/v0.2.1)
after the user publishes its draft, or update the existing 0.2.0 AppImage through
the launcher. Verify the release's `RO-Launcher_amd64.AppImage.sha256`; its digest
is generated from the CI artifact, not predicted from the workstation build.

Historical evidence: the unpublished local 0.2.0 test build was
**RO-Launcher_0.2.0-first-run-20261008_amd64.AppImage**:

- Size: 116394488 bytes.
- SHA256: `fee867c2fac26de155071ce3c0b3fba752b2576a536202335b56aaf2fbe42373`.
- No change was made to the existing installed AppImage or running game.

1. Transfer that exact file to the fresh Arch desktop user; check `sha256sum` and
   make that file executable if the transfer lost its executable bit. Run it as
   the desktop user, **not sudo**. Keep the account's normal desktop HOME/session.
2. Prefer a new ordinary desktop account without launcher/UMU state. If the test
   account already has data from the failed 0.2.0 attempt, **do not delete it**;
   keep it for the retry test. A separate fresh account is the strict cold-start
   control. Record which case was tested.
3. Do **not** manually create app data, `runtime/`, `prefixes/`, a prefix or Steam
   Runtime. Do not import a prepared prefix or install UMU/Wine just to make the
   primary path work. Leave kernel/security/graphics tweaks unchanged.
4. Start from a terminal to preserve a bounded-by-duration log:

   ```sh
   ./RO-Launcher_0.2.1_amd64.AppImage 2>&1 | tee ro-first-run.log
   ```

   If an existing AppImageLauncher opens a desktop-integration dialog, choose
   **Run once**, not integration/replacement. Alternatively prefix that command
   with `APPIMAGELAUNCHER_DISABLE=1`. This bypasses only the host desktop wrapper;
   it does not change the launcher, supervisor or game environment contracts.

5. Add the actual client EXE; leave primary nndsk-ro-proton selected, no automation
   required. Press **Preparar entorno**. Expected stages: host capability check;
   versioned nndsk/UMU download and integrity; session handshake; Steam Runtime
   versioned download/SHA256/mtree; wineboot; Gecko/graphics/vcredist/d3dx9/fonts;
   audio configuration; owned-process cleanup; **Listo**, then OpenSetup/patcher.
6. Verify outcomes per stage:

   | Stage | PASS | FAIL / preserve |
   | --- | --- | --- |
   | UI | Visible window, fresh catalog, no system-Wine prerequisite | Launch stderr; extraction fallback only if FUSE explicitly fails |
   | Host | Required capabilities pass or actionable specific blocker | Keep `host-*` message; a real missing host requirement is not a prefix corruption |
   | Artifacts | runtime/nndsk-ro-proton-0.1.0-dev.2 and runtime/umu-launcher-1.4.4 with receipts | Download/status/integrity message, not manual replacement |
   | Supervisor/prefix | Managed root created automatically beneath prefixes/; no canonicalization error | First error and Tools/Juego logs |
   | Steam Runtime | Under `${XDG_DATA_HOME:-$HOME/.local/share}/umu/steamrt4`, integrity/mtree completes | Preserve actual first Python/network/container error, not just initial CRITICAL lines |
   | Provisioning | Stage commands succeed, final components/manifest validated and Listo | Exact component name and non-zero exit retained |
   | Retry | Reattempt same failed setup safely; no unknown-data deletion, no orphan session | Preserve root and logs; never manually mkdir prefixes as workaround |
   | Game | Graphics/audio/input in the real host; launch/exit/relaunch | Real ICD/32-bit audio/game compatibility still separate from bootstrap |

7. If setup fails, copy the UI's complete error and the relevant tail of terminal
   **and Tools/Juego logs**. Terminal alone need not contain emitted Tauri UI logs.
   Keep receipts and the failed state. First diagnostic commands (read-only):

   ```sh
   python3 -c 'import sys,ssl,lzma,bz2,zlib,ctypes; print(sys.version); print(ssl.get_default_verify_paths()); print(ssl.create_default_context().cert_store_stats())'
   ls -ld "$HOME/.local/share/ro-launcher" "$HOME/.local/share/ro-launcher/prefixes" "${XDG_DATA_HOME:-$HOME/.local/share}/umu/steamrt4"
   ```

   Only if the first error is network/CDN related, also run:

   ```sh
   curl -I --max-time 20 https://repo.steampowered.com/steamrt4/images/latest-public-beta.txt
   ```

   Only for rendering failure, use `vulkaninfo --summary` if available. No initial
   full environment dump, huge journal export, certificate injection or privileged
   package operation is needed. Review logs for credentials before sharing.

Assembled-sidecar environment smoke also passed two real Proton initializations
using `RO_FIRST_RUN_SESSIOND` pointing to the AppImage-extracted ro-sessiond and
`RO_FIRST_RUN_APPDIR` to seed conflicting APPDIR/PYTHONHOME/PYTHONPATH/LD_LIBRARY_PATH/PATH.
The sidecar BuildID and non-empty .text/.rodata section comparisons match the built
release binary; whole-file hashes differ because the bundler patches ELF metadata.

Local gates passed: lint, format check, 201 frontend tests, frontend build; Rust
fmt/clippy with warnings denied, 555 unit tests plus orphan/Yama integration, seven
explicitly ignored/manual tests; release invariants and signed AppImage build.
Real Wine 7.16 and nndsk prefix init/re-init and supervisor cleanup passed. Packaged
AppImage and the debug frontend under Wayland showed the empty-HOME UI. The exact
`npm run tauri:dev` forced-X11 path started but stayed hidden on this host; it is not
reported as a full visual PASS. No new clean-host or full component/game acceptance
is claimed. User's separate Arch host, Vulkan/audio, marked-corrupt UMU state and
complete downloaded first-run remain explicitly pending.
