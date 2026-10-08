#!/bin/bash
# Fixed reviewed client-feature selection for the current internal assembly.
# Pure output only; never activates a service, edits data or installs anything.
set -euo pipefail
[[ $# == 1 ]] || exit 2
bash "$(dirname "$0")/version-mode.sh" "$1" >/dev/null
# The selected 0.9.8 release is Backup-only. Full Restore stays in development.
# Other future versions and stable packages need their own explicit selection.
if [[ $1 == 0.9.8-beta.4 ]]; then
  printf '%s\n' t4-manager-actor-service
elif [[ $1 == 0.9.8-rc.1 || $1 == 0.9.8 ]]; then
  printf '%s\n' product-private-backup
fi
