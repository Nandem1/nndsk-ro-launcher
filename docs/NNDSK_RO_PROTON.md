# nndsk-ro-proton integration

The product default is `nndsk-ro-proton 0.1.0-dev.2`. Wine 7.16 old-WoW64 remains
the separate, hash-specific SakuraRO compatibility profile with managed DXVK
2.6.2. A nonempty server runner override still takes precedence over the global
setting. Existing user selections are not silently changed by discovery.

Source: [Nandem1/nndsk-ro-proton](https://github.com/Nandem1/nndsk-ro-proton), native
fork of Proton-CachyOS, maintained recipe on the default `ro-runtime` branch.
The three patches remain separate general Wine API changes; no game/protection
files are modified. The launch topology change belongs to this launcher, not the
Wine patchset.

## Identified published package

- Catalog ID: `nndsk-ro-proton-0.1.0-dev.2`
- Artifact: `nndsk-ro-proton-0.1.0-dev.2-linux-x86_64.tar.zst`
- Bytes: `399605131`
- SHA-256: `208e6d4735c9a25c4f14b6465712a1069ce53aaf4d5a12eb5a3967abe962ead4`
- Binary source snapshot: `93bda981a6e8b7bdadb46fa3497f84df8dfc6fe2`
- Base Proton: `3edf6fbb8af940de5c65b9dd0fbf366b51a218a8`
- Base Wine: `b5f2dc7b5906ef864f83df8fef94c9f539eaad2d`

This is a notice/metadata-only revision of the accepted local dev.1 build. All
existing runtime code/data and the six module hashes are identical; patchset
revision remains 1. This is not an upstream-ready or universal runtime.
COW tracking requires the recorded Linux userfaultfd/pagemap features and
4096-byte pages. Software-KSP persistence uses Wine/DPAPI in the prefix; it is not
TPM storage or Windows-equivalent isolation.

## Automatic download and optional offline import

The versioned [runtime prerelease](https://github.com/Nandem1/nndsk-ro-proton/releases/tag/v0.1.0-dev.2)
provides the binary, sources/notices, manifest, preservation inventory and
checksums. The launcher uses an exact versioned HTTPS URL, expected size and
SHA-256; it does not follow `latest` or silently trust a downloaded manifest.
It downloads on demand when preparing the selected runtime's environment.
The user's final GUI installation/game test remains separate from packaging tests.

Offline installation remains available using **Importar y usar nndsk-ro-proton**
or the CLI:

```sh
/absolute/path/to/ro-launcher --import-runtime /absolute/path/to/nndsk-ro-proton-0.1.0-dev.2-linux-x86_64.tar.zst
```

The CLI only installs the runtime; it does not change preferences, launch Wine,
create a prefix, or start a game. The UI import-and-use action explicitly updates
the global preference without changing server overrides.

Both paths use the same pinned size/hash check, private verified archive snapshot,
safe zstd/tar extraction, manifest/six-module verification and non-replacing
transactional installation. Public package receipts identify the pinned HTTPS
source; original dev.1 receipts retain their local source identity. Unknown
or invalid existing destinations are preserved; successful repeated imports reuse
the existing runtime. Downloads use an exclusively created private directory;
the blocking installer owns cleanup and the operation lease even if its async
waiter is cancelled. Unknown existing roots are rejected before download, not
backed up/deleted to "repair" them.

The original dev.1 and earlier CachyOS descriptors remain available for existing
installations, not as the new default. Existing preferences and per-server
overrides stay unchanged. Selecting dev.2 explicitly creates its own prefix;
it never relabels, deletes or migrates the dev.1 prefix.

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
