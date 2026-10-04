# Fresh account generation after #624 NONPASS

This document retains the #628/#630 generation and its original prerequisites.
The new UID48046 source generation and the later #630 NONPASS observation are
scoped separately in [ADMISSION_BOUNDARIES.md](ADMISSION_BOUNDARIES.md); its
fixed identities supersede the historical invocation identities below.

This successor generation is source-only. Immutable #624 at
`5bc852e847d1e29f50c92687e3b11f5a129525fb` ended NONPASS in `account-create`.
A separately reviewed read-only original-file observation found the unknown
`CREATE_MAIL_SPOOL` configuration-item diagnostic in the account command's
stderr. It did not establish child completion, account state, quiescence or
baseline preservation. The original UID48044/account/stages are not reused,
queried for recovery, repaired or cleaned by this successor.

## Narrow cause and correction

Upstream shadow's [`useradd` option parser](https://github.com/shadow-maint/shadow/blob/4.19.0/src/useradd.c)
passes `-K`/`--key` to the login.defs table; failed lookup exits `E_BAD_ARG`.
[`getdef.c`](https://github.com/shadow-maint/shadow/blob/4.19.0/lib/getdef.c)
does not list `CREATE_MAIL_SPOOL`. That setting belongs to
`/etc/default/useradd`, read separately. This source mechanism explains the
observed diagnostic, but does not promote an unobserved old child status.

`--no-create-home` is **not** a mailbox suppression option: mailbox creation
is outside the home-creation conditional. The successor therefore removes
only the invalid `--key CREATE_MAIL_SPOOL=no` and requires the existing fixed
root-owned `/etc/default/useradd` to contain exactly one unambiguous
`CREATE_MAIL_SPOOL=no` line. Missing, duplicate, yes, quoted, whitespace-varied,
oversized or unsafe input refuses before effects. The original descriptor,
full file metadata, pathname, ancestors and hash remain checked at every
existing source gate. It does not edit global defaults, use `--system`, relax
child-status admission or ignore a warning to obtain success. Installed
account tools remain the trusted OS baseline; their defaults are not guessed.

The independent defaults prerequisite for #628 returned NONPASS before any
#628 stage/account/guard invocation. A separately reviewed fixed file-only
classifier subsequently observed an existing root-owned regular single-link
defaults file, mode `0600`, with the exact `no` category. No raw configuration
is published. This explains the earlier exact-`0644` prerequisite refusal;
it establishes neither account state nor child completion or admission.

This successor admits exactly root-owned `0600` or `0644` for this one fixed
defaults path, with the same original-FD, metadata/hash, no-xattr, single-link,
bounded grammar and repeated source checks. Every other `File` mode remains
scalar and exact. The tuple exception rejects other paths, owners, bounds,
orders, Boolean values and malformed tuples before opening a file. No OS mode
or content is changed, and the earlier NONPASS is not repaired or relabelled.

## Independent fixed generation

- UID/GID48045; account `ov-t4-abort-v2`.
- HOME `/home/ov-t4-abort-v2`; runtime `/run/user/48045`.
- Delivery `/home/kdk_vm/.cache/t4-first-abort-cli-delivery-v4`.
- Root stage `/run/ov-t4-cli-guard-v4`; delivery schema v4.

The native ignored helper's fixed credentials/paths and retained-lineage
paths remain the #628 native checkpoint
`69557f12f6d077ef8f248f8ade87d7e595617efd`. The actual build/freeze head was
`a792e9671090456ffe20269b8a795947e3bea8c6`, with identical Rust/Cargo inputs.
This Python-only successor requires an empty native diff and a new delivery
receipt, not an invented rebuild. Existing #628 frozen ELF provenance remains
unchanged; the old2bf helper is not relabelled. UID48045 was not created by the
failed prerequisite, but fresh account/path/instance absence is still mandatory
at the new invocation; past evidence is not a current reservation.
Normal production recovery behavior is unchanged. The old startup catalogs
keep their original hashes and UID48044 observations. A finite additional
original-parent absence gate covers both fresh instance unit names and their
drop-in directories in all twelve already reviewed systemd search roots;
missing roots remain pinned absent. It rechecks before effects, without
inventing new catalog evidence or querying the old account/manager.

The seven-phase preservation/lifecycle contract in [ROOT_GUARD.md](ROOT_GUARD.md)
otherwise remains: no primary application stop, no global policy change,
known-zero mutations only, permanent stop on uncertainty, original lineage,
normal CLI Abort/re-entry still fenced, and no cleanup on any outcome.
Its older path/native identity paragraphs describe the retained #624
generation; the fixed identities above govern this successor.

Required before a new invocation: parent/peer full-source review, focused and
full source gates, retained exact native gates/freeze plus empty native diff,
fresh delivery and
capacity admission, and explicit exclusive VM lease. A separately reviewed
defaults-only read observation may check the new prerequisite; it cannot
authorize account creation or repair any previous NONPASS.
