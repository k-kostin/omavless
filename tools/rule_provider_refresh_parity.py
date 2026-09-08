#!/usr/bin/env python3
"""Effect-isolated actual refresh oracle; only synthetic counts leave stdout."""
import contextlib
import io
import json
from pathlib import Path
import sys
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import backend


def main():
    raw = sys.stdin.buffer.read(8193)
    if len(raw) > 8192:
        return 2
    cases = json.loads(raw)
    if not isinstance(cases, list) or len(cases) > 16:
        return 2
    result = []
    for statuses in cases:
        if not isinstance(statuses, list) or len(statuses) > 4 or any(type(x) is not int for x in statuses):
            return 2
        rows = {f"synthetic-{index}": {"vehicleType": "HTTP"} for index in range(len(statuses))}
        writes = []
        calls = []
        class Socket:
            def exists(self):
                return True
        def request(_socket, method, endpoint, timeout):
            if method != "PUT" or timeout != 60:
                raise ValueError("invalid fixed request")
            index = int(endpoint.removeprefix("/providers/rules/synthetic-"))
            calls.append(index)
            return statuses[index], b""
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()), \
                patch.object(backend, "service_active", return_value=True), \
                patch.object(backend, "controller_socket", return_value=Socket()), \
                patch.object(backend, "controller_json", return_value=(200, {"providers": rows})), \
                patch.object(backend, "controller_request", side_effect=request), \
                patch.object(backend, "legacy_mutation_lock", return_value=contextlib.nullcontext()), \
                patch.object(backend, "load_store", return_value={"rulesUpdatedAt": 0}), \
                patch.object(backend, "save_store", side_effect=lambda _paths, store: writes.append(dict(store))), \
                patch.object(backend, "invalidate_status_cache"), \
                patch.object(backend.time, "time_ns", return_value=9000000):
            try:
                outcome = backend.refresh_rule_providers(None)
                ok = True
                updated = outcome["updated"]
            except Exception:
                ok = False
                updated = 0
        result.append({"ok": ok, "updated": updated, "attempted": len(calls), "stamp": bool(writes)})
    print(json.dumps(result, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception:
        sys.exit(2)
