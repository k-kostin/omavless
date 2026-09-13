#!/bin/bash
# SPDX-License-Identifier: MIT
# Release archive entry point. No legacy installation or automatic cutover.
set -euo pipefail
[[ $# -eq 0 ]] || { echo 'Usage: ./install.sh' >&2; exit 2; }
bundle_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
exec /bin/bash "$bundle_dir/install-frontend.sh" --native-only
