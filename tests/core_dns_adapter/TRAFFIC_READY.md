# Read-only managed-core traffic readiness

The normal release producer adds `mihomo-traffic-ready.patch` after the
unchanged DNS broker/descriptor patches on Mihomo
`ab405bad5beeeac8b003bb01f60f134f6df54471` and sing-tun
`b50ae28a1409c7bce8e96e6c6966cf57d8ace754`. This GPL-3.0 upstream patch
adds only authenticated-controller GET `/configs/traffic-ready`.
It projects `{"abi":1,"ready":false|true}` from the existing atomic
`tunnel.Status()==Running`; it is not configurable in YAML and grants
no mutation or conditional-close authority.

Mihomo creates listeners/TUN before synchronous provider initialization and
cached profile selection finish. A live controller, SOCKS CONNECT response,
loaded configuration or DNS lease does not by itself prove external traffic
admission. Consumers must bind this projection to the verified new package
family and check it before and after configured-selector observations.
Existing startup/operation deadlines are unchanged. Running is not a
WireGuard handshake, DNS reachability or Internet-connectivity verdict.

The new normal receipt schema is
`omavless-managed-dns-traffic-ready-pair-v1`, with integer
`traffic_ready_abi:1`, `package_flavor:release`,
`broker_feature:release-package` and the exact three-patch map.
Normal build tags remain `with_gvisor`, dependency mode `vendor`,
CGO disabled, trimpath enabled and upstream version linker value v1.19.31.
Do not infer conditional-close ABI or import close-family build flags.
The producer requires all three named Go router tests to actually run/pass,
not a successful zero-test command.

New normal staging/inspection requires that family and corresponding-source
regular endpoint/router/test files plus the exact embedded patch digest.
Historical schema1 two-patch receipts and immutable payloads remain history;
they are not retrospectively readiness-qualified. Experimental and close/K1
producer maps/recipes are unchanged. Runtime compatibility for those families
must be explicit and separate, never a missing-endpoint fallback.

Source tests include Suspend/Inner/Running, rejected query/body/method input,
and the actual controller CORS middleware. Source-only gates do not establish
installed readiness or protocol interoperability. Both native architectures,
new exact app/DNS archive identities and matching installed acceptance remain
required before selecting a new delivery.
