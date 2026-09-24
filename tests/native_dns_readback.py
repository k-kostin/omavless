#!/usr/bin/python3
# SPDX-License-Identifier: MIT
"""Read-only developer evidence for the fixed Linux/systemd DNS fixture.

Not a production readiness implementation, authorization result, leak test or
privileged helper. No writes, policy changes, credentials or private addresses
are returned. A matching resolved property cannot prove who authorized it.
"""
import json
import importlib.util
from pathlib import Path
import re
import subprocess
import time

LIMIT = 16384
LINK_LIMIT = 1024 * 1024
FIXED_DNS = [[2, [198, 18, 0, 2]]]
PROPERTIES = ('Domains', 'DNS', 'DefaultRoute')


class Unavailable(Exception):
    def __init__(self):
        super().__init__('dns_readback_unavailable')


def require(ok):
    if not ok:
        raise Unavailable()


def decode(raw, limit=LIMIT):
    require(isinstance(raw, bytes) and len(raw) <= limit)
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result)
            result[key] = value
        return result
    try:
        return json.loads(raw.decode('utf-8'), object_pairs_hook=pairs,
            parse_constant=lambda _: (_ for _ in ()).throw(Unavailable()))
    except (ValueError, UnicodeError, RecursionError):
        raise Unavailable() from None


def tun_index(raw):
    rows = decode(raw, LINK_LIMIT)
    require(isinstance(rows, list) and len(rows) <= 1024)
    candidates = []
    for row in rows:
        require(isinstance(row, dict))
        info = row.get('linkinfo', {})
        require(isinstance(info, dict))
        if info.get('info_kind') == 'tun':
            index = row.get('ifindex')
            require(type(index) is int and 0 < index <= 2**31-1)
            candidates.append(index)
    require(len(candidates) <= 1)
    return candidates[0] if candidates else None


def property_data(raw, signature):
    value = decode(raw)
    require(isinstance(value, dict) and set(value) == {'type', 'data'}
        and value['type'] == signature)
    return value['data']


def link_path(raw):
    value = property_data(raw, 'o')
    require(isinstance(value, list) and len(value) == 1 and isinstance(value[0], str)
        and len(value[0]) <= 256 and re.fullmatch(
            r'/org/freedesktop/resolve1/link/[A-Za-z0-9_]+', value[0]) is not None)
    return value[0]


def project(domains_raw, dns_raw, route_raw):
    domains = property_data(domains_raw, 'a(sb)')
    dns = property_data(dns_raw, 'a(iay)')
    route = property_data(route_raw, 'b')
    require(isinstance(domains, list) and len(domains) <= 64 and type(route) is bool)
    for row in domains:
        require(isinstance(row, list) and len(row) == 2
            and isinstance(row[0], str)
            and type(row[1]) is bool)
        try:
            require(len(row[0].encode('utf-8')) <= 253)
        except UnicodeError:
            raise Unavailable() from None
    require(isinstance(dns, list) and len(dns) <= 64)
    for row in dns:
        require(isinstance(row, list) and len(row) == 2
            and type(row[0]) is int and row[0] in (2, 10)
            and isinstance(row[1], list) and len(row[1]) == (4 if row[0] == 2 else 16)
            and all(type(v) is int and 0 <= v <= 255 for v in row[1]))
    # Match this documented fixed fixture, not arbitrary provider DNS settings.
    return {'available': True, 'managedLinkPresent': True,
        'rootRoutingDomain': domains == [['.', True]],
        'dnsDefaultRoute': route, 'fixedTunDns': dns == FIXED_DNS}


def capture(argv, timeout, limit):
    # Reuse the bounded pipe reader, never communicate() on unbounded output.
    spec = importlib.util.spec_from_file_location('dns_package_reader',
        Path(__file__).with_name('installed_native_package.py'))
    package = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(package)
    return package.capture(argv, timeout=timeout, limit=limit)


def observe(command=capture):
    """Read one TUN's properties, fence index across reads, return booleans only.

    The index fence catches ordinary disappearance/change; it is NOT a secure
    non-reuse lease and must never be reused to authorize privileged writes.
    Commands share a five-second total deadline. No authorization is requested.
    """
    deadline = time.monotonic() + 5
    def run(argv, limit=LIMIT):
        remaining = deadline - time.monotonic()
        require(remaining > 0)
        return command(argv, timeout=remaining, limit=limit)
    try:
        links = ['/usr/bin/ip', '-d', '-j', 'link', 'show']
        index = tun_index(run(links, LINK_LIMIT))
        if index is None:
            return {'available': True, 'managedLinkPresent': False}
        path = link_path(run(['/usr/bin/busctl', '--system', '--json=short', 'call',
            'org.freedesktop.resolve1', '/org/freedesktop/resolve1',
            'org.freedesktop.resolve1.Manager', 'GetLink', 'i', str(index)]))
        values = [run(['/usr/bin/busctl', '--system', '--json=short', 'get-property',
            'org.freedesktop.resolve1', path, 'org.freedesktop.resolve1.Link', name])
            for name in PROPERTIES]
        result = project(*values)
        require(tun_index(run(links, LINK_LIMIT)) == index)
        return result
    except Exception:
        return {'available': False}


def classify(observation, readback, human):
    """Human receipt is necessary for a cancellation/delay claim, never inferred."""
    require(human in ('accepted', 'cancelled', 'delayed-accepted', 'none'))
    if human == 'none':
        return 'authorization_not_observed'
    if not isinstance(readback, dict) or readback.get('available') is not True:
        return 'dns_readback_unavailable'
    if readback.get('managedLinkPresent') is not True:
        return 'managed_link_absent'
    matches = all(readback.get(k) is True for k in
        ('rootRoutingDomain', 'dnsDefaultRoute', 'fixedTunDns'))
    claimed = isinstance(observation, dict) and observation.get('lastKnownActual') == 'connected'
    if human == 'cancelled':
        if claimed and not matches:
            return 'connected_with_unconfirmed_dns_after_cancel'
        return 'cancelled_but_dns_matches' if matches else 'cancelled_dns_not_ready'
    return 'accepted_dns_matches' if matches else 'accepted_dns_not_ready'


if __name__ == '__main__':
    print(json.dumps(observe(), sort_keys=True))
