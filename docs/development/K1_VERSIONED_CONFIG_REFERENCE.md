# K1 version-bound metadata capture candidate

Source-only successor to [the completed reference capture](K1_EFFECTIVE_CONFIG_REFERENCE.md),
based on `5e2103f68776b6de758bc49e99c2f98d4aec7c85`. All earlier source,
artifact pins, stages, outcomes and the separate pure parser remain unchanged.
This generation is capture-only: no admitted systemd version literal, text
parser call, unit Start, FullVPN lifecycle, installed helper or normal authority.

The new cfg(test)-only `manager_version_reference_fixture` selects only
`omavless-k1-versioned-config-reference.service` under a new literal private
stage. Its unit remains inert `/usr/bin/false` with RefuseManualStart=yes.
The existing pinned workspace zbus dev dependency is unchanged.

## Same-connection sequence

1. Resolve the systemd manager's unique owner once on the literal system bus.
2. Read `org.freedesktop.systemd1.Manager.Version` with Properties.Get to that
   exact owner, requiring variant-of-string, nonempty UTF-8, at most 256 bytes
   and no Unicode control characters. Do not trim or normalize it.
3. RefUnit the one literal own unit; capture the same strict typed Unit and
   Service observations and one nonempty bounded matching dump.
4. Read Version again on the same connection and unique owner while the
   reference remains retained. Require exact string-byte equality.
5. Write exclusive fsynced schema-2 version and configuration receipts. The
   version receipt records both typed values, owner and literal unit with
   `OBSERVED_VERSION_DATA_NOT_ADMISSION`; the configuration marker is
   `OBSERVED_VERSIONED_CONFIG_DATA_NOT_ADMISSION`.
6. Only complete receipt publication permits the sole UnrefUnit. Its known
   acknowledgment permits one combined post-Unref GetAll reply and separate
   fsynced schema-2 state/ack receipts before known successful exit.

Unknown but bounded version strings are preserved as metadata, not admitted.
Missing/wrong types, control characters, oversize or mismatched second reads
stop the sequence. The leaked connection and original stage FD are retained;
failure parks indefinitely with no retry, compensating Unref or destructor
release. Before-Ref failure does not imply a reference was acquired; after an
acknowledged Unref, retention does not claim the reference still exists.
The existing five-second reply wait, queue-eight and 1 MiB decoding bound
remain. Upstream may allocate a message before this tighter bound; connection
setup/send still require the separate outer owned-child deadline.

The dump remains private and opaque. This fixture does not invoke the earlier
pure parser with a guessed `261.2`, infer configured zero from runtime infinity,
or claim that version-string consistency proves a package executable digest.
Trusted manager/root, raw pinned unit identity and exclusive VM ownership
remain assumptions/gates, not properties supplied by the text parser.

## Source checkpoint and remaining gates

The native pure controls cover every request error and malformed reply in the
actual sequence, exact order/owner reuse, wrong variant signatures, unknown
version preservation, byte bounds/control characters, second-version drift,
create-only output collision, typed current state and no post-failure calls.
No ordinary test opens a bus or invokes the ignored capture entry.

The new fixed `version_reference_guard.py` and `version_reference_stage.py`
retain the complete previous observer/loader protections under distinct paths,
markers, schema-2 receipts and pins. The observer requires the separate strict
version receipt before post-state evidence can admit exact own-link cleanup.
It does not interpret an unknown version as an approved one. The raw snapshot
definitions remain pinned to the original query guard, loaded as definitions
only with the new fixed stage for private temporary files.

The frozen native build identifies
`b0c3ed886cf7d51851b0f4586d12c914166e613d`, SHA-256
`346eb5ce4f9a92a1fbb889923e71e65217dc1c9b831c983b4e39986df2e50af5`.
It is not the old b0af build. Native focused controls passed 11 with the actual
entry ignored; formatting and netguard all-target strict Clippy passed. The
native checkpoint source gate passed 589 Python tests with two skips and the
frontend/QML checks. Later outer/loader gates belong to their own exact head.

This checkpoint is not executable acceptance. Full affected gates, complete
root and independent source reviews and an explicit exclusive VM lease are
still required. Outer behavior must retain the reviewed one-invocation full
baseline and known-success-only own-link teardown contract. Unknown/nonzero or
invalid evidence must stop without later queries, signals, cleanup or retry.
No VM call, main/RC merge or release is authorized by this source change.
