# P4 private record codec

This inactive Rust-only codec follows structured guest import Draft #410 at
`08c80c3b3d91482e9d1a413e22cbb6ddc07b937a`. It adds no production caller,
filesystem write, IPC capability, runtime store schema migration, CLI command,
network effect or Python dependency. P4 remains unavailable in the product.

## Storage contract

Version 1 is a JSON object with exactly `schemaVersion: 1`, `interface` and
`peer`. The latter are maps from the existing strict native parser's lower-case
field vocabulary to private string values. There is exactly one interface and
peer; unknown fields and alternate field spellings are rejected. Flavor and
AWG generation are derived from validated credentials rather than stored as
potentially contradictory metadata.

The explicit `WireGuardProfile::private_record` operation writes canonical
addresses, endpoint, optional DNS/MTU/preshared key/keepalive and complete AWG
values. The keepalive range and every AWG generation field survive unchanged;
Mihomo's separate lower-bound normalization is not persisted as source truth.
Source comments, original guest container, labels and administrator metadata
are absent. Equivalent native, direct guest and structured guest inputs share
one private representation after validation.

The codec admits at most 128 KiB of UTF-8 JSON. It rejects duplicate object
keys at every level, trailing JSON, wrong types, unsupported schema versions,
unknown native fields and control-character/whitespace line injection. It
constructs native sections only after field admission, then invokes the same
bounded WG/AWG parser. Encoding also reloads its output and compares private
identity before returning it: canonical expansion near a native line/total
bound fails explicitly instead of creating an unreadable record.

The returned opaque record has redacted Debug and no implicit Serialize,
Display or Clone. Its explicitly named private-byte accessor is intended for
a future same-user persistence boundary. All errors use fixed safe classes;
serde errors and private fragments are never returned. This does not promise
memory zeroization, encryption at rest or secure deletion.

## Verification and remaining gates

Synthetic tests cover standard WG plus AWG 1/2/3/3.1, byte-stable repeated
encoding, identical private identity and Mihomo output after restoration,
equivalent import sources, duplicate root/nested keys, schema/type/unknown
field failures, native field validation, line injection, required fields,
invalid UTF-8 and the byte bound. Fixtures are deterministic invented data;
no real credentials are used. Assertions never dump private records.

Run `cargo test -p omavless-profile --locked private_record` for this slice and
the normal `./tests/run.sh` / `./tests/run-rust.sh` repository gates. Exact-head
results belong to the owning PR. This is new P4 functionality against the
existing native parser contract, not a Python migration/parity claim.

Future work must compose this representation with private-file ownership,
mode/symlink rules, complete-store validation, owner/revision/replay fencing,
atomic publication and rollback. Runtime import/replacement/export and
subscription backup behavior need their own reviewed integration. Real matching
WG/AWG servers, installed-core versions, modes/lifecycle, IPv4/IPv6 and privacy
acceptance remain required before exposure. No VM or live-network result is
inferred from codec tests; AUTO-1, DNS/provider and V0 remain unchanged.

The [store integration review](P4_STORE_INTEGRATION_REVIEW.md) identifies the
current URI-only consumers, mixed-store pointer and duplicate-key hazards, and
the next inactive validation boundary. Its negative domain regressions keep
P4 out of current production import/replacement and store admission.
