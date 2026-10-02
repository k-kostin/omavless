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
