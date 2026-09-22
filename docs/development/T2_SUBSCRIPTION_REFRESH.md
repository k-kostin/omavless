# T2f: explicit single-subscription refresh

Development client slice on T2e, not the complete T2 MVP or a package release.

On Profiles, select a node from the intended subscription and press `s`.
The confirmation names both the selected node and its source subscription;
Enter fetches that subscription's server list through the existing fenced
`plugin.action` / `subscription-refresh` path. Esc cancels without dispatch.
`r` only reloads the UI. Standalone profiles never silently select an unrelated
subscription, even when only one subscription exists. A feed-missing node may
still identify its source for a refresh.

The action sends only the opaque subscription ID, runtime instance, expected
revision and operation ID. It carries no URL, profile credential, mode or
Connect intent. The daemon owns bounded HTTPS/feed validation, atomic store
updates and any active-session reconciliation. Refresh may therefore require
the existing host authorization; it does not bypass networking policy.

Selection, source identity/name, runtime instance and revision are checked again
at confirmation. Search text, inspection pages, repeated key events and cancelled
confirmation do not dispatch provider requests. The existing one-pending-action
lock and exact-retry/unknown-outcome handling are reused, with no automatic retry
or private remote error rendering. Closing TUI does not cancel accepted work.

The footer exposes `s` while browsing and preserves the system authorization
hint during confirmation/execution. English/Russian screens remain plain text.

## Checks and boundary

Seven focused tests cover exact request shape, standalone/stale/filtered targets,
changed confirmation context, keyboard ownership/cancel, receipt fencing/error
privacy, exact retries and EN/RU minimum-size confirmation rendering. The
feature-enabled canonical runtime parser test now admits the real subscription
refresh envelope as well as connection/mode actions; no production runtime
implementation was changed.

Full local Rust validation and developer/QML checks are required before the
candidate is considered checked. Real Foot synthetic confirmation screens were
inspected in both languages. Live provider refresh is a separate attended gate,
not inferred from synthetic callback success. Record exact live evidence in the
candidate PR/RC ledger after execution.

This slice does not add refresh-all, scheduling, subscription editing, an empty
subscription selector or persistent last-success presentation. Empty feeds have
no selectable node and remain managed through the existing plugin. Those
limitations, probes, activity/details and Open app/default packaging remain T2
work; no claim of full subscription management or completed T2 is made.
