#!/bin/sh
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 OmaVLESS contributors
# QML's stable launcher. Native admission is explicit and generation-fenced;
# errors must never fall through to the legacy owner.
blocked() {
  printf '%s\n' 'OmaVLESS frontend ownership is unavailable; no legacy fallback' >&2
  exit 71
}

# A marketplace-only installation does not require the native package. Without
# its canonical selector reader, permit legacy ONLY when ownership artifacts
# are provably absent. Inspect ancestors too: test -e alone hides unreadable
# directories and dangling links. No JSON parsing or state repair here.
legacy_without_native() {
  if [ "${XDG_STATE_HOME+x}" = x ]; then
    state_base=$XDG_STATE_HOME
  else
    case "${HOME-}" in /*) ;; *) return 1 ;; esac
    state_base=$HOME/.local/state
  fi
  case "$state_base" in /*) ;; *) return 1 ;; esac
  [ "${#state_base}" -le 4096 ] || return 1
  remaining=${state_base#/}/omavless
  current=
  while [ -n "$remaining" ]; do
    component=${remaining%%/*}
    case "$remaining" in */*) remaining=${remaining#*/} ;; *) remaining= ;; esac
    case "$component" in '') continue ;; .|..) return 1 ;; esac
    current=$current/$component
    [ ! -L "$current" ] || return 1
    if [ ! -e "$current" ]; then return 0; fi
    [ -d "$current" ] && [ -r "$current" ] && [ -x "$current" ] || return 1
  done
  for artifact in ownership.json frontend-bridge.target; do
    [ ! -e "$current/$artifact" ] && [ ! -L "$current/$artifact" ] || return 1
  done
}

if command -v omavless >/dev/null 2>&1; then
  target=$(omavless plugin target 2>/dev/null) || blocked
  case "$target" in
    legacy) ;;
    rust)
      case "${1-}" in
        native-observation)
          [ "$#" -eq 1 ] || blocked
          exec omavless runtime observation
          ;;
        native-connect|native-disconnect|native-mode|native-profile-rename|native-profile-favorite|native-profile-delete)
          action=${1#native-}
          shift
          exec omavless plugin "$action" "$@"
          ;;
      esac
      if [ "$#" -eq 1 ] && [ "$1" = status ]; then
        omavless plugin snapshot 2>/dev/null && exit 0
        printf '%s\n' 'OmaVLESS native status is unavailable' >&2
      else
        printf '%s\n' 'OmaVLESS native frontend is read-only at this checkpoint' >&2
      fi
      exit 70
      ;;
    *) blocked ;;
  esac
else
  legacy_without_native || blocked
fi
case "${1-}" in native-*) blocked ;; esac
exec python3 "$(dirname "$0")/backend.py" "$@"
