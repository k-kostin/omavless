# Try Omarchy ARM64 smoke findings — 2026-08-26

This report preserves privacy-safe local evidence collected while testing OmaVLESS on Try Omarchy on an Apple Silicon Mac. It contains no profile names, endpoints, share URIs, UUIDs, subscription URLs, keys, or reusable credentials.

## Environment

- Host: Apple Silicon Mac running Try Omarchy
- Guest architecture: `aarch64`
- Guest kernel observed during the test: `7.2.0-2-aarch64-ARCH`
- Try Omarchy runtime observed: `4.0.0.alpha-1`
- OmaVLESS version: `0.7.0`
- Initial tested `main`: `cc24ae4c4e045b39ae74f2a141c70614bf4a84d1`
- D1 / PR #27 exact head checked separately: `694a6314c531f36e7474891f3e3b27a874262693`
- Full VPN lifecycle fix commit: `2e70b8af767b9135fd74f20f9d10cd77d57ec848`
- Mihomo package: `1.19.30-1`
- Mihomo runtime: Meta `v1.19.30` linux arm64
- Mihomo capabilities: `cap_net_bind_service,cap_net_admin,cap_net_raw=ep`
- Private controller path: `/run/user/1000/omavless.1000.controller.sock`

## Finding 1 — Full VPN startup self-deadlock

The original `main` reproduced `Routing -> Full VPN` failure with:

`Private Mihomo controller did not become ready; previous OmaVLESS state restored`

The same failure reproduced on exact PR #27 head `694a6314c531f36e7474891f3e3b27a874262693`; D1 does not change this lifecycle path.

Observed sequence before the fix:

1. `set-mode` acquired the global operation lock.
2. It synchronously restarted `omavless.service`.
3. The new supervisor entered `run-core` through normal pre-dispatch locking.
4. `run-core` blocked on the same operation lock still owned by `set-mode`.
5. Mihomo was not spawned and the controller socket was absent for the full readiness window.
6. Readiness timed out and transactional rollback ran.
7. Once the outer operation released the lock, Mihomo spawned and created the controller within milliseconds.

Classification: OmaVLESS lifecycle bug, not slow Mihomo startup.

Fix in `2e70b8af767b9135fd74f20f9d10cd77d57ec848`: internal `run-core` dispatch bypasses user mutation locking/migration while normal user mutations retain serialization.

Regression coverage includes a real fake-core subprocess proving `run-core` can start while the user mutation lock is held.

## Finding 2 — nested Full VPN selector topology

After removing the lock deadlock, Full VPN reached the private controller but Mihomo rejected the outbound selection.

Generated selector topology is:

```text
GLOBAL
└── PROXY
    └── active private profile
```

Mode semantics:

- Routing: Mihomo `rule`; routing rules target `PROXY` or `DIRECT`.
- Direct: Mihomo `direct`; traffic bypasses proxy selection.
- Full VPN: Mihomo `global`; `GLOBAL` must select `PROXY`, and `PROXY` must select the active private profile.

The previous implementation attempted `GLOBAL -> active private profile`. Mihomo 1.19.30 rejects that because the private profile is not a direct member of `GLOBAL`.

Fix in `2e70b8af767b9135fd74f20f9d10cd77d57ec848` performs and verifies, on the private Unix controller:

1. `PROXY -> active private profile`
2. `GLOBAL -> PROXY`

Any refusal or mismatched readback retains the existing transactional failure/rollback path.

## Full VPN lifecycle validation after both fixes

Try Omarchy live transition matrix on the fix commit:

- Routing -> Full VPN: PASS, approximately 1.51 s
- Direct -> Full VPN: PASS, approximately 3.35 s
- Full VPN -> Routing: PASS, approximately 1.22 s
- Full VPN -> Direct: PASS, approximately 1.20 s

Sanitized live controller verification confirmed that the active profile was selected by `PROXY`, and `PROXY` was selected by `GLOBAL`.

A forced in-memory selector refusal during Direct -> Full VPN confirmed transactional rollback:

- failure observed;
- rollback reported;
- persisted template restored to Direct;
- generated configuration restored to Direct;
- service remained active.

## Automated validation on the fix

Observed results after both fixes:

- Python suite: 159 passed, 3 expected skips
- QML contracts: PASS
- Installed Mihomo validation: 2 passed
- `git diff --check`: PASS before commit

## Finding 3 — repeated authentication prompts

The three authentication prompts seen on TUN recreation were investigated separately. Similar three-prompt behavior is also known to occur on the normal bare-metal Omarchy PC, so it is not treated as Try Omarchy-specific evidence.

Each transition that recreates the TUN causes the user-owned Mihomo process to directly launch three short-lived `resolvectl` children:

1. `resolvectl domain Meta ~.`
   - polkit action: `org.freedesktop.resolve1.set-domains`
2. `resolvectl default-route Meta true`
   - polkit action: `org.freedesktop.resolve1.set-default-route`
3. `resolvectl dns Meta 198.18.0.2`
   - polkit action: `org.freedesktop.resolve1.set-dns-servers`

These are three distinct authorization actions, not retries of one request.

Observed responsibility chain:

1. OmaVLESS restarts its user service.
2. Old Mihomo removes the TUN.
3. NetworkManager notices the removal.
4. New Mihomo creates the `Meta` TUN.
5. NetworkManager detects/adopts the externally created interface.
6. Mihomo launches the three user-owned `resolvectl` commands.
7. systemd-resolved applies routing domain, default-route status and DNS server.
8. Each separately protected resolved D-Bus operation passes through polkit and the Omarchy/Quickshell authentication agent.

OmaVLESS does not directly invoke `pkexec`, polkit helpers, `nmcli`, `resolvectl`, or systemd-resolved for these transitions. Mihomo already has the required network capabilities, but Linux capabilities do not authorize privileged systemd-resolved D-Bus methods governed by polkit.

Classification: expected Mihomo/systemd-resolved/polkit interaction under the observed policy, with a UX cost of three prompts. No evidence tied it to the earlier controller/locking defects. No policy relaxation was added to OmaVLESS.

## ARM64 plugin lifecycle smoke

Baseline before lifecycle exercise:

- plugin enabled;
- Routing mode;
- 56 profiles and 1 subscription (counts only);
- one service, one supervisor, one Mihomo process, one TUN;
- controller ready.

Results:

- Disable: PASS. Stored state preserved; service/processes/TUN removed. A harmless stale controller socket node remained.
- Re-enable: PASS. UI reopened; Routing preference and stored data preserved; plugin remained disconnected as expected.
- Reconnect: PASS. Exactly one service, one supervisor, one Mihomo and one TUN; controller recreated and responsive.
- Restart Omarchy Shell: PASS. UI reopened; connection survived; counts remained exactly one.
- `omarchy plugin validate`: PASS, exit 0.
- Final restoration: PASS. Plugin enabled, Routing active, service/controller healthy.

No duplicate service, Mihomo process or TUN was observed. No ARM64/Try Omarchy-specific OmaVLESS defect was found in this lifecycle smoke.

## Remaining acceptance boundary

This Try Omarchy ARM64 evidence is valuable local integration evidence but does not replace the repository's declared real/bare-metal Omarchy gates where those are explicitly required. The Full VPN lifecycle fix should receive a short bare-metal acceptance smoke on the exact fix commit before merge, including the four mode transitions and real traffic/TUN/DNS behavior.
