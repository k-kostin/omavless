# Managed DNS release candidate: isolated x86_64 removal/reinstall — 2026-09-28

Scope: agent-run, disposable Omarchy VM using the same reviewed
`0.9.0rc1-1` x86_64 app/production DNS packages identified in the
[fresh-install diagnostic](DNS_RELEASE_VM_FRESH_SETUP_2026-09-28.md). This is
not owner-attended acceptance or a published installer. The initial removal
sequence did not test an active VPN lease. No private profile, URL, log or
screenshot is included. The
physical PC and its VPN were untouched.

Steps 1–6 used the initial tested pair. Steps 7–8 used the subsequently
installed production-name candidate built from runtime source
`b739ac6a279981ffde3586d5e70e44a6be43ba70` in the same disposable VM;
they do not retroactively validate the initial pair.

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
6. The exact frontend from source `2f2e86234c5bbf2d5407fcdc166e094883c2ebc9`
   was installed in the separate graphical Omarchy VM. A temporary local
   helper returned `needs_reenrollment/present` to render the state without
   stopping its broker or changing networking. At 1890×2080, the English
   explanation and full-width “Restore DNS enrollment” action were readable,
   with no overlap in the setup card or profile list. The original helper was
   restored and its SHA-256 verified; actual components returned `ready`.
   The user runtime stayed inactive and TUN absent. This verifies only the
   visual state, not a second live re-enrollment or Russian rendering.
7. In a later agent-run check of the same isolated x86_64 VM, a normal
   connection reached `connected`, the managed TUN existed, and the broker
   held one descriptor. The installed production-name `package-guard` was
   invoked directly as root, without starting a pacman transaction. It
   refused with exit 1 and performed no cleanup. A normal Disconnect then
   reached `disconnected`, removed the TUN and returned the broker FD store
   to zero. This verifies the direct read-only guard refusal for one active
   lease, not ALPM replacement/removal under every active or unknown state.
8. A separate connect/Disconnect cycle compared in-memory `resolvectl dns`,
   `resolvectl domain` and `resolvectl default-route` readbacks before,
   during and after the managed lease. All three differed while connected
   and matched their own pre-connect bytes exactly after Disconnect. The
   lease again returned to FD store zero with no TUN. No DNS address or
   private configuration was printed or retained in this evidence.

Limits: active-lease ALPM transaction refusal, unknown/quarantined lease
refusal, wider DNS behavior beyond these resolved readbacks, another
architecture, Russian GUI rendering and owner-attended recovery remain
separate. Default-deny firewall behavior is recorded in the separate
[network diagnostic](DNS_RELEASE_VM_NETWORK_2026-09-28.md). The initial
tested guard used the old word “Experimental” in its refusal text; the
later installed guard had neutral wording. They are distinct exact package
builds, not interchangeable evidence.
