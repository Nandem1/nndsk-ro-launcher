# nndsk-ro-proton integration

The product default is `nndsk-ro-proton 0.1.0-dev.3`. Wine 7.16 Staging/TkG amd64 old-WoW64 remains
the separate, hash-specific SakuraRO compatibility profile with managed DXVK
2.6.2. A nonempty server runner override still takes precedence over the global
setting. Existing user selections are not silently changed by discovery.

Source: [Nandem1/nndsk-ro-proton](https://github.com/Nandem1/nndsk-ro-proton), native
fork of Proton-CachyOS, maintained recipe on the default `ro-runtime` branch.
The original three Wine API patches and their preservation lock stay intact.
dev.3 adds a fourth, scoped ephemeral public-P256 import patch and a runtime C-volume
alias helper. No game/protection files or signature checks are modified. The DOS
launch topology change belongs to this launcher, not the Wine patchset.

## Identified published package

- Catalog ID: `nndsk-ro-proton-0.1.0-dev.3`
- Artifact: `nndsk-ro-proton-0.1.0-dev.3-linux-x86_64.tar.zst`
- Bytes: `399666743`
- SHA-256: `441d2d237649a7a84f7074e94173ded91ee3a593f29156531b6427e3dd631a14`
- Source recipe/snapshot: `5855cfe862b1bc58f8bec75c51669153934116ca`
- Base Proton: `3edf6fbb8af940de5c65b9dd0fbf366b51a218a8`
- Base Wine: `b5f2dc7b5906ef864f83df8fef94c9f539eaad2d`

Patchset revision is 2. Four crypt32/Unix ntdll modules remain byte-identical;
PE32/PE64 ncrypt change and three compatibility assets are pinned separately
(`proton`, `nndsk_volume_aliases.py`, native `c-volume-query.exe`). The helper queries
Wine's actual C GUID and aliases only the existing prefix-local C mapping, without
guessed GUIDs, elevated permissions or replacing collisions. The real HoneyRO
installer completed with the refined ncrypt fix and equivalent diagnostic mapping;
the assembled automatic helper passed both architectures' space-query probes.
No new game/Gepard stability test was requested. The historical dev.1/dev.2 game
evidence does not mark dev.3 as validated. This is not an upstream-ready or universal runtime.
COW tracking requires the recorded Linux userfaultfd/pagemap features and
4096-byte pages. Software-KSP persistence uses Wine/DPAPI in the prefix; it is not
TPM storage or Windows-equivalent isolation.

## Automatic download and optional offline import

New runner selections offer exactly two products: this runtime and the pinned
[Wine 7.16 Staging/TkG amd64](WINE_716_RUNTIME.md). System Wine, arbitrary Steam
Proton and previous managed versions are not advertised as additional choices.
Saved out-of-catalog paths remain visible as preserved legacy selections, without
silently migrating their prefixes. Internal UMU/DXVK and historical artifact
descriptors are dependencies/compatibility records, not additional offered runners.

The versioned [runtime prerelease](https://github.com/Nandem1/nndsk-ro-proton/releases/tag/v0.1.0-dev.3)
provides the binary, sources/notices, manifest, preservation inventory and
checksums. The launcher uses an exact versioned HTTPS URL, expected size and
SHA-256; it does not follow `latest` or silently trust a downloaded manifest.
It downloads on demand when preparing the selected runtime's environment.
Publishing Proton alone cannot update the catalog inside an older installed
launcher: it needs a launcher build containing this descriptor. Saved dev.2
selections still resolve to dev.2; choosing dev.3 is explicit, not an in-place update.
The user's final GUI installation/game test remains separate from packaging tests.

Managed UMU is pinned independently to `1.4.4`; Python 3.10+ remains a host
requirement, not a bundled interpreter. Prefix initialization uses
`waitforexitandrun wineboot -i`, not an empty target or fresh-prefix `runinprefix`.
See [first-run contract, evidence and clean-Arch acceptance](FIRST_RUN.md).

Offline installation remains available using **Importar y usar nndsk-ro-proton**
or the CLI:

```sh
/absolute/path/to/ro-launcher --import-runtime /absolute/path/to/nndsk-ro-proton-0.1.0-dev.3-linux-x86_64.tar.zst
```

The CLI only installs the runtime; it does not change preferences, launch Wine,
create a prefix, or start a game. The UI import-and-use action explicitly updates
the global preference without changing server overrides.

Both paths use the same pinned size/hash check, private verified archive snapshot,
safe zstd/tar extraction, version-specific manifest/six-module/helper verification and non-replacing
transactional installation. Public package receipts identify the pinned HTTPS
source; original dev.1 receipts retain their local source identity. Unknown
or invalid existing destinations are preserved; successful repeated imports reuse
the existing runtime. Downloads use an exclusively created private directory;
the blocking installer owns cleanup and the operation lease even if its async
waiter is cancelled. Unknown existing roots are rejected before download, not
backed up/deleted to "repair" them.

The original dev.1, public dev.2 and earlier CachyOS descriptors remain available for existing
installations, not as the new default. Existing preferences and per-server
overrides stay unchanged. The dev.2 descriptor retains its original checksum, HTTPS
source and recipe revision 1, so existing receipts/fingerprints do not change.
Selecting dev.3 explicitly creates its own prefix; it never relabels, deletes or
migrates a dev.1/dev.2 prefix. A client installed inside an earlier prefix keeps its
installation binding and requires a new installation to move to another pairing.

Each server/runtime pairing obtains a separate managed prefix. Do not copy or
relabel a prefix belonging to another runner. The launcher prepares a new owned
environment when needed and retains the previous environment.

## Invocation and lifecycle

For Proton game/patcher/setup execution, the launcher derives the executable's DOS
path from actual target-prefix drive mappings and verifies its round-trip to the
same canonical file. No Wine-visible helper is hidden or impersonated. CWD,
remaining arguments and graphics environment are unchanged. Ambiguous/unmapped
paths fail explicitly rather than falling back to the Unix helper topology.
Wine 7.16 arguments, sync rules and WebView2 recipe remain unchanged.

The session supervisor, stable `(pid, start_time)` identities and multi-client
ownership remain enabled. Importing a runtime does not stop games using another
runtime/prefix. The runtime's artifact-specific distribution review documents
inherited-source limitations separately from observed game compatibility; no
full upstream compilation reproducibility or complete SPDX SBOM is claimed.
