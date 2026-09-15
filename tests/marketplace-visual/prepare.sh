#!/bin/bash
# Developer-only screenshot staging. Never copy a private working-tree file.
set -euo pipefail
umask 077
[[ $# == 1 && $1 == /* && -d $1 && ! -L $1 ]] || exit 64
source_root=$(realpath -e -- "$1")
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
git -C "$source_root" diff --quiet HEAD -- plugin
capture_dir=$(mktemp -d /tmp/omavless-marketplace.XXXXXX)
git -C "$source_root" archive HEAD plugin | tar -x -C "$capture_dir"
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
