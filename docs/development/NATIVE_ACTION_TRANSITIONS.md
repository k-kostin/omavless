# Native action transition presentation

Scope: the Omarchy plugin frontend. This does not change Rust lifecycle,
network configuration, authorization or the definition of a confirmed
connection. It extends the narrow mode-switch presentation in #296.

## UX decision

- Connect, server replacement and Disconnect have distinct, neutral waiting
  labels. A successful command reply starts a bounded wait for a newer coherent
  snapshot and observation; it does not itself show Connected or Disconnected.
- Native `starting`, `reconnecting` and `stopping` are neutral waiting states.
  A current failed outcome, unknown operation result, failed snapshot read or
  manual-recovery state remains urgent. A retry can temporarily supersede an
  older failed snapshot, but a newer failure is urgent.
- Subscription and profile mutations show a small status beside the relevant
  controls. Other settings mutations show it on Settings. Rejected metadata
  actions retain their contextual error and display only a brief “checking
  current state” while fresh observations arrive; they do not imply that the
  VPN itself failed. The status is time-bounded and cannot silently confirm an
  action.
- During a connection transition, old traffic counters and the connected
  identity are hidden rather than presented as evidence about the new target.

The icon/title/tooltip and card use the same transition classification. Local
regressions test stale observation, a different connected server, intermediate
stopping, actual failure and unknown/recovery priority. Installed EN/RU review
and exact-head network acceptance remain separate from these synthetic tests.
