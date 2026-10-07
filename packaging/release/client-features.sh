#!/bin/bash
# Fixed reviewed client-feature selection for the current internal assembly.
# Pure output only; never activates a service, edits data or installs anything.
set -euo pipefail
[[ $# == 1 ]] || exit 2
bash "$(dirname "$0")/version-mode.sh" "$1" >/dev/null
# These exact selections preserve normal Backup/Restore when freezing the RC.
# Other future versions and stable packages need their own explicit selection.
if [[ $1 == 0.9.8-beta.4 || $1 == 0.9.8-rc.1 ]]; then
  printf '%s\n' t4-manager-actor-service
fi
