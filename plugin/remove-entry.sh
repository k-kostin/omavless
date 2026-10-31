#!/bin/bash
# SPDX-License-Identifier: MIT
# Fixed local entry only. All actual consent/effects/continuation belong to
# the existing reviewed standalone helper; no download or caller arguments.
set -euo pipefail
export PATH=/usr/bin:/bin LC_ALL=C
readonly OV_REMOVAL_HELPER_SHA=9649bb0f503f6ef4ddb94bd0807ac7e09828b7a9703a7bdcb879d1e71d52044a

checked_removal_helper() {
  [[ $# == 0 ]] || return 2
  local script=${BASH_SOURCE[0]} directory root helper path parent owner mode links size actual
  [[ "$script" == /* && -f "$script" && ! -L "$script" ]] || return 1
  directory=$(dirname -- "$script")
  root=$(dirname -- "$directory")
  [[ "$directory" == "$root/plugin" && $(realpath -e -- "$root") == "$root" ]] || return 1
  helper="$root/tools/uninstall.sh"
  for path in "$directory" "$root/tools"; do
    parent=$path
    while :; do
      [[ -d "$parent" && ! -L "$parent" ]] || return 1
      read -r owner mode < <(stat -c '%u %a' -- "$parent")
      [[ "$owner" == 0 || "$owner" == "$(id -u)" ]] || return 1
      [[ "$mode" =~ ^[0-7]{3,4}$ ]] && (( (8#$mode & 0022) == 0 )) || return 1
      [[ "$parent" != / ]] || break
      parent=$(dirname -- "$parent")
    done
  done
  for path in "$script" "$root/manifest.json" "$helper"; do
    [[ -f "$path" && ! -L "$path" ]] || return 1
    read -r owner mode links size < <(stat -c '%u %a %h %s' -- "$path")
    [[ "$owner" == 0 || "$owner" == "$(id -u)" ]] || return 1
    [[ "$mode" == 644 || "$mode" == 755 ]] && [[ "$links" == 1 && "$size" -le 65536 ]] || return 1
  done
  [[ $(stat -c %s -- "$root/manifest.json") -le 8192 ]] || return 1
  jq -e '.schemaVersion == 1 and .id == "kdk.omavless"' "$root/manifest.json" >/dev/null 2>&1 || return 1
  actual=$(sha256sum -- "$helper") || return 1
  [[ ${actual%% *} == "$OV_REMOVAL_HELPER_SHA" ]] || return 1
  printf '%s\n' "$helper"
}

if [[ ${BASH_SOURCE[0]} == "$0" ]]; then
  [[ $# == 0 ]] || { printf '%s\n' 'Run this removal entry without arguments.' >&2; exit 2; }
  helper=$(checked_removal_helper) || {
    printf '%s\n' 'Removal helper is missing, redirected or modified. No removal was started.' >&2
    exit 1
  }
  # Anchor the checked member while the terminal launches it; do not resolve
  # an arbitrary command, eval a path or fall back to a downloaded helper.
  exec {ov_remove_fd}< "$helper"
  [[ $(stat -Lc '%d:%i:%u:%a:%h:%s' -- "/proc/$$/fd/$ov_remove_fd") == $(stat -c '%d:%i:%u:%a:%h:%s' -- "$helper") ]] || exit 1
  actual=$(sha256sum -- "/proc/$$/fd/$ov_remove_fd")
  [[ ${actual%% *} == "$OV_REMOVAL_HELPER_SHA" ]] || exit 1
  exec /bin/bash "/proc/self/fd/$ov_remove_fd"
fi
