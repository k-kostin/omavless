#!/usr/bin/env python3
"""Actual Python route fast paths, no controller/socket/service effects."""
import hashlib
import json
import sys
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import backend


class LiveRequired(Exception):
    pass


def main():
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
            canonical = json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode()
            results.append({'kind':'result','digest':hashlib.sha256(canonical).hexdigest()})
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
