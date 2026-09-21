# Physical x86-64 PC: 0.8.0 pre-install checkpoint

**Verdict: 0.8.0 ACCEPTANCE INCOMPLETE — REAL GATES REMAIN.**

This is executed artifact/static/read-only host evidence, not installed package,
guided-install, visual or live VPN acceptance. The supplied September 16 PC
handoff authorizes local integration and a report PR, not merge, release assets
or marketplace publication. No such publication was performed.

## Exact identities and scope

- Refreshed `main`: `4b970e129fe9a72241cc9e0c7ac434d7d85bd74a`.
- Runtime package source A: `b7fd0a99b8b169f0933e5f43ea4389642015193a`.
- Common frontend r2 source B: `99e842a66241c6d2f3b425f7d73c371d3a653850`.
- Draft #249 remote head: `0109e9a4b210da135e48ac725ed184d9b8cec34f`;
  its final checkpoint is documentation after B. Its CI was green when inspected.
- #250–253 are already merged. They were not replayed. The later #254/#255
  research/documentation updates do not justify rebuilding the accepted runtime.
- Draft #30/#135 and frozen `archive/python-legacy` were left unchanged.
- This writer owns only `dev/080-x86-64-acceptance`; #249 remains untouched.

The actual remote September 16 PC gate supersedes the stale prompt's assertion
that assets/pins are absent. `v0.8.0` already exists as a **prerelease**; its
publication does not imply completion of PC or marketplace acceptance.

Tests below ran in a clean detached checkout of B, not against a locally
modified source tree. The repository's main checkout was fast-forwarded from
its older state; the owner's untracked handoff file was preserved.

## Downloaded artifacts and offline pairing

Versioned public release downloads were retained outside Git and checked before
archive parsing. No `curl | shell`, dependency bypass or root install occurred.

| Artifact | SHA-256 |
| --- | --- |
| `omavless-0.8.0-1-x86_64.pkg.tar.zst` | `57e4599bfeb90e063951a49295ae6154703a0739c3c1accc751b3f1fd4343568` |
| Embedded x86-64 executable | `6b99a443c49dacb21d5f668cd33290aefce132cb0edcae7720617eb4f653edd1` |
| `omavless-0.8.0-frontend-r2.tar.xz` | `82088b4bb8682e8e2b9f9c9001e6f9ff3ad943b0268217930ab056dca26c3b8d` |

`pair-frontend.py` successfully assembled an **x86-64** local pair using the
downloaded package and exact B. Runtime/build/package input equality passed;
its tree digest is
`001506e80ea708a735627aa40a4a8d649f4d0c7d108515c4cb111c43e38c762f`.
Bootstrap pins matched. The regenerated frontend archive is byte-identical to
the published frontend-r2 archive. No Rust release rebuild was necessary.

The inspector verified architecture, schema-3 build identity, embedded ELF
digest, exact payload and unit digests, safe member types/modes, and absence of
unlisted files/install hooks. Package dependencies were inspected independently.
The retained upstream provenance identifies CI build run `35086544726`, Rust
1.98.0 and x86_64-unknown-linux-gnu. The PC has the matching glibc 2.44 build.

The published pair JSON describes the earlier ARM assembly. It is not x86
host evidence. Both assembly JSON records retain their creation-time
`publishedDownloadVerified: false` field; the actual download/hash checks above
are separate later evidence, not a silent rewrite of those records.

Private local retention root: `~/.local/share/omavless-acceptance/2026-09-21/`.
It contains `assets/`, `x86-pair/`, an inspected package extraction and
`recovery/legacy-before.tar.xz`. These are not submission materials.

## Executed tests on B

| Check | Result |
| --- | --- |
| `./tests/run.sh` | PASS: 275 Python cases, 273 passed / 2 skipped; JS and QML contracts passed |
| `./tests/run-rust.sh` | PASS: formatting, workspace tests, Clippy with warnings denied, R0 parity 2/2 |
| Rust test summary | 957 passed, 10 ignored across 72 result groups; conditional no-opt-in cases are not live-core evidence |
| Installed Mihomo validation-effects opt-in | PASS separately: 1 synthetic localhost case, no TUN or private profiles |
| Installed Mihomo bounded-diagnostics opt-in | PASS separately: 1 Rust case exercising synthetic Unix-controller diagnostics, no TCP controller/TUN/DNS |
| Manifest JSON and `omarchy plugin validate` | PASS |
| `qmllint` with installed Omarchy/Qt imports | PASS for all 26 tracked QML files |
| Installed QML component compilation | PASS: valid fixture passed, invalid fixture refused with exit 1, candidate graph passed; isolated runner did not instantiate the plugin |
| Shell syntax | PASS for all 17 tracked shell scripts |
| `git diff --check` | PASS |
| Source symlinks | None in candidate checkout outside excluded Git/build paths |
| Recovery archive integrity | XZ integrity and tar readability passed |
| Report navigation and whitespace | Documentation navigation and `git diff --check` passed |

The two Python skips in the full run were the installed-core opt-in (then run
separately above) and the root-only refusal case. No blanket installed-core,
visual, R6 replay or private provider success is inferred from this suite.

## Read-only PC baseline and preflight

- Physical x86-64 Omarchy; native `omavless` package/executable absent.
- Existing frontend is version 0.7.0, disabled. Mihoro frontend is also disabled.
- Existing private store is schema 3, with 56 profiles and one subscription.
  No active profile pointer; legacy startup preference is enabled. Existing
  profile identifiers/names/endpoints were not exported into this report.
- Native and legacy OmaVLESS units were absent/inactive. `mihomo.service` was
  inactive and disabled. V2RayN/Xray were running and were not stopped.
- An existing local Mihomo v1.19.30 binary has TUN capabilities, but no installed
  package provides dependency `mihomo`: `pacman -T mihomo` reports it missing.
  A loose binary is not sufficient for normal `pacman -U` dependency resolution.
- A private 0600 archive in a 0700 recovery directory preserves the old plugin,
  configuration, state and shell registration before any migration.
- The extracted **actual published ELF**, without installation, reported
  `store-compatibility`: `compatible: true`, recovery `none`.
- Its read-only `cutover-preflight` reported
  `blocked_inconsistent_host_state`, legacy marker phase, core count 0 and TUN
  count 2 at that observation. No activation was attempted.
- A speculative `--version` check was rejected as an invalid semantic command;
  identity is established by build metadata/ELF hashes, not a nonexistent CLI flag.

The host also has a Tailscale interface. Code inspection confirms the current
strict observation counts **all** interfaces with `tun_flags`, and activation
requires an empty host. Quit also requires `visibleTunCount == 0`. Consequently,
an unrelated remaining TUN can prevent migration/clean-Quit proof: this is a
product precondition/coexistence limitation, not merely an overly strict test
harness and not proof that Tailscale currently redirects the default route.
Do not silently stop Tailscale, weaken guards or remove its interface to make a
test pass. Resolve the owner's intended network state before proceeding.

## Remaining attended work

| Gate | PC status |
| --- | --- |
| Install actual x86 package and activate native ownership | NOT RUN — dependency absent; legacy startup/host TUN preconditions unresolved |
| Guided terminal setup and cancellation/partial-effect handling | NOT RUN — requires owner in a real terminal and settled authorization |
| Matching installed frontend, running ELF/units, preserved data | NOT RUN — old installation intentionally retained |
| EN/RU main, Settings, import and subscription confirmation | NOT RUN — static checks are not rendered acceptance |
| Live VLESS Full VPN / Routing, bounded HTTPS/DNS, disconnect/restore | NOT RUN — working V2RayN connection preserved |
| Startup Off, shell restart neutrality, confirmed Quit/reopen | NOT RUN |

ARM acceptance remains exactly as recorded in #249: existing-core guided
success and initial cancel/reopen/onboarding evidence, not missing-core/AUR or
partial-effect-cancel proof. This PC checkpoint adds no ARM claims. AUTO-1,
missing V0 fixtures and previous DNS/provider findings remain unchanged.

Next: with the owner available, inspect and agree the competing VPN/TUN state;
use the existing supported legacy control to set startup Off; satisfy the
Mihomo package dependency through the documented attended path; then follow
the native installation and activation contracts. Do not initialize over the
existing store or hand-edit ownership markers. Every authorizing effect needs
the real-terminal `ready`/`settled` procedure. No current installation or
network state has been changed by this checkpoint.

Final read-only check: private profile store is byte-identical to its backup;
native/legacy OmaVLESS services remain absent/inactive, `mihomo.service` remains
inactive/disabled, and the existing V2RayN/Xray processes remain running.
