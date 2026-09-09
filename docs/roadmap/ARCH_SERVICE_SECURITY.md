# Arch native service capability policy — proposed

Status: review candidate for #178, not installed acceptance or Rust cutover.

## Existing behavior and intended boundary

Python's current user service starts an unprivileged supervisor which executes
the separately provisioned Mihomo binary. Installed file capabilities give the
core `cap_net_bind_service,cap_net_admin,cap_net_raw=ep`. The supervisor does
not receive these capabilities. This proposal preserves that Arch model for
the Rust supervisor; it does not introduce a privileged daemon or IPC method.

The accepted three-prompt systemd-resolved interaction is documented in
[`TRY_OMARCHY_ARM64_2026-08-26.md`](../testing/TRY_OMARCHY_ARM64_2026-08-26.md),
Finding 3. Mihomo invokes three resolvectl operations. File capabilities do not
authorize those D-Bus methods. Normal prompts must be allowed to finish;
cancellation is not evidence of a Rust-only capability failure. No polkit
exception, automatic password handling or OS DNS policy change is proposed.

## Explicit security trade-off requiring review

The previously staged native unit cannot support its own file-capability model:

- `NoNewPrivileges=yes` prevents capability acquisition at core exec.
- Mount sandboxing in the observed user manager creates a user namespace;
  capabilities there do not authorize configuring the host TUN.
- Seccomp-based hardening can implicitly enable NoNewPrivs despite explicit `no`.

The candidate removes PrivateTmp, ProtectSystem and ProtectHome, and explicitly
uses NoNewPrivileges=no. This **reduces isolation relative to the old native
package template**, not relative to the current Python supervisor. It must not
be described as equivalent sandboxing or silently installed as a local override.
The Rust process retains ordinary same-user filesystem access and could execute
other capability/setuid programs available to that user if compromised. The
private socket is not a security boundary against its own account.

Retained controls: fixed packaged executable/arguments, no shell or privileged
IPC, same-user admission, bounded protocol, singleton ownership, private atomic
stores, 0700 XDG directories and 0077 umask. LimitCORE=0 prevents ordinary core
dumps; it is not a promise that all privileged crash collectors are disabled.
Mihomo capability provisioning remains explicit and external. No new setcap or
package-manager operation is added to runtime startup.

An alternative separately managed core service could retain stronger runtime
sandboxing, but would require a new lifecycle/ownership/handoff contract. It is
not needed merely to reproduce the working Python launch model. NixOS needs its
own wrapper/service review; this Arch evidence does not establish Nix support.

## Observed diagnostic evidence (Try Omarchy ARM64, 2026-09-08)

Baseline main: `9512eb03bccabb4e00a6ddadda9ed1e754d5ea94`.
Mihomo: 1.19.30. Tests used temporary user services and synthetic configuration,
no private profile/server. auto-route and auto-redirect were false.

| Policy | NoNewPrivs | Effective capabilities | TUN |
| --- | --- | --- | --- |
| Old native isolation | 1 | zero | absent |
| Same isolation, explicit NNP=no | 0 | 0x3400 | permission denied |
| Legacy-style service | 0 | 0x3400 | created |

A separate no-network probe observed a full identity uid_map in the baseline
and only the caller UID mapped under ProtectSystem/ProtectHome. A further
RestrictRealtime + LockPersonality probe retained the namespace but set NNP=1.
The initial /tmp-config probe was discarded: PrivateTmp hid the input and the
core generated defaults. Corrected probes used a private runtime directory.

Legacy-style shutdown took approximately 25 seconds while normal authorization
was involved, exceeding the initial test subprocess timeout. Subsequent checks
confirmed the service/core/TUN were gone. This is diagnostic evidence, **not**
candidate lifecycle acceptance. Installed units and capabilities were unchanged.

## Required acceptance before merge/activation claims

1. Review this explicit reduced-isolation trade-off and the packaged unit.
2. On the exact candidate, inspect effective runtime/core NoNewPrivs, namespace,
   capabilities and private writable directories (not just unit text/getcap).
3. Prove exactly one service-owned core/TUN and responsive private Unix controller,
   with no TCP external-controller, using the candidate host policy.
4. Allow normal authentication; distinguish successful completion, cancellation,
   failed cleanup and uncertain ownership. Do not impose a short human deadline.
5. Prove shutdown/cleanup; preserve rollback and no duplicate owner on failure.
6. Repeat packaged native Full VPN/configured-readiness acceptance on this
   exact candidate. Issue #183's configured-readiness implementation is already
   closed; the remaining package/host evidence is not an unimplemented #183 fix.

This policy alone does not close #178's startup integration work, add login
activation, expose cutover, switch QML, or permit removing Python at R6.
