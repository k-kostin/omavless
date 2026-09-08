# R5 bounded native support report

The `diagnostics.export` checkpoint supplies a shareable configuration report,
not the live rule/provider `diagnostics.summary` and not complete host doctor
parity. The installed QML/Python owner is unchanged. Python remains the
production owner and migration oracle; it cannot be removed by this change.

## Contract and privacy

The request has exactly empty params. Fixed CLI: `omavless diagnostics export`.
The success envelope contains a schema-version-1 object with scope
`native_configuration`. No destination argument or daemon filesystem write is
accepted. Consumers may save its result using an explicit local private writer;
this checkpoint does not connect the existing QML export action.

Configuration includes only profile/favorite/subscription/custom-rule counts,
configured preset and rule-update timestamp, stored startup enabled/target/mode,
startup-configured and onboarding booleans, and latest subscription timestamp.
All string values are canonical fixed enums; arbitrary extension fields, profile
IDs/names, rule destinations, provider identities, links, endpoints, credentials
and paths are excluded by construction. Inventory cardinality does not increase
output size. The result stays below 4 KiB, within ordinary v1 framing.

The report labels runtime lifecycle as `lastKnownState`; it is not a fresh core
health assertion. A pending routing transaction remains visible as a boolean.
Coverage explicitly says no live host observation, controller query or login
activation verification. There is no service invocation, network probe, log
reader, process enumeration, timestamp source or new cache. Disconnected native
owners can report their configuration; revoked/preparing/stale generations
cannot. Strict complete-store validation and the migration lock cover projection
creation. Invalid/unsafe stores return a fixed error instead of a partial report.

## Reference and intentional differences

`tools/support_diagnostics_parity.py` executes actual Python
`backend.diagnostics_payload` with load/service/controller/routing effects
replaced by synthetic facts. `run` is disabled to catch unexpected host work.
The 26-case Rust differential compares canonical SHA-256 digests of the common
configuration subset. It covers empty/populated stores, all five preset values,
Rule/Global startup modes, enablement, explicit profile selection, legacy
versions and ignored private extensions. Onboarding is the validated-store
boolean used by Python `status_text`, not a new interpretation.

Differences are deliberate and visible:

- legacy plugin/Python version and wall-clock generation time are replaced by
  native implementation/package version;
- OS/service/TUN/controller/file/conflict fields are omitted, not fabricated;
- configured preset is a store preference, not observed loaded policy;
- stored startup preferences never infer old systemd-unit enablement or current
  routing mode when startup has not been configured. Login migration remains
  separate from this read;
- the complete strict native store validator preserves the established legacy
  incompatibility refusal, rather than providing a permissive support parser.

## Acceptance

Deterministic domain parity, private-socket dispatch and actual executable CLI
tests cover exact input, public fields and size, no host/store/revision effects,
malformed/unsafe/symlink stores and revoked/stale generations. No installed
tunnel smoke is required: no host operation or installed launcher changes.
Full workspace/Python gates and exact tested SHA are recorded in the PR.

Remaining support migration: fresh host/core/service/conflict/file-readiness
facts, current routing counts/source, explicit private connection details,
settings-editor composition and installed QML export wiring. This checkpoint
must not be described as complete support diagnostics or Rust cutover.
