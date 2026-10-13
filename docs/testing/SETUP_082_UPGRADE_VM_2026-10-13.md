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
