# OmaVLESS acceptance environments

Status: canonical acceptance policy, updated 2026-08-27.

This document defines how local runtime evidence is classified across all OmaVLESS development environments. It supplements `AGENTS.md` and supersedes older generic wording that treated a bare-metal Omarchy PC as the only valid local acceptance host.

## Accepted evidence layers

### Cloud/static evidence

CI, deterministic tests, code review, static/security checks and documentation/research work.

### Try Omarchy ARM64 acceptance evidence

Omarchy running under Try Omarchy on Apple Silicon is an accepted local integration/acceptance environment for normal OmaVLESS plugin/runtime work, including Quickshell, user-systemd, Mihomo/private-controller, TUN, routing, lifecycle, mode transitions, plugin enable/disable and ordinary live provider/server checks.

For an ordinary runtime change, green exact-head cloud/static checks plus the declared exact-head Try Omarchy acceptance checklist are sufficient to make the change eligible for merge to `main`, subject to normal review and explicit owner approval.

### Bare-metal Omarchy evidence

The physical Omarchy PC is a recommended independent confidence pass by default. It is useful for catching architecture, virtualization and real-host differences, but it is not a general merge prerequisite.

A bare-metal check becomes mandatory only when the task has a concrete reason to depend on behavior Try Omarchy cannot represent reliably, such as:

- physical Wi-Fi/Ethernet or NIC-specific behavior;
- suspend/resume or real network-transition handling;
- x86_64-specific behavior;
- host firewall/kernel/driver behavior sensitive to virtualization;
- hardware-specific routing/networking edge cases;
- a defect known to reproduce only on the physical PC.

Do not retain a hard bare-metal gate merely because an older PR or roadmap said `real Omarchy`, `Omarchy PC`, or similar. Re-evaluate the actual feature and apply this policy.

## Agent freshness rule

Before planning, gating or reporting repository status, an agent with a local checkout must refresh remote metadata first. At minimum run `git fetch origin --prune` (or the equivalent through connected GitHub tooling) and compare the local `main`/current branch against the corresponding remote refs.

Do not infer current project policy from a stale local `main`. Read the current remote `AGENTS.md`, `DEVELOPMENT_ROADMAP.md` and `docs/roadmap/DEVELOPMENT_WORKFLOW.md` before deciding that a gate is required.

## Exact-head rule

Runtime evidence belongs to the exact tested commit SHA. If the candidate changes, repeat checks materially affected by the change. Record whether evidence came from Try Omarchy ARM64, bare-metal Omarchy, or both.

## Security boundary

Acceptance policy does not relax security requirements. Do not weaken OS security policy merely to remove minor UX friction. Keep credentials, profile URIs, UUIDs, private keys, subscription URLs and reusable secrets out of commits, PRs and shareable diagnostics.
