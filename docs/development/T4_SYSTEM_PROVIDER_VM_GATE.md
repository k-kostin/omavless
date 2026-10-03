# Inactive System-provider disposable-account mechanism gate

Base: #580, `3a71e2c3c48b5c70c6d0d8bda0e5f4cd95e02f0c`.

This developer-only driver calls the existing private System historical-Off
review without altering its normal-compiled implementation. It adds no normal
registration, public backend API, receipt publisher or historical mutation
policy. Both filesystem-touching tests are ignored and require the fixed
disposable account `ov-t4-system`, UID/GID 61080, its account-default HOME and
runtime root, and explicit fixture opt-in. Root and the canonical VM account
are rejected before writes. The ordinary CPU test checks only this identity
guard; it does not establish System provenance.

## Real mechanism, synthetic history

A separately reviewed root harness must own the exclusive development VM lease.
It refuses existing account/group/home/runtime/image objects and preserves the
canonical user's runtime, private files, executable, namespace, resolver, routes,
rules and addresses throughout provisioning, execution and cleanup. Actual
global Mihomo/TUN inventories must already be empty. No foreign core or VPN may
be stopped to satisfy that condition.

The first ignored test creates fixed valid startup-Off inputs and generation-2
ownership only in that fresh account. It creates no receipt and no historical
fence. The harness then directly starts the existing packaged login-prepare
unit through the real fresh user's manager, with runtime inactive. Exact
installed login/runtime unit bytes and actual loaded FragmentPaths, no drop-ins
and no pending reload must agree. Global enablement/activation collisions are
refused, not hidden using a runtime mask. Receipt creation remains the genuine
packaged login transaction, including its real process/invocation checks.

For the driver only, the root harness creates a private mount namespace and
read-only binds the exact frozen, root-owned, capability-free test image at
`/usr/bin/omavless`, then drops UID/GID and supplementary groups. Neither the
global installed executable nor either real manager is replaced. This supplies
the existing package self-inode relation; it is **not release attestation or
installed-package acceptance**.

The second ignored test pins the original receipt and ownership marker and
captures the actual `CurrentEpochProof::capture` System source before importing
history. Its synthetic complete Commit history is created in an independently
owned temporary fixture. Only the fixed historical members and current-Off
store/template/desired bytes are copied into the disposable account. The
fixture is never retargeted to the real HOME/runtime; its destructor removes
only its own temporary subtree. No synthetic epoch/receipt constructor runs.
Original receipt and actual manager/package evidence are rechecked after each
import step. The existing private review then must return
`ReviewedOffStillFenced`, preserving the original receipt, ownership marker and
all historical/current private bytes and identities. Ordinary owner startup
must still refuse the surviving fences.

## Acceptance limits

The subsequent [exact-source mechanism checkpoint](../testing/T4_SYSTEM_PROVIDER_MECHANISM_2026-10-03.md)
records a real positive at `fbe5fb9` after correcting the test-only seed preset.
The original cut and its failed attempts remain separately attributed.

The source driver, its CPU identity check and source-retention guards do not
count as real System-positive execution. That result requires a reviewed exact
frozen image, real manager/receipt observations, both exact ignored tests running
and passing, successful owned-fixture cleanup, and the whole-lifecycle canonical
guard. Any package mismatch, manager drift, missing receipt, visible core/TUN,
authorization failure, unexpected activation or cleanup failure is NONPASS.

Even a successful invocation is agent-attended development mechanism evidence,
not formal human acceptance. It does not adopt old-manager recovery, normal
historical startup/profile/connection/batch mutations, package distribution,
release trust, archive UX or restore acceptance. No new normal caller is added.
Exact execution receipts and remaining gates belong to the development PR.
