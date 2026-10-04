# Fixed read-only filesystem diagnostic

This separate diagnostic follows the preserved `90cca2a` refusal: a mapping of
`/usr/bin/dbus-daemon` reported device 29/inode 26297 while the original opened
FD reported device 31/inode 26297. It does not modify that guard or admit any ELF.

The sole target is `/usr/bin/dbus-daemon`, opened read-only/no-follow/nonblocking.
Its exact prior public metadata is required before a single 4096-byte
`PROT_READ | MAP_PRIVATE` mapping at offset zero. The returned address is used
only to select its exact range in `/proc/self/maps`; no mapped bytes are read,
written, executed or hashed. The mapping is removed once and the FD closed once.
Unknown cleanup is NONPASS, never retried. No subprocess or daemon is launched
by the diagnostic; no namespaces, capabilities, network or service changes occur.

The fixed Linux x86_64/glibc ABI is checked before three narrowly typed libc
calls: `fstatfs`, `mmap`, `munmap`. Other platforms/page sizes refuse. The retained
metadata is kernel release, original FD fields, filesystem magic, numeric mount
and parent IDs/device fields, allowlisted filesystem-type label, and selected
mapping identity. Mount root, mountpoint, source, options, arbitrary filesystem
labels and virtual addresses are not retained. Bounded proc reads, two identical
mapping/mount observations and stable FD metadata are required.

`OBSERVED_READONLY_METADATA` is a diagnostic outcome even if device equality is
false. It is not proof of another process's loaded object, content identity,
filesystem mechanism, allowlist admission or compatibility acceptance. The
guest kernel version and filesystem evidence must be evaluated against matching
kernel source before attributing the earlier mismatch to Btrfs.

The separate outer wrapper pins the new diagnostic SHA and unchanged canonical
epoch, preserves all eight baseline categories and IPv4/IPv6 non-timer fields,
and checks the fixed diagnostic process is absent. A ten-second whole-command
timeout bounds the attempt. Fresh create-only staging, full source/wrapper review,
explicit exclusive lease, retained archive and independent quiescence are required
before/after any actual invocation. The separately approved measurement follows.

Primary references: [fstatfs](https://man7.org/linux/man-pages/man2/statfs.2.html),
[mmap](https://man7.org/linux/man-pages/man2/mmap.2.html),
[kernel maps device](https://raw.githubusercontent.com/torvalds/linux/master/fs/proc/task_mmu.c),
[Btrfs getattr](https://raw.githubusercontent.com/torvalds/linux/master/fs/btrfs/inode.c).
The `master` references explain a hypothesis, not the unmeasured guest build.

## One sealed read-only measurement

Measured source `ba053ca8f6a399b9ce4772efc607f56aeefed85f`, probe SHA-256
`8c8c60993aded1faa9c15a48d0af959a109bbf2f6f628bd0169bfb36872d6c90`,
wrapper SHA-256
`09e1f90e24fd15c4d76b0e4580e4ecb45d0ea734bfdeed7f9508aad2bcdbf5d3`.
The full source gate passed 418 Python tests (2 skips), JS/QML and nine focused
pure tests. One separately reviewed invocation returned
`OBSERVED_READONLY_METADATA`; its strict outer wrapper exited zero.

The guest reports kernel `7.2.5-3-omarchy`. Its fixed dbus FD reports device 31,
inode 26297, root:root, regular 0755, nlink 1, size 199176. A read-only private
4096-byte self-mapping made from that **same retained FD** reports device 29 and
the same inode. No mapped bytes were accessed. `fstatfs` reports magic
2435016766 (`0x9123683e`, Btrfs); the selected mountinfo row reports Btrfs,
device 0:29, mount ID 32, parent ID 2. Thus the same-FD discrepancy is reproduced
without daemons or pathname-to-another-process identity inference.

Canonical epoch, all eight baseline categories and all IPv4/IPv6 fields were
preserved. Independent fixed diagnostic-process absence and canonical PID/boot
checks passed. No retry or cleanup occurred; the exclusive lease was returned.
Full private receipt and strict snapshots are retained outside Git in
`t3-fixed-fd-ba053ca-observed.tar.gz`, host directory
`/home/kk/.cache/t3-real-resolved-build.XVxwu8AF/` and guest `/home/kdk_vm/.cache/`,
matching SHA-256
`9c6f90bca42764f3e38609688577a3767f91b6fdf0e27cda92f2379466dfafbe`.
Loaded ELF identity, allowlist adoption and compatibility acceptance remain false.

Matching upstream [Linux v7.2.5 maps code](https://raw.githubusercontent.com/gregkh/linux/v7.2.5/fs/proc/task_mmu.c)
reports the inode superblock device; [v7.2.5 Btrfs getattr](https://raw.githubusercontent.com/gregkh/linux/v7.2.5/fs/btrfs/inode.c)
substitutes the root anonymous device. The version-matching
[Omarchy package recipe](https://github.com/omacom/omarchy-pkgs/blob/7b11c97603dd9d751d803746560ee51640709725/pkgbuilds/linux-omarchy/PKGBUILD)
uses 7.2.5-3 but includes local Btrfs/MM patches. These sources support the
mechanism hypothesis; exact guest build/patch provenance is not established here.
The observed discrepancy does not justify weakening any loaded-object check.
