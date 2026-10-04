# Fixed startup follow-up — source only

This separate generation preserves the complete `0b92f4a` capture source and
its measured inventory SHA
`8232be39e366bdc0540e42fe8988b90256a5c29851dd3fb2a4303eb065bc7361`.
Its independently validated three-member archive SHA is
`c245c86e55754f8155f04c94f9546156d122dd2182908551d9d21c4de8ac1b95`.
That evidence is a successful source capture, not manager-start acceptance.
Private archive paths and raw configuration do not belong in Git.

## Concrete remaining inputs

The captured static default-target closure has 21 nodes: five targets and
sixteen UNIX sockets. No captured static enabled edge reaches OmaVLESS or the
graphical/autostart targets. Installed `WantedBy` declarations were not counted
as enabled links. The D-Bus socket actually executes `systemctl --user
set-environment`; later real user-manager queries may activate the real bus.
The owner-authorized disposable fixture may include those standard socket/bus
effects after the remaining source closure is reviewed. No fake bus, disabled
generator, global mask or canonical UID1000 change is permitted.

Among captured ordinary system lookup paths, vendor `user@.service` and
`user-runtime-dir@.service` win; instance overrides are absent. The vendor
login-barrier and admin faster-shutdown drop-ins have distinct names and both
apply. The latter only changes stop timeout; it grants no cleanup authority.
But high-priority control/transient/generated/attached paths and dash-prefix
drop-ins were not in the first capture. They must be checked before calling
this the effective system-manager definition.

The real manager selects `PAMName=systemd-user`, absent from the captured
admin PAM directory. Vendor PAM configuration was not captured. Exactly two
captured vendor generators are ELF files: environment-d and XDG autostart.
Their hashes alone do not establish their semantics or provenance. There are
eleven captured desktop entries, including unrestricted ones; lack of an
intended GUI session is not a substitute for resolving their generated edges.

## Fixed follow-up source

`startup_followup.py` is a separately versioned copy of the retained-FD
inventory mechanism. The prior source is unchanged. Trusted stdin execution
requires isolated Python `-I -B`, root real/effective/saved IDs and only
`--capture-t4-user-startup-followup-v1`. The create-only output is a different
root-private `/run/ov-t4-user-startup-followup-v1`. No fallback or previous
stage reuse is possible.

The literal additional inputs are:

- Vendor PAM configuration and the two fixed PAM fallback configuration names.
- Exactly two instance/template unit families, their drop-ins, type-wide
  service drop-ins and the relevant dash-prefix drop-ins under eight named
  system-manager lookup roots. Only those selected children are visited;
  unrelated system units are not recursively captured.
- The two already captured generator ELFs, manager/runtime helper, systemctl
  and dbus-broker-launch, plus five fixed installed manual files. These are read
  as data; no generator, ELF, manager or external inspection tool is executed.
- Installed ALPM `desc`/`files` for six fixed package names: systemd,
  systemd-libs, pam, pambase, dbus-broker and dbus-broker-units. The package
  directory is boundedly enumerated; each exact package must have one numeric
  version directory and matching unique NAME/VERSION fields. No missing-package
  fallback or arbitrary discovered file traversal is allowed. The original
  directory FD and complete name set are retained/rechecked.

The six ELF paths must have exactly one owner in those retained package file
lists. The receipt binds captured file bytes/metadata to those local package
records, not to a freshly downloaded or cryptographically authenticated
package. It is not a package signature attestation. A missing or ambiguous
owner, malformed file list, missing ELF, metadata drift or unknown read is a
terminal refusal; it cannot extend admission.

All initial bounds, original source/ancestor descriptors, absence/link records,
repeat source checks, exclusive publication and permanent refusal remain.
Package file lists additionally have a 16384-entry bound. `semantic_admission`
remains false. No VM invocation, account creation, generator execution or
manager activation is claimed by this source checkpoint.

## How the result will close the next decision

Resolve the captured vendor `systemd-user` PAM include closure; review the
selected priority/prefix unit sources in actual precedence; bind the installed
generator manuals and package generation to the expected generator behavior.
Current official upstream implementation explains that XDG autostart generation
and environment generation are separate mechanisms; it does not authenticate
these installed binaries. See the official [XDG generator source](https://github.com/systemd/systemd/blob/main/src/xdg-autostart-generator/xdg-autostart-generator.c)
and [environment generator source](https://github.com/systemd/systemd/blob/main/src/environment-d-generator/environment-d-generator.c).
Any actual unexpected PAM module, override, environment/activation reference or
unresolved version binding remains a named blocker, not blanket acceptance.

Only then propose the whole-invocation fixture with original-source rechecks,
create-only account/HOME/runtime, no skeleton copy, exact credentials and
clean environment, real inactive OmaVLESS units and the reviewed standard
socket/bus lifecycle. Unknown child/process state still means stop and retain.

Pure controls reuse all ten original synthetic-FS tests against the separate
module, plus fixed-root/no-execution, package ambiguity/name/version/duplicate,
package-directory drift and exact ELF membership/non-ELF controls. No actual
host catalog is captured by ordinary tests.
