# R5 Arch service gate — 2026-09-08

Candidate: `531e2a22c8a0f9effa7882294f26d4d96a831b06`, PR #196.
Baseline: `9512eb03bccabb4e00a6ddadda9ed1e754d5ea94`.
Environment: Try Omarchy ARM64; installed Mihomo 1.19.30.

## Static gates

- 630 Rust tests passed, 4 ignored; installed-Mihomo opt-in enabled.
- 272 Python tests passed; QML/i18n contracts passed.
- Four packaging/payload tests explicitly passed.
- Formatting, Python compile, shell syntax, manifest and diff checks passed.
- GitHub Test passed, run 34239381538.

## Service evidence

Initial direct-core candidate-policy test created a TUN and stopped cleanly
after normal human authorization. That did not test Rust ownership.

Subsequent probes executed the real built `omavless daemon` as a temporary
user service with the candidate NoNewPrivileges=no, LimitCORE=0 and UMask=0077
policy. `OMAVLESS_HOME`, XDG_STATE_HOME and XDG_RUNTIME_DIR selected exclusively
new synthetic directories under a private temporary runtime root. Real user
store/ownership state was never changed. The test seeded a synthetic Rust
ownership marker there, not a production cutover. Executable was the workspace
build, not /usr/bin installation. Package directory creation remains untested.

Fixture: synthetic VLESS TCP example using a documentation-only destination,
no private provider or credentials. The synthetic template enabled gVisor TUN
`ovnativetest0`, disabled auto-route/auto-redirect and DNS, configured no mixed
port, and retained synthetic PROXY/GLOBAL selectors. No external HTTPS probe
was sent, so this is not provider or traffic interoperability evidence.

Three runs reproduced:

| Observation | Result |
| --- | --- |
| Rust daemon started by user systemd | PASS |
| Semantic hello over mode-0600 Unix socket | PASS |
| Semantic connect, global mode | exit 0 |
| Semantic status | connected |
| Direct Mihomo children owned by daemon | exactly 1 |
| Synthetic TUN present | yes |
| Semantic disconnect | exit 2, safe generic rejection |
| Status after disconnect | manualRecoveryRequired |
| Stop entire temporary service | PASS |
| TUN after service stop | absent |

The third run additionally inspected state **before** service cleanup:
durable desired.connected=false, direct Mihomo child count=0, TUN still present.
Thus the disconnect did not merely fail to persist intent. Current lifecycle
code requires an empty observation immediately after stop/discard and correctly
refuses to claim disconnected while a TUN remains.

Do not infer the precise residual owner from these counts: surviving helpers,
TUN teardown timing or host management require further attribution. No proof
yet that a longer sleep, forced deletion, relaxed readiness predicate or killing
unattributed processes is correct. A package policy fix alone does not resolve
this lifecycle gate. The known resolved/polkit prompts remain separately
documented normal host behavior, not automatically the cause of this failure.

## Next bounded work

1. Capture same-service process/cgroup membership and TUN FD ownership before
   service stop; keep public evidence to categories/counts.
2. Compare Python's whole-service shutdown with Rust's child-only stop.
3. Define owned helper cleanup or bounded kernel teardown observation based on
   attribution; never remove unrelated interfaces or weaken manual recovery.
4. Add deterministic regression and repeat real service connect/disconnect,
   controller/TCP checks and negative recovery on the exact fix.
5. Complete package-owned XDG directory and activation gates separately.

PR #196 remains Draft. No merge, no installed plugin cutover, no R5/R6 completion.
Final observed service/core/TUN cleanup succeeded; synthetic stores were removed.
