# Private tmpfs ELF bridge — source-only, not executed

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
