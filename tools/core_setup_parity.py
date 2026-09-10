#!/usr/bin/env python3
"""Effect-free actual Python setup inventory oracle. Synthetic booleans only."""
import json
from pathlib import Path
import subprocess
import sys
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import backend


def main():
    results = []
    names = ("cap_net_admin", "cap_net_raw", "cap_net_bind_service")
    for mask in range(8):
        caps = ",".join(name for bit, name in enumerate(names) if mask & (1 << bit))
        output = f"/synthetic/mihomo {caps}=ep\n" if caps else ""
        with mock.patch.object(backend, "find_core", return_value=Path("/synthetic/mihomo")), \
             mock.patch.object(backend.shutil, "which", return_value="/synthetic/getcap"), \
             mock.patch.object(backend, "run", return_value=subprocess.CompletedProcess([], 0, output, "")):
            results.append(backend.core_setup_status(None)["tunReady"])
    print(json.dumps(results))


if __name__ == "__main__":
    try:
        main()
    except Exception:
        print("Core setup oracle failed", file=sys.stderr)
        sys.exit(1)
