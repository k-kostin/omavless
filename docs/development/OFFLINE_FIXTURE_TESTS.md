# HOME-backed offline source-test fixtures

Repeated source-only runs encountered two unrelated environment boundaries:
long Unix socket names exceed Linux SUN_LEN, and private-fixture checks reject
files below an ancestor containing `.git`. Do not delete that Git directory,
weaken the private-file check or change the host's temporary mount to make a
test pass. Compiler output and build temporary storage still belong under HOME.

`tools/test-source-sandbox.sh REPOSITORY PRIVATE_HOME_SCRATCH` runs only the
trusted repository's existing `tests/run.sh`. The operator creates a fresh
0700 directory under the actual HOME cache first. It must be empty and disjoint
from the repository, not an installed configuration or an existing artifact
directory. Inside an unprivileged disposable
mount/user/PID/network/IPC/UTS namespace, that directory appears as short `/tmp`.
The physical host `/tmp` and its quota are not used for fixture storage.
HOME is not reassigned. No source files or installed configuration are writable.

The new network namespace has its own loopback, no host interfaces or external
network. `/run` is shadowed and desktop/bus/agent environment selectors are
removed for the child; pathname Unix sockets elsewhere are not isolated merely
by a network namespace. All inherited OMAVLESS_* live opt-ins/reference overrides
and language startup overrides are also removed before the inner test command.
Start the launcher itself from a trusted shell; an already executed outer shell
startup hook cannot be undone by this script. No VM, host service, desktop
setting, authorization or VPN operation is performed by the launcher. It
requires already installed root-owned non-setuid/non-filecap bubblewrap; missing
support refuses without installation/fallback.
The only command is the existing source gate. No live opt-in should be invoked
through this source runner. Root files remain read-only visible: this is a
trusted developer-fixture environment, not a security boundary against malicious
source or proof that arbitrary tests cannot read private files.

Rust compilation remains a separate HOME-cache operation. This source-only
launcher deliberately does not grant Cargo writable host directories, run
`tests/run-rust.sh`, or borrow installed/hardware acceptance. Rust and terminal
gates keep their own declared environments and exact source outcomes.

Create scratch with `mktemp -d` under an owned HOME cache directory, then pass
that exact path. The launcher never removes scratch or unique failed evidence.
Inspect/dispose only its known completed fixture contents under the retention
policy. A failing gate remains failure even if another environment later passes.

This reusable procedure is a workflow response to repeated environment setup
failures, not a new product dependency or change to test assertions. Record the
selected repository SHA, launcher SHA, environment and actual terminal status
on the owning PR; successful source tests do not establish VM/network behavior.

## Verified source-only scope

The initial launcher artifact
`24d49e8cbcc38f2190612f1a0fb3b2ffef15d48e102ab614e847932a86f5698d`
was independently reviewed and actually exercised against three feature scopes:

| Selected code | Source gate | Exclusions |
| --- | --- | --- |
| Child-only proxy `b5a3c5c13ee7a77ede4d10410cd8347ad892c982` | 326 Python tests, 2 skipped, existing JS/QML contracts pass | No global proxy setting, actual application or installed acceptance |
| Resume fixture `269960f06f9f6568e93789f4f551765b56be5ad5` | 316 Python tests, 2 skipped, existing JS/QML contracts pass | No host sleep/event source, DNS/routes or VM operation |
| Subscription owner `182b1cf80cc43830f96d5aa17fcbf1008d2aeb87` plus the reviewed two-file short-root test correction | 316 Python tests, 2 skipped, existing JS/QML contracts pass | No production timer, active VPN or provider operation |

An independent synthetic forward check also exercised HOME `.git` ancestry,
short AF_UNIX binding under logical `/tmp`, and removal of inherited live
selectors. The dummy script was intentionally a fixture, not product evidence.
Paths constructed directly under HOME remain outside this workaround. Rust's
long-path failures require its separately reviewed fixture helpers, not a claim
that this source launcher ran Rust. Preserve previous failed outcomes; exact
successor-head results belong on their owning PRs. A prose clarification of the
launcher's pathname-socket limitation does not retroactively change this pin.
The clarified launcher
`a011ea0efc8c1a811420e6866900d90c25a67e7e621d1732479bc6a20c8bffe6`
also passed an actual synthetic forward check with both inherited live/reference
sentinels, a short Unix socket, and the no-Git-ancestor invariant. It is used
for the immutable subscription successor's source gate; that result is recorded
on its feature PR rather than borrowed from the earlier code pin.
