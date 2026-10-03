# Private resolved loader inventory proposal

This source-only follow-up starts at measured source
`c51af8b4cc76434af261922050ba9000080dc0b8`. The retained compatibility attempt in
[Draft #595](https://github.com/k-kostin/omavless/pull/595) measured zero cases and
refused an unapproved loaded ELF during private bus/resolver startup. Its receipt
did not retain the unknown pathname. That outcome and the original fixture are
unchanged; no additional execution is implied by this proposal.

`probe.py` loads the exact SHA256-pinned containment source from that attempt. It
reuses its namespace/UID maps, read-only mounts, masked configuration, private
bus policy, resolver capability verification, nonreaping child observations,
exact final raw reaping and permanent uncertainty quarantine. The inherited
bootstrap creates only isolated loopback/dummy-link configuration; its evidence
is explicitly direct-child status until private procfs topology is established.
The inventory source launches only the packaged private bus and resolved. It has
no broker/core launcher, typed DNS setter, systemd manager, TUN lease or external
network peer. No helper ELF build or new dependency is needed.

After readiness, it measures each file-backed `/usr/lib` or `/usr/bin` mapping
through an opened non-symlink FD tied to the observed device/inode. It checks
ELF magic, root-owned package permissions as mapped into the namespace, bounded
size/time, and before/after file identity. Deleted paths, nonpublic paths,
conflicting mapped identities, changed files/maps, unknown child state and
changes to already pinned dependencies refuse. Two complete mapping passes must
match. Anonymous kernel mappings are excluded; this is observed file-backed
ELF inventory, not a claim about every possible future dlopen or code page.

New public ELF identities may be retained only as observations with
`matches_previously_reviewed_loader_object: false`. They do not enter the original
allowlist. Original pinned objects must still match their reviewed hashes.
Private diagnostics remain only in the new private receipt; public results use
fixed classes. Compatibility, DNS restoration, installed acceptance and
allowlist adoption remain false even when inventory succeeds.

The separate `vm-guard.sh` keeps the reviewed current canonical boot/PID/starttime/
executable checks, eight preservation categories and full IPv4/IPv6 comparator.
It adds fixed private bus/subordinate-resolver absence checks and does not let a
missing output directory prevent the remaining read-only process checks. Its
one invocation and create-only stage still need independent source/guard review
and an exclusive VM lease. No VM access or execution has occurred for this slice.

Pure counterexamples cover public unknown observations, changed known objects,
deleted/nonpublic paths, inode replacement despite matching bytes, non-ELF data,
time bounds and mapping changes. The inherited child/canonical guard matrix
remains applicable. A later full compatibility fixture should preserve safely
typed unknown public identities before refusing, without adopting them merely
because this separate inventory observed them.
