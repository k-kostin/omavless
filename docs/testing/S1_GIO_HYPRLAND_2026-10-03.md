# S1 default GIO consumer limitation on Hyprland

Physical x86_64 Omarchy/Linux, private fixture only; 2026-10-03 Moscow
(2026-10-02 UTC). Source context: Draft #543 at
`1f979998c0347ac5e47acc1ad13bf6880be22415`. This additive environment probe
did not modify or execute the staged journal. The implementation's separate
exact-source tests remain attributed to
`4ffe550f9548ce96e443a7845193af35065333f7` in the
[staged private evidence](S1_STAGED_PRIVATE_2026-10-03.md). A documentation-only
follow-up does not turn this probe into a new code-head transaction gate.

**Result:** the installed default GIO consumer does not consume the private
GNOME desktop Manual settings under the actual Hyprland desktop selectors.
The probe refused before sending an application request. App proxy remains
unavailable; no successful consumer integration or installed activation is claimed.

## Installed dependencies and selection

Installed: GLib/GIO 2.88.3, glib-networking 2.80.1, libproxy 0.5.12,
dconf 0.49.0 and gsettings-desktop-schemas 50.1. The GIO module cache advertises
both `libgiognomeproxy.so` and `libgiolibproxy.so` as proxy resolvers and the
installed dconf settings backend. Module presence is not applicability proof.

The observed desktop selectors were `XDG_CURRENT_DESKTOP=Hyprland`,
`XDG_SESSION_DESKTOP=Hyprland`, `DESKTOP_SESSION=omarchy`, without a
`GIO_USE_PROXY_RESOLVER` override. Private child processes retained those
selectors and called real `Gio.ProxyResolver.get_default()` through installed
GJS. The selected object was `GLibproxyResolver`; `is_supported()` returned
true. That Boolean establishes resolver availability, not consumption of a
particular settings surface.

Matching upstream code explains this selection and result:

- [libproxy 0.5.12's GNOME configuration plugin](https://github.com/libproxy/libproxy/blob/0.5.12/src/backend/plugins/config-gnome/config-gnome.c)
  enables the GNOME settings surface only for desktop strings containing GNOME,
  MATE, Pantheon or Cinnamon. Hyprland does not match.
- [glib-networking 2.80.1's GNOME resolver](https://github.com/GNOME/glib-networking/blob/2.80.1/proxy/gnome/gproxyresolvergnome.c)
  reports support only when the desktop selector contains GNOME. Installing its
  module therefore does not make it the ordinary Hyprland default resolver.
- [GIO's default-resolver API](https://docs.gtk.org/gio/type_func.ProxyResolver.get_default.html)
  supplies the selected resolver, rather than an application-constructed proxy
  resolver with the expected endpoint embedded in it.

## Isolated method and actual result

The child environment was cleared and rebuilt with private HOME, XDG
config/cache/data/runtime directories, short temporary storage and a private
`DCONF_PROFILE` containing the standard `user-db:user` fixture declaration.
The final probe used an owned custom session bus with no service directories
or automatic activation and an explicitly spawned owned dconf service, as in
#543. A nonexistent private system-bus address prevented host system-bus access.
`GIO_USE_VFS=local` avoided unrelated GVfs activation; it did not select a proxy
resolver. No proxy environment assignment, GNOME desktop spoofing,
`GIO_USE_PROXY_RESOLVER` override or `GSimpleProxyResolver` was used. No host
system proxy settings or manager environment were read.

Each consumer observation ran in a fresh process. A synthetic socket was bound
and listening on IPv4 loopback with an ephemeral port. This was a TCP stub,
**not** a protocol-ready HTTP proxy or a product listener-readiness proof.
While private mode was None, the fixture set that HTTP endpoint, disabled HTTP
authentication and set an empty bypass list; it then set Manual mode last.
Fresh independent readback verified persisted Manual, the endpoint and empty
bypass before the resolver lookup. The lookup input was a public numeric
documentation-address HTTP URI, with no hostname resolution or application
connection requested. No PAC URL was supplied or evaluated.

| Private state independently read | Default resolver | Lookup selection |
| --- | --- | --- |
| None; all five touched overrides absent | `GLibproxyResolver` | Only `direct://` |
| Manual; loopback HTTP endpoint present; bypass empty | `GLibproxyResolver` | Only `direct://` |
| Restored None; all five touched overrides absent | `GLibproxyResolver` | Only `direct://` |

The Manual preflight refused instead of issuing a direct application request.
There were zero application requests or proxy handshakes; no external/provider
request or traffic routing is claimed. Resetting only the five originally
absent private overrides restored their exact absence; a fresh process checked
both that absence and effective None. Owned service/bus children were stopped
and reaped, and the loopback socket closed. No packet-capture gate was run.

This preflight stopped before journal/consumer composition. It does not prove
a None→Manual staged transaction serving traffic, missing-listener refusal,
prior Manual/PAC consumer restoration, cached-client quiescence or UWSM
environment consumption. The earlier journal/write/drain guarantees are neither
invalidated nor promoted by this result.

## Conservative applicability and consumer follow-up

Before a future installed coordinator can admit this surface:

1. Declare the supported application/desktop/resolver/backend class and verify
   its actual default selection in the exact session and settings profile.
   Neither schema/module presence, `is_supported()` nor setting equality is
   sufficient. Unsupported default semantics must refuse; do not repair the
   result by silently changing desktop selectors or forcing a resolver module.
2. Independently establish the existing owner/revision-bound exclusive loopback,
   TUN-disabled listener and configuration-readback prerequisites. A bound TCP
   socket is not protocol readiness or owned Mihomo authority. Verify actual
   synthetic traffic through that listener with the real default consumer.
   In the declared empty-bypass fixture, Manual must not be counted successful
   after a `direct://` selection or direct fallback; direct requests belong only
   to deliberately tested None state. This is not a product kill-switch claim.
3. Compose the admitted consumer with the staged journal and retained-origin
   settlement fences, then test complete saved None/Manual/PAC restoration,
   listener loss and external edits. PAC consumption requires separately
   controlled private PAC machinery; do not fetch saved/private PAC URLs or
   invoke uncontrolled discovery to manufacture acceptance. Fresh-client proof
   must remain distinct from already-running clients that cache settings.
4. Choose an explicitly reviewed consumer strategy: a genuinely supported
   desktop adapter, or delivery through an applicable consumer environment/API.
   Environment delivery needs its own user-manager/UWSM/broker/session provenance,
   new-app and exact absent-versus-empty restoration gates. It cannot be inserted
   into #543's desktop-only plan, whose manager fields must remain unchanged.
   Passing an endpoint directly to a client proves that configured-client path,
   not consumption of GNOME settings by the normal default resolver.

These are requirements for a later review, not new admission code, policy or
authorization. This result does not declare every possible S1 strategy impossible.
Installed AUTH-writer/session/lifetime provenance and conflict escape remain open.

No production constructor, receipt, method, UI, environment injection, package
installation, shared target compilation, VM, normal desktop proxy/environment,
VPN/TUN/route setting, main/RC merge, release or marketplace change occurred.
