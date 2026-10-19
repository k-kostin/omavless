---
name: omavless-dev-vm
description: Operate an owner-authorized OmaVLESS development VM, especially encrypted reboot/unlock and sequential VM handoff. Not for physical-host VPN changes or source-only fixtures.
---

# Development VM operations

Read the repository's [execution policy](../../docs/development/EXECUTION_POLICY.md)
and [acceptance environments](../../docs/roadmap/ACCEPTANCE_ENVIRONMENTS.md).
This skill conveys a procedure, not additional permissions. Agent-entered
passwords or acknowledgements count only within the owner's authorized VM
scope; never label them human-attended acceptance.

For interactive guest SSH, allocate a terminal in the command tool as well as
requesting a remote PTY (`tty: true` and `ssh -tt`, where supported). A remote
PTY alone can leave sudo displaying a password prompt while the tool has closed
local stdin. Do not start another authorization in that state: inspect the
original operation and settle it under the execution policy before a new attempt.
Non-interactive reads do not require a PTY.

For chained host phases, check the original result schema and required state
before opening the next authorization or effect. Missing desktop environment,
refused Quit or a null JSON projection is not an admitted stopped baseline.
Record fixture administration separately from the product journey; it cannot
earn the user's preparation step. Use the execution policy's driver admission
rule rather than a compound command's final exit code.

Send one command at a time in an interactive administrative terminal, then
observe its outcome before sending the next. Sudo can flush already queued
terminal input: echoed later lines are not evidence that those commands ran.
Keep the previous command's exit status separate from the next action, rather
than treating a returned prompt or batched input as a successful sequence.

For a fixture in another UID's private home, observe file absence as that UID
or an already authorized administrator. A failed `test -e` without parent
traversal permission is not absence proof. Distinguish `ENOENT` from permission
errors and existing symlinks before create-only preparation; do not infer an
empty destination from the desktop user's inability to inspect it.

## Before using the VM

Apply the canonical [VM session lifetime policy](../../docs/development/EXECUTION_POLICY.md#vm-session-lifetime)
when choosing a future launcher. Per-command bounds are not a blanket VM kill
timer; changed policy never reactivates a sealed fixture or amends an already
admitted launch contract.

Identify the designated sole operator, exact VM directory/image, current QEMU
process, QMP socket and SSH endpoint from the local VM application or its
maintained configuration. Check active VM ownership with the orchestrator;
source-only agents must not start, stop or control it simultaneously. Do not
infer disk identity from an SSH port alone, and do not boot a copied disk while
its original is running. Keep machine-specific identities outside Git.

Prefer the existing Omarchy Dev VM application and reviewed local launch,
guest/SSH and unlock helpers. Inspect a helper once and retain its hash and
supported target; re-review changed selection or behavior, not unchanged code
on every reboot. Do not replace it with an improvised host-wide automation.

At a new installed baseline, discover the actual package paths, service names,
CLI help and compositor version before using remembered commands. Cache those
facts for that baseline. In particular, an interface change can make an old
window dispatcher return textual errors despite a zero shell exit; confirm the
requested window geometry or service state, not merely command completion.
For a unit observation, establish its actual installed name and `LoadState=loaded`;
an absent recalled name can otherwise appear inactive/dead without describing
the running service.

For a freshly started `Type=simple` native unit, successful `systemctl start`
does not prove its IPC socket is ready. Before the next configuration mutation,
use bounded read-only native status/hello observations to establish the expected
owned state; do not substitute a fixed sleep or restart loop. A proven absent
socket before connect/send is a not-submitted refusal: retain that original
result and admit a separate command only after readiness is actually observed.
Lost replies or uncertainty after submission still follow the original
operation's recovery policy; later readiness is not permission to resend.

After a frontend update, matching files are not proof that the long-lived shell
loaded the new QML/JS. If the native facts are correct but controls remain old,
use the maintained [frontend reload guidance](../../docs/user/NATIVE_INSTALL.md#verify-the-installation)
before patching the button or private data again. A separately authorized guest
shell restart is not native recovery; verify fresh actual controls and preserve
the original failed gate. Do not automate a physical-host shell restart.

Before installing an experimental bundle, compare the whole app, core, broker,
receipt/schema, enrollment policy and fixture pins with the selected scenario.
The same architecture, core version or informal qualification label does not
prove compatibility. Inspect the app's actual receipt/ABI admission and retain
the complete matched packages; never swap one component or change frozen test
pins to make an unrelated bundle fit. Use separate known-Off images when the
selected scenarios require different bundles.

For setup or compatibility acceptance, use the workflow's
[installation user-path matrix](../../docs/roadmap/DEVELOPMENT_WORKFLOW.md#installation-user-path-acceptance).
Prepare the actual previous public stable installation, preserve its data, then
update only the plugin through the normal Omarchy path. Do not preinstall the
candidate application/companion before checking the user's offered continuation:
that can hide the very upgrade gap being tested. Keep clean-first-use and
partial-setup cases separate. A refused setup is not an accepted update flow.

For a fresh experimental administrative setup, also compare its generated
enrollment bytes and initial directories with the actual runtime consumers.
Matching payload hashes does not validate generated configuration: a canonical
JSON reader may reject a trailing newline. Check which component creates each
required directory; a systemd `RuntimeDirectory` does not create persistent
state. Provision only the explicitly selected initially absent empty directory,
with its exact owner/mode, never synthetic protection records. Run an inert
canonical-byte check and establish actual empty-state admission before Arm.

For a clean **reinstall** fixture, package/config absence is not sufficient:
inventory the selected account's runtime owner/socket/lock directory and login
receipt too. A consumed receipt from the previous ownership generation can
correctly block a new activation, even after packages and persistent state were
removed. Preserve that initial refusal; do not call the resumed installation an
uninterrupted fresh-install PASS. Only separately authorized, sole-operator VM
administration may archive precisely identified unused residuals after stopped
owner, safe-path and original-state checks. A lock acquisition alone does not
exclude a concurrent new start. Never turn this fixture procedure into a generic
user cleanup command, delete unknown receipts, or weaken create-only guards.

Once the whole initially absent baseline is admitted, complete a new continuous
GUI/terminal setup with no administrative interventions inside it. Keep cached
offline validation separate from an empty-cache public-download run. Explicit
candidate tag checkout before enabling is fixture selection, not proof of a
main/Marketplace install.

A fixture-admin removal is not the user's clean-reinstall gate. For a proposed
terminal guide, execute its actual copyable blocks after ordinary widget removal,
without relying on that deleted checkout or machine-specific recovery helpers.
Separate original-partial fixture preparation from the user path. The bounded
[0.9.8 candidate procedure](../../docs/user/CLEAN_REINSTALL_098.md) uses proven
clean state **before** an explicit guest reboot rather than manual volatile-file
deletion. The journal is also volatile: a reboot first would destroy evidence,
not prove cleanup. Record any separate clone/enable prompt and verify the selected
candidate before enabling; a command without `--enable` may still ask interactively.

## Encrypted reboot: successful repeatable sequence

1. Establish that the selected QEMU process belongs to the current user and
   its command line names the selected image and QMP socket. Confirm socket
   ownership and that no second VM operator is active.
2. Observe the **actual selected guest's** serial output or QMP screenshot.
   A black window alone does not prove a passphrase prompt. Wait for the
   unlock prompt, not an arbitrary fixed sleep.
3. Use the reviewed QMP keyboard helper for that target. Supply an authorized
   guest-only passphrase through non-echoing stdin; never argv, environment,
   Git, screenshots or a public log. The known helper negotiates QMP, sends
   each supported character with the correct key mapping, then Enter. Do not
   paste arbitrary shell content or expand its supported alphabet silently.
4. A keys-sent marker proves only input delivery. Confirm actual guest boot
   through its trusted SSH endpoint, capture the new boot ID, and verify the
   normal user/login receipt and scenario-specific service/package baseline
   before admitting the next product operation.

If the prompt, process, socket or boot outcome is uncertain, stop further
input and inspect bounded guest evidence. Do not repeatedly send a password,
disable disk encryption, remove guest authentication or reset a product's
private state to speed up startup. A separately authorized disposable-VM reset
is administrative recovery, never proof of product rollback.

## Handoff and retention

### Graphical preflight before a short confirmation window

Check the actual guest pixels, monitor scale/DPMS, focused window and terminal
rows/columns **before** requesting an expiring product confirmation. Quickshell
can own the lock layer: `hyprctl activewindow` may still name the obscured app,
and `loginctl` alone does not prove the rendered screen is unlocked. Observe the
password prompt and unlock normally; do not send app keys to a covered window.

For an authorized unattended development guest, inspect its installed
`omarchy-toggle-idle` command and record `status`; the normal `stay-awake` mode
prevents idle lock/screensaver during the test. Restore the prior mode afterwards.
This changes only the selected guest's ordinary idle preference, not encryption,
manual authentication or formal human-attended acceptance. If DPMS is off, use
the guest's installed brightness/display command and verify a new rendered frame.
Do not stop lock services or edit the physical host's idle configuration.

Interpret that command's actual status, not just its tooltip: an offered
"Allow Idle Lock & Screensaver" action can mean stay-awake is currently active.
Do not toggle an already correct preference based on the opposite action label.

Retain the effective scale, window rectangle, font and terminal size with visual
evidence. Matching coarse character dimensions does not prove identical glyph
rasterization; a fresh setup can require new independently reviewed references.
Measure the real observe/capture/input path with no product keys first. Separate
no-key diagnostic time from the native confirmation deadline, and never extend
that deadline or weaken image checks to make a slow operator pass. A failed
driver's keys-sent marker is not a native outcome; preserve its original result.

### Verified graphical targeting and geometry

Guest SSH does not necessarily inherit the graphical session environment.
Discover the actual guest compositor instance, runtime directory and Wayland
display before compositor queries or guest capture; do not copy another boot's
signature, socket or UID. Check the installed compositor version and its local
configuration/API before selecting dispatch syntax. Modern Hyprland Lua
dispatchers and legacy string dispatchers are not interchangeable; a textual
syntax error is a failed action even if the shell returned zero. Do not probe
guessed mutations repeatedly or edit global configuration to fit automation.

Bind each affected window to the test's actual PID, verified executable and
current compositor address before focus, float, resize, move or close. Window
addresses can be reused after an earlier client exits; a remembered address or
matching title alone is not identity. Act only on the owned test window, not
the VM viewer or unrelated host/guest windows.

After resize/move, read back the current window rectangle and monitor scale
before clicking or cropping. Floating resize can recenter the window, so old
coordinates can make a healthy layout look clipped. Convert compositor logical
coordinates to the selected capture/input surface using its actual origin,
dimensions and scale; do not assume they equal image pixels. Confirm the
rendered target after input and retain a fresh frame. Graceful close additionally
needs the original process completion and independent window/process absence;
an input-delivered marker or missing old address is not that proof.

For firewall or routing experiments, establish a private bidirectional serial
console and bounded private capture **before** changing guest networking. SSH
may be blocked by the protection being tested. Prefer a Unix-socket QEMU
chardev on the reviewed launcher, with restrictive permissions and no host
network exposure. Authenticate through the guest's normal console; do not
widen firewall rules merely to recover the test transport.

Terminal negotiation can inject cursor/size escape replies into a freshly
attached console's login input. Wait for a clean login prompt, enter the known
guest username, and observe its password prompt before supplying a secret.
If the username is contaminated or unknown, cancel that login or let the
normal getty reset; never send a password to an unidentified prompt. A raw,
non-echoing local console client avoids local echo and forwards guest control
keys, but a password still belongs only at the verified guest prompt.

If a running reviewed VM has only a file serial sink, QMP `chardev-change` can
replace that transport after exact process/image/socket selection and capture
preservation. Read back the selected chardev and permissions. Its
`frontend-open` may remain false until the console client connects: confirm
the actual console and boot, rather than repeatedly issuing the change. This
administrative channel supplies observation, not product recovery or proof
that the interrupted network operation completed.

For a virgl/SDL guest, QMP `screendump` may return `no surface` even while
the visible VM works. Check the maintained VM application's capture guidance
before trying extra display devices or reset. Where authorized, prefer guest-side
capture through the discovered graphical environment (for example, the installed
`grim`) and copy the result into the test's private capture directory. Keep
captures below the owned HOME-backed scope, not loose files in HOME or /tmp.
If viewer-region capture is needed and permitted, identify the
exact QEMU window by PID/title and capture only its visible unobscured region;
exclude unrelated windows and host notification overlays. An independent guest
TTY can supply observation while a retained original serial foreground is
occupied. Observe its real login/password prompts before input; do not detach,
signal or replace the original channel to make room. QMP input completion is
delivery only: wait for the guest's rendered response before the next input.

Pass exact image/boot/source/artifact identities, current network/service
state, live sessions, failed scopes and the next executable gate. Release VM
ownership explicitly before another agent performs visual or live checks.
Keep original failed results and required private captures; clean only known
completed disposable build outputs under the retention policy.

For ordinary source fixtures, use their existing dedicated workflow skill
when available; do not consume this VM merely to run compiler or pure tests.
