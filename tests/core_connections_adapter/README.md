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

Both review entrypoints share an isolated fresh bare object-only export,
excluding local `info/attributes`, replacements, user/global Git configuration
and dirty source. Lazy object fetching, remote Git protocols and terminal
authorization prompts are explicitly disabled. They use `/usr/bin/go` with explicit offline settings,
`GOENV=off`, `GOWORK=off`, `GOTOOLCHAIN=local`, no inherited `GOFLAGS`/compiler
overrides, and the existing `$HOME/go/pkg/mod` and `$HOME/.cache/go-build` caches.
`go mod verify` precedes tests/vendor preparation; this is cache verification,
not acquisition of a compiler/dependency or a published-package attestation.
Missing local dependencies refuse rather than downloading them. The actual
installed compiler version and resulting combined binary hash remain separate
evidence; no particular release compiler is claimed by these developer tools.

The runner exports only the pinned Git objects (never local source edits),
checks/applies this patch in disposable scratch, and runs the exact statistic
and HTTP tests 20 times under Go's race detector. Both entrypoints require the
exact nonempty JSON pass counters; exit zero with zero tests, missing cases,
extra root cases, failures or unexpected skips refuses. Network module fetching is
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
An explicit synthetic-wire opt-in is described below; default invocation still
keeps that skip and cannot claim interoperability.
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

### Explicit synthetic Rust/Go wire opt-in

Add both `--rust-channel-fixture /absolute/frozen/channel_fixture` and
`--rust-channel-fixture-sha256 EXACT_SHA256` to the composition command above.
First separately review/build the test-only `omavless-dns-channel` example from
an isolated export of exact DNS source
`c4e800425243c1b02165f82153e4bf418fe465e6`, using locked dependencies and an
offline compiler environment. Freeze that executable outside a running Cargo
target and retain its source/build/toolchain/digest evidence. A supplied digest
identifies this local artifact; it is not independent compiler proof or release
authentication. Missing/partial/unsafe inputs refuse rather than building or
installing an arbitrary broker.

The opt-in consumes bounded no-follow descriptor bytes, rechecks metadata and
ELF architecture, refuses capabilities, and makes an exclusive private executable
copy without inherited modes/xattrs. The golden corpus is read only from the
exact DNS Git object and its fixed digest, never an ambient path. A separate
hash-pinned GPL-3.0 test-only socket overlay is applied/reversed around this
test; both production DNS patches and the final core composition stay unchanged.

The four real cross-language scenarios are acquire/release, acquisition refusal,
recovery-required release and loss after Ready. All execute twenty times;
bounded duplicate-free JSON must prove parent and every named subcase run/pass
in valid order, with no skipped/missing/foreign/extra case. Exit zero or parent
PASS alone is insufficient. Copied inputs are rechecked after execution.
Default conditional/DNS matrices retain their own exact checks. The existing
Go fixture joins its owned Rust child on normal/failure/timeout cleanup.

These peers use an ordinary regular-file proof and **synthetic** Ready/Released
acknowledgements. No real TUN, resolved operation, installed broker, package,
enrollment, system service or provider is exercised. PASS closes only the declared
synthetic wire gate on its exact source/artifacts, not matched managed-pair
attestation or host DNS restoration. Default output still says `not_run`; only
the explicit completed opt-in can print its four-scenario wire result.

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
