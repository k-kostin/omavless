# P4: disconnected candidate validation-input capture

Source-only successor to #756 documentation head
`1c22e9127f75364e1cfa37161cd23982df67834f`. This is immutable **input capture**,
not a validator, host/startup bridge, permission grant or product activation.
No core, bwrap, namespace, network, VM or privileged action is performed. Rust
compilation and all new controls remain unrun pending Root's resource release
and exact-source boundary review.

### Original compile-only refusal retained

At source `036a02b386f657739ee43299ef0b56120a093598`, the bare non-test
default check (original session `16966`) and feature check (`3586`) both exited
0. The strict all-target clippy original `91724` exited `101` before any test:
the new test fixture referenced `test_temp::home_parent`/`directory_under`, but
the older #756 lineage only supplies the existing exclusive `directory` helper.
No normal control, authentic DATA row or validator/core execution ran. Dependent
gates stopped and target/heavy custody was returned after known closure.

The narrow test-only successor uses that existing `directory("p4-capture")`
under the explicitly selected, verified owned HOME `0700` TMP. These resource
fixtures bind no pathname socket, so no new short-parent API or allocator is
needed. Production capture/reader/renderer/execution source remains unchanged.
Corrected gates need their own immutable source selection; this original
compile failure is not replayed or relabelled PASS.

## One closed constructor, not config/manifest pairing

The explicit `p4-candidate-preparation` feature exposes
`isolated_validation::candidate_capture::CandidateValidationSnapshot` (also
compiled for unit tests). Capture accepts the same validated complete v4 store,
off desired intent, selected profile ID, raw template and actual core bytes as
the #756 preparation boundary, plus the trusted resource directory/UID. It has
no prerendered configuration, supplied digest/flavor or caller resource manifest
argument.

The existing exact RU/CN/IR bundle recognizer must accept the raw template.
Only the checked-in bytes and their canonical Rule/Global/Direct mode-line
variants qualify. Added whitespace, custom options and rendered private configs
refuse, rather than being paired with another template's manifest. Canonical
mode semantics are unchanged: the existing renderer applies the validated
desired mode to the recognized bundle; a rule-line bundle can render Global or
Direct without a new same-mode restriction. The controller is always
`/work/mihomo.sock`, never caller-selected.

The #756 constructor then enforces valid off intent, no candidate active
pointer, credential-derived flavor, bounded/static x86-64 exact core bytes and
private configuration rendering. Unknown core bytes and AWG 1/2 remain refused.
Only after that preparation succeeds are resource files opened. The core pin
remains the one tested compatibility artifact, not a minimum-version,
source-build/provenance or execution grant.

The opaque result owns the same prepared/core carrier, exact recognized raw
template and all mandatory resource bytes. Debug is fixed/redacted; there is no
Clone/Serialize/Display, execution conversion or host method. Explicit private
accessors are not public diagnostics, validation results or cleanup authority.
No secure-erasure, live-cache identity or hostile same-UID attestation is made.

## Existing resource primitive and default-v3 compatibility

Only the existing resource-acquisition loop is extracted into private
`capture_resources`. Membership still comes from the existing exact manifest:
23 RU, five CN and seven IR MRS files, all mandatory even in Direct mode.
Extra cache files are not adopted. The private data/ruleset directory policy,
held directory FD, no-follow/nonblocking ordinary-file reads, UID/mode and
before/after metadata checks are unchanged. Limits remain eight MiB per file,
64 MiB total and at most 32 trusted resource names. Missing, empty, writable,
symlinked, wrong-owner or oversized input refuses; nothing is fetched/repaired.

Existing v3 `ValidationSnapshot::capture` retains the exact ordering:
recognized manifest -> desired validation/connected check -> data directory and
FD resource acquisition -> original private renderer. Its signatures, config
bytes and failure classifications are unchanged. `read_bounded`, `validate`,
static ELF handling, Scratch creation/verification/cleanup, sandbox arguments
and execution/timeout routine are untouched. No lifecycle/owner/lease/startup
call is wired to the new capture.

Captured bytes prove neither valid MRS content nor provider freshness, loaded
rule counts, DNS/route/TUN readiness or successful engine configuration parsing.
Capture reads only the supplied resources and does not prove empty host or
acquisition/ownership authority. Future owner integration must still bind its
revision, lease, exact generation and mutable input snapshots before effects.

## Prepared controls and remaining gate

Seven normal controls are prepared, not yet executed:

- Complete declared membership across all bundles/mode variants, ignoring extra
  files and retaining immutable copied bytes after source mutation.
- Invalid off/active/unknown-core refusal before resource acquisition.
- Arbitrary/prerendered/modified template refusal before manifest selection.
- Missing, empty, symlink, UID, writable, directory and per-file size refusal.
- Mandatory total-byte bound independent of per-file limits.
- Existing v3 error ordering and private rendered-byte/resource parity.
- Fixed errors without private labels, URIs or resource-path fragments.

One separately ignored DATA-only control requires an independently admitted
public `0400` exact core projection. It prepares nine bundle/mode captures with
synthetic cache bytes, privately compares configuration/core/resources and
checks carrier redaction/generation/mode. It never executes those bytes or the
validator. No external binary is embedded in Git, and a missing selection fails
the explicit opt-in rather than silently passing. This future row would prove
only real-byte capture compatibility; synthetic MRS bytes are not valid-engine
resource evidence.

After source review, run only the capture/default-compatibility source controls
and plain non-test/default-off feature checks under separately released target
custody. An authentic DATA row needs its own Root selection. Resource-aware
execution, original finite completion, scratch lifetime/rechecks, host-empty and
owner fences remain a separately reviewed next slice. Production v3, default
features, active replacement/recovery and all P4 exposure gates remain closed.
