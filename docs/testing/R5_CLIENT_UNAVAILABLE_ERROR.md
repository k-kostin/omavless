# R5 client endpoint availability classification

The native unary client now classifies an absent runtime directory using the
existing `SocketUnavailable` error instead of `UnsafeRuntimeDirectory`. A
stopped packaged runtime can normally have no `RuntimeDirectory`, particularly
after reboot. The public fallback remains the existing bounded English
`OmaVLESS runtime socket is unavailable`; it does not falsely assert a
permission problem or infer why the runtime is absent.

This is a client error-classification correction, not login activation. It
creates no directory/socket/lock, starts no service, changes no desired state,
and does not modify daemon directory preparation or recovery. Missing socket
and stale socket without a listener retain their previous unavailable outcome.
An existing symlink (including dangling), non-directory, wrong owner or mode
other than exact `0700` remains unsafe. Socket type/owner/exact `0600` and
connected-peer authentication still run before any private request is sent.
Metadata errors other than absence remain unsafe; there is no repair/fallback.

Deterministic tests use synthetic temporary paths and private-input sentinels:

- absent directory/socket and stale socket return unavailable without writes;
- unsafe directory modes, wrong owner, live/dangling links and regular files
  remain rejected, with no connection reaching a prepared listener;
- existing wrong-peer/no-request-bytes and unsafe-socket tests remain required;
- the real CLI `status` returns exit 2, empty stdout and the exact safe error,
  while a permissive directory keeps its distinct unsafe error and permissions.

Python behavior is not newly implemented or removed: this fixes only the native
client's existing availability/security distinction. No runtime ownership,
protocol envelope, lifecycle, QML or packaging change occurs. Local ARM64 real
executable/socket tests are the applicable host gate; a live VPN or installed
service restart is unnecessary for this pre-connect error-only change. Login,
complete frontend parity and R5/R6 acceptance remain separate.

Try Omarchy ARM64, 2026-09-10: all five focused native-client tests and all 20
real CLI integration tests pass; runtime all-target strict clippy, workspace
format check and diff check pass. Restricted-sandbox socket tests initially
refused socket creation; the same tests passed with ordinary host Unix-socket
access. No installed daemon, VPN, private profile or OS policy was changed.
