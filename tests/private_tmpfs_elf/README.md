# Private tmpfs ELF bridge

This separate branch starts at inventory report `3f70423`; the original fixture,
all original refusals and fixed-FD diagnostic report #597 remain unchanged.
The bridge follows the separately reviewed design: retain strict maps device/
inode matching by mapping exact pinned copies from private read-only tmpfs.
It does not reinterpret Btrfs device numbers or admit observed dependencies.

The exact frozen manifest contains 16 logical records and 15 unique resolved
targets; two loader aliases resolve to the same file. The source embeds that
complete table and rejects any mutation. All original aliases must remain exact.
The only imported helper is the SHA-pinned original containment Python source;
no Rust production code or packaged artifact is rebuilt or modified.

After the existing private namespace/root/proc checks and config masks, the
bridge creates `/elf-copy-store` on a fresh bounded tmpfs. Installed source FDs
must be unmapped-root UID/GID 65534, regular, exact mode, within per-file and
aggregate bounds, stable through bounded hashing/copying, and match each pinned
digest. Exclusive copied files must be namespace-root UID/GID 0, single-link,
exact size/mode/content. Path replacement, short I/O or uncertainty refuses.
The copy supervisor has a 32 MiB per-file hard limit; each daemon restores the
original 4 MiB hard log limit before exec. Core dumps remain disabled and the
128-FD hard limit is unchanged; the private store/aggregate cap is 128 MiB.

All 15 copies finish before any overlay bind or daemon launch. The entire copy
tmpfs is remounted read-only, then each fixed copy is bound read-only at its
resolved private `/usr` target with nosuid/nodev. Both mount and superblock RO
flags and tmpfs type are required. No writable copy FD may remain. Read-only
copy FDs are retained through observation; final targets are tied to these FDs,
copy receipts and pinned hashes. No host `/usr`, canonical service or network
is modified. All helper commands retain frozen unknown-child quarantine rules.

Only private dbus and resolved are launched. Existing isolated loopback/dummy
bootstrap remains; no broker/core, TUN lease or DNS setter runs. Each observed
file mapping must name a known copied target and match the retained copy device/
inode **before hashing**, then the opened target, read-only mount policy and
pinned digest. Copies have a separate UID 0 provenance rule; the installed UID
65534 predicate is not relaxed. Unknown public mappings retain only safe path
diagnostics and refuse; they are never copied or allowlisted automatically.

Two complete mapping passes and final credential/copy/mount checks must agree.
Even success means only observed mappings of private copies of pinned packaged
bytes. It is not installed, broker/core compatibility or future-dlopen acceptance.
Unexpected mapping pathname representation refuses rather than adapting.

The new immutable outer guard pins this source and the original containment/
manifest, preserves canonical epoch, all eight baseline categories and complete
IPv4/IPv6 non-timer state, checks fixed process/root quiescence, and bounds one
whole invocation. No execution is authorized until complete source/guard review,
pure/full gates, sealed hashes and an explicit exclusive VM lease. Any first
NONPASS stops; retain receipts/archive before separately scoped cleanup.

## One sealed inventory attempt: unknown mapping refused

Measured source `eef9151f2df0de3e4da59e2346bc807c9c6dac08` passed 421 Python
tests (2 skips), JS/QML and 12 focused pure tests. Exact probe SHA-256:
`8d179127e1f03861d35d87caf888b775e7cc6c47ceb44b2cfa99e57517a787a7`;
wrapper SHA-256:
`0e89694b781fb0f7828d8e5de2960b7ad3db126fab755539213f2f82b90260bd`.

After separate full review and one exclusive lease, the single invocation
returned **NONPASS** during initial mapping inventory. Its typed
`unknown-public-mapping-v1` refusal identifies
`/usr/lib/libbrotlicommon.so.1.2.0`; the generic exception reason remains
`mapped_object_identity`. The path was not measured, copied or admitted.
No complete initial or final mapping snapshot was retained.

All 15 copy receipts are retained, establishing completion of source/copy hash,
whole-store read-only, per-target bind and prelaunch checks before private bus
and resolved startup. Both private diagnostic streams exist; no cleanup reason
was reported. This is finite-phase progress, not complete mapped-closure proof.
Broker/core execution and DNS mutations remained false.

The canonical epoch, all eight baseline categories and all IPv4/IPv6 non-timer
fields were preserved; only confirmed decreasing address lifetimes differed.
Independent read-only checks found the private root empty/non-symlink, fixed
artifact/launcher/bus/subordinate-resolver processes absent, and canonical PID
938 unchanged. The lease was returned without retry, adaptation or cleanup.

The complete private stage remains in
`t3-tmpfs-elf-eef9151-nonpass.tar.gz`, host directory
`/home/kk/.cache/t3-real-resolved-build.XVxwu8AF/` and guest `/home/kdk_vm/.cache/`,
matching SHA-256
`6ad0b4b500174b047788d80ae49287eddee972d075e481b78cc27c06ea20301b`.
Any new dependency needs independent provenance and a separately reviewed
immutable manifest; this observation grants no allowlist or execution authority.
