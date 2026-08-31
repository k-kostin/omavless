#!/bin/bash
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"

python3 -m unittest -v \
  "$here/test_backend.py" \
  "$here/test_control_protocol.py" \
  "$here/test_control_protocol_probe.py" \
  "$here/test_control_protocol_parity.py" \
  "$here/test_profile_classification_parity.py" \
  "$here/test_vless_authority_parity.py"
if command -v node >/dev/null 2>&1; then
  node "$here/test-i18n.js"
else
  echo "node unavailable: i18n runtime tests not run" >&2
  exit 1
fi
bash "$here/test-qml.sh"
if command -v omarchy >/dev/null 2>&1; then
  omarchy plugin validate "$here/.."
fi
