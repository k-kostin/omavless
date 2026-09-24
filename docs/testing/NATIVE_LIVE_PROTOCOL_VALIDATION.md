# Native V0 live-validation harness

Developer candidate for PR #30's Rust successor. Not V0 completion or a main
release. The original Python PR/head and accepted XHTTP evidence are retained;
new evidence must name the tested native package and harness commit.

## What changed from the old runner

`tests/native_live_protocol_validation.py` is opt-in **test tooling**, like the
other installed acceptance scripts; Python is not added to the application or
plugin. All production actions/parsing belong to the installed Rust owner over
its private versioned Unix socket. No old backend imports, second daemon,
second profile store, credential import or OS-policy installation occurs.

The old schema-1 harness cannot simply be called against native `backend.sh`:
its status, mode and lifecycle interfaces were Python-specific, and suppressed
disconnect exceptions could hide recovery failures. Schema 2 deliberately uses
only internal record IDs and fixed public case classes. It rejects legacy schema,
unknown fields/slugs, duplicate keys/classes, ambiguous Hysteria2 pairs, unsafe
files, malformed UTF-8 and oversized input. It never writes arbitrary feature or
provider labels to results. Read limits: 64 KiB cases, at most eight cases,
64 KiB request, 256 KiB response and bounded total socket deadlines.

The installed binary is root-owned and pinned by SHA-256; peer UID/runtime PID,
0600 control socket, 0700 directory, runtime instance, revisions and operation
IDs are checked. IPC writes use an explicit end-of-request half-close, matching
the Rust unary client. No private record/URI is passed in process arguments.

Feature classification first exports an already-imported record **in memory**,
then calls Rust `imports.classify`; there is no second protocol validator.
Only canonical protocol/transport/experimental-feature flags become classes.
Ordinary VLESS REALITY is not PQ evidence. XHTTP's validated top-level mode is
projected into a fixed enum; download/extra interoperability still needs its own
representative fixtures. TUIC means the canonical accepted v5 parser, not any
unrecognized TUIC URI. No export/preview payload is printed or persisted.

## Private inventory and cases

Use the expected installed binary digest from the exact package acceptance.
Do not silently bless an unexpected executable by accepting whatever is present.

```sh
python3 tests/native_live_protocol_validation.py \
  --runtime-sha256 EXPECTED_PACKAGE_BINARY_SHA256 --inventory
```

Only fixed family counts print. `--prepare-cases /absolute/private/cases.json`
instead creates one representative per available class, preferring the current
active record where suitable. It makes no connection; missing families stay
unavailable. Use a current-user-owned 0700 directory outside every Git worktree.
Files are new, atomic, 0600 and never overwritten. The checked-in
[`example`](../../tests/native-live-protocol-cases.example.json) contains a synthetic
internal ID, not a working profile. Do not copy credentials into it.

Before/after creation, inspect `git status --short`; cases/results must stay
outside Git. Class choices may be narrowed in the private file; do not fabricate
a transport/feature that the imported profile does not exercise.

## Attended live sequence

```sh
python3 tests/native_live_protocol_validation.py \
  --runtime-sha256 EXPECTED_PACKAGE_BINARY_SHA256 \
  --cases /absolute/private/cases.json \
  --output /absolute/private/results.json --confirm-live
```

This runs only in a real terminal. Follow the
[host authorization guard](HOST_AUTHORIZATION_ACCEPTANCE.md): `ready` before
and `settled` after **each** effect, OS passwords only at OS/sudo prompts. There
is no human acknowledgement deadline. Scripted acknowledgements are forbidden.

Start with the already healthy original connection, or disconnected with every
case using the existing last-profile record. The latter restriction prevents
silently changing a disconnected user's saved last profile: current native API
does not provide an independent last-profile-pointer mutation. The harness will
refuse that unsupported restoration scenario before effects.

1. Validate original state and disconnect if connected.
2. For one imported case, revalidate its canonical feature and connect Full VPN.
3. Prove owned service/core process group/cgroup, exactly one core/TUN, coherent
   selected record/global mode and responsive private Unix controller.
4. Validate the generated controller/config policy and attribute TCP listeners
   using one attended read-only `sudo ss`. Only the accepted loopback proxy and
   system-TUN forwarder listeners are permitted, not a TCP controller.
5. Run fixed `https://example.com/`, bound to the observed TUN, with proxy env
   disabled, HTTPS-only, no redirects and 15-second curl bound. Both interface
   counters must increase for positive TUN-use evidence.
6. In `finally`, request/verify disconnect. Cleanup failure is a hard blocker.
7. Restore the exact original profile/mode/last-profile pointer and verify it.

`nativeConfigAdmission` means Rust's actual prepare/validate/readiness path
accepted the generated config; it is not a second arbitrary Mihomo invocation.
Controller/TUN success does not prove DNS authorization, DNS leak protection or
server interoperability. HTTPS failure is kept with a bounded classification;
it is not rewritten into PASS because the controller responds.

An unsettled authorization or unknown connect outcome stops **all** further
effects, including automatic cleanup/restoration. Result is
`manual-recovery-required`, not success. Inspect with the human before explicit
recovery; do not repeatedly connect/disconnect to make a test pass. A definite
failed transition may be cleaned/restored only through separately attended steps.

## Hysteria2 network pair

Normal-network evidence is not an asserted property of every UDP flow. The
restricted half currently refuses before any effect: **NOT RUN — SAFE
UDP-RESTRICTED NETWORK FIXTURE UNAVAILABLE**. No boolean option, random timeout
or repeated run on the same network can substitute for a controlled safe pair.
A future adapter must define/review reversible network switching and real paired
evidence before enabling that case. No persistent firewall/network changes here.

## Privacy and evidence

Public result construction contains only fixed case/protocol/mode enums,
booleans, bounded codes, package hash and generic probe hostname. It excludes
profile IDs/names, endpoints, URI, credential hints, provider data, controller
paths/secrets and raw errors. Unknown exceptions use fixed fallback text.
Inspect the private result before sharing the redacted matrix. Never commit
private files or publish raw diagnostic/config/export/preview output.

Record exact harness/runtime/Mihomo versions separately. This first successor
requires installed live evidence; deterministic tests alone do not close #30.
Historical accepted XHTTP evidence is neither invalidated nor relabelled as
new native evidence. Other experimental families remain fixture-blocked until
real imported fixtures and the required representative coverage exist.
