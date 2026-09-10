# Native shell IPC connection controls

The existing plugin `toggle` and `down` calls previously reached legacy Service
methods which deliberately reject native ownership. They now dispatch through
the same fenced native action path as the visible connection controls.

Toggle uses the selected profile or last profile only from a verified
disconnected presentation, and disconnects a verified connected presentation.
Unknown state never starts a tunnel. Explicit down can request disconnect in
recovery states; existing native action admission/replay remains authoritative.
`ok` means queued locally, not completed or externally reachable. Rejection
uses fixed public text, never private backend errors. Legacy behavior remains.

No daemon or CLI method, credential response, privileged command, or automatic
connection is added. Python remains oracle. Deterministic tests cover native
selection/fallback, connected/unavailable/rejected dispatch and legacy routing.
Installed IPC connect/disconnect and unchanged one-owner observations remain
required before merge; UI compilation alone is not lifecycle evidence.

Profile-targeted IPC (edit, rename, QR and the separately composed file export)
now resolves from validated native metadata instead of the intentionally empty
legacy array. Exact IDs take precedence; duplicate names fail without enumerating
private IDs or echoing input. All actions still use their existing generation,
revision and capability admission. Ten deterministic resolver checks supplement
the eight connection-dispatch checks. No new IPC method is introduced.
