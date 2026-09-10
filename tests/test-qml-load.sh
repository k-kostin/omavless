#!/bin/bash
# SPDX-License-Identifier: MIT
# Opt-in installed Omarchy/Quickshell compile gate; does not instantiate a plugin.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
entry="${1:-$here/../plugin/Panel.qml}"
[[ $# -le 1 && "$entry" = /* && -f "$entry" ]] || { echo 'Expected an absolute QML component path' >&2; exit 2; }
command -v qs >/dev/null || { echo 'NOT RUN: Quickshell unavailable'; exit 77; }
[[ -n "${WAYLAND_DISPLAY:-}" ]] || { echo 'NOT RUN: Wayland session unavailable'; exit 77; }
shell_root=/usr/share/omarchy/shell
[[ -d "$shell_root/Ui" && -d "$shell_root/Commons" ]] || { echo 'NOT RUN: Omarchy imports unavailable'; exit 77; }
fixture=$(mktemp -d /tmp/omavless-qml-load.XXXXXX)
trap 'rm -rf -- "$fixture"' EXIT
cp "$here/qml-load/shell.qml" "$fixture/shell.qml"
for module in Commons Ui services; do ln -s "$shell_root/$module" "$fixture/$module"; done
status=0
QT_QPA_PLATFORM=wayland OMAVLESS_QML_ENTRY="file://$entry" timeout --kill-after=2s 5s qs -p "$fixture" --no-color >"$fixture/result" 2>&1 || status=$?
# Some installed Quickshell versions keep the shell process alive after Qt.quit.
# timeout only stops this isolated instance; readiness must still be explicit.
[[ "$status" == 0 || "$status" == 124 ]] || { echo 'QML component load: FAIL (runner)'; exit 1; }
if ! grep -q 'OMAVLESS_QML_LOAD_PASS' "$fixture/result" || grep -q 'OMAVLESS_QML_LOAD_FAIL' "$fixture/result"; then
  echo 'QML component load: FAIL'; exit 1
fi
echo 'QML component load: PASS (compile only; no visual/runtime acceptance)'
