# T4 real System-provider mechanism checkpoint

Environment: isolated x86_64 Omarchy development VM, agent-attended under an
exclusive reviewed lease. This is not formal human acceptance, an installed
release attestation or normal historical-policy adoption. The ordinary dispatcher
and its pending-fence refusal are unchanged.

## Exact executed cut

| Object | SHA-256 / Git identity |
| --- | --- |
| Tested source, Draft #586 | `fbe5fb912046223b8ac517b35b0d720e9a23da9f` |
| Parent source, Draft #583 | `5f4f0ca2be17edf025f20637d4e1a3e24bfd85a8` |
| Frozen default runtime test ELF | `47ef34e4f0742d20996d41ead4ec5a06eba495c4fae013dc009a107fcffb1c6d` |
| Reviewed external v4 harness | `520a9c477de7b5e1c236e319abb789b78cac644303d3820da556d65692466510` |
| Literal retained-byte v4 loader | `e5bfd5585ad964a9e298d39cd81e5d8abc1d6988834766e4efe11f7cbb3c72b7` |
| Installed global executable, unchanged | `76abb574f611c11c2513436ac4be48d2fb07a69ba35f56127c93c21c6205921c` |
| Exact seed test log | `dec32445e85efc314897b6cb006dfe6aa8d06fbeb0db92bf362f132fb3812a97` |
| Exact System-review test log | `3474c8eed3cd6723dfc7596f8eba6b90ed418dd969fee0b23b7f3e18d20ae7b1` |
| Private typed invocation receipt | `f203330292187ce4926802cddb35612bef3bd9d0efe619e69a33c54cd9185312` |
| Full local Rust gate log | `477cc0aa7ce14a8b59ea1c98a23c71711de600a96eeae30ce2d341209196a6e9` |
| Full local source gate log | `52080169fd00d9f94569b2f277368e36fd88b4bd43a0842238dfd9816cfa55df` |

The ELF was frozen outside Cargo from the explicitly identified default test
build before the workspace build. Neither it nor the reviewed source/harness
was rebuilt or edited during execution. This report's later documentation head
is not the executed code head. Private raw logs and reviewed root orchestration
remain outside Git, mode0600 under a private0700 parent; copied guest/local log
digests agree. Public documentation contains only synthetic identities and
bounded classifications.

## One complete successful invocation

The harness refused pre-existing fixed account/group/home/runtime/image objects.
It pinned the exact stock global unit/generator/environment/PAM inputs and
activation graph before starting a fresh real user manager. Unknown changes
refused; no runtime mask, fake legacy unit or global package modification was
used. This is fixed-stock fixture trust, not a general static proof of arbitrary
generators. Global core/TUN inventories were actually empty.

The fixed seed selector ran exactly once and passed. The existing packaged
login-prepare unit then ran through the real manager with the installed global
binary, producing the original consumed receipt. No test receipt or caller epoch
was supplied. The fixed review selector then ran exactly once and passed through
actual `CurrentEpochProof` System capture and the private historical-Off adapter.
It retained the original receipt, manager/package evidence and historical/current
members, returned `ReviewedOffStillFenced`, and verified ordinary startup still
refused the surviving fences. The read-only diagnostic selector was not needed.

Only the test process received a root-issued private mount namespace with the
frozen image read-only bound at the fixed executable path before dropping to the
fresh UID. That satisfies the existing self-inode relation; it is not a new
product attestation contract. The real managers and globally installed executable
were not replaced. The command-failure receipt was empty.

The whole stage → tests → cleanup guard returned exit0 and its fixed mechanism
PASS classification. Canonical private-file/executable/service identity, package
units, stock activation inventory, namespace, resolver, routes/rules and network
facts were preserved; only the established strictly decreasing numeric address
lifetimes were allowed. Scoped disposable account database creation/removal is
not a claim that every `/etc` byte stayed identical.

Cleanup removed only validated owned objects. Independent read-only checks found
no disposable account/group, home/runtime/root-image tree or disposable-UID
processes; its two system units were inactive with zero PIDs. Private runner
artifacts/logs were retained. The exclusive VM lease was explicitly returned.
There was no second invocation at this checkpoint.

## Local gates and preserved negatives

At exact `fbe5fb9`, full `tests/run-rust.sh` exited0: runtime1124 passed,
36 ignored and one intentionally filtered helper; that helper separately passed.
Workspace/integration/docs, serialized DNS suites, all12 terminal checks,
default/TUI strict clippy and parity passed. Full source checks exited0 with
509 tests reported, two existing skips, navigation/frontend/QML checks passing.
The three focused CPU cases and six source-retention guards also passed.
Both package CI jobs passed; exact-head Test CI completion is tracked on #586,
not inferred from the local run or from this report.

Earlier results remain distinct:

- V1 at parent `5f4f0ca`, image
  `c3dac79b19cd5f1e7829269e66690dc46452490a1adc96d3ecade28aaeae01a6`,
  harness `62c2d58313eb1c555d42c191237f24ea8bf78285566deae33abd49a360400456`
  and loader `26644671aeb889f1f03e3608384f2665efafbead92b435f7fd4b4195a7dd60cb`
  refused the account command before manager/test execution. Cleanup/guard
  completed. Original child stderr was discarded. Source review corrected an
  initial unsupported-option hypothesis: `--no-log-init` is supported despite
  being omitted from help. The invalid `-K CREATE_MAIL_SPOOL=no` pair was removed
  only after pinning the installed defaults and their unique disabled setting.
  That is source-backed diagnosis, not recovered original stderr. Typed negative
  receipt: `598858e45b4254b9a6e0c3abf7c005e225ed374eae17eb5a86920fbefdb3da52`;
  correction: `22ab5a918c816ace471097061a29b0f8f3928352c773a91048e2152379b06fc0`.
- V2 was reviewed but not executed. V3 retained that account correction and
  added uncertain-process-anchor quarantine. At unchanged parent source/image,
  harness `9143a7ae82d8a1a33845872898b8fd8c083b7334158f103b5155bb8d044bfaa7`
  and loader `0806d7b0883973c0b77c3558ee194263808f33d709421463b8c73eedbe69800c`
  reached seed1PASS, then the real login unit returned the public Validation
  category. Review was UNRUN; cleanup/guard passed. Typed receipt:
  `b22dc599786167bfad6d21ca91c84dcaf72622338203bd73be18739c0ba6f22e`.
- Frozen CPU prefix `b0564b7141b189c6f6fa5c6b94e6fd77d4b6d150` reproduced
  `consume_login -> InvalidState` with zero host observations. Its exact log
  hash is `124c155d29f6b4da52c9bffe433028047a09308d499f456da4b031a2291ee0a4`.
  Actual store parsing rejects the seed's literal `routingPreset=default`.
  Correcting only that test input to `roscomvpn-default` passes actual Off
  consumption, preserving the old-input no-host/no-receipt/no-private-change
  negative. Historical imported inputs independently pass actual store/desired
  validation. This establishes a seed defect consistent with V3's category,
  not its unavailable internal stack. The absent-legacy-unit hypothesis was
  not proven and no fake unit or weakened service predicate was introduced.

## What this closes—and what it does not

The same-manager original-receipt System-provider mechanism is no longer an
unrun positive at the tested developer cut. Ordinary uploaded ELF refusal is
not evidence that a reviewed private-image mechanism is impossible.

Normal dispatch remains unregistered. No historical fence is removed, no owner
escapes, and no profile/connection/batch mutation while fenced is adopted.
Old-manager/missing-receipt recovery, crash-safe fresh receipt policy, ownership
rollover, private restore UX, installed distribution/upgrade/rollback and formal
acceptance remain separate. The private adapter's existing source tests retain
same-byte replacement, proof/host loss, final-drop ordering and permanent-refusal
boundaries; this one real invocation does not relabel every synthetic negative
as actual System evidence. See the [owning adapter](../development/T4_SYSTEM_HISTORICAL_OFF_ADAPTER.md)
and [diagnostic contract](../development/T4_LOGIN_STAGE_DIAGNOSTIC.md).
