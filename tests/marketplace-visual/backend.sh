#!/bin/bash
# Synthetic read-only transport for the isolated screenshot harness ONLY.
set -euo pipefail
[[ $# == 1 ]] || exit 64
case "$1" in
  status|native-observation|native-desktop-capabilities|native-core-readiness|native-startup-capabilities)
    exec /usr/bin/cat "$(dirname -- "$0")/fixtures/$1.json" ;;
  *) exit 64 ;;
esac
