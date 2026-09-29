# T4 subscription schedule planning boundary

This checkpoint adds an unused pure Rust planner for a future opt-in automatic
subscription refresh. The existing manual `subscriptions.refresh` and
`subscriptions.refresh_all` operations remain the only production entry points.
No preference is persisted, timer or service is registered, provider GET is
started, host state changes, or QML control appears from this change.

The preference defaults to Off. A future explicit enablement may select a
whole-second interval from 6 hours through 7 days. A successful attempt waits
the full selected interval. A failed attempt retries after 5 minutes, doubling
to at most 24 hours. The planner takes caller-supplied time and an exact native
owner generation/revision fence. It refuses stale fences and timestamp overflow;
clock rollback waits rather than triggering an early fetch. The planner's Due
result is advisory. A future scheduler must recheck owner and revision under
the same serialized owner immediately before admission and again at completion.

Activation requires a separate review of durable preference and attempt-state
schema, trusted time/restart behavior, one-worker registration, shared fetch
permits, cancellation/shutdown, provider failure handling, and privacy-safe
status. Installed ARM64 integration must check explicit enable/disable,
disconnect and ownership races; physical suspend/network transition behavior
requires its own host evidence. None of those gates is claimed here.

## Stacked private preference checkpoint

The next Draft adds an inactive, private `subscription-refresh-preference.json`
beside the ownership marker. A missing file means Off. Only an explicit call to
the new Rust setter may enable a 6-hour to 7-day interval. It acquires the
shared migration lock, proves the exact committed Rust generation, compares a
monotonic preference revision, and publishes a complete `0600` replacement.
Reads reject malformed, oversized, symlinked, public or wrong-generation state.
An ownership transition cannot silently resume a schedule saved by a prior
generation. An explicit Off choice with revision zero rebinds a valid stale
preference to the current generation; enabling it then needs a second explicit
choice against the returned revision. The setter has no socket, CLI or QML
registration.

The persisted file contains only schema, owner generation, preference revision
and interval. There is no URL, profile ID or attempt result. Before enabling a
timer, the next implementation must add durable attempt-begin/outcome history
and crash/restart disposition, bind scheduler admission to the serialized owner
revision, share the existing four-provider fetch permits, and recheck state
before commit. This checkpoint has no provider request or installed behavior.
