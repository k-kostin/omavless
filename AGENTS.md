# OmaVLESS agent workflow

This file is the mandatory entry point for every coding or testing agent working on OmaVLESS: browser/cloud agents, Codex Desktop on macOS, Codex CLI inside Try Omarchy, and agents running on a bare-metal Omarchy PC.

Before planning, implementing, gating or merging work:

1. Refresh remote metadata first (`git fetch origin --prune` or the equivalent through connected GitHub tooling).
2. Compare the local `main` and current branch with their remote refs; do not reason from a stale checkout.
3. Read the current remote versions of:
   - `DEVELOPMENT_ROADMAP.md`
   - `docs/roadmap/DEVELOPMENT_WORKFLOW.md`
   - `docs/roadmap/ACCEPTANCE_ENVIRONMENTS.md`
   - the relevant feature/protocol roadmap referenced there.

GitHub is the source of truth. Do not rely on an agent's private conversation state or a stale local checkout as the only record of a decision, test result, policy or useful implementation.

## Acceptance environments

Evidence is divided into three layers:

- **Cloud/static evidence** — CI, deterministic tests, code review, static/security checks and documentation/research work.
- **Try Omarchy ARM64 acceptance evidence** — Omarchy running under Try Omarchy on Apple Silicon. This is an accepted local integration/acceptance environment for normal plugin, Quickshell, user-systemd, Mihomo/controller, TUN, routing, lifecycle and mode-transition testing.
- **Bare-metal Omarchy evidence** — an additional independent pass on the physical Omarchy PC, normally useful for confidence but not a merge prerequisite by default.

For ordinary runtime changes, a green exact-head cloud/static gate plus the declared exact-head Try Omarchy integration gate is sufficient to make a change eligible for merge to `main`, subject to normal review and explicit owner approval.

A bare-metal Omarchy PC pass is **recommended secondary verification**, not a default hard merge gate. Record it in the roadmap/PR as a follow-up confidence pass when useful.

Bare-metal testing becomes a required gate only when the change has a concrete reason to depend on physical-host behavior that Try Omarchy cannot represent reliably, for example:

- physical Wi-Fi/Ethernet or NIC-specific behavior;
- suspend/resume or real network-transition handling;
- x86_64-specific behavior;
- host firewall/kernel/driver behavior sensitive to virtualization;
- hardware-specific routing or networking edge cases;
- a bug which reproduces only on the physical PC.

Do not label a check "bare-metal required" merely because older roadmap or PR wording said "real Omarchy". Apply the current policy and keep a hard gate only when the task itself has a concrete hardware/virtualization-sensitive reason.

Try Omarchy evidence must still be honest: identify it as ARM64/virtualized evidence and do not claim that it exercised hardware behavior which was not tested.

## Exact-head discipline

Every runtime acceptance report records the exact tested commit SHA. If the candidate changes afterward, repeat the checks materially affected by the change.

## Git discipline

- `main` is the only long-lived development source of truth.
- Use narrow feature/fix/docs branches and PRs; do not commit implementation work directly to `main`.
- Preserve useful work on GitHub before ending an ephemeral/local VM session. Never leave the only copy of a useful commit or test report inside Try Omarchy.
- Runtime/security/network fixes should include regression tests where practical.
- Keep credentials, profile URIs, UUIDs, private keys, subscription URLs and reusable secrets out of commits, PRs and shareable diagnostics.
- Do not weaken OS security policy merely to remove minor UX friction unless a separately reviewed security design explicitly calls for it.

## Cross-agent handoff

An agent handing work to another environment should report at minimum:

- repository/branch and exact HEAD SHA;
- what changed;
- cloud/static results;
- Try Omarchy results, if run;
- bare-metal results, if run;
- remaining required gates versus optional confidence checks;
- push/PR state.

When a useful investigation produces findings that future agents need, store a credential-safe report or update the relevant canonical documentation in Git rather than leaving the result only in chat history.

## Current continuity checkpoint — 2026-08-28

D1 / PR `#27` (advanced Mihomo diagnostics and adaptive polling) is complete and merged. The accepted D1 head was `2c2615247d32e7ac4b2f7ada055d1028586cc639`; merge commit in `main` is `5a50d59ae9fada9c61ee3ed9037f72e35bf09853`.

Before repeating D1 investigation or acceptance, read [`docs/testing/TRY_OMARCHY_D1_ACCEPTANCE_2026-08-27.md`](docs/testing/TRY_OMARCHY_D1_ACCEPTANCE_2026-08-27.md). It preserves the exact-head evidence and the important lifecycle findings from the session, including:

- the confirmed Mihomo selector-readiness race where `/version` was ready while `PROXY` still returned `404`, then accepted the identical request 50 ms later;
- the bounded selector retry semantics now merged into `main`;
- the distinction between that selector race and the separate three-prompt Mihomo/systemd-resolved/polkit behavior;
- the final 5/5 Routing -> Full VPN regression plus the other required mode transitions;
- D1 diagnostics/privacy/polling/provider acceptance;
- the original manual UI result and then-deferred Tab-navigation issue `#37`,
  which is now fixed and merged through PR `#43`;
- the practical cloud-orchestrator -> user -> guest Codex CLI workflow used when Codex Desktop could not address the raw Try Omarchy QEMU window.

PR `#30` / `codex/experimental-protocol-live-gates` has since been rebuilt as
one V0-only commit directly on `main`, with exact head
`a643db595ad5369b5fea200ebc601b4f0f70f18f`. Its static/installed gates and one
representative VLESS XHTTP live case pass. It remains Draft and partially
validated because VLESS Encryption / REALITY PQ, Trojan, Hysteria2 and TUIC v5
fixtures are unavailable; do not fabricate them or call V0 complete.

The missing V0 fixtures do not block independent no-new-protocol-key work.
File-picker onboarding (`#39` / PR `#41`), unified top-level subscription import
(`#40` / PR `#42`) and panel-local Tab navigation (`#37` / PR `#43`) are merged.
Do not restart D1, restack #30 on it, or skip the remaining V0 evidence gate to
begin P4a WireGuard/AmneziaWG implementation.
