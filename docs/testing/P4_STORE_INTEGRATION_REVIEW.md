# P4 private-store integration review

Review date: 2026-10-01. Baseline: private-record Draft #412 at
`b0af70a447e88288321b3de5cc184861bdb6a92b` (use the owning PR for exact
tested successor identity). This review follows structured guest Draft #410.
It adds production-domain negative regressions, not a new store format or
runtime integration. No private fixtures, host transitions or installed tests
are involved. P4 remains unavailable in the product.

## Decision and observed integration gaps

The [private record codec](P4_PRIVATE_RECORD_CODEC.md) is a useful prerequisite,
but inserting its JSON into the current store is not a bounded integration.
The following are code-backed architectural constraints, independent of the
availability of a real WireGuard or AmneziaWG server:

| Current boundary | Integration consequence |
| --- | --- |
| `private_store::PrivateProfile` requires both `uri` and `CanonicalProfile`; `parse_private_store` parses every URI and checks its protocol | A WG record cannot enter the existing typed store; inventing a synthetic URI would create a second secret representation and violate the no-invented-share-scheme contract. |
| `canonical::CanonicalProfile` and `Protocol` cover the four existing URI families | Extending the shared enum immediately affects current rendering, diagnostics, details, probes and subscriptions; an inactive storage codec must not implicitly make those operations available. |
| `normalize_store_state` clears unknown active/last/pinned IDs and derives defaults from the profile list | Filtering WG entries out before invoking the old whole-store parser loses their references and may disable pinned startup. A mixed-store validator must validate one combined metadata graph first. |
| `parse_private_store` initially uses `serde_json::Value` | Re-serializing a nested WG object before passing it to the strict codec has already discarded duplicate keys. Duplicate rejection must happen on original bytes, before conversion to `Value`. |
| Existing normalization preserves unknown extension fields | Stashing a WG envelope in an extension of a v3 URI row does not validate or activate that envelope. It could be retained as opaque private data and ignored by ordinary operations; this is not a supported integration mechanism. |
| `profile_export`, `profile_edit_input`, `apply_profile_import`, replacement and subscription sync retain URI semantics | A loader alone is insufficient: consumers need an explicit tagged credential contract, or explicit unsupported results, before records become writable through the owner. |
| `PreparedPrivateStoreWrite` supplies locked exact-byte publication/restore; preflight and bootstrap call the current complete validator | A second WG file would introduce new consistency/recovery obligations. The existing single-store transaction should remain the eventual publication boundary. |

Current consumers are in
[`private_store.rs`](../../crates/omavless-domain/src/private_store.rs);
the proposed interface names below are design descriptions, not implemented APIs.

No safe inactive v4 loader is implemented in this checkpoint. A partitioned
loader would duplicate validation or lose pointers; reusing `PrivateStore`
would expose unfinished behavior through its existing methods. Merely producing
v4 bytes without a complete reader would create an unusable private artifact.

## Next implementation seam

The next bounded slice should first separate complete store validation from
runtime-capable profile access. Keep the installed `parse_private_store` entry
point and version limit unchanged until a separately reviewed activation.

1. Introduce a private internal credential sum type with an existing-URI case
   and a structured-WG case wrapping the #412 model. Keep explicit redacted
   formatting and no implicit serialization. WG/AWG flavor comes from the
   validated record; any stored discriminator must agree with it.
2. Build an inactive `CandidatePrivateStore` validator over original bounded
   bytes. Reject duplicate keys at all depths before extracting either case.
   Validate the entire shared profile/subscription identity graph once, then
   normalize pointers once. Reuse existing metadata/rule validation rather
   than constructing placeholder URI profiles or partitioning the graph.
3. Keep existing URI record semantics and extension retention explicit.
   Candidate structured rows must have exactly one credential source, never
   both `uri` and a WG envelope. Initially reject provider-managed WG rows;
   provider identity/update/deletion semantics require their own scope.
   The candidate type exposes only safe counts and deliberate private
   encode/decode operations, with no conversion to runtime `PrivateStore`,
   connection selector, renderer, probe or registered control capability.
4. Select the complete candidate schema and old-reader refusal together.
   A version bump must be explicit; #412's record `schemaVersion: 1` is not
   the whole-store version. No runtime migration, write or downgrade is
   implied by successfully validating an in-memory candidate. Never drop WG
   rows to manufacture a v3 downgrade.
5. Only after that contract has tests, prepare a separate owner-bound write
   slice using the existing private file policy, migration lock, exact owner
   generation/revision/replay admission, same-byte comparison, atomic
   publication and verified rollback. Backup/restore must reject unsupported
   schema or preserve and validate every credential variant. Coordinate with
   the separately stacked T4 complete-store admission work before enabling it.

Read-only candidate work can proceed with synthetic keys. Product exposure
still requires explicit typed export/editor behavior, bounded preview framing,
redaction of WG/AWG key material including header-protection keys, canonical
endpoint/probe integration, installed-core compatibility/version gating and
the real-server lifecycle/mode/IP-family acceptance matrix. Those are distinct
gates; lack of real keys does not justify bypassing the architectural work.

## Required regression matrix for that seam

- Existing VLESS/Trojan/Hysteria2/TUIC v1-v3 fixtures keep canonical behavior,
  IDs, ordering, favorites, subscription membership, active/last/pinned
  references, rules, preferences and preserved extensions.
- Mixed URI/WG/AWG candidates retain references to every valid member; duplicate
  IDs across the whole graph, invalid subscription references, conflicting
  credential sources, discriminator mismatches and unsupported versions reject.
- WG-only stores do not acquire defaults from an accidentally empty legacy
  partition. Candidate validation must not mutate the source snapshot.
- Original-byte duplicate keys at the root, row and nested record reject,
  including escaped-equivalent key spellings. Unknown WG fields, injection,
  malformed encodings, excessive nesting and aggregate bounds reject safely.
- Complete-store byte and profile-count limits include structured records;
  private encoder output must reload losslessly and stay within the same bound.
- Safe counts/errors/Debug and ordinary IPC projections omit reusable keys,
  private endpoints and credential-bearing serialized records.
- The production loader/import/replacement and current owner reject unsupported
  candidates before publication or host effects. Cross-family mutation,
  subscription refresh, backup/restore and failed-write rollback preserve all
  unrelated records and exact prior bytes where restoration is required.

## Evidence supplied here

[`p4_store_boundary.rs`](../../crates/omavless-domain/tests/p4_store_boundary.rs)
first proves its invented WG/AWG inputs pass the native parser and strict
private-record codec, then proves the current production-domain import and
replacement refuse both native and structured forms. Mixed v3 rows using WG/AWG
discriminators reject both with and without a decoy URI; an unsupported v4
document rejects through reads and mutations. This avoids mistaking malformed
fixtures for proof that product activation is withheld. Assertions never print
private records. These tests are domain evidence, not an IPC/host acceptance
claim and not proof that arbitrary legacy extension fields are rejected.

Run the focused domain integration test plus `./tests/run.sh` and
`./tests/run-rust.sh`; final exact-head results belong to the owning PR.
Existing native parser/record-codec tests remain their own positive evidence.
No protocol fields or core mapping change in this review. The official
[Mihomo WG documentation](https://wiki.metacubex.one/en/config/proxies/wg/)
and [pinned v1.19.30 option implementation](https://github.com/MetaCubeX/mihomo/blob/v1.19.30/adapter/outbound/wireguard.go)
were revisited for the current one-peer/private-key boundary; this is not a
fresh full release/advisory audit or evidence of compatibility with newer cores.
Repeat that audit before any protocol/core integration PR.

No VM, bare-metal network, provider, AUTO-1, DNS or V0 result changes. Rust
remains the production owner, the Python archive remains frozen, and neither
merge nor release/marketplace publication is authorized by this checkpoint.

## Inactive v4 candidate follow-up

The subsequent `dev/p4-mixed-store-candidate` slice adds an in-memory
`CandidatePrivateStore` validator for a future version 4 document. URI rows
retain their v3 `uri`/`protocol` shape; a WG/AWG row instead has one
`wireguard` private-record object and a matching `protocol`. Original-byte
duplicate keys reject at every depth before JSON normalization. URI and WG
rows share one subscription/profile identity and startup-pointer validation
pass, including WG-only stores. Provider-managed WG rows, ambiguous credential
sources and unsupported versions are refused. Private re-encoding roundtrips
the complete document; public methods expose counts and pointer-presence only.
An in-memory migration from a fully validated v1-v3 document additionally
rejects original-byte duplicates, preserves legacy extensions and pointers,
and can prepare one standalone WG/AWG candidate without writing any bytes.

The next inactive normalization slice makes the deliberate private byte export
write the candidate's validated active/last/startup pointers, rather than
re-emitting stale source pointers that the safe projection already cleared.
Unrelated root, profile and startup extensions survive. Standalone WG append
also refuses a name already used by any profile and an ID already used by a
profile or subscription, matching the existing import collision boundary.
Focused synthetic tests cover stale references, extension retention and the
cross-graph collisions. This remains an in-memory contract only; the installed
owner still refuses v4 bytes and there is no file writer or user-facing WG.

The next inactive metadata slice adds in-memory standalone rename and favorite
updates across the complete mixed profile array. It follows the v3 rules:
subscribed URI rows cannot be renamed, profile names remain unique during
rename, and both standalone and subscribed rows can be favorited. Synthetic
tests preserve exact WG/AWG private record objects, URI, extensions,
subscriptions and active/last/startup references across those updates; missing
IDs and ambiguous names refuse. There is deliberately no delete/replace,
filesystem publication, IPC method or lifecycle transition in this slice.

### Inactive mutation and persistence prerequisite

The successor adds typed candidate URI/WG inputs for standalone import and
replacement, standalone deletion, and an explicit private credential export.
Export reports its format: a URI remains a stored URI, while WG/AWG remains
the strict versioned private record. No native `.conf` or QR export is claimed.
Replacement preserves ID, favorite and active/last/pinned references; URI
extensions survive URI replacement. Converting an extended URI row into the
strict WG shape refuses instead of dropping extensions. Deletion clears only
references to its target and selects the first remaining member of the complete
mixed array for a deleted last selection. Managed URI deletion/replacement
refuses. Candidate loading also rejects profile/subscription ID collisions.
Existing v3 URI delete/replace outputs are the independent semantic comparison
for the URI case; synthetic mixed WG/AWG tests verify unrelated credential,
subscription, pointer and extension preservation and typed export restoration.

The inactive `candidate_store_transaction` runtime module composes this complete
validator with the existing exact-byte private-store writer. Preparation holds
the matching migration lease and pins a durable committed Rust generation;
commit and restore recheck that same lease/generation. Whole-store validation,
private parent/file rules, atomic `0600` replacement, exact source-byte
comparison and byte-exact verified compensation are reused. A semantic no-op
does not replace the file. Concurrent edits and revoked/stale/unsafe ownership
refuse; restoration never overwrites unrelated bytes. Tests publish only
synthetic candidate files beneath a disposable private directory.

Crucially, this constructor accepts **already-v4 stores only**. It cannot
migrate or replace a production v1-v3 source with v4, and it has no daemon,
IPC, CLI, bootstrap, backup, renderer or core caller. Production
`parse_private_store` still refuses v4. Thus no installed store migration or
product activation is performed, and successful candidate validation gives no
permission to write an unreadable installed artifact. Publication experiments
must remain isolated synthetic files. Exact check results and tested SHA belong
to the owning Draft PR.

The next integration must bind this persistence primitive to the one native
owner's revision/replay/recovery admission and active-profile lifecycle
compensation. It must also select a reviewed v4 startup/old-reader refusal and
rollback policy without dropping structured records, compose typed editor and
native export/preview framing, and cover every subscription/backup consumer.
Installed-core compatibility/version gates and matching real-server modes,
lifecycle, IP-family and privacy gates remain prerequisites for user exposure.

The October 2 storage follow-up revisited the official WG documentation and
pinned `v1.19.30` option implementation linked above, plus the
[current `v1.19.32` release notes](https://github.com/MetaCubeX/mihomo/releases/tag/v1.19.32).
It changes no native parser fields or Mihomo rendering/minimum-version policy.
This limited storage review is not installed-core compatibility, a full
intervening-release/advisory audit or permission to use new upstream fields;
those checks remain required for protocol/core activation.

### Inactive private-file editor/export readers

The native-export successor adds canonical native conf and private standalone
editor seeds in the domain. A separate reader follow-up binds those deliberate
private reads to an actual already-v4 `profiles.json` file. The matching
migration lease and committed Rust generation are checked before loading and
after projection; the whole original-byte document validates, and an exact
second file read rejects unexpected concurrent edits. Private parent/file,
permission and symlink rules reuse the existing store boundary. Unsafe,
revoked/stale/missing ownership, corrupt unrelated rows and missing targets
refuse without releasing a projection or changing private state. Managed URI
editor refusal remains the domain contract.

The isolated full-file regression composes private editor/export reads, native
reimport, standalone replacement, atomic publication, subsequent editor read
and byte-exact compensation. It preserves unrelated URI credentials and
active/last/pinned/extension state; no-op reads preserve file inode and bytes.
Injected file and owner-generation changes during projection discard the
private result. Synthetic file modes are restrictive before payload writes.

These remain **inactive library prerequisites**, with no registered IPC,
semantic CLI, frontend, startup, v3 migration or core/lifecycle caller. The
production owner still refuses v4. The future bridge must provide explicit
same-user action/authentication and reviewed bounded credential framing, then
complete revision/replay/recovery, lifecycle, old-reader/rollback and all
consumer gates described above before any user exposure.
