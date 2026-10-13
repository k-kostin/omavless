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

The normal controls acquire no actual core bytes and execute no core, Go engine,
namespace, VM or provider. The separately ignored DATA-only constructor control
requires an explicitly admitted public core projection; it never executes those
bytes. It checks Standard WG, AWG 3/3.1 and four URI families in all three modes,
AWG 1/2 refusal and changed-core refusal, compares config bytes privately and
reports only counts/hashes. A missing opt-in input fails rather than silently
passing. Exact corrected source gates and the authentic DATA result are recorded
below. Normal source suite: 276 controls,
two existing skips, native/JS/QML and 67-link documentation navigation passed.
Source-only formatting/diff checks passed; these are not engine acceptance.

### Qualified public DATA acquisition

Root selected, under owner authorization, the retained public
`mihomo-bin 1.19.31-1` package cache,
not the current host's 1.19.32 binary or any private/VM state. Bounded read-only
tar/zstd parsing (no package scripts or blanket extraction) found 15 members
and exactly one ordinary root-owned `0755` `usr/bin/mihomo` member. Its
61,886,168 bytes computed the exact selected SHA above. The original regular
single-link owner-held package metadata was stable before/after parsing; its
22,054,504 bytes hash to
`3c00443d9a0f6a1d32588464d09d6824a2ac7d058e4efad9829653aa5873f359`.
The payload was projected exclusively as non-executable `0400` data beneath a
fresh owned HOME cache scope and independently hashed/read back. The public
DATA receipt SHA is
`c2f0d1c819d0084e3290aa72503cb7ee67dfc79fd41e5b9527b7761ad41922fc`.

Local public AUR source history `cd2be9b0a19b4c06eddc85c1038b9383119c8291`
pins the x86 package input to the official
[compatible v1.19.31 release asset](https://github.com/MetaCubeX/mihomo/releases/download/v1.19.31/mihomo-linux-amd64-compatible-v1.19.31.gz).
The official release API independently enumerated asset `563462721` with
22,821,836 compressed bytes and SHA-256
`04cf9f09671704f839ddbee2e93069dc831a4123a75281e725d1d96ab9ac1afc`,
matching that historical PKGBUILD. No download, signature verification, package
installation or source-build attestation was performed. Container/package
metadata is qualified supporting history, not a substitute for the exact
expanded core-byte pin. DATA acquisition is not constructor/validator/transport
PASS. The separately selected constructor DATA row below establishes only pure
preparation eligibility, not resource-aware validation or core execution.

### Original fixture refusal preserved

The first focused Rust gate at `57e3de93851b72e56efba66ffa8ee1a53db95160`
(original session `39165`, exit `101`) compiled successfully, then reported
three controls passed, five failed and one DATA-only control ignored. All five
failures reached the existing strict candidate parser's `InvalidShape` before
preparation: the shared synthetic fixture supplied `startup.enabled=false` but
omitted required `startup.target` and `startup.mode`. Dependent gates stopped;
no DATA constructor row or core execution ran.

The narrow test successor supplies the existing complete disabled startup
shape (`last`, empty profile ID, `rule`) and retains the original incomplete
shape as a refusal control. Production parsing, eligibility and host paths are
unchanged. The corrected gates below belong to their new immutable source;
this original failure is not replayed or relabelled PASS.

### Corrected exact-head source and constructor DATA result

All five serial originals below tested
`5cc3581ea852d625b2bac2828bc721220d5ee2e8`; the preparation implementation
is unchanged from reviewed `10de89f47e374b2d273f09711eb5b495a71599c7`.
The existing maintenance target was explicitly loaned, one build job was used,
and target custody was returned after every original child completed. No
installed-core or rule-cache opt-in was inherited. Later documentation-only
heads do not acquire tested-source or core-execution credit.

| Original gate | Original result | Scope |
| --- | --- | --- |
| Focused runtime controls, session `37486` | Exit 0; 8 passed, 1 DATA-only ignored | Hermetic eligibility/refusal/redaction controls |
| Existing domain candidate contract, command chunk `661771` | Exit 0; 9 passed | Typed rendering/pointer and four-family v3 parity |
| Feature-enabled runtime all-target strict clippy, session `41090` | Exit 0 | `p4-candidate-preparation`; warnings denied |
| Default-off runtime library check, session `81698` | Exit 0 | `--no-default-features`; no product registration |
| Explicit authentic constructor DATA, session `95325` | Exit 0; 1 selected test passed | 21 accepted, 7 refused; original body 21.85 s |

The DATA positives were Standard WG, AWG 3 and AWG 3.1 in Rule/Global/Direct
(nine rows), plus VLESS/Trojan/Hysteria2/TUIC in those same modes (12 rows).
All 21 returned configs matched the existing private domain rendering bytes;
only synthetic config hashes/counts were reported. AWG 1/2 in all three modes
refused (six rows), and one changed core byte refused (one row). The selected
`0400` ordinary single-link core data was FD/path-stable and exact-hash checked;
independent post-row readback retained the original core and DATA-receipt hashes.
No source fixture or external binary is embedded in Git.

This is a successful real-byte **pure constructor** row, not a Mihomo config
check, transport, live host-empty proof, startup, TUN/namespace/provider,
production activation or source-build/provenance acceptance. Full workspace
Rust gates are not inferred from these focused results. The next validator API
plan remains design-only.

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

### Concrete existing API seam and bounded gaps

`isolated_validation::ValidationSnapshot::capture(data, uid, desired, store,
template)` currently requires **connected** intent and `PrivateStore` (v3).
It recognizes exact RU/CN/IR bundles and canonical mode-line variants, requires
every declared MRS cache, holds the ruleset directory FD and reads each ordinary
file stably (8 MiB each, 64 MiB total, at most 32). This capture is not directly
usable with candidate v4/off intent. `validate(core_path, scratch_parent, uid)`
then separately reads a static core file; it cannot consume the already captured
#756 exact core bytes without a narrow internal seam.

The next finite change should factor those existing resource-capture and staged
execution primitives internally, retaining the production `capture`/`validate`
behavior and signatures. Add only a feature-gated candidate constructor that
receives the same owner-captured candidate/off intent/template/core byte set,
recognizes the exact bundle, renders with fixed `/work/mihomo.sock`, and owns the
result plus its exact mandatory resource bytes. Do **not** combine an arbitrary
already-rendered config with a caller-selected bundle: the present pure carrier
does not retain template/controller identity, so that pairing could miss reached
inputs. Reusing the same constructor input set closes this gap without a YAML
blacklist or caller-defined resource manifest.

Reuse `Scratch`'s exclusive `0700` directory and held inode anchors, its fixed
`0500` core / `0400` config/resources, and `/usr/bin/bwrap` with mandatory
user/network/PID namespaces, disabled further user namespaces, cleared
environment, dropped capabilities, read-only input/root, 16 MiB writable work
tmpfs and fixed `core -t -d /work -f /input/config.yaml`. Preserve the original
three-second bound; never fall back to host `-t` or install prerequisites.
The existing `run` discards output and ignores terminal kill/wait errors: the
new path must prove original finite child completion before claiming validation
or permitting scratch removal. Unproved child/cleanup remains failure with
retained scope, not a weaker cleanup or retry authority. This is a scoped
completion correction to the existing launcher, not a new supervisor framework.

Owner integration can stay inside `native_coordinator::candidate`: reuse
`candidate_generation`, revision/recovery checks and existing-only migration
lock admission from `candidate_private_read`, then the locked candidate reader's
authorize-before/after and exact source-byte comparison. Keep the migration
lease for the bounded validation rather than releasing it for background work.
Bind the exact owner generation, revision, desired bytes/generation, candidate
source, template, selected ID/mode/core and resource snapshot; recheck all
mutable owner/source inputs before effects and after completion. Config/core/
resource execution uses captured bytes, never path rereads. No private config
or credential enters replay/public outcomes; validation is not a store mutation.

No broad `LifecycleHost` method is needed. Its existing `fresh_observation`
supplies strict core/auxiliary/TUN counts, but does not alone cover user-manager
or controller absence. For an active native owner, the existing trusted
`ProductionOwnershipObserver::verify_disconnected_native` additionally requires
the native daemon active, legacy inactive, strict core/TUN inventories empty,
both core controllers absent and the native control socket present. Use the
appropriate existing trusted observation before and after validation. Login's
`verify_native_empty` instead expects both daemons absent, so it is not the
running-owner proof. Neither repeated observation is an atomic host reservation;
unknown inventory refuses. No quiesce/start/stop/pointer publication is included.

Focused controls must cover mismatched bundle/config, missing/changed resources,
stale revision or owner lease/generation, changed desired/source/template,
nonempty/unknown host, timeout/unknown completion and replacement-preserving
cleanup. Only after those source controls and a separately selected finite
synthetic resource-bearing sandbox row can actual candidate NativeHost/startup
wiring be proposed. Production v3 and active compensation/adoption fences stay
closed throughout.
