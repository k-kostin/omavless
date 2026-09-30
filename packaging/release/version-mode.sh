#!/bin/bash
# SPDX-License-Identifier: MIT
# Pure CI version projection. No filesystem, host or publication effects.
set -euo pipefail
[[ $# -eq 1 && ${#1} -le 32 ]] || exit 2
if [[ $1 =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  printf 'stable\t%s\n' "$1"
elif [[ $1 =~ ^[0-9]+\.[0-9]+\.[0-9]+-rc\.[1-9][0-9]*$ ]]; then
  printf 'candidate\t%s\n' "${1/-rc./rc}"
else
  exit 2
fi
