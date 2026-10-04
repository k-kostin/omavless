# Fixed read-only filesystem diagnostic (source-only proposal)

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
before/after any actual invocation. No VM access has occurred for this proposal.

Primary references: [fstatfs](https://man7.org/linux/man-pages/man2/statfs.2.html),
[mmap](https://man7.org/linux/man-pages/man2/mmap.2.html),
[kernel maps device](https://raw.githubusercontent.com/torvalds/linux/master/fs/proc/task_mmu.c),
[Btrfs getattr](https://raw.githubusercontent.com/torvalds/linux/master/fs/btrfs/inode.c).
The `master` references explain a hypothesis, not the unmeasured guest build.
