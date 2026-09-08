#!/usr/bin/env python3
"""Actual Python route fast paths, no controller/socket/service effects."""
import hashlib
import ipaddress
import json
import sys
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import backend


class LiveRequired(Exception):
    pass


def normalize_mapped_query(result):
    """Only normalize Python-version-dependent mapped IPv6 display spelling.

    Python <=3.13 prints hexadecimal suffixes; 3.14 prints dotted IPv4. Both
    denote exactly the same address. Native retains the established hex form.
    No routing outcome, rule payload or other result field is normalized.
    """
    try:
        address = ipaddress.ip_address(result['query'])
    except ValueError:
        return 'none'
    if isinstance(address, ipaddress.IPv6Address) and address.ipv4_mapped:
        number = int(address.ipv4_mapped)
        result['query'] = f'::ffff:{number >> 16:x}:{number & 0xffff:x}'
        return 'mapped_ipv6_display'
    return 'none'


def main():
    for spelling in ('::ffff:192.0.2.1', '::ffff:c000:201'):
        sample = {'query': spelling, 'outcome': 'direct'}
        if normalize_mapped_query(sample) != 'mapped_ipv6_display' or sample != {
                'query': '::ffff:c000:201', 'outcome': 'direct'}:
            raise ValueError
    raw = sys.stdin.buffer.read(2 * 1024 * 1024 + 1)
    if len(raw) > 2 * 1024 * 1024:
        raise ValueError
    cases = json.loads(raw)
    if not isinstance(cases, list) or len(cases) > 256:
        raise ValueError
    results = []
    for case in cases:
        store = backend.empty_store()
        store['customRules'] = case['rules']
        try:
            with patch.object(backend, 'load_store', return_value=store), \
                    patch.object(backend, 'service_active', return_value=case['connected']), \
                    patch.object(backend, 'routing_status', return_value={'mode':case['mode']}), \
                    patch.object(backend, 'live_route_match', side_effect=LiveRequired):
                result = backend.route_check(None,case['query'])
            normalization = normalize_mapped_query(result)
            canonical = json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode()
            results.append({'kind':'result','digest':hashlib.sha256(canonical).hexdigest(),
                            'normalization':normalization})
        except backend.BackendError:
            results.append({'kind':'invalid'})
        except LiveRequired:
            results.append({'kind':'live_required'})
    print(json.dumps(results))


if __name__ == '__main__':
    try:
        main()
    except Exception:
        print('Route-check oracle failed',file=sys.stderr)
        sys.exit(1)
