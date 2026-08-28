#!/bin/bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 OmaVLESS contributors
set -euo pipefail

plugin_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
plugins_dir="$HOME/.config/omarchy/plugins"
target="$plugins_dir/kdk.omavless"
fresh_install=false
[[ -e "$target" ]] || fresh_install=true

omarchy plugin validate "$plugin_dir"
mkdir -p "$plugins_dir"

# Build the complete tree away from the live plugin directory. Quickshell
# watches enabled local plugins recursively; copying these files directly into
# $target causes one hot reload per file and can race QML object destruction
# against creation. A hidden sibling is ignored by plugin discovery and can be
# moved into place in one operation once the old instance is disabled.
stage="$(mktemp -d "$plugins_dir/.kdk.omavless.install.XXXXXX")"
backup=""

cleanup() {
  [[ ! -d "$stage" ]] || rm -rf -- "$stage"
  if [[ -n "$backup" && -d "$backup" && ! -e "$target" ]]; then
    mv -- "$backup" "$target"
  fi
}
trap cleanup EXIT

cp -a \
  "$plugin_dir/Panel.qml" \
  "$plugin_dir/Service.qml" \
  "$plugin_dir/AdvancedDiagnostics.qml" \
  "$plugin_dir/PlainText.qml" \
  "$plugin_dir/Sparkline.qml" \
  "$plugin_dir/NamePrompt.qml" \
  "$plugin_dir/ImportPreviewPrompt.qml" \
  "$plugin_dir/SubscriptionPrompt.qml" \
  "$plugin_dir/RoutingPresetPrompt.qml" \
  "$plugin_dir/OnboardingWizard.qml" \
  "$plugin_dir/StartupPrompt.qml" \
  "$plugin_dir/RoutingToolsPrompt.qml" \
  "$plugin_dir/RenameWindow.qml" \
  "$plugin_dir/QrWindow.qml" \
  "$plugin_dir/backend.sh" \
  "$plugin_dir/backend.py" \
  "$plugin_dir/uninstall.sh" \
  "$plugin_dir/manifest.json" \
  "$plugin_dir/preview.png" \
  "$plugin_dir/LICENSE" \
  "$plugin_dir/THIRD_PARTY_NOTICES.md" \
  "$plugin_dir/README.md" \
  "$plugin_dir/CHANGELOG.md" \
  "$stage/"
mkdir -p "$stage/templates"
cp -a "$plugin_dir/templates/." "$stage/templates/"
chmod 755 "$stage/backend.sh" "$stage/backend.py" "$stage/uninstall.sh"

if [[ -e "$target" ]]; then
  backup="${stage}.backup"
  mv -- "$target" "$backup"
fi
mv -- "$stage" "$target"

# Existing installs retain their enabled state and bar position. The shell's
# local-plugin watcher debounces the remove/add pair above into one hot reload;
# forcing rescan + enable + a full restart while that asynchronous reload is
# finalizing can crash Quickshell inside QQmlObjectCreator. A fresh install is
# enabled only after normal discovery has completed. Rescan is a fallback for
# shells whose watcher missed the new directory, never an update-time reload.
if [[ "$fresh_install" == true ]]; then
  discovered=false
  for _ in {1..30}; do
    if omarchy plugin list --json 2>/dev/null | jq -e \
      'any(.[]; .id == "kdk.omavless")' >/dev/null; then
      discovered=true
      break
    fi
    sleep 0.1
  done

  if [[ "$discovered" == false ]]; then
    omarchy-shell shell rescanPlugins
    for _ in {1..30}; do
      if omarchy plugin list --json 2>/dev/null | jq -e \
        'any(.[]; .id == "kdk.omavless")' >/dev/null; then
        discovered=true
        break
      fi
      sleep 0.1
    done
  fi

  [[ "$discovered" == true ]] || {
    echo "OmaVLESS was copied but the running shell did not discover it." >&2
    exit 1
  }
  omarchy plugin enable kdk.omavless right
fi

if [[ -n "$backup" && -d "$backup" ]]; then
  rm -rf -- "$backup"
  backup=""
fi
trap - EXIT

echo "OmaVLESS is installed or updated; no tunnel was started."
if ! command -v zenity >/dev/null 2>&1 \
    && ! command -v kdialog >/dev/null 2>&1 \
    && ! command -v yad >/dev/null 2>&1; then
  echo "File import unavailable — file picker missing. Run: omarchy pkg add zenity"
  echo "Clipboard import remains available; kdialog and yad are also supported."
fi
