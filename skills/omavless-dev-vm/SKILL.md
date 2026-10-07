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

## Before using the VM

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

Before installing an experimental bundle, compare the whole app, core, broker,
receipt/schema, enrollment policy and fixture pins with the selected scenario.
The same architecture, core version or informal qualification label does not
prove compatibility. Inspect the app's actual receipt/ABI admission and retain
the complete matched packages; never swap one component or change frozen test
pins to make an unrelated bundle fit. Use separate known-Off images when the
selected scenarios require different bundles.

For a fresh experimental administrative setup, also compare its generated
enrollment bytes and initial directories with the actual runtime consumers.
Matching payload hashes does not validate generated configuration: a canonical
JSON reader may reject a trailing newline. Check which component creates each
required directory; a systemd `RuntimeDirectory` does not create persistent
state. Provision only the explicitly selected initially absent empty directory,
with its exact owner/mode, never synthetic protection records. Run an inert
canonical-byte check and establish actual empty-state admission before Arm.

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

Retain the effective scale, window rectangle, font and terminal size with visual
evidence. Matching coarse character dimensions does not prove identical glyph
rasterization; a fresh setup can require new independently reviewed references.
Measure the real observe/capture/input path with no product keys first. Separate
no-key diagnostic time from the native confirmation deadline, and never extend
that deadline or weaken image checks to make a slow operator pass. A failed
driver's keys-sent marker is not a native outcome; preserve its original result.

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
before trying extra display devices or reset. When permitted, identify the
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
