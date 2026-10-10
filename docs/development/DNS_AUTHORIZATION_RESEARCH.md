# DNS authorization: reuse findings and next implementation boundary

September 25, 2026; developer preparation for #270/#132, not installed behavior.
The owner prefers **one explicit enrollment, then no recurring DNS prompts**.
A single authorization per transition is an acceptable fallback, not the target.
Neither path may grant all desktop programs DNS rights over unrelated links.
Main, RC, release assets and the installed VPN remain unchanged by this research.

## What the other implementations actually establish

| Reference | Mechanism | Reusable finding / limit |
| --- | --- | --- |
| JaguarKovalev legacy fork | Account-wide polkit grant for four resolved actions | Removes those prompts without transferring DNS ownership from Mihomo; not scoped to the application's TUN. Do not install this rule. |
| omarchy-mihoro, `1053ae41ee00aa9a65785d1393f026b9f5447200` | Reload configuration through the core controller, preserving an unchanged TUN; restart fallback | Avoids redundant DNS work, not authorization for actual TUN creation/destruction. |
| Installed Try Omarchy network panel / `omarchy-dns` | Elevate one packaged command, then perform all provider-change steps inside it | Explains one dialog for several changes. Its global/physical-link effects are inappropriate for VPN connect/disconnect. |

Sources: [fork rule](https://github.com/JaguarKovalev/omavless/blob/legacy-0.7-fixed/setup-passwordless-resolved.sh),
[Mihoro reload implementation](https://github.com/huacnlee/omarchy-mihoro/blob/1053ae41ee00aa9a65785d1393f026b9f5447200/scripts/config_enhancer.py#L179),
[Mihoro design](https://github.com/huacnlee/omarchy-mihoro/blob/1053ae41ee00aa9a65785d1393f026b9f5447200/DESIGN.md).
The upstream CLI at `c2b65e374d9d8a27f4eb3d2d2b5136d028563438` uses a user
service, not a root DNS broker. These are comparisons, not imported implementations.

Installed Omarchy evidence is specifically `try-omarchy-runtime 4.0.1-1`, not
a claim about every Omarchy release. The root-owned `0755`
`/usr/bin/omarchy-dns` matched the shell source copy byte-for-byte, SHA-256
`ce6dc8694d7ee39e003ec1d2aefa7768386dc70fe7c096e339bd82b8f45d17fd`.
The installed network panel SHA-256 was
`b6fe8c8b75460074c8bf4942ed541e650f72102cdb4b6f772b6da9ace9bb15b9`.
Its four choices are DHCP, Cloudflare, Google and Custom. `require_root` uses
sudo in a terminal or when a matching passwordless grant is reported, otherwise
pkexec of the fixed packaged path. We did **not** execute that path or inspect
authorization by trying passwords. The owner reported one dialog; reading the
code explains that behavior, not a new live authentication test.

The provider command rewrites resolved/NetworkManager configuration, updates
Wi-Fi/Ethernet connection DNS, reapplies active connections and reloads the DNS
stack. It is not a per-VPN temporary lease API. The panel commits its provider
selection on command exit zero, rather than immediately on click; this is useful
cancellation UX but not complete readback/rollback proof. Do not call this
command from OmaVLESS or copy its global reset behavior into our helper.

## Real-core reload experiment without host authorization

Reproduction (explicit opt-in, not part of ordinary CI):

```sh
python3 tests/dns_core_namespace_probe.py
```

The parent requires an ordinary UID and a root-owned, non-writable regular
`/usr/bin/mihomo`, and hashes it before remapping UIDs. A fresh user + network +
PID namespace child checks all three identities differ and only loopback exists
before creating fixtures. The child copies that exact binary **without xattrs
or file capabilities**, verifies the digest, and uses private temporary config
and a private Unix controller. It has namespace-local capabilities only.
PID-namespace teardown and `unshare --kill-child` contain descendant lifetimes.
The owned core is terminated/joined in `finally`; no installed process is signalled.

The artificial DIRECT-only config has no provider/profile data, remote rules,
proxy servers or public probe. `auto-route: false`, `dns-hijack: []`, and disabled
core DNS listening deliberately test that these settings are not resolved-off.
A scratch-only `resolvectl` stub records **verb names only**, never calls a bus
or edits DNS, and returns success. The core receives only the scratch PATH and
nonexistent private D-Bus addresses, not the desktop environment. This is test
instrumentation, **not** a production PATH wrapper or a permission workaround.

Try Omarchy ARM64, Mihomo 1.19.31, exact copied core SHA-256
`8b8ee3599a86c59778294c8cbe08201b561c5e5e5c4654e28617ce8abc6a7f40`:

| Step | Observed |
| --- | --- |
| Initial synthetic TUN start | `domain`, `default-route`, `dns` calls despite the settings above |
| `PUT /configs?force=true`, same TUN, direct → rule | Same live process/index, controller reports rule, no additional DNS calls during the one-second observation window |
| Reload with different synthetic TUN name | Old-link `revert`, followed by three new DNS setup calls |
| Owned core termination | Another `revert`; owned child exited |

All nine boolean evidence fields passed on the final probe implementation.
This proves bounded behavior of this exact core/fixture, **not** successful
system DNS, actual polkit behavior, HTTPS, arbitrary production reloads, or
unchanged profile/selector/lifecycle semantics. A real reload optimization must
validate complete TUN equality and configured mode/selectors, serialize with
disconnect, compensate failures, and retain our private Unix controller. Do not
copy Mihoro's TCP controller path. Removing unnecessary restarts can reduce
prompt frequency independently; it cannot satisfy #270 at fresh connection,
reboot, changed-TUN settings or teardown.

Default CI runs only mocked guard/projection/cleanup tests for this tool.
No private fixture, TUN, namespace or real core is launched by those tests.

Validation for this follow-up: all **13** new guard/projection tests pass;
`tests/run.sh` runs **339** tests (**337 passed, 2 expected skips**) plus the
Node/QML contracts. The unchanged offline Rust foundation's **53** tests pass.
Python compilation, shell syntax, documentation navigation and `git diff
--check` pass. The previous full Rust workspace evidence remains attached to
the foundation checkpoint; no production or Rust source changes in this follow-up.
Final read-only runtime observation still reports connected Routing, matching
desired/owned profile, one core/one managed TUN, zero auxiliary cores and no
manual-recovery flag. Installed application bytes are unchanged. This does not
claim a new live HTTPS or DNS authorization pass.

## Concrete exclusive-writer integration work

The preferred broker needs a tested way to suppress both core DNS setup **and
teardown**. This is a prerequisite of our selected exclusive-writer design,
not a universal prerequisite of eliminating password dialogs (the fork above
demonstrates the distinction).

For the inspected Mihomo source
`ab405bad5beeeac8b003bb01f60f134f6df54471`, the minimal upstream proposal is a
separate default-disabled `disable-system-dns` option. The spelling is a proposal,
**not an existing supported configuration key**. Required seams:

1. Raw config → listener `Tun` config, with serialization/default behavior.
2. `Tun.Equal`: changes to this option must not retain an old DNS-owning TUN.
3. `listener/sing_tun` → `tun.Options.EXP_DisableDNSHijack`.
4. Prove the pinned dependency suppresses both setup and `Close` revert, without
   changing packet DNS interception, routing, addresses or MTU.
5. Conformance: omitted/false retains old behavior; true suppresses both paths;
   changing the flag forces the intended lifecycle; FD and ordinary creation
   paths cannot retain a teardown writer. Unknown/unpatched core must refuse
   broker activation, not silently ignore an unknown YAML option.

Sources and existing no-flag/FD findings are in the
[foundation report](DNS_TRANSACTION_FOUNDATION.md#existing-core-does-not-expose-the-required-dns-ownership-switch).
The subsequent [review-only patch and reproduction instructions](../../tests/core_dns_adapter/README.md)
implement these three wiring points on the exact source above. Its two Go test
functions pass (three config/default/serialization subcases and equality), and
the complete core builds with Go 1.26.8, CGO disabled, default tags. This is a
scratch test executable, **not** an upstream PR, replacement package, approved
distribution choice, or the installed `with_gvisor` build.

`tests/dns_core_ownership_probe.py` runs that explicit SHA-256 candidate only in
fresh user/network/PID namespaces, copying without file capabilities and using
the same scratch-only DNS verb recorder. All **11** final facts pass:

- omitted/false preserves core setup and close calls;
- true suppresses setup and close, including changed-TUN reload;
- true→false resumes core DNS setup; false→true performs the old owner's revert
  and suppresses subsequent calls;
- the controller returns the explicit policy boolean;
- an externally created FD with false still reverts; true suppresses that revert;
- isolation and joined, clean owned-core exits pass.

The FD fixture explicitly assigns its synthetic address and brings its private
link up: FD mode skips that core configuration and cannot be tested honestly
with just an open, unconfigured descriptor. Readiness requires configured TUN
state, not merely a controller socket and a pre-existing link. Forced process
termination is not accepted as evidence of a successful core Close path.

The unchanged stock core is the negative control: it is rejected with fixed
`unsupported_dns_ownership_capability`, not considered compatible because an
unknown YAML key was accepted. These are test-tool checks, not a newly installed
production admission path. Bounded quiet windows, a synthetic system-stack
fixture and absence of recorded commands do not establish live system DNS,
production gVisor/IPv6/routes/packet interception or a secure privileged lease.
Review the adapter/distribution choice before installing anything; do not make
users maintain a hidden fork. DNS-0's core mechanism now has executable evidence;
its production integration and managed-link identity/restore gates remain open.

Adapter checkpoint validation: **16** new effect-free probe guard tests;
`tests/run.sh`: **355 run, 353 passed, 2 expected skips**, plus Node/QML
contracts. Python compilation, shell syntax, documentation navigation and diff
checks pass. No application/Rust runtime source changed, so prior workspace
evidence is retained rather than labelled as a fresh host gate. Read-only host
observation remains connected Routing, one owned core/managed TUN, no auxiliary
core and no recovery flag. The installed core hash is unchanged.

## Snapshot restoration is not just three effective properties

An additional source audit found a prerequisite for the real adapter which the
boolean fake-host model cannot establish. In systemd v261, the public
`DefaultRoute` property returns the **effective** boolean; the internal setting
can instead be automatic (`-1`). Setting the observed boolean back can change
future behavior even if immediate readback matches. `SetDefaultRoute` takes a
boolean and does not encode that automatic state. Also, `Revert` resets other
link settings, including LLMNR, mDNS, DNSSEC and DNS-over-TLS, not only our three
properties. [Getter/setter](https://github.com/systemd/systemd/blob/v261/src/resolve/resolved-link-bus.c),
[internal defaults/reset](https://github.com/systemd/systemd/blob/v261/src/resolve/resolved-link.c).

Therefore:

- Never treat the existing three-property diagnostic readback as a complete
  restorable snapshot. It checks our applied fixed policy, not original intent.
- `SnapshotCaptured` in the offline model must mean the future adapter has
  proven restoration semantics, not merely read three booleans/arrays.
- Prefer an exclusively created, pristine broker-owned link with known baseline
  for initial enrollment; externally managed/ambiguous existing links must be
  refused before effects unless an exact restoration method is established.
- A whole-link revert is legal only with proof that the entire affected link
  configuration belongs to this lease. Never reset a physical link to simplify
  recovery, and never equate a matching effective default-route with automatic
  policy restoration.

Read-only introspection confirmed the installed resolved exposes SetLinkDNS,
SetLinkDNSEx, SetLinkDomains, SetLinkDefaultRoute and RevertLink. No write call
was made. Capturing extended DNS port/server-name metadata, default-state
provenance, and resolved restart/recreation belongs to the real adapter gate.

## Primary and fallback authorization paths

Both paths must share fixed operations, exclusive DNS ownership, bounded inputs,
serialization, actual readback and uncertain-outcome handling:

- **Primary:** explicit administrator enrollment installs a package-owned
  narrowly authorized helper; normal enrolled transitions request no password.
- **Fallback:** one explicit authorization launches one fixed-purpose operation
  which performs and verifies the whole DNS transaction. This is not permission
  for QML to run arbitrary root commands, nor a blanket polkit grant.
- Never silently fall back after a denied enrollment or unknown broker outcome:
  that could duplicate effects or create another prompt chain. Present refusal
  or recovery explicitly. A password count alone is not correctness evidence.

## Next session: no ceremonial repeat

The subsequent [creator-held TUN authority experiment](DNS_TUN_AUTHORITY.md)
found that a capability-free FD consumer can still change TUN owner/persistence.
A tested ioctl/FD-export filter blocks those changes; the actual core starts and closes
with that filter and empty capabilities. UDP ancillary compatibility and external
FD-extraction/process isolation remain unproved. This advances a restricted-consumer
candidate, not a root broker or host cutover.

First finish the core DNS-off capability and real lease/restore integration,
then root service/package/typed D-Bus implementation and fault tests. The
prototype is **not yet installable merely because the owner is present**.
The remaining design must distinguish ordinary unprivileged clients, unrelated
managers and stale/reused links from an already-root/CAP_NET_ADMIN adversary;
do not promise security against a host administrator from a name/FD check.

Once an exact candidate exists, the attended sequence is one rejected-or-accepted
enrollment action with settled confirmation, accepted enrollment if necessary,
then connect/mode/disconnect/readback/recovery on that candidate. Do not ask the
owner to cancel three legacy dialogs again. Keep original Routing/profile and
startup Off; package removal/revocation must preserve unrelated DNS. Follow the
[host authorization procedure](../testing/HOST_AUTHORIZATION_ACCEPTANCE.md).
