# S1 independent desktop readback candidate

Development checkpoint, 2026-10-01, stacked on the private persistent-dconf
experiment in #418. This slice remains in the opt-in `omavless-s1-observer`
crate and does not enable App proxy or write host settings.

`independent_desktop_readback` launches the existing fixed, pinned, read-only
observer helper as a new process. It compares the full typed desktop snapshot:
all 16 keys, effective/default/user layers and writability. A changed override
which equals the default is still a difference. A helper launch, timeout,
decoding or observation failure returns an error; it cannot be reported as a
matching persisted value. The expected snapshot stays in memory and neither
the comparison nor the runner prints private proxy values.

The installed-dconf fixture now saves an intended complete snapshot inside its
private temporary directory before its writer calls `Settings.sync`. After the
writer exits, a separate child reads the same private persistent database and
compares it with the intended snapshot. The successful service path matches;
the path where the owned dconf service is killed before commit differs even
though `Settings.sync` returned. The prior same-backend optimistic read and
exact restoration tests still pass. A synthetic test also distinguishes an
absent override from an explicit override equal to its default.

This is a read-side primitive, not a commit receipt or compare-and-swap lock.
A matching independent snapshot cannot prove an admitted write has stopped:
after timeout, lost reply or process death, a delayed request may still commit.
The future coordinator must establish a reviewed same-owner request-drain
barrier before treating readback as commit confirmation, and retain the durable
unknown-outcome journal whenever that barrier or independent observation is
unavailable. External desktop writers and ABA remain separate conflict gates.

The fixture uses only its own bus, dconf service, schema-backed private database
and child processes. No default desktop proxy values, manager environment,
profiles, real session service, VPN/TUN or route state are read or changed.
The normal runtime and package do not call this helper; write admission still
unconditionally refuses. Installed session provenance, per-field journaled
writes, restoration under crash/foreign edits, owned listener and new-app
consumption remain open.
