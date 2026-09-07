#!/usr/bin/env python3
"""Effect-isolated Python file-export oracle; only content hashes escape."""
import hashlib
import json
import sys
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import backend


def main():
    raw = sys.stdin.buffer.read(8 * 1024 * 1024 + 1)
    if len(raw) > 8 * 1024 * 1024:
        raise ValueError
    cases = json.loads(raw)
    if not isinstance(cases, list) or len(cases) > 256:
        raise ValueError
    results = []
    for case in cases:
        try:
            captured = []
            with patch.object(backend, "load_store", return_value=backend.validate_store(case["store"])), \
                 patch.object(backend, "atomic_write", side_effect=lambda _path, text: captured.append(text)):
                backend.export_file(None, case["id"], "/unused-synthetic-destination")
            if len(captured) != 1:
                raise ValueError
            results.append(hashlib.sha256(captured[0].encode()).hexdigest())
        except backend.BackendError:
            results.append(None)
    print(json.dumps(results))


if __name__ == "__main__":
    try:
        main()
    except Exception:
        print("Profile export oracle failed", file=sys.stderr)
        sys.exit(1)
