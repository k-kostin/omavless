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
  controls, including a refresh launched from a subscription row on the main
  page. A successful subscription action there has a brief row-local result,
  not a persistent top-level notice. Metadata work does not turn the VPN bar or
  header into a global Working state or erase an otherwise confirmed routing
  highlight. Other settings mutations show it on
  Settings. Rejected metadata
  actions retain their contextual error beside profile or Settings controls
  and display only a brief “checking current state” while fresh observations
  arrive; they do not imply that the VPN itself failed. Subscription errors
  keep their existing local banner. Unknown outcomes and manual recovery remain
  global. The status is time-bounded and cannot silently confirm an action.
- During a connection transition, old traffic counters and the connected
  identity are hidden rather than presented as evidence about the new target.
- Rename/replace of the *active* profile is not store-only: the Rust owner
  quiesces and recovers its core. Its frontend waits for newer confirmed
  connected facts; deleting the active profile waits for confirmed disconnect.
  Edits to inactive profiles and favorite changes retain only local feedback.

The icon/title/tooltip and card use the same transition classification. Local
regressions test stale observation, a different connected server, intermediate
stopping, actual failure and unknown/recovery priority. Installed EN/RU review
and exact-head network acceptance remain separate from these synthetic tests.

## Installed PC-VM rendering checkpoint, 2026-09-28

The isolated Omarchy VM had this branch's `Panel.qml` and `Service.qml`
installed byte-for-byte (SHA-256 `617a565367718831486fa0e4bad013b86b1ed45416ed287e61eb272d91a991f7`
and `7c2f2c22ce2b89e8cbd072cde545f73cb90367f471f292da33a185b61d70a754`).
It used the separately pinned experimental DNS-pair runtime, not a released
package. An agent-run real widget click switched confirmed Routing to Full VPN,
then a second click switched it back. A time-series capture of the second
transition showed a neutral blue `SWITCHING MODE` icon and bordered
`Switching to Routing…` card, with stale connected identity/counters withheld and controls
temporarily disabled. It did not display the red failure shield or a premature
Disconnected state. After verification the panel returned to Connected with
Routing selected; explicit Disconnect restored a clean disconnected runtime
and released DNS/TUN. Private captures remain outside Git.

This checks the rendered English success path only. Failed authorization,
unknown results, Russian layout and owner-attended acceptance remain separate
gates; neither these images nor a Connected label prove internet reachability.
