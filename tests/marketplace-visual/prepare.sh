#!/bin/bash
# Developer-only screenshot staging. Never copy a private working-tree file.
set -euo pipefail
umask 077
[[ $# == 1 && $1 == /* && -d $1 && ! -L $1 ]] || exit 64
source_root=$(realpath -e -- "$1")
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
git -C "$source_root" diff --quiet HEAD -- plugin manifest.json
# Reuse the HOME-backed private fixture namespace, not quota-limited /tmp.
# Do not repair permissions or follow a substituted cache parent.
[[ $HOME == /* && -d $HOME && ! -L $HOME ]] || exit 64
capture_parent=$HOME
for component in home .cache ovt; do
  if [[ $component != home ]]; then
    capture_parent=$capture_parent/$component
    if [[ ! -e $capture_parent && ! -L $capture_parent ]]; then
      mkdir -m 700 -- "$capture_parent"
    fi
  fi
  [[ -d $capture_parent && ! -L $capture_parent ]] || exit 64
  [[ $(stat -c '%u' -- "$capture_parent") == $(id -u) ]] || exit 64
  capture_mode=$(stat -c '%a' -- "$capture_parent")
  (( (8#$capture_mode & 07022) == 0 )) || exit 64
done
[[ $capture_mode == 700 ]] || exit 64
capture_dir=$(mktemp -d "$capture_parent/omavless-marketplace.XXXXXX")
git -C "$source_root" archive HEAD plugin manifest.json | tar -x -C "$capture_dir"
cp -- "$here/backend.sh" "$here/backend.mjs" "$here/shell.qml" "$capture_dir/"
cp -- "$here/setup-runtime.sh" "$capture_dir/plugin/setup-runtime.sh"
mkdir -m 700 -- "$capture_dir/fixtures" "$capture_dir/runtime"
for kind in status native-observation native-desktop-capabilities native-core-readiness native-startup-capabilities; do
  node "$here/backend.mjs" "$kind" > "$capture_dir/fixtures/$kind.json"
done
ln -s /usr/share/omarchy/shell/Commons "$capture_dir/Commons"
ln -s /usr/share/omarchy/shell/Ui "$capture_dir/Ui"
git -C "$source_root" rev-parse HEAD > "$capture_dir/source-commit.txt"
git -C "$source_root" ls-tree -r HEAD -- plugin > "$capture_dir/source-tree.txt"
printf '%s\n' "$capture_dir"
