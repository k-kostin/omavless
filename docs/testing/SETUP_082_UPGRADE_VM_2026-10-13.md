# Prior-public-stable setup failure: x86_64 VM

## Scope and identities

Agent-operated Omarchy 4.0.4 x86_64 development VM, 2026-10-13. This is an
installed prior-stable upgrade reproduction and manual disconnected migration,
not guided-updater, human-attended, ARM64 or VPN-connectivity acceptance.
The physical PC was inspected read-only; no physical package, service, routing
or profile changes were performed.

Before testing, the previous development overlay and matching NVRAM were
archived after independently confirming guest shutdown and no disk writer.
Protected historical recovery evidence remains inside that archived overlay.
A fresh overlay over the unchanged clean baseline isolated this experiment.
Raw logs, images, stores, backups and archives remain outside Git.

Later sections distinguish the newly tested guided successor from this initial
manual reproduction. None is evidence of a published fix.4 download.

| Item | Exact identity |
| --- | --- |
| Historical public frontend `v0.8.2` | `f442714362620c18e1bbaa6415d9e0c2e08c0a8a` |
| Public predecessor package source | `22e23e64c49b8110088b6be3f063b5e641ba0853` |
| Public `omavless-0.8.2-1-x86_64.pkg.tar.zst` SHA256 | `381a1427b2728f35bfd49247e4373238deb4ef53c5850eec2ad68e91de382b6c` |
| Public predecessor ELF SHA256 | `3ff795316aa331463aff4e26571546433bdfd9317f348712d135be2f0a8d28a1` |
| Updated main frontend | `b4ef783e14676099b9d6429304c57542ff948804` |
| Public `v0.9.8-fix.3` pair producer | `8d02f9d7dd1a9063a82e9dcd93cb19cf4c5d8524` |
| Public `omavless-0.9.8-1-x86_64.pkg.tar.zst` SHA256 | `bf27584874c7e75d40f856739783f66fdf6af5652e37ba638232a2bc5a3c7750` |
| Public `omavless-dns-0.9.8-1-x86_64.pkg.tar.zst` SHA256 | `3309fb54ab92c5f1f30f6ba3c925ddc153619841dc4ae5640f4f279383f6b464` |
| New app ELF SHA256 | `118a12989f737e7c5f7ee167d10f6487ab6ca16ba4ed34f8dae71e813a384bd4` |

## Reproduction

1. Install the public predecessor and stock Mihomo 1.19.32. Complete the old
   setup through its real terminal; original invocation exited 0. Import one
   synthetic, nonfunctional RFC5737 profile via stdin. Never connect it.
2. Full Quit the predecessor through its native protocol. Original invocation
   exited 0. Confirm Desired disconnected, startup Off and runtime inactive /
   disabled / PID 0. Record the post-Quit profile/template/ownership baseline.
3. Run the ordinary `omarchy plugin update kdk.omavless --yes`, then enable
   the plugin. Original command exited 0. The frontend is main 0.9.8, but the
   installed application remains public 0.8.2 and the DNS companion is absent.
4. Open the actual panel: inventory is `needs_attention / missing`; no working
   install/update continuation is offered. This matches the owner's reported
   dead end. Frontend-only remove/add is not an application-package upgrade.

This establishes a real prior-stable user-path gap. It does **not** establish
the cause of issue #772: that reporter described both 0.9.8 packages installed,
so their partial-setup failure still needs a precise failed-stage result.

## Manual continuation, original outcomes

Separate private config and state backups were retained. Both public new
archives and receipts were verified before a normal paired pacman transaction.
The package transaction exited 0; profiles and ownership remained byte-identical.

After installation, main reported `needs_broker / present`: an existing Rust
owner without the newly required selected companion has another missing guided
continuation. Run the reviewed enrollment/selection operation once, with
separate DNS consent and guest-only sudo authorization. Original invocation
exited 0. First enrollment was genuinely absent; no existing enrollment was
repeated. Profiles, ownership and Desired remained byte-identical; the private
pre-managed template backup exactly matched the old template.

Main then reported `needs_runtime_start / present`. Click **Check again** and
the actual **Start OmaVLESS** button once. Runtime became active; the unit stayed
disabled, Desired and actual state stayed disconnected, and the synthetic
profile remained. Final component inventory was `ready / present`. The broker
was active with an empty FD store; links remained only loopback and the guest
Ethernet interface. No TUN, DNS lease, VPN connect or network health claim.

Opening the first-use wizard exposed a separate projection problem: it showed
stock Mihomo 1.19.32 / missing capabilities after the managed pair was selected.
The shared stock core was not granted capabilities to conceal this mismatch.
The wizard was closed without connecting or changing startup preferences.

## Ordinary remove and add after the paired update

Inspect the exact clean plugin checkout and the installed Omarchy removal
implementation, then run ordinary `omarchy plugin remove kdk.omavless --yes`
in the guest. Original invocation exited 0 and removed only the identified
frontend checkout. The QML destruction handler schedules the existing native
removal watcher: runtime subsequently exited cleanly with status 0 and remained
disabled. Thus an immediate post-remove active-PID sample is **not** proof that
the runtime remains running indefinitely. The app/DNS packages, profiles,
Desired and ownership remained; the broker stayed active with empty FD store.

Run ordinary `omarchy plugin add https://github.com/k-kostin/omavless.git
--enable --yes`. Original invocation exited 0 and restored the same main head.
Component inventory was `needs_runtime_start / present`; the actual panel
offered **Start OmaVLESS**, not a package-install prompt. After clicking that
button on the settled panel, the runtime became active with a new PID, the unit
remained disabled, and inventory returned `ready / present`. Profiles, Desired
and ownership still matched their saved pre-upgrade bytes. This is agent-operated
disconnected remove/re-add evidence, not full package/DNS uninstall or connected
removal acceptance.

## Source corrections and remaining gates

- Setup successor `c8b86b484593f6a0b00525b57cda456366c91298` adds precise safe
  failure stages and a prior-stable update-help card, not an automatic updater.
- Predecessor qualification `953308874be5802d9248d8042716c5dfee989109`
  qualifies fixed public bytes without executing the old application or claiming
  live transaction authority. Normal runner at that head: 715 Python tests,
  two declared opt-in skips, JS/QML contracts, exit 0.
- Physical read-only inspection found the exact bundled default with only its
  canonical Global mode header changed. Published template preparation rejects
  it despite selected rendering recognizing canonical mode variants. Separate
  source correction `a9fa44c154fd203d66cdbd0e27de1f992c00bbb6` admits only exact
  default Rule/Global/Direct variants and preserves original/Desired bytes.
  Independent source review found no blockers; seven managed-selection tests
  were independently rerun successfully. It is not yet installed acceptance.

Still required: the complete guided paired-updater path for a qualified
predecessor, safe existing-Rust partial setup, correct managed-core onboarding
projection, the reporter's specific failure stage, and friendly removal at all
setup states under the separate 0.9.9 contract. Do not call the original issue
resolved, normalize private templates, repeat unknown enrollment, or substitute
this manual migration for those product gates.

## Partial DNS continuation successor

Frontend source `32fe70b7b8eefafb980c403bff8b381976f04878` adds the real
**Continue DNS setup** entry for a stopped, valid but unselected managed pair.
`54dc275f21c967391c53932ec6f874c341dca33f` additionally fences effects behind
settled native actions without disabling read-only inventory during uncertainty.
The installed native pair remains the exact public fix.3 pair listed above;
the native default-mode correction is not installed by this frontend test.

Two separate agent-operated VM scenarios completed through the actual button
and terminal, not a mock installer:

- Existing enrollment: source32, explicit `START`, normal guest sudo, then
  `SELECT`. The terminal reached its components-prepared message and independent
  inventory became `needs_runtime_start / present`. No new enrollment or
  repeated Rust activation was requested.
- First enrollment: source54, explicit `FIRST`, separate `DNS` consent and
  normal guest sudo. After root empty-state admission, enrollment, broker start
  and selection completed. The actual terminal again reached components-prepared;
  independent inventory became `needs_runtime_start / present`.

These were deliberately prepared unselected fixtures, not claims that a prior
unknown operation had completed. The old selector was retained privately.
Before the second scenario, the known empty broker was stopped, its independently
inspected stale fixed socket removed administratively, and its enrollment revoked
through the shipped root guard. Stop/revoke original invocations exited 0. This
is fixture administration, not automatic cleanup or product recovery. An ordinary
idle stop leaves its socket behind under `RuntimeDirectoryPreserve`; no blind
socket removal was added to the installer.

After each scenario profiles, Desired, ownership and restored selector matched
their saved bytes. The runtime remained inactive/disabled until the separate
actual **Start OmaVLESS** button was used after acknowledging the closed setup
terminal. Final native snapshot was successful with one synthetic profile,
Desired false, saved startup configured/disabled and last-known Disconnected;
component inventory was ready, runtime active/disabled, broker FD store empty,
and links only loopback/Ethernet. No VPN-connectivity acceptance is inferred.

At source54 the normal runner exited 0 with 726 Python tests, two declared
opt-in skips and JS/QML contracts; installed-import qmllint passed for affected
QML. Separate network/home/runtime-isolated offscreen rendering inspected the
real RequiredComponents in EN/RU with effects available/blocked. The controls
projection confirmed Continue disabled under the effect fence while Check/help
remained available. This is synthetic component evidence, not an installed
pending/unknown native-operation scenario. The normal installed EN FIRST path
does not substitute for the unrun installed legacy-owner or fault matrix.

The post-start fixed-fact waiter from #777 has a synthetic delayed-read regression
and never repeats enrollment/start. The shipped broker is **Type=notify**, not
Type=simple, and sends initial READY after its socket/ACL checks. Consequently
the mock does not establish a public startup race or the reporter's root cause.

For the remaining paired-package updater, the reviewed bounded phase split is:
UPDATE owns original exclusions through replacement and installed/private-state
verification, then ends with packages-updated/DNS-required. A later explicitly
chosen DNS setup is a new operation earning fresh admission; it never inherits
authority from an UPDATE result. No updater, main release or physical-PC update
is accepted by the partial-DNS checks above.

## Guided x86_64 successor: package update, DNS and Start

Installed frontend `5c4aead288273080d16bd705c270bfbed9c290e7` was paired with
native/DNS producer `5fb00e20b2ff6c50be0a42d9eaacc63d5304b5e6` by the normal
offline release-triple inspector. Both archives were newly built, not modified
public packages. App SHA256:
`dc82ca68e3659169221ec4bb335c1dec5c8938a9a2f040937dcf2c742652c601`;
DNS SHA256:
`63c3b7f7382f7ab2a54106021bbd6d61a7bdcfb8a407ab2e7dcf11b81bc8bba6`.
These are **unpublished** fix.4 candidate bytes, transported into the exact
digest-qualified cache for this VM gate; no public HTTPS-download PASS is claimed.
ARM64 pins/build are deliberately absent rather than copied from an older pair.

The guest first completed the genuine public 0.8.2 setup and stdin import, then
its genuine Full Quit. A preliminary developer-only helper package phase also
completed and preserved all private config/state bytes. Ordinary pacman downgrade
and guarded removal of its still-unenrolled, inactive DNS package returned the
fixture to the exact stopped public predecessor. For the guided successor test,
the exact default template was deliberately changed only from Rule to Global
as fixture administration; that baseline was retained privately. This is not a
product repair or a modification to an actual private provider configuration.

The installed **Update required components** button opened the real terminal.
The candidate was extracted from the pinned application archive and independently
verified its own ELF, original predecessor and private-state admission. Separate
`UPGRADE`, ordinary guest sudo and pacman confirmations produced
`packages_updated_dns_required`; the original helper completed. Profiles, Desired,
ownership and the entire pre-DNS config/state comparison were unchanged. Runtime
remained inactive/disabled, broker inactive, and there was no managed TUN.

After closing the terminal and acknowledging settled prompts, the real
**Continue DNS setup** button opened a new operation. Explicit `FIRST` and separate
`DNS` consent completed first enrollment, broker start and selection. No repeated
Rust activation or initialization occurred. The original Global template backup
was byte-identical, the managed template retained Global, and profiles/state
still matched the saved baseline. The UI then offered the separate **Start
OmaVLESS** action. Its actual button started the runtime without enabling it or
connecting: final inventory `ready / present`, one synthetic profile, Desired
false, saved startup configured/Off and last-known Disconnected; broker FD store
zero and no managed TUN. This is agent-operated disconnected setup acceptance,
not tunnel/connectivity or human-attended acceptance.

Selected-core inventory now reports the bundled file's capabilities as present;
the stock core remains without granted capabilities. One remaining packaging
finding is explicit: this ordinary source builder did not embed the upstream
version linker value, so the bundled `-v` string uses its default `1.10.0` and the
strict desktop version projection is null. Receipt/ELF/source identity is verified,
but an accurate displayed core version is **not** PASS and needs a packaging
correction before final delivery.

### Loaded-QML boundary and workflow correction

An actual failed UI observation was retained: after atomically replacing the
old frontend, disable/enable and rescan left old 0.8.2 dependencies mixed with
the new panel. CLI inventory was correct, while old SetupState parsing projected
`needs_attention / null`; a new Panel also reported a property absent from the
old Service. An independent inert real Quickshell Process confirmed stdout
completion preceded exit with the exact 21-byte inventory, excluding the proposed
collector-order explanation. The documented deliberate **guest-only**
`omarchy restart shell` established a fresh engine and correct visible Update,
Continue DNS and Start controls. No core or VM restart was used for this refresh.
Future acceptance must verify loaded distinctive controls/dependencies, not
substitute installed hashes or a rescan success. A shell restart is not silently
added to an installer or declared universally VPN-neutral.

The final normal runner at `3d6370f0473afb6bf6994075d919291a932b0190` exited 0:
736 Python tests, two declared opt-in skips, JS/QML contracts. Native 22 focused
upgrade tests and combined selected-feature clippy passed; affected installed-
import qmllint, manifest validation, syntax and documentation navigation passed.
The DNS build exposed an unrelated test-path limit: a long HOME cache TMPDIR
still exceeded the Unix socket limit after Go's generated test suffix. A single
qualified short owned HOME compiler directory allowed the unchanged pinned tests
to pass; both temporary short directories were removed after known build exit.
No fixture socket assertion or security admission was weakened to bypass it.

Remaining before external delivery: accurate embedded core version and any
resulting new package identities, ARM64 matching build/installed gate, public
download verification, remaining installed fault/RU matrix, explicit release/main
authorization and separately attended physical-PC update. Issue #772's own failed
stage remains unknown; no additional reporter comment or resolved claim was made.

## Corrected core version: repeated guided upgrade

The previous core-version failure was preserved, not relabelled. The ordinary
DNS producer now embeds the pinned upstream release's `constant.Version` linker
value. Exact new native/DNS producer:
`b82a51ba8d63d251b3040c6c71470cfcafb85347`; installed frontend:
`b80dc946ccf945deda9b0dda2503ca223c63084b`.

| Artifact | SHA256 |
| --- | --- |
| x86_64 app archive | `fd196541ca94276d742d4a1c911c841792cf99e043e11266d55619920db735a9` |
| x86_64 DNS archive | `29d37fb8c960b4e8e38f8a387c2449a4318adced414627b60162804452c90d29` |
| Managed core ELF | `365d7a4ee4e24a0ceb11037bbc791a77e23823a8864769677fc97f4ae9b501e8` |

A separate clean baseline overlay completed the genuine public 0.8.2 setup,
synthetic stdin profile import and native Quit. Its exact default template was
changed only to canonical Global for the fixture. With the newly built matched
archives in the digest-verified cache, the actual **Update required components**
button completed ordinary UPGRADE/sudo/pacman and returned
`packages_updated_dns_required`, without connecting or activating the runtime.
The actual **Continue DNS setup** button separately completed FIRST/DNS and
returned `needs_runtime_start`. Profiles, Desired and ownership stayed
byte-identical; the original Global template backup matched its saved bytes.

The test driver's tool-owned QEMU was unexpectedly terminated by Codex after
DNS setup had completed. This was retained as an infrastructure interruption,
not a product failure or proof of setup retry safety. The same overlay booted
normally: selected pair and preserved bytes were independently confirmed;
enrollment was not repeated. The actual **Start OmaVLESS** button then returned
`ready`. The real first-use wizard displayed **Installed version: 1.19.31**,
TUN device present and managed-core file capabilities present. Stock Mihomo
1.19.32 remained without file capabilities. Runtime active/disabled, Desired
false, startup Off, one synthetic profile, broker FD store zero, no TUN.
Native Quit and normal guest poweroff completed before archiving this checkpoint.

VM-driver correction: a long-lived guest launcher must not inherit a bounded
command tool's process lifetime. Running the unchanged reviewed launcher in a
temporary user-systemd service preserved observation between tool calls;
there was no autostart, launcher edit or physical VPN/system change. Confirm
shutdown and absent disk writers before swapping overlays. A checkpoint is
administrative preservation, not product rollback acceptance.

At producer b82, the normal runner passed 738 Python tests with two declared
opt-in skips plus JS/QML contracts. At frontend b80, 22 native upgrade tests,
manifest validation, affected QML with installed Omarchy imports and shell
syntax passed. These are still unpublished x86_64 candidate/cache gates, not
public download, ARM64, connected or human-attended acceptance. Issue #772's
reporter-specific failed stage remains unknown.

## Fresh install and cancelled-DNS continuation

A separate empty baseline overlay, boot
`f14d07b1-121a-45dd-850f-6bd675cb129b`, contained no OmaVLESS application,
DNS package, stock Mihomo or private profile/state. Ordinary Omarchy plugin add
cloned the public repository; the checkout was explicitly pinned to candidate
`8d8d273c31a1d7414d47c47a30137ae07e8b6a1e` before enable and a deliberate guest
shell reload. This is the reviewed candidate flow, **not** acceptance of main's
unchanged installer. App/DNS remain the exact b82 pair listed above.

This frontend shares the updater's fixed digest-keyed cache with fresh install.
Both archives were preloaded for unpublished-candidate acceptance: real cache
owner/mode/link checks, exact hashes, package identities, both receipts and
repeated fresh-machine admission still ran. No curl shim, alternate URL, TLS
bypass, production Python/Cargo fallback or manually invoked pacman replaced
the actual **Install required components** button and its normal INSTALL,
guest sudo and pacman confirmations. The paired transaction completed without
stock Mihomo installed or any CORE prompt.

After the packages and private configuration had been prepared, DNS consent
was deliberately cancelled with Enter. The actual terminal reported the exact
`dns-consent` stage and a neutral partial-completion message. Both runtime and
broker were inactive/PID0; state files had not yet been initialized. Closing
that terminal and reopening the widget offered **Continue DNS setup**, without
another package installation. Its actual FIRST/DNS operation enrolled and
selected the pair, performed the normal first Rust activation, and ended ready.
The initial-install runtime unit was enabled; this is service startup, **not**
VPN autoconnect. Desired false, saved autoconnect false, empty profile list,
broker active/FD store0 and only loopback/Ethernet were independently confirmed.

The actual first-use wizard displayed core1.19.31/capabilities present, accepted
the builtin Russia routing choice, and completed its Finish later action with
no profile or connection. A settled ordinary panel displayed **Disconnected**
and the honest empty-profile message. The transient initial observation before
settling was not used as a final state claim. No setup/sudo terminal remained.

Source verification at candidate8d: normal runner passed 748 Python tests with
two declared opt-in skips, JS/QML contracts; all plugin QML passed qmllint with
installed Omarchy imports; manifest, shell syntax and diff checks passed.
Independent review found no blocking findings in the two-file shared-cache
change. Existing same-user cooperative pathname and abrupt cache-publication
interruption limits were not reclassified as stronger guarantees.

Still separate: ARM64 matched packages/installed acceptance, real published
fix.4 download verification, remaining fault/localization matrix and authorized
main/release/physical-PC delivery. No additional issue772 reply was sent.

## Cold stopped predecessor: missing volatile exclusions

A separately attended physical-PC attempt did not complete. Read-only inspection
after the owner confirmed all authorization prompts were settled found the app
still at the authentic public 0.8.2 bytes, DNS absent, and the frontend updated.
Profiles, Desired and ownership matched their retained baseline; the runtime
was inactive/disabled/PID0 and its volatile directory was absent. The closed
terminal's original native error was not retained, so that attempt's precise
failure stage is unknown. A generic message printed after `settled` is not
evidence that `settled` itself failed.

Source inspection exposed a real uncovered boundary: the updater required
existing volatile owner/migration locks. Those objects disappear after reboot.
The new regression failed with `Environment` before the correction. Producer
`fce963e59b4cdeda49a5a1f71f3cc4cc3beaaa47` admits canonical absent volatile
objects only after explicit UPGRADE and stopped, disconnected native-owner
qualification. It holds the original newly opened exclusions through the
package child and rechecks durable state under both locks. Existing unsafe
permissions, symlinks, hardlinks or changed names remain refusals: no chmod,
unlink, service Start or shell-created ownership bypass is added. A later
refusal can leave harmless canonical volatile objects; this is not zero-write
or hostile-same-UID race exclusion.

| Corrected artifact | SHA256 |
| --- | --- |
| x86_64 app archive | `a538e992008ddf1ca1e905a9d29ea7a6755eece7b3cf0ace7a827af7de701e22` |
| x86_64 DNS archive | `8f0aba0a95c3d0596b35e46d3b643e713d7dc3f75e418299c3c90ac6b7813d71` |

The normal matched-triple assembler verified frontend
`7148acb567f0c6fa9c0b0b26c617eef49932ffc9` against that producer. Unpublished
archives were preloaded into the exact digest-qualified cache, not downloaded
from a fictitious public release.

The genuine 0.8.2 fixture was fully closed, then genuinely rebooted. Before any
application Start or updater input, boot
`52e79663-29e8-48de-bd91-7fde023b5495` independently showed inactive/disabled/PID0,
absent runtime directory and absent migration lock, with private bytes unchanged.
The actual fresh-engine Update button and UPGRADE/sudo/pacman operation completed
with `packages_updated_dns_required`. Both installed package receipts matched
fce; profiles, Desired, ownership and canonical Global template were byte-identical.
The updater itself created the canonical 700 directory and 600 single-link locks.
Runtime remained stopped and no managed TUN appeared. No helper seeded those
locks, enabled the old runtime, or ran a compensating recovery.

After closing the real update terminal and acknowledging settled prompts in
the panel, the actual Continue DNS action completed separate FIRST/DNS and
normal guest sudo. The original operation reached components-prepared; profile,
Desired and ownership comparisons remained equal and the pre-managed template
backup matched. Closing and acknowledging that terminal exposed the actual
Start button. It started the app without enabling the unit or connecting:
native observation reported Disconnected, manualRecoveryRequired false,
managedTunCount0 and no core process. The real wizard displayed core1.19.31
and managed file capabilities. Broker active/FD store0; only loopback/Ethernet.
This additional gate is disconnected setup continuation, not VPN connectivity.

Fixture-driver failure is retained separately: an initial Quit refused because
the widget was not enabled. After inspecting that known refusal, the widget was
enabled and a new admitted native Quit returned shutdown true before the
authoritative pre-reboot baseline was captured. The initial compound fixture
command continuing after Quit refusal is not counted as successful acceptance;
future fixture effects must check each original outcome before proceeding.

At frontend714, the normal runner passed 748 Python tests, two declared opt-in
skips and JS/QML contracts. All 25 focused native updater regressions passed;
selected-feature clippy, formatting and diff checks passed. Independent review
found no blocking finding in the cold-exclusion correction. This is a real
agent-operated x86_64 cold-upgrade gate, not proof of the physical attempt's
precise error, the reporter's separate issue772 stage, ARM64 installation,
public-download behavior or connected/human-attended acceptance.

## Confirmed PC refusal: ancillary state-directory siblings

The next separately attended PC attempt supplied the actual native refusal:
`private_state_incompatible` immediately after UPGRADE, before sudo/pacman.
The wrapper retained its fixed phase/exit, and the owner confirmed `settled`.
Read-only inspection found ordinary retained development directories next to
the three canonical state files. The updater's blanket directory census
rejected every unrelated entry; it demonstrably rejects that shape regardless
of the canonical store's validity. No profiles or ancillary files were deleted
or moved to force acceptance. The installed native package stayed public0.8.2.

Producer `49f024b32157c0a6fbb3f11501435bb041dd9950` removes only that census.
This package-only operation never consumes or migrates unrelated siblings;
authority remains the pinned private directory, exact captured canonical
members, original exclusions, ownership/receipt checks and repeated absence
checks for every reserved transaction/candidate name. It does not tolerate
unsafe canonical files, interpret ancillary contents or infer that pending
work completed. Independent complete-delta review found no blocking finding.

The ancillary directory/file/late-arrival regression genuinely failed with
PrivateState before the correction, then passed. A preliminary incorrectly
qualified `--exact` filter selected zero tests and is not acceptance. All
27 focused updater tests passed, including the 60 reserved pending-name/type/
timing combinations (file, directory and dangling symlink; before acquisition
and under retained locks), canonical-file replacement/unsafe-file refusals,
child custody, cancellation and post-replacement verification. Normal runner:
748 Python tests/two declared opt-in skips, JS/QML exit0. Selected-feature
clippy, fmt, manifest validation and diff checks passed.

New unpublished matched pair: app SHA256
`0a9e2916bc702ca3a12d5c3400d7fb5a00f0b77a58d1ef1b36e086dc3c237cce`;
DNS SHA256 `a15414daa9b027e64c22d45537df7f00bd7a7ae1c89dccca0da37e4a6034ef40`.
Normal source build/tests, package staging and triple inspection passed for
frontend `6ec644395b3f1f202c5b0210fde91b1c05c8e73b`; selected core/broker ELFs
are unchanged, but the source receipt is newly bound to producer49f.

Actual installed ancillary-sibling gate then passed in boot
`547aadda-92cd-42b0-b360-c4c771c23eb9`: public0.8.2, stopped/disabled/PID0,
runtime/migration lock absent after reboot, and retained synthetic directories
with a control note alongside canonical state. The actual Update button,
UPGRADE, guest sudo and pacman transaction returned
`packages_updated_dns_required`; installed receipts identify producer49f.
Profiles, Desired, ownership, Global template and the ancillary note all matched
their before bytes/hash. Runtime stayed inactive/disabled, DNS not enrolled or
started, no TUN. The original terminal completed and was closed; later DNS and
Start are separate actions, not part of this new package-only acceptance.

This fixture was reconstructed administratively from the previous disconnected
gate, not through a claimed product rollback. Original native Quit refused;
no success was inferred, and a separately authorized guest service Stop was used.
The empty broker was explicitly stopped, its verified stale fixed socket removed
and enrollment revoked. An initial DNS package removal correctly refused its
still-installed application's exact dependency; the separately admitted app
downgrade followed by ordinary guarded DNS removal completed instead. The known
synthetic template/selector fixture was restored only after baseline comparisons.
Those administration outcomes remain distinct from updater acceptance. Boot
completed before new input; the disk-unlock input was not attributed to the agent.

Workflow follow-up: future updater fixtures must include ordinary retained
siblings as well as clean directories. Development notes belong in the separate
retained-work namespace, not the application's state root; nevertheless those
unconsumed siblings must not invalidate an otherwise admitted package update.
Do not repair a user's profile store, move private state, or weaken pending
transaction fences to make an installation demonstration pass.

## Attended PC continuation and clearer setup prompts

The owner completed the separately attended producer49f update: retained
wrapper result packages_updated/exit0/settled, both exact package receipts
verified independently. The first reopened panel still projected generic
attention while the installed fixed inventory returned needs_dns_setup/present.
After the owner's deliberate shell reload, the actual Continue DNS action
appeared. With registration demonstrably absent, the owner chose FIRST and
completed the separate DNS/app preparation. Read-only observation subsequently
returned ready/present, native Disconnected and manualRecoveryRequired false.
Profiles, Desired, ownership and the pre-managed template backup matched the
retained baseline. No connection-health or whole issue772-resolution claim is
made; no physical shell restart, DNS consent or VPN toggle was entered by the
agent.

Frontend `708a27bbf33b3ef4a86ca21e97c688ed14a72774` clarifies FIRST and START in
separate EN/RU lines and offers optional RELOAD only after successful direct
upgrade completion/cleanup. Exact RELOAD is separate whole-shell consent, not
automatic install behavior; Enter skips. It rechecks the stopped/DNS-pending
state and never converts shell failure into another package update. Success
after install/resume and failed upgrade do not offer reload. The successor
`4409778493c815905bf236003cda6ca5958ccd32` additionally refuses an unsafe runtime
directory or any occupied setup-lock name, including dangling symlinks; it
does not delete a blocker. Native package inputs/pins remain producer49f.

Both language versions of the exact production prompt functions were inspected
in actual guest foot windows, 1890px width/scale1.666, with the choice blocks,
spacing, warnings and input prompt visible and readable. This was an explicitly
inert terminal preview: effects mocked, Enter cancellation used; not live
enrollment or whole-shell-restart acceptance. The nonsurface guard successor
does not change those rendered strings. The initial installed resume attempt
refused a preexisting setup lock; that refusal is retained, not silently removed
or counted as a prompt-flow PASS. Guest idle preference was restored and private
baseline comparisons remained equal. Full independent source review covers the
reload boundary; dedicated deterministic tests cover exact consent, changed/
busy/unsafe state, failed reload, no retry and direct-entry action selection.
The new successful explicit RELOAD branch still needs its own installed gate
before final delivery. No physical frontend or main replacement accompanied
these presentation changes.

## Both-architecture emergency packages and installed 0.9.8 continuation

October 14 candidate: common app/DNS producer
`b65695ee0609c2a971289109975678b8da87c946`, frontend pins at
`c9ace6ffda45ac7f47005dd5a1443a93a6d601d6`.
All five producer CI checks passed. Independent strict offline inspection
checked all four archives, package metadata, paths/modes, payload/ELF hashes,
privileged scriptlets/units, fixed upstream/patches, corresponding source and
receipts. Protected runtime inputs equal the previously installed producer49f,
tree SHA256 `a7913b324cd805a9a1b0bc2ff9558fe9dc84b2b4dda224ec0fc6fe20bfff6fcf`.
This equivalence is not a claim that binaries built by different toolchains
have identical bytes or installed behavior.

| Architecture | App archive SHA256 | DNS archive SHA256 |
| --- | --- | --- |
| x86_64 | `56072c7bd870b082882d61a4e8bc5d99ef93b954b8cf2e7634230149d1568dea` | `2514cdd7e25856cf0823377eeaa509802ce5dd9c2f9253f5f7360cbcbd8e478c` |
| ARM64 | `903fa863a5648341670224965c9da226e3eb9a47e35e80a61b68e4e51dcade59` | `fc51c20e5c1fbe58fc17d3266ddca5ab20b560c2653843d8512fe1050d2ab26e` |

Boot `d3db783e-4679-4050-81d7-4bd500baa722` is the same isolated x86_64
guest, not the physical PC. Original inactive state and canonical files were
preserved before an explicitly administrative reboot. The earlier empty volatile
setup lock disappeared with `/run`; no product lock-removal/recovery PASS is
claimed. Initial unprivileged SSH reboot refused before effect; the separately
authorized guest sudo reboot completed and normal encrypted unlock/boot was
verified before continuing.

The actual production `offer_shell_reload` helper from frontendc9 was invoked
in a real terminal with real stopped/DNS-pending facts and no mocked effects.
Exact RELOAD restarted the guest shell (old process exited, new process answered
and actual bar/panel returned); native setup remained needs_dns_setup/present.
This is an installed helper gate, not a second full updater invocation. Earlier
guided updater evidence plus deterministic direct-entry/cleanup/consent tests
cover its call placement. Enter closed the original terminal; no pending
authorization remained.

For existing-public-0.9.8 coverage, both genuine fix.3 x86_64 packages were freshly
downloaded anonymously over HTTPS and matched the published archive hashes
`bf27584874c7e75d40f856739783f66fdf6af5652e37ba638232a2bc5a3c7750` /
`3309fb54ab92c5f1f30f6ba3c925ddc153619841dc4ae5640f4f279383f6b464`.
They were installed together with ordinary guarded pacman on the proven
inactive/empty fixture; receipts identify producer8d02f9d7. The known synthetic
template was administratively set to the ordinary bundled Rule default;
the prior Global fixture remains preserved. This is deliberately not a claim
that a user's custom/Global policy can be silently reset to make old packages
work. Profiles, Desired and ownership were unchanged.

The actual c9-loaded panel then offered Continue DNS setup, not a reinstall
dead end. FIRST, separate DNS/sudo, original terminal completion/close, explicit
prompts-closed acknowledgement and separate Start all completed. Final native
observation was observed/Disconnected/manualRecoveryRequired=false, Desired
false, runtime active but disabled, broker FD0 and no managed TUN. Profile,
Desired and ownership bytes matched the baseline; the managed template backup
matched the exact synthetic Rule original. The old package's wizard still
reported its old stock-core discovery; this is not relabelled as the new
native inventory correction passing.

The new exact producerb656 x86_64 pair was subsequently installed together after
a separately recorded administrative clean stop on that observed disconnected
fixture. Broker PID0/FD0 and empty private directory were independently checked;
only its inspected root-owned, single-link fixed stale socket was removed before
the unchanged package guard and normal pacman transaction. Enrollment and user
selection were preserved, not revoked or manufactured. This administration is
not a product update/rollback receipt. Actual Start existing DNS broker, terminal
completion/close, prompts-closed acknowledgement and separate Start passed.
Final ready/present, observed/Disconnected/recoveryfalse, Desiredfalse,
runtime active/disabled, no TUN; canonical profile/Desired/ownership/template
backup unchanged. The actual wizard reports the selected bundled core1.19.31
and capabilities present, matching direct version/capability reads.

The pin-only frontend delta passed 63 marketplace-setup and 10 updater-frontend
tests, manifest validation, shell syntax and diff checks. Its full CI remains
separate from the already green producer workflow. ARM64 archives/builds are
verified; **installed ARM64 acceptance is still pending**. Public fix.4 download,
clean public provisioning and main promotion are separate remaining gates until
their actual receipts are recorded. No new VPN/network-health, reporter-cause,
whole issue772-resolution or marketplace-approval claim follows. The physical
PC was not modified during this candidate pass.

## Immutable fix.4 public downloads and empty x86_64 setup

Validation prerelease `v0.9.8-fix.4` was published at frontend/tag
`791ac5454f9de3d7e7958c8ea7821e7e676716f2`, with the four producerb656
archives above, common frontend, prepublication delivery JSON and SHA256SUMS.
All seven assets were downloaded again anonymously through their fixed GitHub
HTTPS URLs and matched local inspected bytes. Frontend SHA256:
`6f442000945497b93225a33c5c422adfc92ee8bf7793a989ccd26a20e12091dd`;
delivery JSON `685a2ab84c1ffad047cf2ed169b8e88d82150dd4b6264c75c35ef495e32dbb33`;
SHA256SUMS `22d41aa8c1fd7ab674759a824c9779661ff2a6d0bd64c44a32acd8c48e5344ac`.
The immutable JSON is the prepublication record: its false download/ARM-installed
fields are retained, not edited into a subsequent result. This paragraph and
PR #778 record the independently observed download outcome. Latest/stable
remains fix.3; main, old tags/assets and Marketplace were not changed.

After proven clean guest poweroff/QEMU exit with no disk writer, its previous
installed overlay and matching NVRAM were retained. A new overlay of the
original read-only clean baseline booted normally with encrypted unlock. No
snapshot/reset is attributed to product recovery. Sole-operator custody was
retained; the existing host VM rule kept its window on workspace2, without
fullscreen/focus takeover or a host rule change.

New x86_64 boot `7c81b9a9-17d1-40b4-942a-fd3d24a821a0` had no application,
DNS or stock-Mihomo package, executable, private config/state, plugin or package
cache. The ordinary `omarchy plugin add` cloned the public repository without
enabling it. The candidate tag was deliberately selected and verified before
the separate enable; this is exact-candidate provisioning, not a claim that
main/Marketplace already points to it.

Actual Install required components, INSTALL, public production downloads,
normal guest sudo and pacman installed both pinned packages. No package cache
was preloaded or a download function replaced. Deliberate Enter cancellation
at the separate DNS consent produced `dns-consent` and the truthful partial
completion message, with legacy target, stopped broker/PID0/FD0 and no TUN.
The original terminal then closed; no pending sudo/pacman/terminal or setup lock
remained. Actual Continue DNS/FIRST, separate DNS/sudo, first normal activation
and setup completion passed without another package transaction. Final ownership
was Rust, components ready/present, native observation observed/Disconnected/
manualRecoveryRequired=false, Desired false, broker FD0, no TUN and empty
profiles. The user runtime was enabled by ordinary first activation; login
VPN autoconnect stayed Off. This is distinct from the preserved disabled-runtime
existing-user gates above.

The actual wizard showed bundled core1.19.31 and capabilities present; optional
file-picker absence remained visible and was not silently installed. Skip for
now/Finish later completed onboarding with the original routing default. The
settled actual panel displayed Disconnected and empty profiles, not the transient
post-mutation unverified projection. Installed app/core/broker hashes exactly
matched the inspected x86_64 delivery record. No VPN connection was attempted.

Final791 normal runner passed 753 Python tests/two declared opt-in skips plus
JS/QML. Published/tag binaries are still producerb656; later evidence-only
changes neither replace them nor move the tag. Required remaining promotion
checks are installed ARM64 and final candidate CI. Their absence is not a
request to repeat already completed x86_64 provisioning or physical-PC setup.
Issue772 stays open pending the reporter's diagnostics; no new reply or claim
of its exact root cause was posted.
