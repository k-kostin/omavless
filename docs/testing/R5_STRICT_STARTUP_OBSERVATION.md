# R5 strict startup inventory foundation

The existing process/TUN helpers are bounded, best-effort projections. A zero
can mean either no matching objects or incomplete observation. They must not be
reused as proof that a new login may safely start a core.

This checkpoint adds separate strict, bounded inventory functions. Existing
diagnostic/lifecycle callers keep their current behavior; no login trigger or
production empty-host adapter is activated here.

## Strict inventory contract

- A result is available only after complete bounded directory enumeration.
  Read errors and overflow refuse instead of truncating to an apparently empty
  or healthy result.
- Process lookup accepts a trusted bounded exact Linux command name. Numeric
  process entries and their `comm` files must have valid types; unsafe links,
  malformed/oversized input and unreadable observations refuse. Non-process
  entries still count toward the scan budget.
  `comm` is compared as bytes, allowing unrelated empty or non-UTF-8 names:
  the [Linux name API](https://www.man7.org/linux/man-pages/man2/PR_SET_NAME.2const.html)
  specifies a bounded null-terminated byte string, not a Unicode identifier.
- TUN lookup supports the symlinked interface directories used by real sysfs.
  A missing `tun_flags` attribute on an existing interface is an ordinary
  non-TUN device, but an inaccessible or vanished interface is not proof of
  absence. Enumeration and result counts are bounded.
- Errors contain only fixed public classifications, never process names,
  interface names, filesystem paths or raw operating-system output.

These are snapshots, not an atomic kernel inventory. Concurrent process exit or
interface churn may cause a conservative refusal. Future fixed host integration
must combine them with service state, controller **absence** (not merely failed
readiness), ownership locks and rechecks immediately before intent publication.
Hidden process namespaces or restricted procfs views cannot establish global
absence; the host contract must validate that it observes the actual host view.

## Remaining login integration

Do not construct `NativeLifecycleHost` solely to run validation: its cleanup
behavior is not read-only. Existing startup validation must be factored to
consume the transaction's exact store/template snapshot. Any temporary config
and Mihomo `-t` data directory must be explicitly isolated, bounded and cleaned;
calling validation against the persistent data directory is not a claim of
read-only validation.

Trusted once-per-user-manager ordering, legacy unit conversion, package
capabilities, and real login enabled/disabled acceptance remain #178/R5 gates.
Strict inventories alone do not satisfy them.

## Reference and validation

Python remains installed production owner and migration oracle. Existing
tolerant Rust helpers remain unchanged; strict failure semantics are a new
native safety boundary rather than literal Python count parity. Synthetic
filesystem tests cover ordinary results, incomplete inputs, unsafe file types,
bounds and error privacy. Full differential/reference suites remain required.
No installed plugin, service, DNS, route, core or private store is changed, and
no live VPN smoke is claimed or required for these unregistered helpers.
