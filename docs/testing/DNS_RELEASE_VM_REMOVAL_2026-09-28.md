# Managed DNS release candidate: isolated x86_64 removal/reinstall — 2026-09-28

Scope: agent-run, disposable Omarchy VM using the same reviewed
`0.9.0rc1-1` x86_64 app/production DNS packages identified in the
[fresh-install diagnostic](DNS_RELEASE_VM_FRESH_SETUP_2026-09-28.md). This is
not owner-attended acceptance, a published installer or a test of an active
VPN lease. No private profile, URL, log or screenshot is included. The
physical PC and its VPN were untouched.

1. With the broker active but no TUN, ordinary `pacman -Rns` for the two
   packages reached the PreTransaction guard and refused. The transaction
   removed neither package; the broker remained active.
2. The user runtime was disabled/stopped. The broker was disabled/stopped and
   independently observed inactive/dead, PID zero and FD store zero, with an
   empty private journal, no core, TUN or listener. Only the exact remaining
   root-owned inactive socket node was removed. The reviewed broker's fixed
   `--revoke` then succeeded, and its package guard returned success.
3. The same ordinary two-package removal then succeeded. Both package units,
   the broker socket and enrollment were absent; the initial private profile
   store remained present. No purge or network operation was performed.
4. The same pair was reinstalled in one normal pacman transaction. The Rust
   target and selected pair survived in private data, while the broker and
   user runtime were inactive with no TUN. The older setup logic showed only
   `needs_broker`, offering no safe guided continuation.
5. The corrected frontend reported `needs_reenrollment`. Its separate
   `restore-enrollment` action accepted `DNS` consent and ordinary VM sudo,
   then registered only the current numeric UID and started the broker. The
   final facts were `ready/present`, Rust target, selected pair true, broker
   active/FD store zero, private store preserved, user runtime **still
   inactive**, and TUN absent. No package, profile, pair or startup mutation
   was repeated. Synthetic tests also cover rejection of wrong target, stale
   socket, non-stopped broker, cancelled consent and failed enrollment.

Limits: active/unknown/quarantined lease refusal, actual DNS restoration,
default-deny firewall behavior, another architecture, exact installed GUI
rendering and owner-attended recovery remain separate. The currently built
production guard still uses the old word “Experimental” in its refusal text;
the refusal itself is correct, but that wording should be reconciled before a
user-facing release. A message-only source change requires a new matching
package build and does not retroactively change this test's artifact identity.
