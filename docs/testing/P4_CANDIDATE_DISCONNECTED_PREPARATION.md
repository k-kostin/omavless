# P4: opt-in disconnected candidate preparation

This source slice continues Draft #545 at
`868bd4db4b8c619545a8b1a80df557304431a649`. It prepares private inputs for a
future resource-aware validator; it does **not** wire candidate v4 into
NativeHost, startup, IPC, installed storage or the lifecycle owner.
Production v1-v3 parsing, default features, active replacement/deletion refusal
and recovery/lease/revision fences remain unchanged.

## Pure eligibility boundary

The runtime module is available only with the explicit
`p4-candidate-preparation` feature (and in unit tests). The default feature set
is empty. Callers provide an already-validated complete `CandidatePrivateStore`,
an existing `DesiredState`, rendering inputs and immutable core bytes. No
filesystem, environment, process, network, host observation or publication is
performed.

Preparation first requires valid off intent: `connected=false`, an empty
desired profile ID and the existing schema/generation constraints. Any
store-active pointer refuses, even if desired intent is off. Neither off intent
nor this pointer check proves a live host is empty. The selected credential's
validated type supplies its flavor; callers cannot grant eligibility with a
protocol discriminator, flavor claim, version string or supplied digest.

Only this exact compatibility selection is admitted:

| Core bytes | Selected credential | Pure preparation eligibility |
| --- | --- | --- |
| Reviewed x86-64 static Mihomo artifact below | Standard WG, AWG 3, AWG 3.1 | Eligible for later config validation |
| Same artifact | Existing URI families | Legacy rendering parity only; no provider/V0 acceptance |
| Same artifact | AWG 1 or AWG 2 | Refuse before rendering or any future quiesce |
| Any other bytes, architecture, dynamic ELF or version label | Any flavor | Refuse |

The selected SHA-256 is
`ba7a74ed6bbc3098930e6e172fa9b7d4ef7e42a77f7a516046d61fbeb3bfcfe6`.
It identifies the Mihomo Meta 1.19.31 x86-64 artifact exercised by the scoped
[#548 standard-WG](https://github.com/k-kostin/omavless/pull/548) and
[#552 AWG 3/3.1](https://github.com/k-kostin/omavless/pull/552) transport rows.
The implementation computes this hash from the actual supplied bytes, bounds
them to 128 MiB and checks ELF64/little-endian/x86-64 headers and bounded program
headers without `PT_INTERP` or `PT_DYNAMIC`. An artifact version label is not
an alternative input. This is one tested compatibility row, **not** an
earliest/minimum-version claim, a core source-build/provenance proof, permission
to discover or execute a binary, or a claim that every renderer option works.

Existing domain parsing/rendering validates the complete v4 graph, selected ID,
credential, template, mode, controller and rules. There is no duplicate protocol
parser. The result owns the exact core bytes, private config, private readiness
name, intent generation and mode. Its Debug is fixed/redacted; it has no
implicit Clone/Serialize/Display, filesystem path, execution method or lifecycle
capability. Deliberate private accessors are for a separately reviewed validator,
not public diagnostics. No secure-erasure claim is made.

## Checks and limits

Synthetic unit controls cover credential-derived flavors, strict v4 and
duplicate-key refusal, unknown core/version-label refusal, AWG 1/2 policy
refusal, malformed off intent and active pointers, bounded/static ELF shape,
four-family v3 URI rendering parity across all three modes, and private
Debug/error redaction. Private policy-table and carrier-model tests are clearly
distinct from authentic public-constructor admission. A synthetic ELF never
passes the public exact-byte core gate.

No actual core bytes are acquired, no real positive preparation is asserted and
no core, Go engine, namespace, VM or provider is executed by these controls.
Rust compilation/unit gates are pending while the separately owned master
acceptance row holds the heavy-build pause. Diff checks are source-only; they
are not acceptance evidence.

## Next bounded integration, not enablement

The useful next step is one opt-in **disconnected-only** validator that reuses
the existing isolated-validation resource/sandbox boundary with captured exact
core/config/resource bytes. It must retain current lease/revision/generation
ownership and prove the real host is empty before effects. A plain host
`mihomo -t` can fetch resources and is not an acceptable replacement. Acquisition
identity, resource closure, original finite command completion and final input
rechecks need their own reviewed recipe and exact-head evidence.

Only after that validator is accepted can candidate-aware NativeHost/startup
wiring be reviewed. It must reject unsupported core/flavor **before** any
quiesce, preserve production v3/default paths and refuse active replacement or
recovery until connection-pointer publication, compensation and canonical
restart/adoption are closed. This proposal grants none of those paths.
