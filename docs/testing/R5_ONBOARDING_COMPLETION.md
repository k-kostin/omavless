# Native onboarding completion

The existing Python `onboarding-complete` command remains the installed owner.
This checkpoint supplies its registered native `onboarding.complete` equivalent
and fixed `omavless onboarding complete` CLI, without activating the frontend
bridge, changing login behavior or claiming R5/R6 completion.

The operation sets only the canonical store completion flag. Profile contents,
selection, startup preferences, rules and extension metadata survive ordinary
store normalization. It never observes or restarts Mihomo and never changes
desired state, including when the native owner is connected.

Exact operation/revision parsing, native generation fencing, shared mutation-ID
namespace, migration lock, private atomic write and compensation are reused.
The preset pending barrier blocks admission before cached replay. Already
complete is a true no-op: unlike Python's unconditional save, Rust preserves
the exact existing bytes and revision. Invalid stores are not repaired/reset.
Cached completion is not a generic permission to change any other setting.

## Evidence contract

- `tools/onboarding_completion_parity.py` executes the actual Python main
  dispatch branch, with argument parsing, private-store access, locks and host
  effects replaced by synthetic in-memory adapters. Neither real HOME nor
  installed files are read or written. Bounded stdin and digest-only output
  keep private source fields out of argv, failures and reports.
- 21 differential cases cover v1/v2/v3, missing/false/true flags, empty/populated
  stores, extensions and malformed inputs. Complete normalized store digests
  are compared, not just the final flag.
- Native coordinator tests preserve connected desired state and host-call
  count, share replay/revision semantics and verify no-op behavior.
- Private Unix socket tests cover registration, replay, no-op, stale revision,
  exact parameters, pending-marker refusal, unsafe store and revoked/stale
  ownership. Executable tests check actual CLI framing and rejected extras.
- Existing prepared private-store fault tests remain the atomic-write/uncertain
  restoration reference. No new writer or lifecycle state machine is added.

Run full Rust/Python gates and focused socket/CLI tests on the exact candidate.
This store-only operation needs no live server/TUN fixture or visual change;
real local private-file/socket/CLI tests are the applicable host integration.
Installed QML/Python ownership and private production data remain unchanged.
