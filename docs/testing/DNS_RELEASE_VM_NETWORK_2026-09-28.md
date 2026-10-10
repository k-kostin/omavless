# Managed DNS pair: isolated VM network diagnostic (2026-09-28)

This is agent-run development evidence, **not** owner-attended release
acceptance. No private profile, subscription URL, server address, SNI, public
exit IP, raw runtime log or screenshot is retained here.

## Exact candidate and environment

- Frontend source: `4a7f171617530031f6d4c129c3d07e65816efdc3` (stacked
  Draft #312); application and DNS package source:
  `b739ac6a279981ffde3586d5e70e44a6be43ba70` (stacked Draft #310).
- Installed x86_64 `omavless` package SHA-256:
  `9716fe089e08550f382deca4f2dfdafefed76ffddacf37b3b5d0ab23b312b9b7`;
  `omavless-dns` package SHA-256:
  `cf28cfb6cc2016f278419dd22bac5944680ceec58e41e60c3bde5d51866eafb6`.
  Both are unpublished offline candidate archives, not authenticated release
  downloads.
- Disposable clean x86_64 Omarchy VM, default-deny incoming UFW. The physical
  PC and its active VPN were not changed.

## Observations

1. Native owner and managed DNS pair reached ready. Subscription probe
   completed for 35 servers; six answered fixed HTTPS probes (81–206 ms).
   This checks individual server paths without proving active-TUN reachability.
2. Two standalone test profiles could show confirmed Connected/TUN/DNS facts,
   yet their local mixed-proxy HTTPS attempts did not complete. They were not
   used as working-server acceptance evidence.
3. One responsive subscription profile reached confirmed Connected in Full
   VPN. HTTPS through its local mixed proxy returned 200, but HTTPS routed
   through `Meta` timed out. The built-in current-route HTTPS check reported
   `request_failed`. The route selected `Meta`, whose local IPv4 address was
   `198.18.0.1/30`.
4. With explicit VM-only administrator authorization, a temporary UFW rule
   limited to inbound `Meta` traffic destined for `198.18.0.1` restored
   IP-based TUN HTTPS, DNS-name HTTPS and the built-in HTTPS check. The latter
   returned `ok` in 850 ms in a repeat cycle. Restricting source to the
   configured TUN peer had not worked; inbound return packets have other
   source addresses.
5. In this temporary condition, Rule, Direct and Full VPN modes each reached
   confirmed Connected and HTTPS 200. Switching to a second responsive
   subscription profile preserved confirmed Connected and HTTPS 200.
6. OmaVLESS was disconnected. Its desired and actual state were both
   Disconnected, the broker FD store was zero, `Meta` disappeared, and the
   temporary UFW rule was deleted. No firewall exception remains in this VM.

The successful HTTPS checks cover these selected IPv4 targets and two
responsive servers only. They do not establish IPv6, UDP, every server,
no-leak behavior, or formal owner-attended acceptance. Normal setup must not
silently modify a firewall. The administrator-facing prerequisite and its
reversal are described in [native installation](../user/NATIVE_INSTALL.md).
