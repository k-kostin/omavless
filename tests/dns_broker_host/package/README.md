# Local experimental package fixture

This is **not** the normal OmaVLESS package or release pipeline. No production
installer, CI publication, marketplace flow, or default runtime selects it.
It stages already-reviewed **local** binaries without downloading, compiling,
installing, enabling, enrolling or connecting anything.

Package: `omavless-dns-experimental`, version `0.9.0rc1-1`, one explicit
`aarch64` or `x86_64` architecture. The source receipt records an exact repository
revision and SHA-256 for both binaries, the reviewed unit and lifecycle guards.
Review the retained Mihomo/sing-tun patches and their pinned upstream sources
before supplying a binary; matching a user-supplied hash alone is not a code
review or signature/authenticity guarantee.

## Stage and inspect (ordinary user)

Invoke `python3 tests/dns_broker_host/package/stage.py` with:

- `--broker`: reviewed local `omavless-dns-broker` ELF64 binary;
- `--core`: reviewed local patched Mihomo ELF64 binary;
- `--broker-sha` and `--core-sha`: independently recorded lowercase SHA-256 pins;
- `--revision`: exact 40-character repository source commit;
- `--arch`: `aarch64` or `x86_64`, checked against both ELF machine headers;
- `--output`: a **new**, outside-Git build directory whose parent already exists.

The tool refuses symlinks, nonregular/unowned/hardlinked or oversized inputs,
incorrect hashes and wrong architectures. It does not print source paths or
binary contents. All `PKGBUILD` sources are fixed local filenames with exact
checksums; no mutable URL, `SKIP` checksum or network fetch exists. An incomplete
staging directory is left for inspection, never automatically recursively erased.

After source review, an ordinary-user `makepkg` invocation can build the staged
recipe without installing it. Inspect the archive's file list, `.PKGINFO`,
`.INSTALL`, unit, hook and hashes before any separate owner-approved `pacman -U`
in a visible terminal. Tests in this directory do not run makepkg or pacman.

## Installed effects — explicit experimental installation only

The archive installs only these fixed destinations:

- `/usr/lib/omavless/omavless-dns-broker`;
- `/usr/lib/omavless-dns-experimental/mihomo`;
- `/usr/lib/omavless-dns-experimental/package-guard`;
- `/usr/lib/systemd/system/omavless-dns-broker.service`;
- `/usr/share/libalpm/hooks/omavless-dns-experimental.hook`;
- `/usr/share/omavless-dns-experimental/reviewed-inputs.json`.

The package scriptlet sets
`cap_net_bind_service,cap_net_admin,cap_net_raw=ep` **only** on that candidate
Mihomo path. It does not replace `/usr/bin/mihomo`, the installed application,
its config or frontend. No enrollment file is shipped and no service is enabled
or started. Arch's ordinary systemd package hook may reload unit metadata; that
is not activation. Verify actual capabilities before accepting enrollment:
post-install scriptlet failure cannot be treated as an atomic installation abort.
Do not grant the broker executable file capabilities; its root service's fixed
capability bounding set is a different boundary.

## Replacement/removal gate

The fixed read-only guard requires root, zero arguments, protected runtime
directories, no private journal/staging/**any unknown entry**, and a loaded unit
that systemd confirms inactive/dead with MainPID=0 and NFileDescriptorStore=0.
Missing properties, bus failure, timeout, failed/active service and unknown state
refuse. It never emits observed property values or private content.

The actual abort mechanism is an ALPM **PreTransaction** hook with `AbortOnFail`
for this exact package's Upgrade/Remove. `.install` pre_upgrade/pre_remove call
the same guard as defense in depth; their failure alone is not a reliable
transaction-abort mechanism. This follows the [official ALPM hook contract](https://man.archlinux.org/man/alpm-hooks.5).

The guard performs no stop/start, DNS reset, FD-store removal, journal unlink,
enrollment change or forced recovery. The unit has no automatic start/restart
path, so an ordinary user cannot race an inactive service into acquisition while
the package is replaced. Root can override package hooks; root/CAP administrators
are outside this ordinary-user boundary. Administrative force flags/disabled hooks
must not be used as a recovery procedure.

Clean package removal still leaves all runtime/journal/enrollment data untouched.
A leftover socket is not proof of a live lease and is not removed by this package;
the broker refuses to adopt unknown socket state. Resolve it through the explicit
attended recovery procedure, not a blanket cleanup hook. Unknown retained writes
continue to require the safe epoch boundary described in the [host contract](../README.md).

## Remaining installed gates

Source/package tests establish fixed paths and abort policy, not real ALPM
acceptance. Before calling the experimental path usable, prove in the VM:
actual install and capability readback; no unsolicited startup; loaded unit and
ACL admission; successful fixed DNS lease; refusal of removal/upgrade with an
active or quarantined lease; clean release then normal removal; no change to the
stock core/application; and explicit recovery without disposing of unknown state.
The review-only package does not close #270 by itself.
