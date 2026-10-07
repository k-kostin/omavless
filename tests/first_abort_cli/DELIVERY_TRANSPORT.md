# Fresh quoted SSH delivery generation

This generation follows the stopped #634 delivery attempt. The operator's
v5 create-SSH command passed Python's arguments separately through the actual
`guest.sh` wrapper. Its `exec ssh ... "$@"` preserves local argument boundaries,
but SSH joins remote arguments with spaces for the remote shell. Python's
semicolon/multiline `-c` source consequently lost its quoting; the observed
remote shell syntax failure returned exit2 before Python. The v5 attempt stopped
there without a later query, retry or cleanup. That outcome does not establish
current guest paths, account/child state, quiescence or preservation. All failed
scopes and original evidence remain retained.

## New fixed generation

- UID/GID48047; account `ov-t4-abort-v4`.
- HOME `/home/ov-t4-abort-v4`; runtime `/run/user/48047`.
- Delivery `/home/kdk_vm/.cache/t4-first-abort-cli-delivery-v6`.
- Root stage `/run/ov-t4-cli-guard-v6`; delivery schema v6.
- Native source checkpoint `285233049bc6c5e1356de0ffcc167185c9e1761d`.

The fixed native credentials, paths and exact diagnostic self path change only
under cfg(test). The existing 31-record success transcript, stopped-owner
predicates, budgets and terminal refusal behavior are unchanged. No production
trace, override, permit or changed recovery rule is added. Old UID48046/v3/v5
identities describe their historical generation and cannot authorize this one.
A source-only consistency regression ties the diagnostic UID and exact helper
self path, native fixture credentials/paths, lineage and loader generation to
the root guard's fixed identity. It never executes an ignored native fixture.

## Pure transport construction

`delivery_command.create_delivery_command(reviewed_source)` returns a tuple
containing exactly one remote command string. That string uses `shlex.join` on
the fixed `/usr/bin/python3 -I -B -c` invocation. The operator passes the tuple's
single member to `guest.sh`/SSH as one argument; it must not split that string
into SSH argv again. The source is bounded UTF-8 text, without NUL. Import and
construction perform no filesystem, process, SSH or guest operation.

The inert regression models the actual five-line guest wrapper (source SHA256
`112c2e160b6f2dfda1f6419c9f24a879d75f011d981da66b13213bdc2459633a`),
local shell argument preservation, its SSH "$@" forwarding and SSH's remote
space-join. It proves exact Python argv reconstruction for semicolons, multiline
quotes, Unicode and literal shell substitution characters. A negative control
reproduces the old loss of the Python-source boundary and exposed semicolon
syntax. No SSH, shell command, guest script or Python delivery program is run
by that regression; shell execution and successful guest delivery remain
separate unrun gates.

## Provenance and remaining gates

This native checkpoint requires an independent ignored-helper build and an
explicit locked normal-CLI build, then a separately reviewed host freeze and
new original-FD receipts. Do not relabel the old058 ELF. Only trusted original
mode0500 single-link frozen files outside Cargo can supply actual delivery.
Historical Cargo original mode/link/alias records are build provenance alone;
they never supply executable or guest admission authority. Existing strict
receipt validation and the single-link mode0500 delivery checks remain intact.

The previous reviewed host-seal source may guide a new proposal; neither its
old identities nor its known-zero result authorize this generation. New source
pins, honest build/normalization/alias records, native and guard heads, and
loader pins must be checked together. A Python-only pin commit may reuse the
new native build only after its complete Rust/Cargo diff is proved empty.

Full source and Rust workspace/Clippy/TUI/parity checks, complete root and peer
review, fresh original file and capacity admission, and the root operator's
exclusive VM lease remain required. Only the root operator may execute the
guest workflow. This source checkpoint performs no VM, account, service,
network or ignored native-fixture invocation. No product T4 PASS, merge,
candidate integration or publication is claimed.

## Python-only elapsed-boundary continuation

The loader and root guard check elapsed time immediately after potentially
blocking reads/rechecks and before the next create, copy write, owner/mode
change, sync, descriptor transition or owned successful writer close. Copy
writes require exact integer full counts; bool/float aliases and unknown/short
results are terminal. The guard retains its own deadline-aware create-only
evidence sink rather than changing the historical support module. Its one
bounded write and each file/directory sync must remain inside the budget.

Fresh typed WNOWAIT completion must still be within both local and whole-run
budgets before the one reap. Raw reaped status must exactly equal the admitted
exit code shifted into its exit-status field; arithmetic aliases are refused.
A late reap remains unknown, never returned as zero or followed by log reads.
Exact typed terminal output checks the budget after write/before flush and
after flush, permanently sealing success or uncertainty. No second output
attempt, syscall cancellation or undo of an already-started effect is claimed.

Mocked delayed-return controls execute these reached copy, evidence, wait and
terminal paths without any process, ELF, account, service, network or guest
operation. Native inputs are unchanged. New source/guard head pins may reuse
only the known-zero original frozen checkpoint285 artifacts with honestly
retained actual build headb69 after complete native-source equivalence checks;
they do not claim a new native build or repeat original normalization. Every
new host delivery remains create-only and separately reviewed; old sealed
deliveries and failed scopes stay retained.
