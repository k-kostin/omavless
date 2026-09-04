#!/bin/bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 OmaVLESS contributors
set -euo pipefail

fail() {
  echo "OmaVLESS package payload staging failed." >&2
  exit 2
}

if [[ $# -ne 2 ]]; then
  echo "Usage: stage-payload.sh ABSOLUTE_DESTDIR ABSOLUTE_PREBUILT_BINARY" >&2
  exit 2
fi

destdir=$1
binary=$2

# This helper assembles a package payload. It must never be usable as a live
# host installer, and it must not follow a package-root symlink into another
# tree. Requiring the caller to create DESTDIR also keeps its write boundary
# explicit.
[[ "$destdir" == /* && "$destdir" != / ]] || fail
[[ "$binary" == /* ]] || fail
[[ -d "$destdir" && ! -L "$destdir" ]] || fail
[[ -f "$binary" && ! -L "$binary" && -x "$binary" ]] || fail

reject_symlink_components() {
  local candidate=$1
  while [[ "$candidate" != / ]]; do
    [[ ! -L "$candidate" ]] || fail
    candidate=${candidate%/*}
    [[ -n "$candidate" ]] || candidate=/
  done
}

reject_unsafe_file_target() {
  local target=$1
  [[ ! -L "$target" ]] || fail
  [[ ! -e "$target" || -f "$target" ]] || fail
}

reject_symlink_components "$destdir"
destdir=$(realpath -e -- "$destdir")
[[ "$destdir" != / ]] || fail
reject_symlink_components "$destdir"

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
repo_root=$(cd -- "$script_dir/../.." && pwd -P)

binary_target="$destdir/usr/bin/omavless"
unit_target="$destdir/usr/lib/systemd/user/omavless-runtime.service"
license_target="$destdir/usr/share/licenses/omavless/LICENSE"
notices_target="$destdir/usr/share/licenses/omavless/THIRD_PARTY_NOTICES.md"
docs_target="$destdir/usr/share/doc/omavless/README.md"

for target in \
  "$binary_target" \
  "$unit_target" \
  "$license_target" \
  "$notices_target" \
  "$docs_target"; do
  reject_unsafe_file_target "$target"
done

for directory in \
  "$destdir/usr" \
  "$destdir/usr/bin" \
  "$destdir/usr/lib" \
  "$destdir/usr/lib/systemd" \
  "$destdir/usr/lib/systemd/user" \
  "$destdir/usr/share" \
  "$destdir/usr/share/licenses" \
  "$destdir/usr/share/licenses/omavless" \
  "$destdir/usr/share/doc" \
  "$destdir/usr/share/doc/omavless"; do
  reject_symlink_components "$directory"
  [[ ! -e "$directory" || -d "$directory" ]] || fail
done

install -d -m 0755 -- \
  "$destdir/usr" \
  "$destdir/usr/bin" \
  "$destdir/usr/lib" \
  "$destdir/usr/lib/systemd" \
  "$destdir/usr/lib/systemd/user" \
  "$destdir/usr/share" \
  "$destdir/usr/share/licenses" \
  "$destdir/usr/share/licenses/omavless" \
  "$destdir/usr/share/doc" \
  "$destdir/usr/share/doc/omavless"

# Recheck the created path before publishing files. This is a package-build
# boundary, not a defense against a hostile process racing in the same build
# directory, but no pre-existing symlink is accepted at any destination.
for target in \
  "$binary_target" \
  "$unit_target" \
  "$license_target" \
  "$notices_target" \
  "$docs_target"; do
  reject_symlink_components "$(dirname -- "$target")"
  reject_unsafe_file_target "$target"
done

install -m 0755 -- "$binary" "$binary_target"
install -m 0644 -- \
  "$repo_root/packaging/systemd/omavless-runtime.service" \
  "$unit_target"
install -m 0644 -- "$repo_root/LICENSE" "$license_target"
install -m 0644 -- "$repo_root/THIRD_PARTY_NOTICES.md" "$notices_target"
install -m 0644 -- "$script_dir/README.md" "$docs_target"
