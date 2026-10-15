# Managed Wine 7.16 Staging/TkG amd64

The only offered fallback beside nndsk-ro-proton is the original Kron4ek
`wine-7.16-staging-tkg-amd64` artifact. This is a validated compatibility anchor,
not a claim that any runner is universally the most stable for every RO client.

- [Original release](https://github.com/Kron4ek/Wine-Builds/releases/tag/7.16).
- Archive: `wine-7.16-staging-tkg-amd64.tar.xz`, 57117116 bytes.
- SHA256: `6d9e960b93afa273c329a42116ffe7a4f0fc738f8b8b9c0242082ea3f58e971e`.
- Managed entrypoint: `<app-data>/runtime/wine-7.16-staging-tkg-amd64/bin/wine`.
- Reported version: `wine-7.16.r0.gaa2eb6ee ( TkG Staging Esync Fsync )`.
- Real old-WoW64 layout: ELF32 wine, ELF64 wine64, i386/x86_64 Unix modules.

The public archive checksum matches the publisher's release notes. All 2544
regular files and archive symlinks match the previously installed TkG artifact;
the existing local installation/prefix is not adopted or relabelled. The archive
and six essential binary/config hashes remain pinned. Readiness additionally
requires these two regular, in-root Windows modules from that exact archive:

| Anchor | Size | Mode | COFF machine | Optional-header class | SHA256 |
| --- | ---: | --- | --- | --- | --- |
| `lib/wine/i386-windows/ntdll.dll` | 878795 | `0644` | I386 `0x014c` | PE32 `0x10b` | `3a3886bf06e0f275ec24af33d580d4734e1aea61008a0a5188168e23cf0fd463` |
| `lib/wine/x86_64-windows/ntdll.dll` | 834838 | `0644` | AMD64 `0x8664` | PE32+ `0x20b` | `a97c91a3963c86f7e9cfc6bae90256ea2919aea8d8995aef816b1988fd6af43d` |

Rust parses DOS/PE/COFF and optional-header bytes; directory names cannot satisfy
the architecture gate. Hash, size and mode checks remain independently required.
Missing, malformed, swapped, symlinked or special-file anchors are not Ready,
before preparation or launch. No `file`/`objdump` subprocess is required.
An existing non-Ready runner remains preserved and refused under F-06; this gate
does not add a repair or replacement policy. Installation
uses the private snapshot, receipt, safe extractor, operation lease and
non-replacing activation already used by nndsk-ro-proton. Unknown destinations
are preserved; cancellation does not release a blocking installer's lease.

Download occurs on demand before environment preparation, game, or tool launch.
Selecting this Wine does not download UMU/Proton. DXVK 2.6.2 remains a separate
managed prefix component; the existing accelerated WebView2 recipe and graphics
overrides remain unchanged. FSYNC is selected from the artifact's declared
`fsync-unix-staging.patch` + `fsync_futex_waitv.patch`; NTSync is never enabled.

This old-WoW64 artifact requires host Wine libraries for both 32 and 64 bits,
including the 32-bit dynamic loader. It is not a self-contained container. The
`host-elf32` loader preflight remains separate from Windows PE qualification:
neither gate proves the other. The existing prefix setup also needs a winetricks
script supporting the required
verbs (bundled nndsk script if installed, otherwise the existing host dependency).
These dependencies must be reported, not silently installed through sudo or
hidden by downloading another runner. No native DLL or game/protection patch is
introduced by this catalog change.

Saved global/server paths outside the offered catalog stay selected and visible
as legacy configurations. New managed paths own distinct prefixes; per-server
overrides retain precedence. Distribution is a pinned direct download from the
original publisher, not a repackaged Wine build under our runtime project.

Real archive regression (no game launch; requires multilib). Without the
`real-runtime` feature this verifies import/reuse, the PE anchors and `--version`:

```sh
RO_LAUNCHER_TEST_WINE716_ARCHIVE=/absolute/wine-7.16-staging-tkg-amd64.tar.xz \
  cargo test -p ro-launcher wine716_real_archive_is_reusable_and_keeps_legacy_sync_and_layout \
  --locked -- --ignored --nocapture
```

F-10a's `real-runtime` feature also runs the source-built PE32 console fixture
through production Wine invocations and `ro-sessiond`, in a new subprocess HOME.
It initializes Wine's default win64 old-WoW64 prefix, writes an ownership-only
manifest with no installed components, quiesces, reuses the prefix and executes
again. Both nonce files and
exit code 42 are checked, as are AppImage environment sanitation and an empty
owned-process census. No existing launcher data or game is used. Reset and the
UMU/Proton leg remain separate qualifications. The console harness alone inherits
`WINEDLLOVERRIDES=mscoree,mshtml=` to suppress optional Mono/Gecko installer dialogs;
this is recorded in evidence and does not alter production provisioning.

```sh
cargo build -p ro-sessiond --locked
i686-w64-mingw32-gcc -nostdlib -Wl,--entry,_mainCRTStartup \
  -o /absolute/scratch/nonce.exe tests/fixtures/pe/nonce.c -lkernel32
RO_LAUNCHER_TEST_WINE716_ARCHIVE=/absolute/wine-7.16-staging-tkg-amd64.tar.xz \
RO_WINE716_TEST_PE32=/absolute/scratch/nonce.exe \
RO_WINE716_EVIDENCE=/absolute/scratch/wine716-evidence.json \
  cargo test -p ro-launcher --all-features --locked \
  wine716_real_archive_is_reusable_and_keeps_legacy_sync_and_layout \
  -- --ignored --nocapture
```

Use a working desktop `DISPLAY`, or `xvfb-run -a` before `cargo test` on a
headless host. `.github/workflows/runtime.yml` runs this Wine leg on dispatch and
weekly schedule, caches the exact archive by SHA256, and uploads evidence JSON
with the commit, runner/fixture digests, manifest, sanitized keys, stable sessiond
identity and cleanup result. Missing prerequisites/artifacts fail rather than skip.
The real archive test also removes, corrupts, substitutes and symlinks each PE
anchor in its disposable imported copy, checks non-Ready/refusal and restores it.
Synthetic tests separately reject wrong architectures with matching fixture hashes.

This gate does not qualify a Ragnarok launch, Gepard/xShield, DXVK rendering,
audio or every distribution's multilib support.

The existing HoneyRO and Sakura compatibility observations remain historical
anchors. A new game release/hash still requires its own validation; discovering
a familiar file or folder name is not proof of runner/prefix compatibility.
