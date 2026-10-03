# Private resolved loader inventory proposal

This inventory-only follow-up starts at measured source
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
one invocation and create-only stage need independent source/guard review
and an exclusive VM lease. The separately approved first attempt is recorded below.

Pure counterexamples cover public unknown observations, changed known objects,
deleted/nonpublic paths, inode replacement despite matching bytes, non-ELF data,
time bounds and mapping changes. The inherited child/canonical guard matrix
remains applicable. A later full compatibility fixture should preserve safely
typed unknown public identities before refusing, without adopting them merely
because this separate inventory observed them.

## First sealed inventory attempt: object identity refused

Exact source `4337950172cb1dcc5da8df082a28af3d15e661d8` passed 406 source
tests (two existing skips), JS/QML contracts, seven new pure counterexamples and
20 inherited guards. The frozen launcher SHA256 was
`cacde22fe43b0ab0f246f80f6a6314c92579db2bbf165dded5faec46f659b767`;
the reviewed outer guard was
`5293025bcf9d25ed2e6bfc7e1157f50e39ede8fa368e0733f43345708fa75282`.
Fresh current-epoch/package ELF/subordinate-map preflight passed before staging.

One approved inventory-only invocation returned **NONPASS** with
`mapped_object_identity`, before either complete mapping snapshot was retained.
No broker/core or DNS setter ran; no observed object was allowlisted. The grouped
identity predicate did not retain the failing pathname or actual predicate
values, so this receipt cannot attribute failure to ownership, permissions,
size, device or inode. Do not infer a specific loader or filesystem explanation.

The outer guard preserved the canonical boot/process epoch, all eight original
categories and every IPv4/IPv6 non-timer field. Only decreasing numeric address
lifetimes differed. Independent read-only checks verified the private root was
empty, fixed fixture/private-bus/subordinate-resolver processes were absent, and
canonical MainPID938 was unchanged. No cleanup uncertainty was reported.

Complete private staging remains retained, with identical guest/host archive
SHA256 `0a8b3c6a18ce602e95c53b34c1b4954b2b989dcbe256143dcbf7d32cd32860b9`.
The lease was returned without cleanup, retry, policy relaxation or canonical
mutation. Further source diagnosis should retain safely typed public refusal
facts before rejection; any additional actual invocation needs separate review
and an exclusive lease. This attempt establishes neither a usable inventory nor
actual resolved compatibility.
# Public refusal diagnostics (measured source `90cca2a`)

The next diagnostic retains one bounded public `/usr/lib` or `/usr/bin` pathname,
expected maps device/inode, and original-open-FD device/inode/uid/gid/mode/nlink/size
when `mapped_object_identity` refuses. Individual boolean predicates explain the
grouped refusal without changing any identity acceptance condition. This record
is explicitly **not** proof of loaded ELF identity, reads no content, contains no
digest, and cannot admit an object to the allowlist. Private, malformed, deleted,
traversal and overlong paths are refused without opening or recording them.

`vm-guard-diagnostic2.sh` is a separate immutable proposal using fresh create-only
stage `t3-resolved-loader-inventory-review-2` and the new exact source hash. The
original `vm-guard.sh` and first-attempt archive remain unchanged. The second
invocation below received separate full review and an exclusive lease.

The one approved diagnostic invocation of source
`90cca2ace91fc3434c19627c45fa3e85cd82aa5a` retained **NONPASS**, reason
`mapped_object_identity`. For public path `/usr/bin/dbus-daemon`, maps identified
device **29**, inode **26297**; the original opened FD identified device **31**,
inode **26297**, uid/gid **65534**, mode **33261** (regular 0755), nlink **1**,
size **199176**. Only `device_matches_maps` was false; all six other predicates
were true. No content was read or digest recorded for the refused object. This
establishes the mismatched field, **not** its filesystem mechanism or loaded ELF
identity. No identity predicate was relaxed and no allowlist was adopted.

Broker/core execution and DNS mutations remained false. Canonical epoch, all
eight baseline categories and all IPv4/IPv6 non-timer fields were preserved;
only confirmed decreasing address lifetimes differed. Independent read-only
quiescence verified the private root empty/non-symlink, fixed artifact/launcher/
bus/subordinate-resolver processes absent, and canonical PID 938 unchanged.
No retry or cleanup occurred; the exclusive lease was returned.

Receipt, strict snapshots and private logs are retained outside Git in
`t3-loader-inventory-90cca2a-nonpass.tar.gz`, host directory
`/home/kk/.cache/t3-real-resolved-build.XVxwu8AF/` and guest directory
`/home/kdk_vm/.cache/`, matching SHA-256
`8b7777f11263f5657beb9ebcaee845256a9eaac88d2a02026d4e6808c3aead25`.
Measured source SHA-256 is
`974284868219f32efe2f983b857ee8ed722db6360d4a62986d3fac3b896c73e0`;
wrapper SHA-256 is
`679e4757c65803d275a3ddfa5ed766e39cbbc1e607dde32ae53fec7f0d9c2b97`.
The source gate passed 409 Python tests (2 skips) and JS/QML checks. Neither this
diagnostic nor either earlier refusal is compatibility or installed acceptance.
