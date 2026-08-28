# Live protocol validation

This opt-in V0 harness records repeatable, credential-free evidence for the
experimental protocol adapters already present in OmaVLESS 0.7.0. It does not
implement a new protocol and it does not replace manual inspection of the
generated configuration, service or TUN on the Omarchy machine.

## Security boundary

- Import real profiles through the normal OmaVLESS UI before using the harness.
- Never put a profile URI, password, UUID, key, subscription URL or provider
  name in the cases file.
- The cases file contains only OmaVLESS's local profile ID plus bounded public
  classifications. It must be owned by the current user, be a regular file,
  have mode `0600`, and must not be committed.
- The harness never writes profile IDs, names, endpoints or raw backend errors
  to its results. Results contain only case slugs, protocol/feature classes,
  pass/fail stages, bounded error codes and the installed core version.
- The HTTPS probe URL must contain no credentials, query or fragment. Only its
  hostname is written to the results.
- The runner switches to Full VPN (`global`) for each case, disconnects after
  every probe, and attempts to restore the previous routing mode and active
  profile. A `manual-recovery-required` result is a hard failure.

## Prepare local cases

1. Back up `~/.config/omavless/` and disconnect unrelated VPN clients.
2. Import representative profiles through the OmaVLESS UI. Use at least two
   independent servers/providers where the roadmap requires independence.
3. Copy `tests/live-protocol-cases.example.json` outside the repository, replace
   the example local profile IDs, and set mode `0600`:

   ```bash
   cp tests/live-protocol-cases.example.json ~/.config/omavless/v0-cases.local.json
   chmod 600 ~/.config/omavless/v0-cases.local.json
   ```

Use generic case IDs. Link the same Hysteria2 profile tested on UDP-friendly and
UDP-restricted networks with a generic `pairId`. A restricted-network case may
set `expectSuccess` to `false` and list `connect`/`probe` as expected failure
stages; this records the controlled failure but does not prove its cause by
itself, so the paired successful run remains required.

## Run

Run the repository checks first, then the explicit live command:

```bash
omarchy plugin validate "$PWD"
./tests/run.sh
python3 tests/live_protocol_validation.py \
  --cases ~/.config/omavless/v0-cases.local.json \
  --output ~/.config/omavless/v0-results.local.json \
  --mihomo ~/.local/bin/mihomo \
  --confirm-live
```

The output is created atomically with mode `0600`. Do not attach the private
cases file to a PR. Before attaching or summarizing results, inspect the output
and report only the generic result matrix. Also record separately:

- exact OmaVLESS and Mihomo commits/versions;
- Omarchy version and whether the network was UDP-friendly or intentionally
  UDP-restricted;
- `omavless.service`/Mihomo child relationship and private Unix controller;
- absence of a TCP external-controller listener;
- generated-config validation and Full VPN / Routing / Direct regression;
- manual protocol-specific observations required by `PROTOCOL_ROADMAP.md`.

V0 remains incomplete until the live matrix includes representative VLESS
XHTTP modes, VLESS Encryption/REALITY PQ, Trojan, Hysteria2 and TUIC v5 evidence.
