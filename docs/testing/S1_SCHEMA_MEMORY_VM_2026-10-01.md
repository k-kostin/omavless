# S1 installed-schema memory-backend contract in Omarchy Dev VM

This is a scoped public-schema/GIO semantics gate, not writable App proxy
acceptance. Tested source was `b3704947fa4474f0b15faebe860d3e797d670083`,
stacked on #401. The exact all-feature `omavless-s1-observer` test executable
had SHA-256
`a3566c9347a1ff5bb474a99cdefb56a30490ee8811763d0f0570eb099dfaf65e`;
its digest matched after transfer to the guest.

The delegated Omarchy Dev guest was verified as user `kdk_vm`, x86_64 and KVM
before execution and cleanup. Installed versions: kernel `7.2.5-3-omarchy`,
`glib2 2.88.3-1`, `dconf 0.49.0-1`, and
`gsettings-desktop-schemas 50.1-1`. Schema/backend/module/profile override
variables were absent; no override was introduced to make the check pass.

Under a 20-second enclosing timeout, all three opt-in tests passed:

- `installed_schema_supports_typed_memory_override`;
- `installed_schema_all_fields_restore_exact_memory_override_presence`;
- `installed_default_backend_is_the_supported_dconf_type`.

The full-field fixture validates the five fixed installed schemas and all 16
key paths/types/ranges through the existing observer. Each case creates an
explicit fresh GIO memory backend and checks that the Settings object uses
that exact backend. It covers absent override, explicit value equal to default,
and explicit empty strings/lists where representable (the third case repeats
default for other types). There are 48 field/case iterations. Each applies a
typed synthetic value, checks all other fields remain unchanged, then restores
the exact prior user layer with set or reset as appropriate. Full canonical
snapshot equality checks effective/default/user/writable data after restoration.
Synthetic strings include quotes, equals signs and a newline; none is executed
or used as an endpoint. No manager environment is read.

Every write/reset targets an explicitly supplied memory backend. The separate
default-backend test constructs a Settings object and checks its backend type
only; it does not read proxy values or write to dconf. All test output contains
only fixed test names/results. The temporary guest executable and its dedicated
directory were removed afterward; no package or runtime installation occurred.

Local all-feature observer tests passed with these three installed-only tests
ignored, and strict all-feature/all-target crate Clippy passed. The installed
gate above deliberately runs those three ignored tests in the controlled VM.
Results apply to the exact source/binary and guest versions listed here.

This proves public-schema compatibility and exact memory-backend restoration
semantics. It does **not** prove dconf persistent write/flush behavior, locked
administrator layers, foreign-write races, session/backend/profile continuity,
manager/broker/activation provenance, owned listener readiness, new-app
consumption or installed crash recovery. It does not promote unverified
observations into authority or change unconditional S1 write refusal. Physical
PC, ARM64 and NixOS evidence are not claimed. Remaining gates are retained in
the [observer contract](../development/S1_READ_ONLY_OBSERVER.md) and
[S1 foundation](../development/S1_PROXY_FOUNDATION.md).
