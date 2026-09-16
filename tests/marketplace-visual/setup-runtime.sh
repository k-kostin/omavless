#!/bin/bash
# Presence projection ONLY. This fixture never implements installation.
set -euo pipefail
[[ $# == 1 && $1 == components ]] || exit 64
printf 'ready\tpresent\n'
