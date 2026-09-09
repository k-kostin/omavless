# OmaVLESS incremental Rust migration contract

Status: selected implementation direction and migration contract. Updated
2026-09-03.

This document is authoritative for implementation language, migration order and
Python/Rust parity gates. It complements [`CONTROL_PLANE.md`](CONTROL_PLANE.md),
[`ARCHITECTURE.md`](ARCHITECTURE.md), [`TUI_APP.md`](TUI_APP.md) and
[`PLATFORM.md`](PLATFORM.md).

## 1. Decision

The long-term OmaVLESS application stack is selected as:

```text
Omarchy bar frontend                optional future desktop GUI
QML / Quickshell                    Rust / GPUI or later reviewed GUI toolkit
        |                                      |
        +------------------+-------------------+
                           |
                    semantic control API
                           |
                           v
                  Rust OmaVLESS runtime
                  Rust domain/core logic
                           |
                 +---------+---------+
                 |                   |
                 v                   v
             Mihomo core       future optional core
             external          compatibility backend

Standalone client:
  Rust CLI + Rust Ratatui TUI
```

The target user-facing application package must not require Python, `pip`, a
virtual environment or Python packages at runtime. The normal end state is one
primary `omavless` Rust executable providing at least:

```text
omavless
omavless tui
omavless daemon
omavless status
omavless connect
omavless disconnect
omavless doctor
```

plus package-owned service/integration resources. Mihomo remains an external,
separately packaged networking core rather than being embedded into the
OmaVLESS executable.

"One binary" means one primary OmaVLESS program for daemon/CLI/TUI and shared
logic. It does not mean statically embedding the Linux kernel, libc, systemd,
Mihomo, configuration files or future privileged NetGuard components.

## 2. Why Rust is selected

The choice is architectural rather than a benchmark exercise.

Rust fits the planned product because OmaVLESS is becoming a long-running local
service plus several clients and contains security-sensitive parsing, bounded
IPC, network/process supervision and cross-distribution packaging.

Expected benefits:

- Cargo dependencies are resolved and compiled during build rather than being
  installed as a user-managed runtime environment;
- Arch and NixOS can package a deterministic native executable without a venv;
- daemon, CLI and TUI can share typed domain models and protocol definitions;
- ownership and type checks reduce classes of lifetime/concurrency mistakes in
  a persistent runtime;
- Tokio-style asynchronous I/O is a natural fit for sockets, probes, process
  supervision and bounded concurrent work;
- Ratatui provides a native terminal UI without introducing a Python runtime;
- the same internal crates can later back an optional GUI client.

Rust does not remove dependencies. Crates remain dependencies of the source and
build graph and are pinned/reviewed through Cargo metadata and `Cargo.lock`.
The difference is that ordinary users do not need to maintain a matching set of
Python modules beside the application at runtime.

## 3. No big-bang rewrite

The current Python backend is validated production behavior and must not be
thrown away in one replacement PR.

Migration uses a strangler/differential model:

```text
same sanitized fixture/input
          |
     +----+----+
     |         |
     v         v
 Python      Rust
 reference   candidate
     |         |
     +----+----+
          |
   compare semantics
          |
       parity gate
          |
   Rust becomes owner
          |
 Python retained briefly
 as rollback/oracle
          |
 later removal gate
```

Each migration slice must be small enough that agents can state exactly which
behavior moved and which Python code still owns the production path.

Do not create a second permanent implementation. Dual execution exists only for
bounded migration evidence.

## 4. What parity means

Parity is defined by the product contract, not accidental implementation detail.

Depending on the slice, compare:

- accepted versus rejected input;
- stable machine error/category;
- canonical profile semantics;
- private/public field ownership;
- redacted preview output;
- generated Mihomo configuration where exact output is contract-relevant;
- subscription identity and duplicate handling;
- state migration results;
- routing decision/configuration semantics;
- bounded status/diagnostic fields;
- lifecycle outcomes and rollback behavior.

Do not preserve a known Python bug merely to produce byte-for-byte equality. If
a differential test exposes a bug or unsafe ambiguity, update the explicit
contract, add a regression fixture and make both sides obey the corrected
behavior before the Rust side becomes canonical.

For purely internal formatting, semantic equality is preferred over brittle
byte equality. For stored schemas, public API envelopes, security-sensitive
redaction and generated core fields, exactness may be required.

## 5. Differential-test privacy rules

Parity tooling must be safe to run with real private profiles during local
acceptance.

- reusable credentials never appear in process argv;
- credential-bearing inputs use bounded stdin, private files or the already
  private store;
- private fixtures remain untracked and mode `0600` where applicable;
- shareable results use fixture IDs/classifications rather than server names,
  endpoints, UUIDs, passwords or subscription URLs;
- neither implementation may dump raw input on assertion failure;
- test logs and CI use only credential-free fixtures.

The existing V0 privacy discipline remains applicable to migration tooling.

## 6. Cargo/workspace target

The repository should evolve toward one Cargo workspace. Exact crate names can
change when implementation starts, but dependency direction should resemble:

```text
crates/
  omavless-protocol/       control protocol/framing/types
  omavless-domain/         profile/store/routing semantics
  omavless-mihomo/         Mihomo config/controller/core adapter
  omavless-host/           narrow host capability contract
  omavless-runtime/        canonical daemon/coordinator
  omavless-cli/            semantic CLI/client helpers
  omavless-tui/            Ratatui client

src/bin or product package
  omavless                 one primary user-facing executable
```

Host implementations may be crates/modules such as Arch and NixOS adapters, but
shared domain/runtime crates must not import Omarchy-specific QML integration.

Commit `Cargo.lock` because OmaVLESS is an application, not only a reusable
library. Dependency additions require the same security scrutiny as new Python
or shell dependencies today.

Avoid PyO3/embedded-Python or a permanent Rust<->Python FFI layer unless a later
concrete blocker justifies it. During migration, language-neutral fixtures and
bounded subprocess/test interfaces are easier to remove and audit.

## 7. Runtime dependency target

The migration is complete only when the normal standalone/package runtime has:

```text
Python interpreter dependency:  no
venv dependency:                no
pip dependency:                 no
Python package dependency:       no
Rust/Cargo toolchain at runtime:  no
```

The Rust/Cargo toolchain is a build/development dependency. Users install the
built package and run `omavless`; they do not use `cargo run`.

Dynamic Linux/system libraries remain normal package dependencies when required.
Full static linking is not a project requirement.

## 8. Relationship with the current plugin

The existing Omarchy plugin remains a supported frontend during migration.
Its QML/Quickshell code is not being rewritten to Rust merely for language
uniformity.

The migration direction is:

```text
current
QML -> backend.py -> Mihomo

transition
QML -> legacy compatibility bridge -> Rust runtime -> Mihomo

final
QML -> omavless semantic CLI/socket -> Rust runtime -> Mihomo
```

The plugin must never own a second Rust runtime while Python still owns the
same tunnel. Ownership cutover is transactional and follows
[`CONTROL_PLANE.md`](CONTROL_PLANE.md).

## 9. Plugin feature work during migration

Current plugin-version work does not stop while Rust migration begins.

Two lanes are allowed in parallel:

### Lane P — finish the current plugin product surface

Continue bounded work which is primarily QML/presentation or completes already
accepted plugin behavior, for example localization batches, bug fixes,
accessibility/navigation, existing diagnostics presentation and opportunistic
V0 fixture validation.

### Lane R — migrate backend/runtime capability to Rust

Begin immediately with isolated components which already have deterministic
contracts and fixtures.

The lanes converge: a large new backend feature should not be implemented
Python-first if the Rust layer which should own it is already being built or can
reasonably land first.

In particular, P4 WireGuard/AmneziaWG parser work should begin only after the
Rust protocol/domain adapter boundary is usable. The goal is to avoid writing a
large new credential parser in Python and then immediately rewriting it.
The current QML plugin may consume the new Rust-owned semantic behavior through
the compatibility/runtime bridge.

Small Python changes needed to keep the current published/plugin branch safe are
still allowed. "Rust target" must never be used as a reason to leave a current
security or correctness bug unfixed.

## 10. Migration stages

The active delivery ledger in `DEVELOPMENT_ROADMAP.md` owns scheduling. The
canonical technical stages are:

### R0 — Rust workspace and parity infrastructure

Goal: introduce Rust without changing production tunnel ownership.

The initial workspace checkpoint uses `omavless-parity` as a reusable result
boundary. Implementation-specific adapters emit bounded sanitized reports; the
comparator reads regular non-symlink files, compares by public case ID and emits
only bounded mismatch IDs. It never executes adapters or prints facts,
fingerprints, paths or parser fragments. Complex/private canonical results use
a fingerprint rather than entering shareable output.

Local compilation requires the Rust toolchain plus a C linker. Omarchy can
install these through Settings and `omarchy pkg add gcc`, respectively; CI runs
the same locked workspace checks on the native runner architecture.

- create Cargo workspace and CI/toolchain checks;
- commit `Cargo.lock`;
- establish credential-free golden/differential fixture format;
- add a bounded parity runner which can compare Python and Rust results without
  leaking private values;
- document crate/dependency direction;
- no QML/runtime cutover and no TUI yet.

### R1 — control protocol parity

Goal: make the already isolated T1a protocol layer the first Rust-owned logic.

The R1 checkpoint adds `omavless-control-protocol`: a pure Rust library for v1
request/response parsing, encoding, stable errors, negotiation and bounded
unary stream helpers. A credential-free differential gate compares the Python
oracle and Rust outcome through the R0 sanitized report boundary. Neither probe
opens a socket, dispatches a method or enters the production plugin path.

- implement v1 frame/envelope validation in Rust;
- reuse `tests/control_protocol_cases/` as a language-neutral corpus;
- compare Python and Rust acceptance/errors/limits;
- add fuzz/property tests where useful for decoder bounds;
- after parity, Rust becomes canonical for the future runtime protocol while
  the Python implementation remains temporarily as a compatibility oracle.

### R2 — profile/protocol adapter parity

Goal: move pure credential parsing and config semantics before new protocol
families expand the Python backend.

The first bounded slice establishes `omavless-profile::Protocol` and strict,
bounded profile-link classification. Its sanitized differential adapter exposes
only the canonical protocol or a fixed error class; no URI or credential data
leaves stdin.

The second bounded slice adds the future Rust-owned VLESS authority and public
preview foundation: scheme/UUID/host/port validation, DNS/IPv4/IPv6 host
classification, percent-decoded suggested labels, existing preview sanitation
and fixed credential-safe parity facts. Its 31-case synthetic differential
corpus never emits URI, UUID, server, label or password material.

The third bounded slice adds the future strict VLESS query envelope and coarse
transport/security metadata: form decoding, field-count/UTF-8/allowed-field
bounds, case-insensitive duplicate and alias rejection, provider-metadata
bounds, transport/security normalization, certificate-verification booleans and
XHTTP mode vocabulary. Its 47-case synthetic differential exposes only fixed
classifications, public enum values and booleans. Python still owns the
production path and remains the oracle while REALITY keys/PQ, VLESS Encryption,
transport options, XHTTP `extra`, identity and Mihomo rendering move in later
slices.

The fourth bounded slice adds source-preserving Vision flow and packet-encoding
semantics. Both Vision source variants normalize to Mihomo's supported flow,
with TCP plus TLS/REALITY constraints enforced; `xudp` and `packetaddr` remain
the only packet encodings. Its 19-case synthetic differential exposes only
fixed classifications and public enum vocabulary. Python still owns the
production path and remains the oracle while VLESS Encryption, transport
options, XHTTP `extra`, identity and Mihomo rendering move in later slices.

The fifth bounded slice adds REALITY public-key/server-name requirements,
canonical raw URL-safe X25519 key spelling, optional short-ID bounds,
presence-aware PQ booleans and ML-DSA fail-closed handling. Its 43-case
synthetic differential exposes only fixed classifications and boolean facts;
public keys, server names, short IDs and spider metadata never leave the
adapters. A follow-up canonical-Base64 correction covers every legal final
character for a 32-byte raw URL-safe key. Python still owns the production path
and remains the oracle for this accepted slice.

The sixth bounded slice adds the accepted VLESS Encryption grammar: native,
xorpub and random modes; 0-RTT/1-RTT vocabulary; canonical 32-byte and
1184-byte client keys; padding structure and aggregate bounds; and fixed safe
errors. Its 42-case synthetic differential exposes only public enum values,
counts and booleans. Raw Encryption text and client key material never leave
the adapters or enter the Rust metadata model. Python still owns the production
path and remains the oracle for this accepted slice.

The seventh bounded slice adds established VLESS transport-option parity:
second path decoding and normalization, credential-safe host/service-name/
fingerprint presence and Unicode facts, ALPN splitting/trimming, alias-conflict
handling and TCP-header validation. Its 34-case synthetic differential never
emits private path, host, service, fingerprint or ALPN values. Accepted head
`8f65628f57b8a7caaa801beb6b44a3a7894bbed6` merged in PR `#72` as
`256669d7155766c9d1a0e05fe41a186cbd639458`. Python still owns production and
remains the oracle.

The eighth bounded slice adds the XHTTP `extra` decoder/shape foundation. It
accepts empty input as an empty object; bounds raw UTF-8 at 12 KiB, nesting at
depth 8, visited values at 160, keys at 128 UTF-8 bytes and strings at 2048
UTF-8 bytes; rejects duplicate object keys at every depth; and requires an
object root. It intentionally preserves accepted Python numeric forms including
non-finite and overflow/arbitrary-size lexemes, while escaped lone surrogates
fail closed under the fixed `invalid_json` class. Its 62-case synthetic
differential exposes only acceptance, fixed error classes and safe shape facts;
raw keys, strings and malformed fragments never leave the adapters. Accepted
head `bf8aa50e3b3c1fa67cc9baf0924fe38d2a5da469` merged in PR `#74` as
`a97871703c27c403f8be715889daa82ee755821e`.

The ninth bounded slice adds normalized top-level XHTTP option parity:
supported and compatibility allowlists, host/path/mode checks, recursive
`extra` rejection, server-only defaults, bounded headers, Python-compatible
scalar/boolean/enum/token handling, session alias and placement constraints,
and `xmux`/reuse semantics. Private header/token/session/padding values remain
in memory only with redacted debug/error behavior. Its 115-case synthetic
differential stops exactly at `downloadSettings` by replacing only the Python
nested-download helper with a fixed sentinel, preserving every preceding
production error in order without validating the next slice. Accepted head
`50ae48f73448c720c41d89defe7d843ac0ed35ba` merged in PR `#78` as
`fb08101e7d15b5484a1c4d729cf715e171b87161`. Python still owns production and
remains the oracle.

The tenth bounded slice adds nested XHTTP `downloadSettings` parity. It models
the secondary endpoint privately and preserves Python validation order for
address/port, XHTTP network, none/TLS/REALITY security, TLS and REALITY options,
nested transport settings, bounded headers, defaults, aliases and recursive-
download rejection. Its 188-case synthetic differential exposes only fixed
errors, endpoint/value classifications, booleans and counts; endpoint, SNI,
REALITY public key/short ID, spider path, fingerprint and header values never
leave the adapters or appear in `Debug`. Accepted head
`58781daebb728a12d2639a5621d698982cfd03db` merged in PR `#80` as
`f4c9d9eccfd0b76439c66265b700c19847b7f445`. Python still owns production and
remains the oracle for this accepted slice.

The eleventh bounded slice composes the accepted VLESS authority, query,
REALITY/PQ, Encryption, transport and XHTTP semantics into the future canonical
private VLESS model. It adds credential-safe preview semantics, normalized
subscription identity and semantic Mihomo proxy rendering without entering the
production path. Its 49-case synthetic differential compares only fixed safe
facts and parity fingerprints; reusable UUID, endpoint, key, path, host, header
and generated-config material never enters shareable output. Accepted head
`7b186f79349b7d81101b55945154db44b3e1c8f3` merged in PR `#83` as
`0b2a7d5f147c1c373ae491f1206695532e8c07d7` after cloud and Try Omarchy ARM64
acceptance. Python still owns production and remains the oracle; no runtime,
store, service or controller cutover occurred.

The twelfth checkpoint is the R2j pure-VLESS consolidation and privacy audit.
It found one remaining credential-bearing derived `Debug` implementation on
the authority preview and replaced it with bounded presence/classification
facts. All 49 canonical differential cases remained green, alongside 71 Rust
and 259 Python/QML tests on Try Omarchy ARM64. Accepted head
`d82bf63bec44b259e67f422f967509d417fad4bc` merged in PR `#86` as
`5ecbd7d13c1fb0ae1fb73bced61b40bd4e1bf184`. Pure-VLESS adapter parity is now
complete. The later R2-R4 foundation in PR `#90` accepted the remaining adapter
and domain boundaries. Python still owns production until R5/T1; no runtime
cutover occurred here.

Migrate:

- protocol classification;
- VLESS/Trojan/Hysteria2/TUIC parsing;
- canonical profile representation;
- redacted import preview;
- endpoint extraction;
- subscription identity inputs;
- Mihomo profile/YAML rendering semantics.

After R2, new P4 WG/AWG adapters are Rust-first. Python-only implementation of a
new large protocol parser requires an explicit roadmap exception.

P4 development is not globally gated on the fixture-constrained V0 track.
WG/AWG instead carries its own private-fixture, installed-core, security and
live-host acceptance ledger; product exposure still waits for that evidence and
the compatibility/runtime bridge.

### R3 — private store, subscriptions and routing semantics

Goal: move stateful domain logic while Python still remains the production
runtime owner.

Migrate and parity-test:

- store schema validation/migrations;
- favorites/selection/startup semantic state;
- subscription parse/merge/identity/atomic refresh commit logic;
- routing preset/custom-rule semantic model;
- safe import/export transformations;
- deterministic config assembly around migrated adapters.

Remote fetch/process ownership may remain Python until R4; snapshot/commit
semantics must already be language-neutral.

### R4 — Mihomo operations, probes, diagnostics and host readiness

Goal: move OS/network-facing unprivileged runtime capability into Rust.

Migrate under exact behavioral gates:

- stable core entry-point discovery;
- Mihomo config validation invocation;
- private controller client and bounded diagnostics;
- latency/probe orchestration;
- traffic/status sampling;
- owned-process/service observation;
- host capability/doctor model for Arch and NixOS.

Use asynchronous bounded I/O where appropriate; do not turn Tokio into a reason
to widen concurrency or retry semantics beyond the existing contract.

### R5 / T1 — Rust canonical runtime cutover

Goal: make `omavless daemon` the one canonical owner.

Implementation status: **foundations accepted; ownership cutover pending**.
PRs #96-#100, #102, #104, #106-#108, #110, #111, #113, #117-#121 and
#125-#134 and #136 provide the private control
socket/owner lock, desired-state and reconciliation model, package-unit
contract, canonical all-family rendering, read-only private-store/config
preflight, the bounded mutation coordinator and exact v1 connect/disconnect
parsing. The native
supervisor and host adapter prove fixed-argv parent ownership, private-controller
readiness, explicit-mode private config staging and bounded child cleanup with
installed Mihomo. The internal owner engine combines mutation serialization with
deterministic, fail-closed lifecycle transactions. Semantic operation digests
cover every connect/disconnect input without exposing credentials.
The shared Python/Rust migration lock, private ownership marker and deterministic
cutover/rollback state machine now reject stale, partial and duplicate-owner
states. The ownership-gated offline binder produces bounded v1 mutation
responses and preserves exact operation replay before revision checks. These
layers now include a fail-closed legacy Python reader with exact marker-schema
and private-filesystem parity. Legacy mutation admission and commit both check
that marker under the shared lock, so preparing/native ownership quiesces
Python mutation without disabling read-only compatibility or the internal
supervisor path. PR #137 supplied production owner construction and PR #138
accepted ownership-gated socket registration without replacing the Python
lifecycle owner. The locked production-host preflight proves disconnected,
adoptable and inconsistent classification from fixed systemd, process, TUN,
private-controller and exact active-config facts without changing host state.
PR #139 accepted the transition bootstrap/handoff at squash commit
`3b22cb987007ab85150f5352016267aa15138905`, providing the seam required to
compose the cutover coordinator with that runtime before a production host can
execute the transaction. Failed native stop still blocks legacy restoration,
incompatible runtime verification cannot switch the bridge, and incomplete
compensation preserves the preparing marker for manual recovery.
Native profile rename/favorite/delete now also has v1-v3 normalization,
Python differential parity, a private atomic writer, and exact bounded v1
request envelopes with domain-separated replay digests. Its offline owner
transaction now uses the shared revision/replay coordinator, prepared
exact-byte compare/commit/restore under the established Python/Rust migration
lock, trusted desired-plus-observed active state, and fail-closed lifecycle
compensation. The lock spans preparation through lifecycle/store compensation;
the byte comparison is an additional unexpected-change guard rather than a
standalone lock-free CAS. Pre-effect lock acquisition failures are explicitly
uncached and do not change revision, allowing the same operation ID to retry
after contention clears. Active rename quiesces while preserving connected
intent and recovers the candidate; active delete durably disconnects before
store removal and reconnects the exact old profile if store commit fails. Any
uncertain store/runtime restoration requires manual recovery. PR #138
registers the engine only behind a committed Rust ownership marker, so Python
remains the installed production profile-mutation owner until the full cutover
gate lands.

Native subscription add/update/delete now also has an exact offline v1 request
boundary. It admits only normalized name/URL and existing subscription-ID
intent plus operation metadata, rejects generated IDs, fetched entries and
caller-supplied host observations, and uses a subscription-specific replay
digest which distinguishes omitted from explicit revision. The private intent
types cannot be formatted, cloned or serialized. At this checkpoint fetching,
generated data and IPC registration remained later owner work; this parser did
not change the production path. PR #125 supplies the complete bounded
atomic-store mutation boundary and Python differential parity; PR #126 supplies
the exact request protocol and replay identity.

PR #127 adds the credential-private offline feed decoder
and optimistic store snapshot/commit seam. It bounds already-fetched bodies at
5 MiB and 1,024 supported candidates, accepts UTF-8 raw or strict
base64/base64url feeds, canonicalizes and deduplicates all currently supported
profile families, and exposes only fixed codes/counts. The runtime orchestration
captures URL plus `updatedAt` under one short owner lease, performs injected
transport outside that lease, then re-acquires serialization to compare and
atomically replace one fully validated latest store. Concurrent subscription
renames and unrelated store changes survive; deletion, URL replacement or a
competing refresh conflict. Record IDs and timestamps come only from trusted
owner callbacks; the refresh token advances monotonically even if the wall
clock repeats or moves backwards, and fails closed at integer exhaustion. This
is an intentional correction to the millisecond-collision weakness in the
legacy Python timestamp. At the PR #127 checkpoint this remained offline and
unregistered: it contained no HTTP client, IPC dispatch, lifecycle or
production ownership change.

The native subscription-store boundary deliberately accepts only ASCII or
punycode URL authorities and RFC 3986 IPvFuture address characters. This is a
fail-closed narrowing of the legacy Python `urlsplit` edge behavior: raw
Unicode authorities and non-RFC IPvFuture spellings must be canonicalized by a
future admission layer before they may reach the atomic mutation boundary. It
prevents Unicode NFKC delimiter ambiguity without widening remote fetching or
making the offline boundary responsible for URL repair.

The first-install private-store bootstrap is a separate offline Rust boundary.
Under the shared migration lock it can publish only the fixed
`~/.config/omavless/profiles.json` name and the exact credential-free Python
v3 empty-store payload. Publication uses atomic create-if-absent semantics and
never replaces a concurrent winner. A winner or pre-existing target is accepted
only after its same-user `0600`, regular non-symlink shape and complete v1-v3
store payload validate; malformed or unsafe state fails closed and is never
reset. The existing config directory must already be same-user `0700`. This
helper remains unregistered from CLI, IPC, daemon startup and the current
plugin path. This checkpoint was accepted in PR #128.

PR #129 binds offline profile rename/favorite/delete to the shared
revision/replay coordinator, migration lock, exact-byte store transaction and
active-profile lifecycle compensation. PR #130 adds the corresponding offline
connection transaction and startup-reconciliation boundary: compatibility
`activeId`/`lastId` pointers follow verified desired/runtime outcomes, stale
startup state is repaired, and uncertain restoration becomes a manual-recovery
blocker without stopping an adopted healthy tunnel. PR #133 converges the
connection and profile paths behind one offline coordinator, shared revision,
replay namespace, migration lock, lifecycle executor and global recovery
barrier.

PR #134 adds the production-safe subscription HTTP boundary and joins
subscription add/update/delete to that coordinator. Requests use fixed GET and
safe headers, platform-verified TLS, a 25-second global timeout, at most five
manually revalidated redirects, a 32 KiB response-header cap and a 5 MiB body
cap. Ambient proxy credentials, cookies and authorization forwarding are not
accepted. Every target must remain HTTPS or canonical loopback HTTP. Remote
work occurs outside the Python/Rust migration lock; commit re-acquires the
shared lease and uses trusted runtime observation. Transport, feed and store
failures expose only stable credential-free outcomes.

Through PR #134 these owners remained absent from production `RuntimeServer`
dispatch and capabilities. PR #136 then added one exact semantic mutation
dispatcher and
an ownership-gated coordinator constructor. Connection, profile and
subscription requests use the same bounded accepted/error response contract.
The private ownership marker is checked while holding the shared migration lock
before admission, so cached replay cannot bypass an ownership change, and is
checked again before every store/lifecycle effect and after subscription
network work. Any legacy, preparing, rollback, missing, malformed or unsafe
marker returns `capability_unavailable` without host/store effects.

PR #137 adds the production native-owner construction boundary. It resolves
only package/current-user paths, validates a committed private `rust` marker,
and holds the shared Python/Rust migration lock continuously across marker
validation and startup reconciliation. Constructing the owner alone does not
expose IPC or change the production owner.

PR #138 accepted the ownership-gated socket-registration checkpoint at
candidate head `b9be21a9b44c7c0debb816e042bd2469a9e86639` and merge commit
`5cc355ca87c4963cdd988a76c04395b7732f5b1a`.
`omavless daemon` remains read-only for legacy, preparing, rollback, missing,
malformed and unsafe ownership. Only a successfully constructed committed Rust
owner registers the accepted connection/profile dispatcher and advertises
those methods. The registered owner is pinned to the exact committed marker
generation; rollback followed by a later Rust generation cannot revive stale
revision, replay or host state. Status and capability reads recheck ownership
under the shared lock, and mutation admission still rechecks before every
effect. Subscription socket mutations remain withheld until bounded remote
fetch can run without blocking status or urgent disconnect. The draft performs
no marker write, transactional cutover or plugin-bridge switch, so Python
remains the installed production owner and R5 is not complete.

PR #139 resolved the in-process transition-bootstrap seam before a
production cutover host is attached. The coordinator durably enters
`cutoverPreparing`, releases the migration lock only for candidate startup,
and verifies one exact runtime identity. The candidate independently acquires
the lock, validates the exact preparing generation, reconciles while
mutation-gated, and later promotes in memory only after observing the immediate
committed `rust` successor. The coordinator must reacquire and revalidate the
preparing marker before bridge/commit work. Marker-write errors are followed by
a durable re-read, and rollback restores an exact captured desired-state
snapshot. The production host, full crash matrix and installed bridge cutover
remain pending.

PR #140 accepted the bounded production-host composition at squash commit
`5aba03a3eb29b45899cc8927703bacdad788b36f`. It binds the accepted
transaction to private desired/store/config state, fixed
`omavless.service` and `omavless-runtime.service` operations, the private
runtime socket and same-candidate host observation. It also removes the
incorrect `/usr/bin/mihomo` assumption from the native host in favor of the
documented absolute override, user-local and absolute-PATH discovery classes.
No CLI/socket cutover method or concrete plugin bridge is included, so the
installed Python owner remains unchanged.

PR #141 accepted the first concrete bridge component at squash commit
`f23f599b8dc49e30cb3d118b015b1324f82c4ee8`: a fixed private target
selector fenced to the exact preparing generation and its immediate committed
or compensated successor. Only the matching migration lock and unchanged
preparing marker can replace it. The reader accepts absent state only for an
exact legacy owner and fails closed for every stale, unsafe or ambiguous
combination. QML/backend launcher integration and semantic command mapping are
still absent, so this selector cannot activate a cutover by itself.

PR #142 accepted the first fixed executable-side semantic mutation mapping at
tested head `977d38a4cf6274286e8731d315c6b5a327a26b83` and squash commit
`006ea7944242d076e1bfa59be85e10be92106df4`: connect/disconnect and profile
rename/favorite/delete map to exact registered v1 methods. Unknown or extra
arguments fail before socket dispatch, rename input is bounded and supplied
through stdin rather than argv, and no raw method/JSON passthrough exists.
Subscription CLI dispatch, read projections and QML/backend launcher
integration remain later checkpoints, so the installed production owner is
unchanged.

The next bounded read-projection checkpoint adds exact empty-parameter
`profiles.list` and `subscriptions.list` methods plus fixed `profile list` and
`subscription list` CLI commands. Only an exact committed Rust owner advertises
or serves them. Their bounded same-user responses contain opaque record IDs,
local display labels, protocol/favorite/stale metadata and counts, but no
profile URI, endpoint, subscription bearer URL, identity key or protocol
credential. This enables later bridge composition without switching QML or
making the cutover transaction reachable.

The next fixed production-host fault-matrix checkpoint joins the pure
transaction matrix with real test adapters for the two fixed user services,
private runtime socket, exact candidate replies, desired state, ownership
marker and frontend bridge. It proves rollback after runtime-start failure,
wrong bootstrap generation and inconsistent status, while an uncertain native
stop preserves the preparing blocker for manual recovery. No public cutover
entry point or installed-owner switch is added.

The subsequent native routing-mode checkpoint implements the contract's exact
`routing.set_mode` request and fixed semantic CLI mapping. It persists an
offline mode without host work, replaces the owned core for a connected mode
change, restores the prior connected target after ordinary candidate failure,
and blocks on uncertain cleanup. It is registered only for an exact committed
Rust owner; the plugin launcher and production owner remain unchanged.

The subscription-delete socket checkpoint registers only
`subscriptions.delete` and maps it
from a fixed semantic CLI command. It is safe on the existing serialized owner
because deletion performs no provider fetch. Subscription add/update remain
offline until the server can keep reads and urgent disconnect responsive while
bounded remote I/O is in flight.

The client-admission checkpoint handles at most 16 same-user unary
exchanges concurrently. Each frame retains its fixed five-second I/O deadline;
a partial/slow peer no longer monopolizes the accept loop, and saturation drops
the newly accepted stream rather than allocating an unbounded worker. The
native owner mutex continues to serialize semantic state operations.

The following fetch/commit checkpoint registers `subscriptions.add` and
`subscriptions.update`. A reservation-free preflight resolves exact cached
replays without another fetch and rejects invalid, stale, conflicting,
ownership-revoked, busy and manual-recovery requests before network I/O. At
most four strict provider fetches run concurrently outside both the
owner mutex and migration lock. Finalization re-enters the one owner, re-parses
and re-admits the original request, and rechecks durable ownership/revision
before decoded entries can reach one atomic commit. Status and disconnect are
therefore admitted while fetch is in flight; a concurrent disconnect advances
revision and makes that fetched result conflict. Fixed CLI add/update commands
take the private name and URL as two bounded stdin lines rather than argv. The
installed Python/QML path and cutover reachability remain unchanged.

The inactive Arch payload checkpoint provides a deterministic package-build
boundary for an already-built `omavless` executable, the fixed hardened user
unit, licenses and package documentation. It writes only below an explicit
absolute non-symlink package root, fixes artifact modes and rejects unsafe
destinations. It never builds from the marketplace plugin, invokes a package
manager, enables/starts a unit, edits ownership/private state or touches a
tunnel. Product/plugin version 0.7.0 and the pre-release native workspace
version 0.1.0 therefore remain distinct; no 0.8.0 release is implied.

The next single-refresh dispatch checkpoint registers exact
`subscriptions.refresh` requests and a fixed `subscription refresh ID` CLI
command. The URL never appears in the request or argv: the native owner carries
one non-formatable private store snapshot across bounded provider I/O, sharing
the existing four-fetch cap. Completion re-enters the serialized owner and
rechecks global revision, exact Rust ownership, subscription URL and update
identity before committing. A concurrent disconnect wins and exact operation
replay does not fetch twice. Refresh-all scheduling, the QML bridge and the
production cutover remain out of scope.

The explicit editor-read checkpoint adds `subscriptions.edit_input` for one
opaque local ID plus a fixed semantic CLI command. The private same-user socket
may return exactly the validated stored name and bearer URL only for this
intentional request, while capabilities, status, lists, errors, logs, argv and
diagnostics stay credential-free. The read holds the shared migration lock
through exact ownership revalidation and store lookup. It performs no network,
store or lifecycle mutation and does not activate the QML bridge or cutover.

The inactive refresh-all transaction checkpoint preserves the current
all-or-nothing batch behavior without registering a live control method. It
captures every ordered private subscription identity under one short lease,
fetches and decodes sequentially outside both leases, caps aggregate retained
private URI bytes at the private-store bound, and revalidates the entire set
before at most one atomic replacement. One failed provider, stale member,
invalid feed, count overflow or timestamp exhaustion produces no write; an
empty set performs no network, clock read or write. Live `refresh_all` remains
blocked on an implementation of the accepted long-operation scheduling
contract rather than extending the five-second unary deadline.

The following inactive long-operation checkpoint fixes that protocol boundary
without starting a worker or advertising a socket method. Exact
`subscriptions.refresh_all`, `operations.get` and `operations.cancel` parsers
require the current daemon instance ID plus a client operation ID, reject extra
provider/URL/scheduling input and produce only bounded safe projections. A
pure per-instance registry models one
active job, exact replay/conflict identity, monotonic progress, cooperative
cancellation versus the final commit fence and bounded FIFO terminal retention.
The primitive exposes both directions of the shared operation-ID collision
check, but the future executor must invoke them atomically under the one owner
lock, acquire the global remote-fetch permit per provider,
enforce the whole-job deadline, bind exact Rust ownership, register methods and
add fixed CLI commands. The QML/Python production path remains unchanged.

- singleton/peer/private-socket boundary;
- desired/actual state reconciliation;
- one mutation queue;
- connect/disconnect/mode lifecycle;
- profile/subscription mutations through IPC;
- Rust host adapter contract;
- packaged Arch runtime first, NixOS host package/module as the second target;
- QML plugin switches to semantic CLI/socket bridge;
- Python backend remains only as explicit disconnected rollback during the
  compatibility window.

This is the T1 control-plane cutover, not an additional daemon track.

The subscription transport deadline checkpoint gives manual redirects and the
final body read one shared monotonic budget. An enclosing native job can only
shorten the fixed provider timeout. This closes a prerequisite for bounded
refresh-all execution without registering that operation or changing the
installed Python owner. Loopback tests cover redirect/body deadline exhaustion,
zero-budget no-I/O and a shortened actual request timeout.

The incremental batch preparation checkpoint builds on that transport budget.
It performs at most one provider fetch per step, using the same four-permit pool
as native single-subscription requests. Busy admission performs no I/O and
consumes the original 30-minute job budget. Cancellation is checked before and
after transport and decode; any failure destroys the private prepared prefix.
Only a complete batch can be handed to the existing all-or-nothing store
transaction. Preparation is compared against the accepted offline refresh-all
reference. No worker thread, IPC method or CLI is registered by this slice.

The operation-registry/composite-owner integration is still required: start and
retry admission, shared operation-ID collision checks, exact ownership/revision
revalidation, the authoritative cancellation/commit fence, terminal progress
publication and bounded worker lifetime must be joined under the single owner.
The worker cancellation flag alone is deliberately not a commit authorization.
Python remains the installed production owner.

### R6 — Python runtime retirement / native application gate

Goal: prove that the product no longer requires Python before TUI implementation
starts.

Required evidence:

- all production backend operations needed by the plugin are Rust-owned;
- normal plugin + standalone package path runs with Python unavailable;
- no `backend.py` process is required for connect, routing, subscriptions,
  diagnostics, startup or lifecycle;
- store migration from existing installations passes;
- rollback/recovery plan is documented and tested before legacy code removal;
- differential/golden corpora remain after Python oracle deletion;
- Arch package contains the native runtime/CLI path;
- Nix package work can use the same binary contract;
- no venv/pip/Python module setup is part of installation or runtime.

Only after R6 may T2 implementation begin.

### T2 — Ratatui TUI

TUI implementation is deliberately downstream of Rust migration rather than a
vehicle for performing it.

- Rust + Ratatui is the selected first full application UI;
- Crossterm or an equivalent reviewed terminal backend may be used;
- TUI is a client of the already accepted Rust runtime;
- it does not contain compatibility calls into Python;
- plugin and TUI concurrently use the same canonical runtime.

Design/prototyping notes may be prepared before R6, but production TUI feature
implementation must not outrun the native-runtime gate.

## 11. Future GUI / GPUI

A desktop GUI is a possible later client, not part of the TUI migration gate.
GPUI is an attractive Rust-native candidate because it can support a rich,
keyboard-first GPU-rendered Linux UI, but it must remain behind the semantic
control API just like Ratatui and QML.

Do not make GPUI a dependency of the daemon, CLI or TUI crates. A future GUI may
be distributed as a separate binary/package (for example `omavless-gui`) so a
headless/TUI installation does not acquire a graphics stack merely to run the
VPN daemon.

GPUI selection becomes final only when a GUI track is scheduled and its Linux,
packaging, accessibility and maintenance costs are re-evaluated. Replacing GPUI
later must not require rewriting the runtime.

## 12. Acceptance rules for every Rust slice

Every R-stage PR must state:

- Python owner before the change;
- Rust candidate/owner after the change;
- exact inputs/outputs covered by parity;
- known intentional differences and why they are correct;
- deterministic tests run;
- real-host checks required or explicitly not applicable;
- whether Python code can be removed yet;
- whether the production plugin path changed.

A Rust rewrite is not accepted merely because tests are green if the tests only
exercise the new code. Migration PRs need evidence against the existing
behavior or an explicit contract fixture established before replacement.

For system/network slices, Python/Rust parity alone is insufficient: exact-head
Try Omarchy/Arch/Nix gates remain required according to the normal acceptance
policy.

## 13. Rules for new backend features

After R0 starts, agents should ask where new backend logic belongs before adding
it.

- QML-only presentation behavior stays QML.
- A current security/correctness fix may patch Python immediately and must add a
  regression fixture suitable for Rust.
- New pure profile/store/routing logic should prefer the Rust domain boundary
  once the owning R stage exists.
- New protocol adapters after R2 are Rust-first.
- New persistent lifecycle/background behavior after R5 belongs only in the
  Rust runtime.
- Do not create new long-lived Python dependencies for functionality already
  scheduled for Rust unless an explicit blocker is documented.

## 14. Definition of migration complete

Before frontend bridge activation, the native unary client must enforce the
same private socket/peer boundary as the server and correlate every success or
error reply to its request ID. The client checks occur before private input is
sent. Synthetic socket regressions cover peer rejection without disclosure,
endpoint permissions/symlinks and mismatched replies. This strengthens the
existing Rust CLI/cutover-host client; QML remains on its Python owner.

The migration is complete when all of the following are true:

```text
Omarchy plugin UI:        QML/Quickshell frontend
Canonical runtime:        Rust
Domain/profile logic:     Rust
Mihomo adapter/control:   Rust
CLI:                      Rust
TUI:                      not required for R6, but Rust when T2 lands
Python production path:   none
Runtime Python dependency:none
Primary package binary:   omavless
External proxy core:      Mihomo
```

At that point Python source may remain only in historical tests/migration tools
if there is a concrete reason, but installing and operating OmaVLESS must not
require Python. The preferred final state is to remove obsolete Python runtime
code entirely and keep language-neutral regression corpora as the preserved
behavioral record.


### Native unified import-preview boundary (2026-09-06)

The pure Rust domain now composes the existing strict import classifier and
canonical profile adapters into `preview_import`. Clipboard text and bounded
file contents use one semantic path. It validates profile contents (classification
alone is insufficient), checks duplicate subscriptions against the caller's
authoritative snapshot, and preserves Python `classify_import`'s version-1
confirmation shape. It neither fetches nor mutates a store.

The profile projection is explicitly **private UI data**, not sanitized
diagnostics: it includes endpoint, SNI, label and the established masked hint.
Debug output stays redacted; subscriptions return no URL. Differential tests
reuse both canonical corpora and compare digests without printing projections,
including whitespace/file-content variants and invalid/duplicate input.

The following dispatch checkpoint registers exact `imports.classify` only for
the committed native owner and adds `omavless import preview` with bounded
private stdin. The owner rechecks its exact generation and reads the current
private store under the migration lock; duplicate status cannot be supplied by
the client. V1's 32-KiB string and 64-KiB escaped-frame limits apply. Synthetic
socket and executable tests cover success, malformed input/store, unsafe store
modes, revoked/stale ownership, unchanged revision/store/host, and no credential
echo. The existing 225-case domain differential remains the Python oracle.

Python still owns production import, file/clipboard acquisition, confirmation
and persistence. The installed QML launcher is not switched. The remaining
bridge must use this private input and explicit confirmation before any mutation;
neither checkpoint retires Python or claims fixture interoperability.

### Confirmed new-profile import dispatch (2026-09-07)

The native `profiles.import` / `profile import` command now adds one explicitly
named standalone profile behind the committed owner. It reuses canonical input,
private-store normalization, the shared mutation/replay coordinator and locked
compare-before-replace writer. IDs are owner-generated after admission.
Duplicates and capacity errors produce no replacement; exact retries do not add
twice. Uncertain write failures require verified restoration or manual recovery.
Neither active/last pointers nor the host lifecycle are changed.

A bounded Python oracle runs actual `backend.import_profile` with load/save,
UUID and service effects replaced. Both canonical corpora plus name/duplicate
cases compare normalized-store digests without publishing private payloads.
The deliberate native boundary requires a confirmed name and a single profile
link rather than implicit name fallback or prose extraction.

This closes new-profile persistence, not active-profile replacement, QML routing,
package cutover or Python retirement. The current installed plugin still calls
Python. Replacement and its lifecycle recovery remain the next import slice.

### Existing standalone profile replacement (2026-09-07)

`profiles.replace` and `profile replace ID` add the distinct confirmed editor
operation, not an upsert. The existing profile mutation coordinator and private
store transaction preserve identity/favorite/startup state and reject missing,
managed or duplicate-name targets. Active replacement reuses the accepted
quiesce/recover/restore sequence; inactive writes use the compensated store-only
commit. Exact no-ops and replay do not cause another restart.

112 effect-isolated Python import/edit comparisons cover canonical families
and naming. Real temporary-file recovery tests cover successful replacement,
candidate failure with byte-exact restoration and failed recovery. Isolated
private socket tests cover registered dispatch, revision/replay/no-op and
ownership revocation. These are deterministic lifecycle-adapter tests, not
provider or installed native-owner acceptance. QML/editor wiring and controlled
cutover remain pending; the installed Python owner is unchanged.

### Explicit profile-export checkpoint (2026-09-07)

The committed native owner now serves exactly `profiles.export` for explicit
QR/file purposes, with a fixed CLI and bounded credential-bearing success.
No destination path, file write, QR subprocess, ordinary list credential or
store/lifecycle mutation is added. Generation/permission/malformed-store
checks remain under the migration lock. Actual Python `export_file` parity
uses effect-isolated writes and synthetic canonical corpora; comparison output
contains digests only. Socket/CLI checks cover private release, ordinary-list
non-release, invalid purpose/path, revoked ownership and unsafe stores.

The 109-case export comparison records 105 identical outcomes and four explicit
pre-existing store-validation differences: Python permits invalid XHTTP-extra
options while Rust strictly rejects the complete store. A positivity assertion
ensures the corpus actually exercises successful standalone/managed exports,
not matching rejection of malformed fixtures. Legacy stores with these options
must be detected before cutover and retain a repair/export route through the
legacy owner; native export does not weaken store validation or claim full
legacy-store compatibility.

QML acquisition, QR rendering, destination selection and editor wiring remain
Python-owned. This is a native semantic prerequisite, not installed frontend
acceptance, product protocol expansion, cutover or Python retirement.

### Legacy-store cutover refusal checkpoint (2026-09-07)

The fixed production cutover host now validates the complete same-user private
store during its locked pre-effect snapshot, before writing `cutoverPreparing`.
Previously this validation happened during staging after the preparing marker;
even an incompatible inactive record could therefore gate legacy repair/export
and enter unnecessary compensation. The guard uses the existing strict parser
and file-permission policy, never a permissive recovery parser.

Four synthetic mixed-store XHTTP incompatibilities plus corrupt, permissive,
symlink and missing stores now prove refusal before marker/bridge/service
mutations. Exact private bytes and legacy ownership survive; after releasing
the failed attempt and explicit repair, a new snapshot succeeds. The export
oracle from #168 establishes which rejected XHTTP records remain readable by
the legacy owner. Issue #169 retains the future user-facing repair guidance
and installed cutover acceptance; this guard does not activate cutover.

### Read-only store compatibility diagnostic (2026-09-07)

The fixed `omavless store-compatibility` command shares the complete strict
store validator and migration lock with the production pre-effect guard.
It uses the established private home/store location, accepts no path or extra
arguments, needs no Python, controller, GUI or host command, and does not start
a daemon. Its bounded credential-free JSON contains `schemaVersion: 1`,
`compatible`, `code`, safe English `message` and `recovery` identifiers:

| Code | Meaning | Recovery |
| --- | --- | --- |
| `compatible` | Full native store validation passed | `none` |
| `store_unavailable` | Missing, unsafe or unreadable private store | `restore_private_store` |
| `store_requires_repair` | Invalid/oversized data or unsupported stored configuration | `legacy_repair_or_export` |

Exit zero means a report was delivered, **not** that migration is permitted:
consumers must inspect `compatible`. Lock contention and unsafe environment
fail with a fixed error and no report. A successful check does not establish
frontend, host, ownership or complete cutover readiness. No raw parser errors,
record IDs, names, credentials or private paths are returned.

The diagnostic may create/acquire the operational migration lock; it never
writes the profile store, ownership marker, desired state or service state.
Failure does not reset or repair data automatically. Keep legacy ownership;
use its explicit repair/export where possible, or restore a valid private
backup if malformed data prevents the plugin from opening it. Retry the check
after repair. No permissive parser or bypass is introduced.

The executable regression covers all four established legacy XHTTP mismatch
cases, empty valid store, missing/corrupt/invalid-UTF8/oversized data, unsafe
permissions, symlinks, contention, rejected arguments and empty external-tool
PATH, checking exact bytes and absent owner/socket publication. Existing
Python export parity remains the reference for intentional strict differences.
Issue #169 still requires installed frontend guidance and actual cutover
acceptance; Python remains the installed owner and rollback/oracle.

### Standalone profile editor-input checkpoint (2026-09-07)

`profiles.edit_input` plus the fixed `profile edit-input ID` command now return
the explicit private name/link needed by the accepted replacement operation.
The read shares the locked exact-owner/private-store projection helper with
import preview and QR/file export, without widening either method. Managed
profiles fail before editor release, rather than only when replacement is
confirmed. Ordinary reads remain credential-free.

54 positive canonical/name cases compare digests against actual Python
`edit_profile` seeding: only the GUI launch is replaced, generated private temp
files are checked for mode/cleanup, and no fixture data enters argv or logs.
Missing/managed cases and actual private socket/CLI tests prove bounded exact
requests, rejection after ownership change, unsafe-store refusal and no writes
or lifecycle effects. The shared import/export regression gates are rerun.

Python still owns the installed editor, dialogs and clipboard/file helpers.
The semantic editor pair is ready for later frontend composition, not proof of
an installed QML bridge, cutover, provider maturity or Python retirement.

### Private custom-rule editor read checkpoint (2026-09-07)

`routing.custom_rules.list` / `routing rules` now provides the fixed read-only
prerequisite for the routing-tools frontend. The existing complete strict store
validator, private-file policy, migration lock and exact native-generation fence
are reused. Only original-order `id/kind/value/action` fields reach the explicit
private editor response; extension metadata and profile/provider content do not.
Destinations remain private and are absent from ordinary status. The existing
128-rule/1024-byte value bounds fit v1 response framing; no query or path input
is accepted and no mutation, revision bump or host work occurs.

17 positive cases compare canonical response digests against actual Python
`custom_rules_text`, covering all match/action combinations, IPv4/IPv6, empty
and full lists, extension-field exclusion and v1/v2 stores. Synthetic inputs
travel over bounded stdin; neither oracle failures nor mismatch output echo
destinations. Private-socket and executable tests cover exact requests, maximum
list encoding, safe rejection, corrupt/unsafe/symlink stores, revoked/stale
ownership, sensitive success output only and unchanged bytes/host calls.

Python still owns installed custom-rule reads/add/delete and routing-tools UI.
Rust gains only the ownership-gated semantic read; no live tunnel test or
installed frontend/cutover acceptance is inferred. Remaining routing mutations,
provider refresh/check, frontend composition and R6 are still pending. Python
remains the reference and rollback; it cannot be removed yet.

### Native onboarding completion checkpoint

`onboarding.complete` / `onboarding complete` now supplies the store-only
first-use completion action through the shared native owner. Actual Python
command parity and exact private socket/CLI gates are described in
[`R5_ONBOARDING_COMPLETION.md`](../testing/R5_ONBOARDING_COMPLETION.md).
No-op/replay avoids duplicate writes, interrupted preset recovery blocks
admission, and no host observation or connection/startup transition is made.
Python still owns the installed onboarding frontend until cutover.

### Client-side desktop helper checkpoint (2026-09-07)

The fixed native `desktop` CLI now supplies bounded clipboard/file acquisition,
zenity editing, qrencode PNG output and atomic private exports without adding
daemon methods or touching store/lifecycle ownership. The boundary, Python/QML
oracle evidence, explicit private output, cleanup and acceptance matrix are in
[`DESKTOP_HELPERS.md`](DESKTOP_HELPERS.md). Native GTK4 picker fallback is
deliberately not claimed; do not switch the installed QML picker until that
existing Python fallback or an explicit dependency policy is addressed.
Installed QML composition and exact-head desktop smoke remain distinct gates.
Python remains the installed owner and cannot be removed yet.

### Native live-rule diagnostic dispatch checkpoint (2026-09-07)

The fixed `diagnostics.summary/rules/providers` methods compose accepted Mihomo
projection primitives with exact native-owner fencing and detached bounded
private Unix-controller reads. Python remains the installed owner and oracle;
no frontend activation, private store mutation, new controller, tunnel or
ownership cutover is performed. Python cannot be removed yet.

An effect-free oracle compares actual Python loaded-rule/provider projections
using synthetic inputs and digest-only output. The native projection fills the
previous missing provider `updatedAt`, fixes REJECT prefix classification and
generic UUID redaction, and explicitly reduces per-collection byte budgets to
fit v1 framing. Provider map order is canonical rather than controller insertion
order. Additional canonical credential/endpoint redaction strengthens the old
exact-name/link filter; it never changes stored credentials.

Deterministic socket tests must prove bounded slow-controller behavior,
private modes/peer checks, no lock held across I/O, status/disconnect admission,
stale completion refusal and no diagnostic store/revision effects. Exact-head
installed Mihomo synthetic-controller acceptance remains separate from an
installed native-owner/UI cutover; the latter is not claimed by this checkpoint.

Installed Mihomo 1.19.30 can answer `/version` before loading configured rules
and providers: `/providers/rules` transiently returns `{"providers":null}` even
when an inline provider is configured. Null remains a rejected projection, not
fabricated empty-provider evidence. The isolated acceptance waits under a fixed
deadline for the expected synthetic rules/provider counts before exercising
diagnostics. This is fixture readiness, not a production lifecycle-readiness
fix; an early diagnostic may safely return `core_rejected` until initialized.
The follow-up lifecycle gate must distinguish controller liveness from complete
config/selector readiness before claiming the overall runtime ready.

The subsequent [configured-readiness checkpoint](../testing/R5_CONFIGURED_READINESS.md)
introduces native-host admission/observation of requested mode and selected
members, initialized collection shapes, and bounded controller I/O. It retains
the stronger exact fixture checks above: provider download success and complete
rule contents are not inferred from collection shape. The subsequent startup-only
Full VPN correction restores generated nested selectors through owned-child
authenticated private requests without mutating ordinary observations.
Production preflight now shares the configured predicate for active-profile
admission with authenticated fixed GETs and private-state/process revalidation.
Packaged Full VPN acceptance remains an explicit #183/#178 follow-up; there is
no installed-owner cutover.

### Native bundled routing preset checkpoint (2026-09-07)

Fixed preset selection now prepares the three checked-in bundles without
client paths/YAML or network fetching. The serialized native owner composes
template, private-store preference and desired mode through one compensated
plan and the existing active replacement lifecycle. `keepMode` preserves
canonical native desired state; first-run selection chooses Rule. No-op,
metadata-only, replay and generation-fenced rollback have distinct paths.

An actual Python `use_bundled_template` oracle compares all three bundles,
three modes and both selection semantics, plus first-run missing templates,
via bounded synthetic stdin and full
template/store digests. File/member, desired-generation, core-failure and
socket tests supplement parity. Files are individually atomic; ambiguous
multi-member compensation requires manual recovery. No installed frontend or
native owner cutover occurs, and Python remains production owner/oracle.

A fixed durable pending marker covers the gap between individually atomic
members. It is created and synced before writes and removed only after full
verified commit or restored files plus recovered old runtime. Interrupted or
ambiguous completion blocks the shared native mutation/replay admission and
startup reconciliation; no automatic recovery or arbitrary cleanup API is
introduced. Subprocess exits after each member exercise restart refusal before
host effects. Marker safety/uncertain restoration and cross-method socket
rejection supplement the existing synchronous compensation tests. This does
not promise cross-file atomicity, a recovery journal, or installed cutover.

### Native custom-rule mutation checkpoint (2026-09-07)

The native semantic boundary now includes custom-rule add/delete, preserving
canonical matching, ordering, duplicate/capacity rejection and complete private
store normalization. Exact IPC and fixed stdin-based CLI reuse the one owner's
revision/replay/ownership fences, private atomic writer and active replacement
compensation. No parallel lifecycle owner or direct QML store mutation is added.

An effect-isolated oracle runs actual Python `save_custom_rule` and
`delete_custom_rule`; 32 synthetic positive/negative/boundary cases compare full
normalized-store digests through bounded stdin. Native transactional recovery
is deliberately stricter than legacy save-first behavior: it verifies active
ownership before effects and requires proven original-config recovery after a
failed candidate, without exposing raw Python/backend errors. Protocol/socket
and fault tests supplement parity; installed native owner/frontend acceptance
remains distinct. Python still owns production and remains oracle/rollback.

### Native support configuration checkpoint

The independent [support configuration report](../testing/R5_SUPPORT_DIAGNOSTICS.md)
adds exact `diagnostics.export` plus fixed CLI with a bounded shareable
counts/preferences projection. Actual Python support-output subset parity and
private-socket/CLI privacy gates cover this read. It explicitly omits fresh
host/service/TUN/controller observations and does not infer effective login
autoconnect from stored preferences. Installed QML/Python ownership and the
remaining support/settings/host integration gates are unchanged.

### Native route-check fast-path checkpoint (2026-09-07)

The independent `routing.check` checkpoint ports exact mode/custom-rule/
disconnected outcomes using the canonical private store and ordered rules.
It adds fixed stdin-only CLI syntax, private Unix semantic dispatch, owner
revocation checks and a digest-only actual-Python differential oracle. The
production plugin/backend remains unchanged at that checkpoint. Connected unmatched live probes,
latency scheduling and TUN traffic collection remain separate migration gates;
this checkpoint must not be called complete route-check parity or Python
retirement. Scoped IPv6 is deliberately fail-closed rather than accepting a
client-selected host interface.

### Native exact-attribution Routing observation checkpoint

The follow-up [bounded live observation](../testing/R5_ROUTE_OBSERVATION.md)
implements the connected/unmatched Rule path with exact held-probe TCP tuple
attribution, owned controller PID and accepted-socket inode proof, three-second
detached work, and desired/store/config/revision revalidation. It also corrects
the Python reference's unsafe global hit-counter and destination-only fallback.
Pure fast paths remain unchanged. This is not production cutover or complete
Routing acceptance: current installed file-capability/proc visibility makes the
native live path unavailable unless ownership proof is possible; #178 remains a
separate packaging gate. Synthetic no-TUN gates must not be reported as private
VPN interoperability evidence. Latency scheduling and TUN telemetry remain
separate migration work.

### Cloud owner/batch integration checkpoint (2026-09-04)

The integration branch combines the long-operation registry from #154 with
#156's budgeted preparation worker. The offline native coordinator now owns
start/replay, shared operation-ID collision checks (including external-fetch
preflight), progress/cancellation, shutdown revocation and final atomic commit.
Worker steps run outside the owner/migration locks; completion reacquires the
migration lock and rechecks ownership, global revision and every store member.
Empty batches do not write, read the timestamp source or increment revision.

At that historical checkpoint this was not a registered background service.
The subsequent merged #161 checkpoint below supersedes this registration gap.
At the owner-only checkpoint the production owner constructor,
RuntimeServer scheduler and semantic CLI do not invoke these APIs yet. The next
slice must bind the actual runtime instance ID, supervise one worker, share the
existing fetch pool, yield on Busy, terminalize spawn/panic/shutdown failures,
and expose only the reviewed fixed start/get/cancel methods. A private supervisor ticket can reclaim a dropped/panicked worker and permits
a new batch without restarting the owner; a stale ticket cannot revoke its
successor. The scheduler must retain and use it. No production
cutover or Python retirement is claimed by deterministic owner integration.

### Registered scheduler/IPC checkpoint (#161 merged 2026-09-08)

The successor to the owner-only checkpoint above registers the reviewed fixed
start/get/cancel methods in RuntimeServer and the semantic CLI. One supervised
worker shares the unary fetch pool; shutdown revokes then joins, and network
work runs outside the serialized owner. Native production ownership remains a
prerequisite. Synthetic Unix-socket and worker tests cover the new composition.
The accepted head is `aacff3a323d49bac671d78b2a7d04f535a43c79a`; merge is
`5fc602f95903fb061dcb13e0d2663f77e663ba0f`. Local Try Omarchy ARM64 gates passed
580 Rust tests (4 existing ignored), installed-Mihomo opt-ins, strict clippy and
parity; the unchanged Python/QML reference gate passed 270 tests. Actual private
Unix sockets and production HTTP transport against controlled loopback fixtures
cover success, failure, cancellation, ownership revocation and shutdown. This
does not claim private-provider or installed QML acceptance. Interrupted preset
state and uncertain batch persistence block the shared owner safely.
See `docs/testing/R5_BATCH_SCHEDULER.md` for current evidence boundaries;
`CLOUD_BATCH_SCHEDULER_HANDOFF_2026-09-04.md` is historical. Frontend compatibility
client integration and the actual R5/T1 transition remain implementation work;
this is not production cutover or permission to remove Python.

### Offline login-intent decision boundary

The pure planner described in
[`R5_LOGIN_INTENT_PLANNER.md`](../testing/R5_LOGIN_INTENT_PLANNER.md) separates
first-login preferences from current desired state. Daemon restart and an
already-consumed login preserve exact durable intent, including explicit
disconnect. First login resolves canonical configured preferences or refuses
unresolved legacy enablement, with bounded generation changes only when needed.
It performs no host effects, writes, IPC registration or installed activation.
Once-per-login consumption, ownership/lock proofs, configured readiness and
legacy unit conversion still belong to the future fixed host integration.

The subsequent [offline login transaction](../testing/R5_LOGIN_INTENT_TRANSACTION.md)
applies those plans to private files under owner/migration locks and ownership
generation fences. A pending/consumed receipt protects against blind retry of
interrupted writes. This is not multi-file atomicity or cross-boot recovery;
uncertainty remains explicit. No production caller or login unit is added.

The [read-side startup barrier](../testing/R5_LOGIN_STARTUP_BARRIER.md) now checks
the canonical receipt under the migration lease before native reconciliation.
Unsafe/pending/stale receipts refuse without host effects; matching consumed
receipts preserve normal current-intent recovery. Candidate cutover refuses any
receipt. This does not activate the applying transaction or establish a trusted
login epoch. Installed Python ownership, #178 and the R5/R6 gates remain.

The [strict startup inventory foundation](../testing/R5_STRICT_STARTUP_OBSERVATION.md)
adds separate bounded process/TUN reads that refuse incomplete observations
instead of returning zero. Existing tolerant projections and production callers
remain unchanged. Fixed service/controller absence checks, snapshot-input
validation, trusted login ordering and installed activation are still required.
