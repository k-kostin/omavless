# User DNS presets — selected 0.9.9 candidate

Status: implementation candidate, October29. Source tests and isolated numeric
core feasibility are separate from new-package, installed and final delivery
acceptance. This does not change published main or qualify K1/App Proxy.

## User outcome

Settings offers Preset default, Google and Cloudflare. Changing a choice saves
application configuration only while the runtime is independently settled Off.
The next fresh connection uses it, including a previously authorized login
autoconnect; saving never connects, changes startup preferences or reloads a
running core. Direct is an active connection mode, so it also requires Disconnect
before saving. No pending/applied preference state or extra startup authority is
introduced.

Preset default applies no override: it retains the currently selected routing
template's DNS. A custom template retains its own DNS and is read-only here.
The saved choice remains available if a bundled routing preset is selected later.
WG/AWG imported per-outbound DNS retains its existing ordered IPv4 / omitted /
unsupported IPv6 or mixed-address contract.

## Boundary and representation

The optional private-store field `dnsPreset` is a closed string enum:
`preset-default`, `google`, `cloudflare`. Absence means default and is never
materialized by reading or normalizing an old store. An unchanged save preserves
its exact input bytes. Present null, wrong types and unknown IDs refuse. The
named field is admitted in strict v3/v4 Backup shapes; encrypted archives retain
it, and Restore's existing startup-Off conversion does not erase it. There is
no store-version migration or new file, journal or dependency.

`dns.configure` requires exactly `preset`, `operationId`, `expectedRevision`.
The plugin bridge adds its existing instance fence as `dns-configure`; fixed CLI
`omavless plugin dns-configure INSTANCE REVISION OPERATION` accepts the ID on
bounded stdin. Unknown fields/IDs and duplicate wire keys refuse. Ordinary
owner, migration, recovery, revision, operation collision and replay gates apply.
Before any store replacement, the owner reads desired intent and independently
observes settled Off. Only the compensated private-store writer commits. It does
not mutate template, Desired, controller, core, service or broker policy.

`ui.snapshot.dns` contains saved `preset` and template `source` from the fixed
private template read: `bundled`, `custom` or `unavailable`. This is saved intent
and edit eligibility, not active DNS or network health. Missing/unsafe template
reads are unavailable. Older snapshots without this extension disable the new
controls rather than inventing custom/default availability.

## Transient rendering

Exact ORIGINAL template identity is checked before any overlay: the three
checked-in bundles and their canonical Rule/Global/Direct variants, plus setup's
exact persisted managed-default counterparts. A name, stored routing label or
the mere presence of managed DNS flags grants no eligibility. Existing managed
custom templates retain their accepted bytes.

Only the four DNS list values change in a transient copy:

| ID | default-nameserver | proxy-server-nameserver | direct-nameserver | nameserver |
| --- | --- | --- | --- | --- |
| preset-default | Original | Original | Original | Original |
| google | `8.8.8.8` | `8.8.8.8` | `https://8.8.8.8/dns-query` | `https://8.8.8.8/dns-query#PROXY` |
| cloudflare | `1.1.1.1` | `1.1.1.1` | `https://1.1.1.1/dns-query` | `https://1.1.1.1/dns-query#PROXY` |

TUN, listener, fake-IP/filter, IPv6, managed flags, controller and rule/proxy
content remain unchanged. Normal native preparation and isolated startup/login
validation share the DNS transform; validation captures the original resource
manifest first. Generic domain rendering, saved templates, setup selection and
Restore staging do not acquire the overlay. No hot reload, caller URL/YAML,
root DNS API or DNS-health admission is added. The fixed host resolver remains
`198.18.0.2` under the existing managed lifecycle.

## Affected clients and qualification

QML Settings places three explicit value controls beside routing settings. The
selected value remains visible; saving/unknown/stale/custom/active states cannot
dispatch another save. The existing metadata progress/error/unknown surface owns
the response. TUI Settings uses 1/2/3 followed by its normal explicit confirmation;
language/theme/0-reset remain local appearance settings. Both clients refresh
the owner's saved preference rather than optimistically changing selection.

Required source coverage: legacy absence/no-op bytes, invalid schema, strict
Backup v3/v4 and encrypted Restore-Off preservation, exact bundled/managed-default
eligibility, custom rejection, protected rendering and unchanged WG DNS; real IPC
revision/replay/recovery/owner fences; EN/RU bounded projections and client gates.
An unchanged V1 crypto vector is backward-compatibility regression evidence,
not a repeat of installed KDF acceptance.

Before integration, earn the exact new native pair on both architectures, coherent
pins/remover/export, independent source review and affected installed EN/RU QML/TUI
review. Installed evidence must show Off-save without network effects, next
connection's actual DNS path, restart-Off persistence and normal cleanup. The
earlier numeric HTTP A/B fixture is not public HTTPS/bootstrap or installed proof.
No Full VPN/AUTO1 acceptance replay is implied. Physical NIC/suspend claims are
outside this user-DNS change; existing held physical gates remain held.

The owning PR records failures, actual witnesses, package identities and remaining
gates. Normal development merge, independent merged-push CI and a NEW committed
distribution/source-CI/fresh-inspection/FIRST-bootstrap admission still follow.
