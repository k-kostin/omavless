# Native rule-provider operation registration

This successor composes the [accepted bounded adapter](R5_RULE_PROVIDER_REFRESH.md)
with the same scheduler, terminal registry and mutation owner as subscription
refresh-all. It does not switch installed QML/Python ownership, add a second
daemon, or retire Python. The complete semantic contract is in
[`CONTROL_PLANE.md`](../roadmap/CONTROL_PLANE.md).

## Ownership and privacy review

The start boundary accepts only instance/operation/revision metadata. Provider
targets come solely from the validated live controller map, never client
names/URLs, raw controller methods or configuration. Ordinary responses expose
only progress counts, method/state and fixed errors. Controller identity and
private store/config fingerprints have no Debug or serialization surface.

Canonical connected Rule state, ownership, revision, desired generation and
exact private store/config bytes are checked under the shared lease. Those
checks perform no controller I/O. Bounded discovery and PUT work run outside
owner/migration locks and use the existing four-permit pool. Discovery occurs
before admission, so failed/no-provider/stale starts create no operation record;
retained operation-ID replay performs no rediscovery.

Transport pins socket device/inode and same-user peer PID during discovery.
Each connected FD is checked before any request bytes; after path replacement,
the old job cannot PUT to a successor. Final timestamp commit repeats a
nonblocking connect/metadata/peer check while holding the owner lease, sending
no HTTP request and waiting for no response. This closes the after-last-PUT
identity gap without holding the lock across controller reads or writes.

Successful native stamp is monotonic and all-success-only. All current store
data is preserved through complete canonical normalization; the private writer
verifies replacement and compensation. Failed/uncertain restore enters the
shared manual-recovery barrier. A failed job does not claim to undo already
successful core/provider PUT effects. Disconnect/cancel remain responsive and
prevent stale final stamp; cancellation cannot interrupt Mihomo's own fetch.

## Evidence boundaries

The primitive's actual-Python discovery/path and all-target/all-success oracle
remains required. Registration tests add shared method/ID namespace, replay,
monotonic timestamp, exact snapshot rejection, cancellation, no timestamp after
partial failure and socket replacement. A real private semantic socket test
uses synthetic Unix controller work held in flight to check prompt start/status,
retry, cancellation, disconnect and terminal publication. All fixture data is
credential-free; no real profile store or installed runtime is changed.

Installed-Mihomo adapter evidence uses synthetic loopback HTTP provider data,
no TUN and no core-owned TCP listener. It proves the actual core PUT behavior,
not a physical network, provider interoperability or installed native frontend
cutover. Python remains installed owner/oracle/rollback until those R5/R6 gates.

Exact candidate SHA, counts and required CI belong in the owning PR after all
affected local gates finish. Registration is not merge-ready merely because
the predecessor's adapter gate passed.
