# R5 startup validation snapshot boundary

The current native startup validator now captures store/template inputs once
and renders from that exact snapshot. The rendering seam validates connected
desired state, template bounds and canonical profile/mode configuration without
reading mutable files. Tests compare Routing, Full VPN and Direct output to the
existing canonical domain renderer. Python remains installed owner/oracle;
this does not register native login or switch the frontend.

NNP/capability checks, fixed Mihomo arguments, three-second validation timeout
and the existing data directory policy remain unchanged. The temporary config
uses exclusive private creation. Its original descriptor remains held through
validation; cleanup verifies type, ownership, mode and inode identity and refuses
to delete a replacement. A collision is not permission to overwrite a file.
No private input is returned in error output or debug formatting.

## Important: this is not isolated core validation

The snapshot seam is **not yet suitable for the read-only login transaction's
production host adapter**. The existing validator still passes the persistent
data directory, and canonical templates can retain resource fields.

Pinned upstream evidence for installed Mihomo v1.19.30:

- [`main.go`](https://github.com/MetaCubeX/mihomo/blob/v1.19.30/main.go)
  runs configuration parsing for `-t`; it is not a filesystem/network sandbox.
- [`config/config.go`](https://github.com/MetaCubeX/mihomo/blob/v1.19.30/config/config.go)
  explicitly describes geodata loading/downloading during rule parsing.

The checked-in opt-in `test_mihomo_validation_effects.py` provides a controlled
counterexample: GEOSITE validation attempts an HTTP request to a synthetic
loopback server even with `-t` and a fresh temporary data directory. The server
returns 503; validation fails. No provider credentials, installed config,
external network target, TUN or route change is used. Output is discarded;
only a request-observed boolean and exit status are asserted. This proves that
scratch `-d` alone is not a no-network guarantee. It does not claim every
resource or every core version behaves identically. An initial GEOIP/MMDB-only
probe completed without triggering the expected rejection; that case was not
used as positive download evidence.

Run the effect probe explicitly with
`OMAVLESS_TEST_MIHOMO=/usr/bin/mihomo python3 -m unittest -v tests/test_mihomo_validation_effects.py`.
Without the opt-in it skips, including ordinary cloud execution.

## Next integration boundary

Before wiring login readiness, establish bounded resource-aware isolated
validation or explicitly refuse cases that cannot be validated safely. Do not
silently remove resource fields and call the original config validated. Trusted
login ordering, legacy conversion, packaged activation and frontend cutover
remain separate work; #178/#196/R5/R6 are not completed here.

Declared local gate is snapshot/private-file regression, existing native/core
tests and the controlled installed-core loopback effect probe on Try Omarchy
ARM64. No installed plugin, actual profile or system service is modified.
