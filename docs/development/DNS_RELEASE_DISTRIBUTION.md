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
2. Recheck the installed binary hashes, capabilities and unit metadata.
   Initialize only an absent private store and validate compatibility before
   changing ownership. Show the numeric desktop UID and broker's fixed effects before
   a **separate**, attended administrator enrollment. No NOPASSWD sudoers or
   account-wide polkit grant is installed.
3. With no VPN lease and an empty broker state, perform the exact fixed-target
   enrollment for that UID, then explicitly enable/start the broker. A timeout
   or unknown answer stops the sequence; inspect before retrying.
4. With the user runtime stopped and Disconnected, prepare only the exact
   bundled default route template with its private backup and select the
   installed managed pair **before the Rust cutover**. The candidate runtime
   resolves its bundled core through that selection; activating first can
   start and then roll back. Custom YAML must refuse automatic rewriting.
   Only then perform disconnected ownership cutover and enable the user
   runtime; login/startup remains Off until the owner opts in.
5. Confirm the fixed package/selector, actual broker readiness and one real
   DNS/TUN/HTTPS cycle. A local selection flag alone proves neither enrollment
   nor Internet connectivity.

The stacked first-use draft changes `plugin/setup-runtime.sh` to require two
exact, same-source package pins, inspect both local archive identities, install
them in one normal `pacman` transaction and request separate DNS enrollment
before template preparation/selection and disconnected Rust cutover. It
removes the stock-Mihomo AUR offer. The automatic package-install path also
requires an absent application and DNS package registration, absent user
runtime unit and absent `Meta` TUN, rechecked immediately before pacman.
A missing executable alone is not fresh-install evidence; ambiguous or damaged
ownership is manual attention, not an implicit repair/upgrade.
The RC frontend now has exact hashes for both architecture packages in
`runtime-release.json` and `dns-release.json`. These are validation-only
prerelease pins; they do not prove that the corresponding GitHub assets are
published or that a clean download-to-activation path passed. An
already installed but incomplete app receives bounded setup guidance; a
connected native owner retains explicit Disconnect even when setup discovery
needs attention. The isolated x86_64
[fresh-setup diagnostic](../testing/DNS_RELEASE_VM_FRESH_SETUP_2026-09-28.md)
used local offline asset transfer in place of unpublished GitHub downloads.
It is not a marketplace install or formal owner-attended acceptance. The
offline frontend triple assembler now checks two reviewed archives and the
committed frontend against the common package source. Immutable asset delivery
and clean guided acceptance are separate gates;
an offline caller-supplied hash is not release authenticity.

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
After a clean package removal/reinstall which preserves Rust ownership and the
selected pair but revokes the broker enrollment, the frontend offers a distinct
`Restore DNS enrollment` action. It requires a stopped user runtime, absent
TUN/socket and a stopped zero-FD broker, then separately asks for DNS consent.
The root broker's fixed empty-state guard refuses residual enrollment,
journal/lease or unknown state. Success enrolls the current UID and starts the
broker without selecting a different pair, enabling the user runtime or
connecting a VPN. A generic broker failure keeps manual recovery guidance.
The same unprivileged stopped-broker observations also occur after a clean
package update that preserves enrollment; they cannot prove revocation. The
frontend therefore separately offers `Start existing DNS broker`. That action
checks fixed root-owned enrollment metadata before a start attempt and never
enrolls, enables the user runtime or connects. An absent or unsafe enrollment
refuses before starting the service; the owner may then select the distinct
re-enrollment action only after confirming clean removal/revocation.

The PC VM has default-deny UFW and requires an inbound `Meta` allowance for
its tested IPv4 TUN flow. A rule restricted to the configured peer as the
source did **not** restore traffic: returning packets have remote source
addresses. A temporary rule restricted to input on `Meta` and destination
`198.18.0.1` restored the built-in HTTPS check, IP-based TUN HTTPS and
DNS-name HTTPS with a reachable subscription server. It was removed after
testing. Normal setup must present an explicit, reversible administrator
firewall prerequisite and a read-only diagnostic; it must not silently install
a broad interface rule or infer all-host IPv6/UDP behavior from the VM's IPv4
HTTPS result. `Connected` means confirmed owner/core/TUN/DNS facts, not proof
that every destination works. See the sanitized
[VM network diagnostic](../testing/DNS_RELEASE_VM_NETWORK_2026-09-28.md).

## Promotion gates

The attended installed-owner DNS cycle tool now accepts an explicit
`--pair release` with `--release-core-sha` and `--release-broker-sha` from the
separately inspected package; the historical default and its
`--experimental-*-sha` pins still address only the experimental package.
It also checks the fixed production broker unit bytes. Run it only in an
isolated, already installed VM with a screened private profile and a human at
each authorization barrier. It does not install packages, enroll an account,
change the firewall, test package removal or recover an uncertain state.
Its passing HTTPS/DNS/mode cycle cannot substitute for the other promotion
gates below or authenticate caller-supplied digests.

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
