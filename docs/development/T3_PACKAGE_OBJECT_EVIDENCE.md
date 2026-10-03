# T3 provisional package-object evidence

Inactive research on sealed frontier input
`22b3a74ea0848f27b5cbf352f11abab31051f273` (#564). This is a new independent
branch, not a change to that accepted composition or an adopted product package.

The isolated child module `conditional_package_evidence.rs` opens only
`/var/lib/omavless-close-research-fixture/mihomo` and `source-receipt.json`.
These are **provisional fixture locations**, not installation or runtime search
paths. Nothing creates them, executes a privileged helper, installs a package,
changes selection, or supplies a normal close permit. The existing ManagedPair,
package pins, ABI transport, owner admission and MissingAttestation behavior are
unchanged. Python remains a frozen reference, not a runtime dependency.

## Actual objects and limits

All ancestors are opened component-by-component without following symlinks,
retained as directory descriptors, and checked for root ownership and absence
of group/world write or special bits. Both regular files must be root-owned,
single-link, nonempty and bounded; core mode is 0755 and receipt mode is 0644.
The reader retains descriptors plus inode/device, mode, owner, size and
change/modification timestamps. Rechecking compares named objects through the
same ancestor chain and refuses replacement, unlink, mutation or permission
drift. It never repairs files or falls back to another path.

Receipt JSON uses duplicate-rejecting strict structs, including the nested
build object. Source commits, all three production patch identities, ABI 1,
build flags and architecture must match the explicitly provisional schema.
The core hash is computed from the same retained object. Actual ELF64
little-endian machine/header checks distinguish x86_64 and aarch64; this is
not a complete executable-format validator or proof that build claims are true.

The result binds to the exact nonreusable Session Arc and its unreaped
parent-owned child. The captured `/proc` executable and source descriptor must
both identify the same root-owned core object. An interpreter plus a different
script, matching bytes in a different inode, a replacement process, a cancelled
session or a changed source path cannot satisfy that relation. Any failed
recheck permanently poisons this evidence object. There is no conversion to
CandidateEffectPermit, no public constructor, IPC, or controller write.

The receipt is a root-stated **source composition** claim corroborated by object
identity, hash and ELF architecture. It does not independently prove compiler
execution, immutable distribution authenticity or a matched broker. The fixed
`managed-dns-source-composition-only` label is deliberately not an installed
package-pair identity. #555 built no broker and skipped Rust/Go DNS interop;
neither is relabelled PASS. Trusted receipt issuance, actual broker object/wire
pairing, both architecture artifact selection, upgrades/revocation and normal
product adoption remain separate owner-reviewed gates.

## Provisional research receipt

This sample describes the reviewed #555 x86_64 developer core, not a released
asset. An aarch64 fixture needs independently recorded actual build evidence;
changing the architecture labels on this hash is not acceptance.

```json
{
  "schema": "omavless-close-research-only-v1",
  "architecture": "x86_64",
  "abi": 1,
  "pair": "managed-dns-source-composition-only",
  "source": "8d6877c49d400aa723e01032c62a95d6559fecc3",
  "dns_source": "c4e800425243c1b02165f82153e4bf418fe465e6",
  "mihomo_commit": "ab405bad5beeeac8b003bb01f60f134f6df54471",
  "sing_tun_commit": "b50ae28a1409c7bce8e96e6c6966cf57d8ace754",
  "conditional_patch_sha256": "0858827e1af00c3ed3196f021b0dbc76ce34a8de7aa7130d7085614d149acc8f",
  "dns_patch_sha256": "d5ebe9d6b37f6b76599fc3c2dd25adbfb774ca0121beeb79c5768a9a08d7ff37",
  "tun_patch_sha256": "2556c82aafbeb598a817d43042cf2069c6f209433c7a506395df581b4e31e2ab",
  "build": {
    "go_version": "go1.27.0-X:nodwarf5",
    "tags": "with_gvisor",
    "cgo": false,
    "buildvcs": false,
    "dependency_mode": "vendor",
    "goos": "linux",
    "goarch": "amd64"
  },
  "core_sha256": "3b1da75d3c9fd8440216f9c256c6c59da812faae88debc936f3c72fef9724544"
}
```

## Gates

Ordinary tests exercise strict parser/composition rejection, actual user-owned
file/ancestor rejection, nonblocking FIFO/symlink/directory rejection and
retained-object substitution/hardlink/mutation primitives. Synthetic ELF header
cases exercise both machine classifications. Passing primitive tests on
user-owned files is **not** positive root-owned package evidence.

The ignored `root_owned_package_objects_bind_actual_parent_owned_core_in_dev_vm`
test additionally requires `OMAVLESS_CLOSE_OBJECTS_VM=1`, an unprivileged runner,
separately provisioned exact root-owned fixture objects and an explicit exclusive
Dev-VM lease. It starts one actual parent-owned core with no TUN, DNS or TCP
listener, captures and rechecks object evidence, then verifies cancellation
refusal and owned cleanup. It contains no provisioning/root helper and performs
no conditional close. This real-root positive is **pending**, not silently
substituted by ordinary user-owned fixtures. Root-owned replacement/ancestor,
wrong-child and wrong-architecture VM negatives remain part of that gate.

Exact source/static and any later immutable VM results belong on the Draft PR.
No release, main/RC merge, package installation or normal activation follows
from this object-reader research.
