# S1 manager environment layer counterexample

Inactive synthetic research, October 3, 2026. This characterizes a missing
restoration prerequisite; it does not implement a manager writer, trusted
capture/drain API, application/session admission or installed S1 availability.
The [owning foundation](S1_PROXY_FOUNDATION.md) and
[architecture boundary](../roadmap/ARCHITECTURE.md#12-appsystem-proxy-boundary)
still require exact prior state. Effective equality does not replace that rule.

## Existing observation loses layer provenance

The [host observation contract](S1_HOST_OBSERVATION.md) selects the complete
typed systemd `Manager.Environment` property. The opt-in observer's
`gio_host::read_manager_environment` reads that property and
`project_manager_environment` produces the fixed ten-variable
`EnvironmentSnapshot`, preserving effective absence versus present empty.
Both remain read-only and unverified; `Observation::admit_writes` always refuses.

At the already reviewed systemd source pin
`583679fe4924e0afbf4dffa861f8b7c6b7be87eb`, the
[`Environment` getter](https://github.com/systemd/systemd/blob/583679fe4924e0afbf4dffa861f8b7c6b7be87eb/src/core/dbus-manager.c#L199-L218)
calls `manager_get_effective_environment`. That
[`merge`](https://github.com/systemd/systemd/blob/583679fe4924e0afbf4dffa861f8b7c6b7be87eb/src/core/manager.c#L4158-L4211)
combines transient and client environment, with the client value winning.
`SetEnvironment` and `UnsetEnvironment` modify the client layer only. A
successful complete property query therefore cannot distinguish these baselines:

| Original transient | Original client | Captured effective | Client after apply B then restore captured A |
| --- | --- | --- | --- |
| A | absent | present A | present A: original client absence lost |
| absent | A | present A | present A: original client retained |

Both transactions can reach the existing pure planner's Released phase with
equal effective readback. In the first case a later transient change remains
masked by the newly created client override. The same ambiguity exists when A
is an empty string. This is a counterexample to exact layer restoration from
effective capture, not evidence of a reachable production defect: no manager
writer or production transaction currently consumes this observation.

## Bounded executable characterization

`app_proxy::transaction::tests::manager_layers` uses the existing
`EnvironmentValue`, full `State`, real private `FieldJournal` storage and
`Transaction`. Its two fixed ten-entry arrays model transient/client layers;
the desktop snapshot is synthetic and unchanged. Every emitted effect must be
a fixed `UserManager` field. Present maps to a synchronous model assignment,
including empty; absent removes that model client assignment. No D-Bus,
systemd/UWSM process, process environment, desktop setting, core or VM is used.
The model's `drain` acknowledges only its already-completed array assignment;
it establishes no real asynchronous request ordering or manager provenance.

Five tests run across all ten case-sensitive keys. They characterize the alias
above and subsequent transient update, a known original client override,
initially effective-absent restoration under unchanged layers, masked transient
drift surfacing after an unset, and indistinguishable external same-value ABA.
Unknown layer provenance is not manufactured from a fake Boolean.

Initially effective absent implies both model layers are absent. Restoring that
case by unset preserves original absence only while the underlying layers remain
unchanged. A transient edit while the owned client override is active can be
invisible until unset: the model then refuses confirmation, retains pending
journal evidence and never reaches Released. This conditional model case does
not admit an installed safe subset or prove absence of concurrent edits.

An external client edit away from and back to the intended value has identical
observable values and journal bytes. The comparator cannot distinguish it from
no edit. The ABA test records that limitation explicitly; passing it is not an
exclusive-ownership or foreign-edit recovery guarantee.

## Remaining gates and evidence boundary

Before manager write admission, establish the exact mutable-layer provenance
needed for original presence restoration, or retain conservative unsupported
classification where it cannot be established. Do not downgrade the contract
to effective-value restoration, destructively probe the installed manager,
guess client absence from equal values or edit UWSM files as a workaround.
Actual authenticated writer/session/lifetime binding, same-origin unknown-request
drain, crash takeover, journal/listener lifecycle, conflict escape and supported
new-application launch consumption remain separate gates. Default UWSM scopes
inherit their parent environment; manager updates alone do not repair that
parent or existing applications. The negative GNOME-settings applicability
evidence and bounded fresh-child consumer evidence remain unchanged.

Focused gate:

```sh
cargo test --locked -p omavless-runtime manager_layers
```

A counterexample test PASS means the information-loss example is reproduced and
bounded as described. It is not a manager restoration PASS, real-systemd execution
or global S1 acceptance. The owning Draft PR records exact source/check results;
no installed or VM gate is claimed by this source-only slice.

Local source-only verification: all five focused tests PASS across every fixed
key; strict runtime all-target clippy and workspace formatting PASS.
`./tests/run.sh` PASS (326 Python tests, two existing opt-in skips, all JS and
QML contracts). The initial source invocation was NON-PASS: three existing
private-output tests rejected HOME scratch because its ancestry contains `.git`.
Repeating with private short `/var/tmp` fixture scratch passed, with the guard
unchanged. The compiler target was independently created below HOME cache; it
did not replace any installed binary. Raw check logs remain outside Git. No
real systemd/GIO consumer or VM execution was performed for this slice.
