# T3 conditional-close core prerequisite

Review-only patch against MetaCubeX/Mihomo v1.19.31, exact commit
`ab405bad5beeeac8b003bb01f60f134f6df54471`. Upstream modified code and its
patch remain GPL-3.0; this is not upstream adoption or a new published core.
Normal DNS pair, package pins, runtime methods and TUI controls are unchanged.

## Guaranteed target, not probabilistic UUID identity

Each tracker receives a canonical nonzero decimal `omavlessCloseToken` from
one monotonic manager-lifetime counter before publication. A token is never
reassigned to an already enrolled tracker. Saturation never wraps: later
trackers remain observable but have no conditional-close token. The two
production TCP/UDP constructors enroll new objects once; they do not pool
TrackerInfo. A future pooling/re-enrollment change requires new review.

The private controller snapshot includes the token. A fixed
`POST /connections/{id}/close-conditional` accepts exactly one quoted canonical
decimal `If-Match` value, no body or query. Under the same mutex as Join/Leave
it compares the exact ID/token and removes that tracker. It then closes the
retained original object outside the lock. A delayed old Leave cannot remove
a successor. Failure has no unconditional DELETE fallback or close-all effect.

Responses are 204 for this successful close, 404 for missing, 409 for changed
incarnation, 503 for unavailable identity/not-running, 502 for an effect failure, and 400
for malformed input. A lost reply stays unknown; later absence is not a
receipt and must not trigger an automatic resend. Tokens are scoped to the
verified owned core incarnation, not globally stable or user-facing identity.

The fixed read-only `GET /connections/conditional-capabilities` reports exact
ABI 1 and `ready` only when Mihomo's tunnel is Running. Configs/rules/listeners
and CONNECT 200 appear earlier during startup and are not a readiness proof.
The close handler also refuses while suspended/loading. This protocol report
is not immutable package attestation or serialization against concurrent reload.
The normal owner must supply the latter independently before activation.

The existing ordinary controller DELETE remains upstream behavior. OmaVLESS
does not call it or expose either endpoint to its clients in this checkpoint.
Matched-core package attestation, bounded owner-private row handles,
confirmation/replay/expiry, pre-effect child/controller and revision checks,
concurrency integration and installed EN/RU review are still required.

The inactive Rust candidate binds an unreaped child, retained directory/socket
FDs, exact peer credentials and a non-reusable session identity. Discovery and
close require the exact typed capability report; stale targets, dead children,
replaced sockets and suspended/ambiguous ABI send no effect. A private effect
permit has no production constructor. Unknown/partial receipts are never
retried. This is a transport prerequisite, not new runtime/IPC/TUI behavior.

## Offline review

With dependencies already in the Go module cache, use a private scratch parent
in the home directory and an existing repository containing the pinned objects:

```sh
python3 tests/core_connections_adapter/review.py \
  --source /absolute/local/mihomo \
  --scratch-parent /absolute/private/home-scratch
```

The runner exports only the pinned Git objects (never local source edits),
checks/applies this patch in disposable scratch, and runs the exact statistic
and HTTP tests 20 times under Go's race detector. Network module fetching is
disabled. It never uses sudo, private profiles, TUN, an installed daemon or
the host controller. The temporary reviewed source is removed after the test.

Tests include UUID reuse, delayed old Close, simultaneous confirmation,
one-use outcome, counter saturation, re-enrollment refusal, malformed and
ambiguous HTTP inputs. Matched-package acceptance must be recorded before the
feature is activated.

## Real socket gate (candidate only)

`loopback.py --core /absolute/candidate --scratch-parent /absolute/private/scratch`
starts a separate disposable core with TUN and DNS disabled, a private controller,
and two real HTTP CONNECT tunnels to a synthetic loopback echo listener. A wrong
token must reject while both tunnels continue; a matching token must close only
its original tunnel and leave the other live. Explicit replay must return 404.
It never contacts a provider, installs a package, modifies system proxy settings
or uses the installed controller. Ordinary offline tests cover the harness's
bounded responses and rejection of unsafe inputs before process creation.

The exact candidate binary and VM results are recorded in the owning PR; this
is real candidate-core behavior, **not** matched native package, production
owner admission or installed TUI acceptance. No private profiles are used.

## Exact managed-DNS composition gate

`managed_composition.py` combines the conditional-close patch with the managed
DNS patches from exact OmaVLESS snapshot
`c4e800425243c1b02165f82153e4bf418fe465e6`, not whichever branch is currently
checked out. The DNS patch SHA-256 is
`d5ebe9d6b37f6b76599fc3c2dd25adbfb774ca0121beeb79c5768a9a08d7ff37`;
the sing-tun patch SHA-256 is
`2556c82aafbeb598a817d43042cf2069c6f209433c7a506395df581b4e31e2ab`
against upstream commit `b50ae28a1409c7bce8e96e6c6966cf57d8ace754`.
Unknown bytes or unavailable Git objects refuse before building. Local dirty
source, Git replacement objects, user configuration and hooks are not input.

```sh
python3 tests/core_connections_adapter/managed_composition.py \
  --mihomo-source /absolute/local/mihomo \
  --sing-tun-source /absolute/local/sing-tun \
  --dns-repository /absolute/local/omavless \
  --scratch-parent /absolute/private/home-scratch
```

With dependencies already cached, it exports pinned objects into private
disposable HOME scratch and vendors the exact locally patched sing-tun.
Module/toolchain downloads and ambient Go workspaces are disabled. Conditional
tests run 20 times with race instrumentation; 11 managed-DNS Go cases run 20
times. Bounded Go JSON receipts must prove execution of all seven conditional
cases and these DNS cases; a zero-test success or unexpected skip refuses.
The separate opt-in Rust↔Go DNS interop case is explicitly skipped, not PASS.
A separate GPL-3.0 **test-only** overlay shortens temporary DNS socket paths;
it is hash-checked and reversed before compiling the core. The two production
DNS patches remain byte-for-byte unchanged. This avoids Linux Unix-socket path
overflow, not a runtime DNS change.

The disposable binary is built with `with_gvisor`, CGO off and explicit
`-buildvcs=false`: provenance comes from the pinned archives/patches, not a
possibly unrelated parent repository. Its build metadata is checked and the
real two-tunnel loopback gate runs. The runner reports source/patch/toolchain
and binary identities, then deletes its scratch and binary. The executed
toolchain and binary hash are review evidence, **not** a release artifact,
production package receipt or architecture-independent identity.

This gate does not build/install the DNS broker, exercise Rust↔Go DNS interop,
enable system DNS/TUN, use provider credentials or replace the installed core.
It proves that the pinned source patches coexist and that conditional close
works in that composition. Managed DNS-pair acceptance, matched immutable
companion-package receipt, owner admission/operation reservation and installed
EN/RU close-confirmation review are separate requirements.

## Private UDP conditional-close gate

The optional `--udp-loopback` flag runs twenty `udp_loopback.py` repetitions
against the exact combined binary within the builder's existing disposable lifetime. Default
composition behavior, source pins, production patches and build flags are
unchanged. It adds no source package, runtime admission, public method or UI.
The helper can also review an explicitly supplied disposable candidate using
`--core /absolute/candidate --scratch-parent /absolute/private/home-scratch`;
that binary's provenance must be recorded separately, not inferred from its name.

Two retained SOCKS5 TCP UDP associations and two distinct owned IPv4 loopback
UDP echo targets produce two private UDP rows. Both client UDP source sockets
are retained. A wrong incarnation must return exact empty 409; both echoes and
the exact tracker identities must survive. Matching close must return exact
empty 204; the core patch retains the selected object under atomic ID/token
comparison and calls its packet connection's `Close`. The unselected tracker
must retain its exact identity and remain usable. Subsequent absence only
corroborates the receipt; it is never a substitute effect receipt.

Unlike TCP, UDP has no EOF receipt to observe. The original association remains
open, and a subsequent application datagram through the **same source socket**
must echo with a new tracker token while the other tracker remains unchanged.
This deliberately demonstrates automatic application reconnection: closing one
datagram tracker does not prohibit the next packet creating another. As the
upstream NAT cleanup is asynchronous and UDP delivery is not guaranteed, this
application check permits at most five datagrams, each with a fresh synthetic
challenge. It does not promise the very first post-close packet is delivered,
a permanently disconnected application or durable packet blocking.

Conditional effects are never retried. Lost replies, nonempty/changed receipts
or timeouts abort the fixture without inferring success from a snapshot or
reissuing close. Only the fixture's owned core is terminated during cleanup.
Rows, IDs, tokens, random controller secret and ephemeral endpoint values remain
private memory/private scratch and are never printed. Output is fixed verdicts
and public binary hashes only. DNS, TUN, provider traffic, installation, system
proxy and host-service changes are excluded. Echo threads, sockets, core child
and temporary config/database are bounded and cleaned up.

Offline regressions cover receipt ambiguity/loss without retries, strict local
SOCKS negotiation, association liveness, exact unfragmented loopback datagrams,
bounded application retry, foreign/malformed/duplicate row identities and
unsafe-path refusal. This candidate-core developer gate is not installed
matched-package/owner-admission/TUI acceptance or protocol-provider UDP proof.
