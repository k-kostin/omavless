# R5 exact-attribution Routing observation

## Boundary

This checkpoint supplies the connected/unmatched Rule-mode branch of the existing
`routing.check` semantic API and stdin-only CLI. Global/direct/custom/disconnected
fast paths remain pure. No protocol, UI lifecycle, default owner, service, TUN or
privilege change is included. No caller-controlled port/path/URL/process API is
introduced. The probe may establish the user's explicitly requested destination
through the owned proxy, but sends no application payload.

The old Python fallback selected a recently incremented **global** rule counter,
or a new connection to the same destination, even when unrelated traffic caused
it. It could also infer VPN from an unknown chain. Those are incorrect attribution,
not behavior to preserve in Rust. The reference now requires the exact held TCP
tuple and known policy chain; immediate REJECT with no observable connection is
unavailable, not invented block evidence. Unused global-counter helpers are removed.

## Bounds and privacy

- Three seconds total native budget, existing four detached-work permits.
- Private controller directory 0700/socket 0600, same UID and owned-core PID.
- Existing 512 KiB controller HTTP/JSON cap; at most 2048 connection rows.
- Exact source/inbound IPv4 loopback address/port, TCP, HTTPS CONNECT, target/443.
- At most 16 chain entries of 256 bytes; only fixed terminal/selector categories.
- Rule type 80 bytes, payload 256 bytes, existing private-fragment redaction.
- At most 4096 descriptors and 1 MiB/8192 proc TCP rows for the owned PID only.
- Query bytes are not sent until the accepted server socket's inode is proven
  owned; a listener-only check would permit a rebind race.
- Revision, desired state, complete private store/config digests and PID are
  revalidated; no raw bytes/digests or private fragments enter ordinary responses.
- No raw controller error, connection ID, chain, process metadata or path output.

The result is private local UI data: query and a public rule destination may be
present, as in the existing route-check contract. It must not be pasted into a
shareable support report. There is no unbounded general socket/process scanner.

## Executable evidence

Focused tests cover exact tuple versus concurrent same-destination noise,
ambiguous connections/chains, safe policy categories, bounds, redaction, and an
effect-isolated actual Python projection oracle returning digests only.

Real loopback TCP/private Unix-controller tests verify fixed request bytes,
accepted-PID proof, wrong-PID no-request rejection, and probe EOF cleanup. A
private semantic socket test changes store, desired state, config, ownership and
disconnect state during blocked controller work: stale results are discarded
while status and urgent disconnect remain admitted.

The installed-Mihomo opt-in test starts an isolated, credential-free no-TUN core
with a fixed loopback REJECT rule, tests native collection, and calls actual
`backend.live_route_match` against that core. Only the Python controller-path
resolver is redirected to the synthetic socket; controller/probe logic is real.
No real private store, provider, endpoint, DNS or external network is used.

Installed file capabilities hide `/proc/<pid>/fd` on this Try Omarchy guest.
As with the existing diagnostic no-TUN test, the **test only** uses a byte-identical
non-capability copy in its exclusive 0700 temporary directory; it is removed
after the owned child is stopped. Production must never use that as a workaround.
The registered live path remains unavailable if its real owned socket cannot be
proven. #178 and exact installed ownership acceptance remain outstanding.

Python retains its legacy five-second poll budget and existing host ownership
boundary; the new native three-second deadline/PID proof are intentional stronger
constraints. Redaction uses each language's established bounded diagnostic helper;
native additionally rejects an excessive private-fragment budget rather than
discarding redaction inputs. These are not claims of complete Python retirement.

## Acceptance commands

`OMAVLESS_TEST_MIHOMO=/usr/bin/mihomo ./tests/run-rust.sh` includes the installed
synthetic core and oracle tests; `./tests/run.sh` includes Python/QML contracts.
Record exact-head counts and results in the PR after running them. No real VPN
acceptance or public protocol fixture claim is implied by these synthetic gates.
