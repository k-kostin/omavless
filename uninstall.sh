#!/bin/bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 OmaVLESS contributors
set -euo pipefail

purge=false
case "${1:-}" in
  "") ;;
  --purge) purge=true ;;
  *) echo "Usage: ./uninstall.sh [--purge]" >&2; exit 2 ;;
esac

unit="$HOME/.config/systemd/user/omavless.service"
startup_unit="$HOME/.config/systemd/user/omavless-autostart.service"
data="$HOME/.config/omavless"

# Removal must not leave an enabled full-route TUN service behind. Failure is
# ignored only when the service is already absent/inactive; a service that is
# still running stops the cleanup with its unit and data intact.
disable_failed=false
systemctl --user disable --now omavless-autostart.service >/dev/null 2>&1 || disable_failed=true
systemctl --user disable --now omavless.service >/dev/null 2>&1 || disable_failed=true
if systemctl --user is-active --quiet omavless.service; then
  echo "omavless.service is still running; nothing was removed." >&2
  exit 1
fi
if [[ "$disable_failed" == true ]] && {
  systemctl --user is-enabled --quiet omavless.service \
    || systemctl --user is-enabled --quiet omavless-autostart.service;
}; then
  echo "An OmaVLESS service is still enabled; nothing was removed." >&2
  exit 1
fi

rm -f -- "$unit" "$startup_unit"
systemctl --user daemon-reload

if [[ "$purge" == true ]]; then
  rm -rf -- "$data"
  echo "OmaVLESS runtime integration and saved profiles were removed."
else
  echo "OmaVLESS runtime integration was removed; saved profiles remain in $data."
fi
