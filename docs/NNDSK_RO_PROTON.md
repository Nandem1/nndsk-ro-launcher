# nndsk-ro-proton integration

The product default is `nndsk-ro-proton 0.1.0-dev.1`. Wine 7.16 old-WoW64 remains
the separate, hash-specific SakuraRO compatibility profile with managed DXVK
2.6.2. A nonempty server runner override still takes precedence over the global
setting. Existing user selections are not silently changed by discovery.

Source: [Nandem1/nndsk-ro-proton](https://github.com/Nandem1/nndsk-ro-proton), native
fork of Proton-CachyOS, maintained recipe on the default `ro-runtime` branch.
The three patches remain separate general Wine API changes; no game/protection
files are modified. The launch topology change belongs to this launcher, not the
Wine patchset.

## Identified preservation build

- Catalog ID: `nndsk-ro-proton-0.1.0-dev.1`
- Artifact: `nndsk-ro-proton-0.1.0-dev.1-linux-x86_64.tar.zst`
- Bytes: `398862860`
- SHA-256: `75b0c916ccf6e7fcd64ed2afe576c2c0bc6a75ef63a8d74ad6dd9bee629d4d9f`
- Binary source snapshot: `93bda981a6e8b7bdadb46fa3497f84df8dfc6fe2`
- Base Proton: `3edf6fbb8af940de5c65b9dd0fbf366b51a218a8`
- Base Wine: `b5f2dc7b5906ef864f83df8fef94c9f539eaad2d`

This is the accepted local preservation build, not an upstream-ready or universal
runtime. COW tracking requires the recorded Linux userfaultfd/pagemap features and
4096-byte pages. Software-KSP persistence uses Wine/DPAPI in the prefix; it is not
TPM storage or Windows-equivalent isolation.

## Local installation while public redistribution is pending

The inherited component source/license audit is incomplete. No fictional release
URL or automatic download is exposed. Import the identified local archive using
the UI's **Importar y usar nndsk-ro-proton**, or deploy it without starting the UI:

```sh
/absolute/path/to/ro-launcher --import-runtime /absolute/path/to/nndsk-ro-proton-0.1.0-dev.1-linux-x86_64.tar.zst
```

The CLI only installs the runtime; it does not change preferences, launch Wine,
create a prefix, or start a game. The UI import-and-use action explicitly updates
the global preference without changing server overrides.

Both paths use the same pinned size/hash check, private verified archive snapshot,
safe zstd/tar extraction, manifest/six-module verification and non-replacing
transactional installation. Receipts identify a local source snapshot. Unknown
or invalid existing destinations are preserved; successful repeated imports reuse
the existing runtime. The earlier CachyOS descriptor and receipt identities remain
available for existing installations, not as the new product default.

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
runtime/prefix. Publication/automatic download requires closing the source audit
and authorizing a binary release separately.
