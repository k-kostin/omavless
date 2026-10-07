# Opt-in private Backup client design

Status: feature-only SOURCE candidate for final review, based on API source
`4983f392d15c123e76e7b71182b45715cf812652`. Synthetic rendering is separate
from installed acceptance; no VM action, default selection or publication.
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

## Required gates before any installed selection

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
An eventual exact-head installed Backup gate must separately retain original
client outcomes and check no owner/profile/Desired/history change. It does not
accept Restore preview/confirmation, fresh-install portability, default product
activation, SLEEP/network/OS transfer or whole T4.
