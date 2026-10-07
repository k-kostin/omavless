#!/bin/bash
# Fixed reviewed client-feature selection for the current internal assembly.
# Pure output only; never activates a service, edits data or installs anything.
set -euo pipefail
[[ $# == 1 ]] || exit 2
bash "$(dirname "$0")/version-mode.sh" "$1" >/dev/null
# Scope belongs to this beta, not all future versions or stable/RC packages.
if [[ $1 == 0.9.8-beta.4 ]]; then
  printf '%s\n' t4-manager-actor-service
fi
