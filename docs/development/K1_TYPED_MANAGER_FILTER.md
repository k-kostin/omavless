# K1 typed manager-property fixture

This new immutable test generation follows the retained #599 NONPASS.
Its literal unit and stage are `omavless-k1-typed-filter-fixture.service` and
`/run/omavless-k1-typed-filter-fixture`; previous names/stages remain untouched.
It has no production caller, package input, arbitrary IPC, namespace transition
or network mutation. It still tests invalid `/dev/null` setns interpretation.

Text omission is never equivalent to a verified empty property. The fixed
`typed_manager_properties.py` reads exactly eight Service properties from one
literal system-manager object using `/usr/bin/busctl --system get-property`.
Six Exec arrays must be byte-exact `a(sasbttttuii) 0` lines, EnvironmentFiles
must be `a(sb) 0`, and SystemCallFilter must be `(bas) false 0`. Wrong polarity,
signature, count, missing/extra tokens or bytes, nonzero process exit and any
ownership uncertainty refuse. No `~` interpretation or unknown-property fallback
is accepted. The exact argv/property list cannot be selected by a caller.

The independently inspected v261 [getter](https://github.com/systemd/systemd/blob/v261/src/core/dbus-execute.c)
serializes the syscall allowlist boolean and array; the [CLI formatter](https://github.com/systemd/systemd/blob/v261/src/systemctl/systemctl-show.c)
does not give a reliable empty-structure-array text receipt. A separate read-only
VM query verified all eight exact typed receipts on the retained inactive unit.
That observation does not execute or validate the kernel namespace filter.

## Acyclic source pins and refusal

The helper reads the unchanged original query guard through one no-follow FD,
requires exact root/0600/one-link bounded shape, checks stable metadata and its
SHA256, and compiles only those retained bytes under a fixed non-main name.
No original main/snapshot/runner is invoked. Only its existing raw owned-child
command capture is reused, with STAGE replaced by the new literal private root.
Thus no temporary file or operation touches the old fixture stage.

The immutable original guard pins only old unused artifacts; helper pins that
guard; new runner pins helper/original guard and both new unit bytes; new outer
guard pins runner/helper/original guard/probe; create-only loader must verify
the complete staged graph before publication. This avoids a circular hash.

Query exceptions print only a fixed refusal marker. Runner failure stops before
start, next case or unlink. The outer guard now refuses any nonzero runner exit
before its next snapshot/command: a failed helper exit can never be promoted
into evidence its uncertain nested child was reaped. Unknown state retains all
artifacts. Successful-only cleanup still requires exact quiescent owned state.

Offline tests use realistic mocked typed replies/omissions and cover exact fixed
argv, immutable-source tamper, missing/changed signatures/count/polarity, extra
output, command failure, persistent raw-wait uncertainty, and failure propagation
before any start/unlink. The older artifact source and old failure archives are
unchanged. Source gates, independent full review, frozen hashes, new create-only
staging and an exclusive VM lease are required before actual execution. No
installed/filter/canonical namespace authority or complete K1 acceptance follows.
