# Managed DNS pair: release-distribution contract (proposal)

Status: proposed for 0.9 review, **not approved for normal installation**.
Draft #302 proves a combined app/UI source and an installed x86_64 VM
diagnostic; Draft #303 builds experimental pair packages on both native
architectures. A later stacked draft stages a separate `omavless-dns` package
candidate and a release-only runtime selector. Neither that uninstalled
candidate nor the experimental manual enrollment is a public release path.
The broker's security and host
contracts remain in [DNS broker composition](../../tests/dns_broker_host/README.md)
and [authorization research](DNS_AUTHORIZATION_RESEARCH.md).

## Deliverable and identity

Prefer two version-matched, separately inspected Arch packages:

- `omavless`: unprivileged Rust runtime, TUI and user units;
- `omavless-dns`: reviewed Mihomo adapter, fixed-purpose root broker, one
  system unit, lifecycle guard, licenses and complete corresponding source.

The application must depend on the companion package for supported 0.9
connections; a stock `mihomo` binary or a package merely providing the name
must not satisfy the managed-pair check. The ordinary AUR Mihomo setup remains
historical 0.8.x behavior, not a hidden substitute. The production companion
must use its own fixed package/core/receipt paths instead of presenting
`omavless-dns-experimental` as a release. Its package scriptlet may grant only
the reviewed core file capabilities; the broker receives no file capability.
Installation does not enroll an account, enable/start either service, open a
firewall, connect a profile or edit private routing settings.

For each architecture, record one exact Git source commit and the SHA-256 of
both packages, the frontend archive, both executables, corresponding source,
upstream commits, patches and licenses. Independently inspect the Arch package
metadata, ELF architecture, file list, scriptlet, ALPM hook and receipt. The
release assembler must refuse mismatched architecture, product version,
source identity, pinned upstream or any unreviewed path. A downloaded CI
artifact is build evidence, not an authenticated release asset or installed
acceptance. Final frontend bootstrap pins come from immutable published asset
hashes, never a mutable tag, latest manifest or a self-reported receipt.

## Explicit first-use sequence

The panel must distinguish a missing companion, missing explicit enrollment,
invalid user selection and a running/held/unknown broker; none is `ready` merely
because `/usr/bin/omavless` exists. It must provide bounded guidance without
printing profile IDs, DNS values, provider URLs or privileged-command output.

1. From observed Disconnected with startup Off and settled authorizations,
   obtain both exact pinned packages over HTTPS, verify both hashes and package
   identities, then invoke normal `pacman` without `--nodeps`, blanket
   `--overwrite`, `--noconfirm` or background sudo. Preserve existing profiles.
2. Recheck the installed binary hashes, capabilities and unit metadata. Show
   the actual numeric desktop UID and the broker's fixed paths/effects before
   a **separate**, attended administrator enrollment. No NOPASSWD sudoers or
   account-wide polkit grant is installed.
3. With no VPN lease and an empty broker state, perform the exact fixed-target
   enrollment for that UID, then explicitly enable/start the broker. A timeout
   or unknown answer stops the sequence; inspect before retrying.
4. With the user runtime stopped and Disconnected, prepare only the exact
   bundled default route template with its private backup and select the
   installed managed pair. Custom YAML must refuse automatic rewriting. Start
   the user runtime afterward; login/startup remains Off until the owner opts in.
5. Confirm the fixed package/selector, actual broker readiness and one real
   DNS/TUN/HTTPS cycle. A local selection flag alone proves neither enrollment
   nor Internet connectivity.

The stacked first-use draft changes `plugin/setup-runtime.sh` to require two
exact, same-source package pins, inspect both local archive identities, install
them in one normal `pacman` transaction and request separate DNS enrollment
before template preparation/selection. It removes the stock-Mihomo AUR offer.
The current `runtime-release.json` and `dns-release.json` intentionally have
**empty** package maps, so the public download path remains unavailable. An
already installed but incomplete app receives bounded setup guidance; a
connected native owner retains explicit Disconnect even when setup discovery
needs attention. This is source/test work, not a claimed installed first-use
acceptance. The frontend release assembler still needs authenticated two-asset
pairing and immutable published pins before metadata can be populated.

## Replacement, removal and firewall

The companion's fail-closed ALPM PreTransaction guard must remain effective
for both Upgrade and Remove. An active, quarantined or unknown lease, live
broker, preserved socket, journal entry or FD store refuses replacement.
Never auto-stop a connection, unlink the journal/socket, call `systemctl clean`
or force a transaction to make the upgrade proceed. After an independently
confirmed clean Disconnect, stop the broker and prove original DNS readback,
no TUN/listener, empty private journal and FD store zero. An administrator may
then resolve only an inspected stale socket before rechecking the guard.

For uninstall, explicitly revoke the old enrolled UID while the reviewed
broker binary still exists and the state is proven empty; disable the system
unit before removing its package. No old experimental enrollment or user
selection may silently authorize a future production reinstall. A migration
from the experimental path needs a named, tested clean-state procedure and
fresh opt-in. Removal of the Omarchy frontend alone must not be mistaken for
removal of a root broker, user service, package or private profiles.

The PC VM has default-deny UFW and requires an inbound `Meta` allowance for
its tested IPv4 TUN flow. That temporary, peer/address-scoped diagnostic rule
was removed after each test. Normal setup must present an explicit, reversible
administrator firewall prerequisite and a read-only diagnostic; it must not
silently install a broad interface rule or infer all-host IPv6/UDP behavior
from the VM's IPv4 HTTPS result. `Connected` means confirmed owner/core/TUN/DNS
facts, not proof that every destination works.

## Promotion gates

- Both architecture packages and one common frontend built from a reconciled
  exact source, with immutable asset pins and a complete source/license receipt.
- Fresh VM installation and existing-0.8.x migration, including refusal and
  recovery after every package/enrollment/template/service step.
- Active/unknown/quarantined removal and upgrade refusal; clean removal and
  explicit revocation without residual host DNS or unattended restart.
- One real working-server DNS/TUN/HTTPS/mode/server-change/Disconnect cycle,
  including a default-deny-firewall host and negative authorization cases.
- Formal owner-attended acceptance under the host procedure, separate main,
  release and marketplace authorizations. Agent-run VM diagnostics and CI are
  not substitutes for those gates.

This contract records the next implementation target without declaring #270,
#132, normal distribution or 0.9 RC release readiness complete.
