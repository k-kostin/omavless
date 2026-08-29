#!/bin/bash
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"

python3 -m unittest -v \
  "$here/test_backend.py" \
  "$here/test_control_protocol.py" \
  "$here/test_control_protocol_probe.py"
bash "$here/test-qml.sh"
if command -v omarchy >/dev/null 2>&1; then
  omarchy plugin validate "$here/.."
fi
