#!/bin/bash
# SPDX-License-Identifier: MIT
# Committed frontend selection, never an environment/caller-supplied channel.
setup_channel() {
  [[ $# == 0 ]] || return 2
  local directory channel manifest metadata
  directory=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P) || return 1
  channel="$directory/setup-channel.json"
  if [[ ! -e "$channel" && ! -L "$channel" ]]; then printf 'public\n'; return; fi
  [[ -f "$channel" && ! -L "$channel" ]] || return 1
  metadata=$(stat -c '%u:%a:%h:%s' -- "$channel") || return 1
  [[ "$metadata" == "$(id -u):644:1:77" || "$metadata" == '0:644:1:77' ]] || return 1
  # Byte-exact closed DATA also refuses duplicate keys and unknown fields.
  printf '%s\n' '{"schemaVersion":1,"kind":"unpublished-offline-candidate","version":"0.9.9"}' \
    | cmp -s -- - "$channel" || return 1
  manifest="$directory/../manifest.json"
  [[ -f "$manifest" && ! -L "$manifest" && $(stat -c %s -- "$manifest") -le 8192 ]] || return 1
  jq -e '.schemaVersion == 1 and .id == "kdk.omavless" and .version == "0.9.9"' \
    "$manifest" >/dev/null 2>&1 || return 1
  printf 'offline-candidate\n'
}
