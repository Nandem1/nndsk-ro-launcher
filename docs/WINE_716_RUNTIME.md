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
and six essential binary/config hashes are pinned in the catalog. Installation
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
existing prefix setup also needs a winetricks script supporting the required
verbs (bundled nndsk script if installed, otherwise the existing host dependency).
These dependencies must be reported, not silently installed through sudo or
hidden by downloading another runner. No native DLL or game/protection patch is
introduced by this catalog change.

Saved global/server paths outside the offered catalog stay selected and visible
as legacy configurations. New managed paths own distinct prefixes; per-server
overrides retain precedence. Distribution is a pinned direct download from the
original publisher, not a repackaged Wine build under our runtime project.

Artifact regression (no game launch):

```sh
RO_LAUNCHER_TEST_WINE716_ARCHIVE=/absolute/wine-7.16-staging-tkg-amd64.tar.xz \
  cargo test -p ro-launcher wine716_real_archive_is_reusable_and_keeps_legacy_sync_and_layout \
  --all-features -- --ignored --nocapture
```

The existing HoneyRO and Sakura compatibility observations remain historical
anchors. A new game release/hash still requires its own validation; discovering
a familiar file or folder name is not proof of runner/prefix compatibility.
