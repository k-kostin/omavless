# Opt-in private Backup client

Status: bounded agent-attended installed Backup-only acceptance at exact source
`662ba08767993374d7101f2e7bdfd723d337898f`, based on API source
`4983f392d15c123e76e7b71182b45715cf812652`. SOURCE and installed evidence are
separate below. Default selection, whole T4 and release remain unapproved.
Draft #701 and its scope46 evidence remain unchanged. This is a client of the
[existing private-pair API](T4_PRIVATE_PAIR_NORMAL_API.md), not a new owner.

## Smallest vertical slice

User task: create an encrypted copy of the current supported private pair.
The payload is profiles/subscriptions and the finite supported routing template,
not all Settings, runtime state, host configuration or a support report.

Choose the Rust TUI Settings surface first; QML and Restore are excluded.
The implemented selector is `tui --developer-private-backup`, ordinary-user
only, never a secret argument. Compile the client only through the existing explicit
`t4-manager-actor-service` opt-in (with TUI), and require an explicit private
Backup client selection. Ordinary TUI invocation/default builds stay unchanged.
The Settings entry is usable only with current owned metadata and the existing
`backup.create` capability. Capability display is not admission or a reservation.
Opening the entry performs no filesystem or VPN mutation.

Use one bounded absolute destination text field (160 UTF-8 bytes in this first
70x24 client; the runtime's 4096-byte bound is unchanged) rather than a shell-based or
general-purpose file browser. The field is private local presentation, never
argv, environment, activity logs or clipboard. No directory enumeration,
destination creation/probe, shell expansion, generic IPC or URI import occurs
in the client. The existing Rust destination guards remain authoritative;
relative/unnormalized/unsafe paths are rejected, not silently repaired.

Two ephemeral masked passphrase fields must match. Owned fields and the
serialized secret frame are zeroizing; no Debug/Clone/serialization of an editor
or request and no plaintext render/reveal/copy command. JSON/library temporary
wiping and terminal-library/event-buffer wiping are not guaranteed. Before confirmation, explain that the archive contains
reusable credentials and that a lost passphrase cannot be recovered. Name the
chosen private destination and make exclusive/no-overwrite behavior explicit.
Only "Create encrypted backup" submits; navigation, editing and Escape do not.

## Closed client transition

| State | Permitted next action and factual result |
| --- | --- |
| Unavailable | Explain missing/current ownership or capability; no submit |
| Editing | Bounded destination and masked secret entry; Escape wipes and returns |
| Confirming | Show private-pair scope, destination, credential/loss warning; cancel wipes |
| Submitted | One original request; repeated Enter/editing/ordinary mutations cannot resend |
| Completed | Only strict original successful reply; no VPN/revision effect claimed beyond API |
| Known denied | Closed pre-effect allowlist only; explain no overwrite/unavailable/changed without remote text |
| Unknown | Preserve uncertainty; no new operation ID, refresh-and-resend, file cleanup or inferred success |

Capture the genuine current instance/revision before editing. Any intervening
metadata change invalidates confirmation rather than silently rebasing it.
Generate one operation ID at submission and move the owned request once into a
capacity-one worker. Do not reuse the ordinary cloneable action/job request for
secrets. Use only existing `backup.create`, its fixed schema/confirmation, normal
credential-checked transport and original 120-second deadline. The coordinated
Backup-only adapter consumes the Request; its immutable deadline is clock DATA,
not a lease or public caller-selected timeout. It reuses normal directory/socket
and peer checks and the normal codec, with one nonblocking connect and original
deadline checks before/after frame writes and reads. Unknown connection, partial
write, timeout and malformed reply never retry. These are sampled bounds, not
atomic kernel preemption, cancellation or effect rollback. Default transport
is unchanged. No daemon
start, lease constructor, receipt/history adoption or backend/protocol change.

Decode only the existing closed success fields and known pre-effect error
allowlist. Transport loss, backend manual recovery, malformed/unfamiliar result
and publication ambiguity remain UNKNOWN. Never render remote error details.
Closing the interface is not cancellation of a submitted backend operation.
Destroy secrets on cancellation/completion/denial/unknown; metadata retained for
correlation is not authority. The private zeroizing destination label may remain
visible until the editor is discarded, to identify an uncertain original file;
it is never a source/destination authority. No automatic replay is included.
Submitted/UNKNOWN survives leaving Settings and re-entry in the SAME TUI;
only read-only navigation/local appearance remains available. A missing worker
or result delivered after the original deadline becomes sticky UNKNOWN before
positive acceptance, regardless of event-loop delivery/tick ordering.

## SOURCE controls and rendering

Synthetic state controls must cover cancel-before-submit, wrong target/path,
masking/no raw secret formatting, mismatched/oversized secrets, stale instance/
revision, capability absence, repeated Enter, late/foreign response, exclusive
destination denial, every UNKNOWN branch and no hidden retry. Default entry and
wire absence controls must remain. All examples/captures use synthetic values.

The repository UI review and localization skills guided EN/RU implementation;
verify narrow and normal terminal sizes, selection/focus, secret masking and
destination/warning visibility independently from handlers. Eight public
synthetic TestBackend SVG/PNG views were personally inspected: EN/RU70x24
editing/confirmation/UNKNOWN and EN/RU100x32 confirmation. They show full target,
masking and warnings; they are not actual terminal/font/installed acceptance.
SOURCE gates: 165 feature TUI tests and 148 default TUI tests PASS; these include
11 state controls and six Settings/rendering controls. Three actual local socket
controls cover expired-before-write/no peer bytes, possible-write then UNKNOWN
without retry/cancel, and real normal transport denial without a current factory.
The short test clock is DATA, not live engine authority. Runtime all-target
strict Clippy passes default+opt-in, no-default+opt-in and default without opt-in.
Catalog328 bounded EN/RU keys and documentation navigation/diff/fmt are checked.
Initial compile-only TrySendError/visibility/style failures and the independent
late-delivery ordering finding remain recorded; no backend guard was weakened.
The first test command omitted explicit cache/jobs flags and completed in8.33s;
all subsequent checks explicitly use HOME cache/TMPDIR and jobs2.
An exact-head installed Backup gate must separately retain original
client outcomes and check no owner/profile/Desired/history change. It does not
accept Restore preview/confirmation, fresh-install portability, default product
activation, SLEEP/network/OS transfer or whole T4.

## Installed UI62, 2026-10-07

ROOT alone operated the disposable VM, normal package installation, genuine
current owner, actual Foot/TUI and private inputs/captures. Tested runtime is
only `662ba08767993374d7101f2e7bdfd723d337898f`; documentation successors are not
new runtime acceptance. The installed runtime ELF is 9,193,200 bytes, SHA256
`852a7d80dc7f73f64d2c868227ac58ce33b6bb50d6b8e8bd82535561543dac70`;
the whole app package is 3,400,186 bytes, SHA256
`c9afaf60c818d3d2aa4e4bc7f98f088cb606e2b7f91cdd1df6693e3f281e0865`.
Normal defaults plus the sole existing developer opt-in were selected, not
default Backup UI. The matching whole LegacyMeta2
bundle was independently verified unchanged, not a copied ownership receipt.

Original installed states: English and Russian at70x24, too-small
submission refusal and cancellation of confirmation passed. One English
original Create displayed Completed. A separate read-only domain-authentication
oracle, original `3b8838` exit0, authenticated that archive under the entered
private passphrase, proved exact pre-operation pair bytes, and refused the
wrong key. It did not invoke runtime/Restore or write a pair. Profile/template
bytes and metadata, audit history, native Off and revision remained unchanged.
No private values, paths, ciphertext, raw logs or images are included here.

Russian confirmation followed by one NEW existing-destination request `8fdf7b`
displayed native Denied (`215f00`). The fixed read-only observer `0474be`
original0 proved ciphertext hash and original archive metadata unchanged,
exact pair bytes/full metadata and audit hashes preserved, and the same native
owned Off result and revision. It did not overwrite the archive or resend the
English operation. A grouped Denied display is not an exact wire-code attestation;
the unchanged source's exclusive publisher and these original postconditions
bound the selected EEXIST scenario.

Explicit Ctrl+C `50be64` closed the TUI only; its wrapper retained the native
original exit0 (`913b11`). Normal Foot closure completed its GUI original
`65262` → `b37f53` exit0. No UI-close-caused daemon stop is claimed.
The first administrative stop `a68237` used the wrong service name and exited5;
it also stopped login preparation through the existing PartOf relationship.
This is not positive stop evidence and is neither erased nor attributed to UI.
Correct unit inventory `acddc1` preceded stopped-state original `c8aa65` exit0:
both canonical units inactive/dead, zero PIDs and success0, Meta absent, with
pair/archive/history preserved. A QMP powerdown ACK was not OS shutdown proof;
normal verified guest poweroff `ff62e9` / original `75257` → `1e35a5` exit0 and
original QEMU `18762` → `9ab3f0` exit0 completed separately. This administration
does not establish product rollback, recovery or fatal descriptor survival.

The known completed whole VM image and firmware state were sealed and retained
privately. No private values/paths or raw screenshots/journal are published.
The source author did not operate or independently resample the guest; ROOT's
original operations and separately inspected captures supply installed evidence.
The installed scope remains Backup-only, agent-attended and explicitly opt-in;
it is not Restore UX, default activation, human/physical-host acceptance,
whole-settings export, Internet/DNS health or whole T4. No VPN transition or
Restore was selected.
