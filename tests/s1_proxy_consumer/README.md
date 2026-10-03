# Private S1 child-context consumer gate

Developer-only applicability evidence for the S1 inherited-environment strategy.
Normal Rust runtime, desktop settings, user manager and UWSM are untouched.
Python/GJS here are disposable test tooling, not a production fallback.

```sh
python3 tests/s1_proxy_consumer/private_environment.py \
  --core /absolute/retained/standalone-candidate \
  --scratch-parent /absolute/private/home-scratch
```

The helper requires the separately reviewed standalone candidate SHA-256
`aef9a6f24bde8101f59afbe06c67e159b36e3fcddb24ae6e88c667d4a8b987ff`.
Its source/build provenance belongs to [T3 Draft #541](https://github.com/k-kostin/omavless/pull/541),
not this consumer test or a package receipt. It is not the combined-core #549
binary. No binary/source is built, copied, distributed or installed here;
upstream Mihomo licensing remains GPL-3.0. The new harness is MIT.

The fixture launches two owned core children with fresh private configs and
HOME, loopback-only mixed listeners, DNS/TUN/IPv6/process lookup disabled and
no external controller. A separate owned numeric IPv4 loopback HTTP target
returns exact challenge receipts. HTTP CONNECT and SOCKS5 negotiation plus
complete target responses establish protocol readiness before each proxy
consumer request; a bound/listening socket alone cannot pass.

Fresh installed GJS children call `Gio.ProxyResolver.get_default()` and ordinary
`Gio.SocketClient.connect_to_uri()` for the HTTP **target**, not the proxy.
The default resolver must be `GLibproxyResolver`, supported, and return exactly
the intended proxy with no direct fallback. No resolver object/module override,
GNOME desktop spoofing, manually implemented client proxy handshake or hardcoded
application proxy is used. The negotiated connection's actual remote address
must match the owned proxy port, independently of the target HTTP nonce receipt.
Direct connections are admitted only for deliberate absent/empty baselines.

Child environments are rebuilt, never patched into the parent. The ten S1
lower/uppercase HTTP/HTTPS/FTP/all/no-proxy variables are exactly synthetic and
checked in each child, including absent versus empty. No inherited proxy,
LD/module/PX override or private environment data is copied. Only the three
actual public Hyprland/Omarchy desktop selectors are retained; a different
context refuses before fixture activation. HOME/XDG directories are private;
nonexistent private bus endpoints prevent host bus activation and `GIO_USE_VFS=local`
avoids unrelated GVfs activation without selecting a resolver.

Three original child contexts are covered: all selected variables absent,
HTTP variants present empty, and a synthetic prior manual HTTP proxy. For each,
HTTP and SOCKS target environment consumption succeeds, then a fresh child
uses the exact saved original context again. No already-running child's
environment is rewritten. Missing, lost and bound-only listeners refuse before
consumer launch; a separate actual default-GIO dial after listener loss must
fail specifically during connection, delivering no request directly to target.
Refusal is not inferred from an unrelated resolver/preflight error.

All input values travel through private child environment/stdin, not argv or
logs. Fixed bounded JSON verdicts and a public binary hash are the only output.
Raw GErrors/stderr/HTTP values are discarded. Socket/header/response limits,
child deadlines, owned process cleanup and thread joins bound the fixture;
temporary configs, target sockets and children are removed/reaped. No provider,
PAC, hostname resolution, external request, normal desktop/settings write,
manager/UWSM/service change or VM is used.

## What this does not prove

[UWSM 0.26.7 documents](https://github.com/Vladimir-csp/uwsm/blob/v0.26.7/README.md#3-apps-and-slices)
that scopes (the default `uwsm app` type) inherit their launching parent's
environment, whereas services use the manager activation environment. This
gate proves neither path nor that future manager writes reach ordinary new
Omarchy apps. It is consumer applicability and **child launch-context**
restoration, not journal/manager restoration or repair of existing applications.

The [earlier Hyprland GNOME-settings limitation](../../docs/testing/S1_GIO_HYPRLAND_2026-10-03.md)
still stands. This alternative input surface does not activate or weaken the
desktop-only journal, AUTH-writer/session/lifetime fences, foreign-edit handling,
owned-core package/config admission or conflict escape requirements. HTTP-only
application traffic here does not establish HTTPS, all-proxy, PAC, every
application, cache invalidation, multi-session policy or installed S1 readiness.

[Exact local evidence](../../docs/testing/S1_CHILD_GIO_2026-10-03.md) distinguishes
installed consumer tests from offline harness checks and source CI.
