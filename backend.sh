#!/bin/sh
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 OmaVLESS contributors
# QML's stable launcher. Native admission is explicit and generation-fenced;
# errors must never fall through to the archived legacy owner.
blocked() {
  printf '%s\n' 'OmaVLESS requires the native package and committed Rust ownership; see docs/user/NATIVE_INSTALL.md. No legacy fallback.' >&2
  exit 71
}


if command -v omavless >/dev/null 2>&1; then
  target=$(omavless plugin target 2>/dev/null) || blocked
  case "$target" in
    legacy) blocked ;;
    rust)
      case "${1-}" in
        watch-plugin-removal)
          [ "$#" -eq 1 ] || blocked
          exec omavless plugin watch-removal
          ;;
        native-quit)
          [ "$#" -eq 4 ] || blocked
          # Quickshell kills its direct Process child when the plugin unloads.
          # Keep that child a waiting wrapper, so the explicitly confirmed
          # bounded Rust exit can finish final verification after hiding UI.
          omavless plugin quit "$2" "$3" "$4" &
          wait "$!"
          exit "$?"
          ;;
        cleanup-runtime|cleanup-qr)
          # Service startup precedes QML ownership discovery. Select here,
          # never by its still-default nativeOwner flag. This only reaps dead
          # desktop-helper scratch; it must not stop the tunnel or runtime.
          [ "$#" -eq 1 ] || blocked
          exec omavless desktop cleanup
          ;;
        native-subscription-probe)
          [ "$#" -eq 5 ] || blocked
          exec omavless subscription probe "$2" "$3" "$4" "$5"
          ;;
        native-subscription-probe-results)
          [ "$#" -eq 3 ] || blocked
          exec omavless subscription probe-results "$2" "$3"
          ;;
        native-subscriptions-refresh-all|native-providers-refresh)
          [ "$#" -eq 4 ] || blocked
          case "$1" in
            native-subscriptions-refresh-all) exec omavless subscription refresh-all "$2" "$3" "$4" ;;
            native-providers-refresh) exec omavless routing refresh-providers "$2" "$3" "$4" ;;
          esac
          ;;
        native-operation-get|native-operation-cancel)
          [ "$#" -eq 3 ] || blocked
          case "$1" in
            native-operation-get) exec omavless operation get "$2" "$3" ;;
            native-operation-cancel) exec omavless operation cancel "$2" "$3" ;;
          esac
          ;;
        native-desktop-capabilities)
          [ "$#" -eq 1 ] || blocked
          exec omavless desktop capabilities
          ;;
        native-startup-capabilities)
          [ "$#" -eq 1 ] || blocked
          exec omavless capabilities
          ;;
        native-startup-configure)
          [ "$#" -eq 4 ] || blocked
          shift
          exec omavless plugin startup-configure "$@"
          ;;
        native-support-report)
          [ "$#" -eq 1 ] || blocked
          exec omavless diagnostics export
          ;;
        native-clipboard-copy)
          [ "$#" -eq 1 ] || blocked
          exec omavless desktop clipboard-copy
          ;;
        native-onboarding-complete)
          [ "$#" -eq 4 ] || blocked
          shift
          exec omavless plugin onboarding-complete "$@"
          ;;
        native-core-readiness)
          [ "$#" -eq 1 ] || blocked
          exec omavless desktop core-readiness
          ;;
        native-connection-test)
          [ "$#" -eq 1 ] || blocked
          exec omavless runtime test
          ;;
        native-profile-details)
          [ "$#" -eq 2 ] || blocked
          exec omavless profile details "$2"
          ;;
        native-ping)
          [ "$#" -eq 1 ] || blocked
          exec omavless runtime ping
          ;;
        native-traffic)
          [ "$#" -eq 1 ] || blocked
          exec omavless runtime traffic
          ;;
        native-routing-rules)
          [ "$#" -eq 1 ] || blocked
          exec omavless routing rules
          ;;
        native-routing-check)
          [ "$#" -eq 1 ] || blocked
          exec omavless routing check
          ;;
        native-diagnostics-summary)
          [ "$#" -eq 1 ] || blocked
          exec omavless diagnostics summary
          ;;
        native-profile-qr)
          [ "$#" -eq 2 ] || blocked
          exec omavless profile export "$2" qr
          ;;
        native-profile-file)
          [ "$#" -eq 2 ] || blocked
          exec omavless profile export "$2" file
          ;;
        native-pick-report-export|native-pick-profile-export)
          [ "$#" -eq 1 ] || blocked
          exec omavless desktop "${1#native-}"
          ;;
        native-export-write)
          [ "$#" -eq 1 ] || blocked
          exec omavless desktop export-file
          ;;
        native-qr-render)
          [ "$#" -eq 1 ] || blocked
          exec omavless desktop qr-data-uri
          ;;
        native-profile-edit-input)
          [ "$#" -eq 2 ] || blocked
          exec omavless profile edit-input "$2"
          ;;
        native-subscription-edit-input)
          [ "$#" -eq 2 ] || blocked
          exec omavless subscription edit-input "$2"
          ;;
        native-profile-editor)
          [ "$#" -eq 1 ] || blocked
          exec omavless desktop edit
          ;;
        native-observation)
          [ "$#" -eq 1 ] || blocked
          exec omavless runtime observation
          ;;
        native-import-preview)
          [ "$#" -eq 1 ] || blocked
          exec omavless import preview
          ;;
        native-import-clipboard|native-import-file|native-import-path)
          [ "$#" -eq 1 ] || blocked
          case "$1" in
            native-import-clipboard) exec omavless desktop clipboard-read ;;
            native-import-file) exec omavless desktop pick-import ;;
            native-import-path) exec omavless desktop file-read ;;
          esac
          ;;
        native-connect|native-disconnect|native-mode|native-profile-rename|native-profile-favorite|native-profile-delete|native-profile-import|native-profile-replace|native-subscription-add|native-subscription-update|native-subscription-delete|native-subscription-refresh|native-routing-preset|native-custom-rule-add|native-custom-rule-delete)
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
  blocked
fi
blocked
