# K1 retained-activation reference successor

Developer-only source successor to #610, immutable
`2405e5e3663efd36301162973e0842c99ec6f13a`. No normal runtime dispatch or
production authority changes. Old helpers, unit files, fixed stages and all
NONPASS results remain unchanged. This document grants no VM execution,
account/manager activation, cleanup of old artifacts, merge or release authority.

The one #610 invocation returned NONPASS. A separately reviewed file-only
diagnostic found exact original pins, preflight and before-baseline phase
receipts, and a strict refusal at `before-baseline` with `helper_known_zero=false`.
There was no baseline, helper log, native receipt, later phase or result. The
native helper was not attempted. No historical exception cause was established.

There is an independently reproducible source-policy counterexample: the old
#609 link `/run/systemd/system/omavless-k1-versioned-config-reference.service`
targets `/run/omavless-k1-versioned-config-reference/fixture.service`, outside
the frozen inventory's allowed target roots. That original query guard remains
byte-identical. This does not turn the old failure into a PASS or authorize
removing its retained link.

## Narrow read-side addition

The new definitions-only `retained_activation_guest_guard.py` copies the frozen
snapshot/raw-owned-wait support. Its only added target admission is the exact
old link and exact old fragment pair, never a directory prefix. It retains
original root-owned ancestor directory FDs, an O_PATH/no-follow link FD and a
read-only/no-follow regular fragment FD. The old stage must be root0700, the
fragment root0600/single-link/no-xattrs, with bounded original-FD bytes pinned to
`8916ea57f66461fcc1096c74589388221a7826e65bf64e97ea77c3b3809de81c`.
Full link/file metadata and stable parent identities are compared to named
objects before and after reads. Retargeting, replacement, unknown metadata,
wrong hashes and aliases refuse. Any inventory failure permanently latches.

Both the old symlink record and original-FD fragment digest remain in the full
before/after activation catalog. Nothing is hidden, excluded, normalized or
globally masked. All fifteen canonical snapshot categories and full-network
comparison remain; only the existing monotonic address-lifetime countdown rule
is permitted. Old activation identity is admitted before the first snapshot
and rechecked with the observer's other pins before publication and cleanup.

## New inert capture generation

The literal `omavless-k1-retained-activation-reference.service` and root stage
`/run/omavless-k1-retained-activation-reference` are new. A copied cfg(test)-only
native entry/parser binds this exact tuple, pinned unit bytes and same-owner
typed Manager.Version `261.2-1-arch`. The finite original sequence remains one
connection, RefUnit, selected typed properties and Dump, repeated Version,
strict configured-text facts, acknowledged Unref, and one post-Unref GetAll.
Unknown RPCs park the retained connection without compensating calls or retry.
The unit is inert `/usr/bin/false`, RefuseManualStart=yes, never started.

This observes configured zero watchdog and fixed append paths, not full runtime
admission, network namespace ownership, kill-switch completion or lifecycle
proof. The outer retains the original unit FD throughout. Only the direct
original-FD helper's raw known-zero exit plus strict receipts/current cgroup
absence/exact own link permit removal of the **new** link and daemon-reload,
under trusted-root/exclusive-VM assumptions. No unrelated all-UID process scan
is added; this narrow inert-only proof cannot be reused for effectful lifecycle
or recovery. All twelve durable before-boundary and terminal refusal records
remain. Unknown/nonzero/malformed results permit no cleanup or after-queries.

Native checkpoint `cb4cdc15e9648fec28ecff52bd6ca8027447f976` passed seventeen
focused tests (actual VM entry ignored). Its frozen ELF SHA-256 is
`ff177355ef3c893fe980075de1ba3bae0a7d1eaeb99bec2879c6c78c0acad8eb`.
New create-only user staging is
`/home/kdk_vm/.cache/k1-retained-reference-cb4cdc1-stage-1`; loader delivery must
be trusted host/root code, not sudo execution of staged user-writable source.
Pins are acyclic: loader → observer → read-side support/unit/ELF. Four pure
inventory tests reproduce old refusal and cover exact admission, aliases,
identity/hash/owner/mode/link/xattr errors and terminal no retry. Nine inherited
outer controls cover phase failures and cleanup boundaries. Full gates and
complete root/independent source review are still required before any lease.
