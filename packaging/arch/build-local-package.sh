#!/bin/bash
# SPDX-License-Identifier: MIT
# Explicit developer packaging only: never compile, download, install or activate.
set -euo pipefail
export LC_ALL=C
fail() { echo 'OmaVLESS local package build refused or failed.' >&2; exit 2; }
[[ $# -eq 3 && $EUID -ne 0 ]] || fail
builddir=$1
binary=$2
expected_sha=$3
[[ $expected_sha =~ ^[0-9a-f]{40}$ ]] || fail
[[ $builddir == /* && $builddir != / && $binary == /* ]] || fail
[[ -d $builddir && ! -L $builddir && -f $binary && ! -L $binary && -x $binary ]] || fail
no_symlinks() {
  local path=$1
  while [[ $path != / ]]; do
    [[ ! -L $path ]] || fail
    path=${path%/*}
    [[ -n $path ]] || path=/
  done
}
no_symlinks "$builddir"
no_symlinks "$binary"
builddir=$(realpath -e -- "$builddir")
binary=$(realpath -e -- "$binary")
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
repo_root=$(cd -- "$script_dir/../.." && pwd -P)
case "$builddir/" in "$repo_root/"*) fail ;; esac
[[ -z $(find "$builddir" -mindepth 1 -maxdepth 1 -print -quit) ]] || fail
[[ $(stat -c %u -- "$builddir") == "$EUID" ]] || fail
[[ $(git -C "$repo_root" rev-parse HEAD) == "$expected_sha" ]] || fail
[[ -z $(git -C "$repo_root" status --porcelain --untracked-files=normal) ]] || fail
count=$(git -C "$repo_root" rev-list --count HEAD)
epoch=$(git -C "$repo_root" show -s --format=%ct HEAD)
[[ $count =~ ^[0-9]+$ && $epoch =~ ^[0-9]+$ ]] || fail
architecture=$(uname -m)
case $architecture in
  aarch64) machine='AArch64' ;;
  x86_64) machine='Advanced Micro Devices X86-64' ;;
  *) fail ;;
esac
# Inspect, never execute an arbitrary caller-supplied binary.
header=$(readelf -h -- "$binary" 2>/dev/null) || fail
[[ $header == *'Class:'*'ELF64'* && $header == *'Data:'*'little endian'* ]] || fail
[[ $(sed -n 's/^ *Machine: *//p' <<< "$header") == "$machine" ]] || fail
elf_type=$(sed -n 's/^ *Type: *\([^ ]*\).*$/\1/p' <<< "$header")
[[ $elf_type == EXEC || $elf_type == DYN ]] || fail
entry=$(sed -n 's/^ *Entry point address: *//p' <<< "$header")
[[ $entry =~ ^0x[0-9a-fA-F]+$ && ! $entry =~ ^0x0+$ ]] || fail
binary_hash=$(sha256sum -- "$binary"); binary_hash=${binary_hash%% *}
[[ $binary_hash =~ ^[0-9a-f]{64}$ ]] || fail

chmod 0700 -- "$builddir"
mkdir -- "$builddir/payload" "$builddir/home" "$builddir/config"
bash "$script_dir/stage-payload.sh" "$builddir/payload" "$binary" || fail
staged_hash=$(sha256sum -- "$builddir/payload/usr/bin/omavless"); staged_hash=${staged_hash%% *}
[[ $staged_hash == "$binary_hash" ]] || fail
printf 'schemaVersion=1\nsourceCommit=%s\nbinarySha256=%s\narchitecture=%s\nprovenance=caller-supplied-prebuilt\n' \
  "$expected_sha" "$binary_hash" "$architecture" \
  > "$builddir/payload/usr/share/doc/omavless/build-identity.txt"
chmod 0644 -- "$builddir/payload/usr/share/doc/omavless/build-identity.txt"
tar --sort=name --mtime="@$epoch" --owner=0 --group=0 --numeric-owner \
  -cf "$builddir/payload.tar" -C "$builddir/payload" usr || fail
payload_hash=$(sha256sum -- "$builddir/payload.tar"); payload_hash=${payload_hash%% *}
sed -e "s/@COUNT@/$count/g" -e "s/@SHORT_SHA@/${expected_sha:0:12}/g" \
  -e "s/@ARCH@/$architecture/g" -e "s/@PAYLOAD_SHA256@/$payload_hash/g" \
  "$script_dir/PKGBUILD.local.in" > "$builddir/PKGBUILD"
# Use the root-owned distribution configuration, not per-user build hooks.
# Force every makepkg output below the explicitly selected build directory.
printf '%s\n' 'source /etc/makepkg.conf' \
  'BUILDDIR="$PWD/build"' 'PKGDEST="$PWD"' 'SRCDEST="$PWD"' \
  'SRCPKGDEST="$PWD"' 'LOGDEST="$PWD"' 'PKGEXT=".pkg.tar.zst"' \
  > "$builddir/makepkg.conf"
[[ $(git -C "$repo_root" rev-parse HEAD) == "$expected_sha" ]] || fail
[[ -z $(git -C "$repo_root" status --porcelain --untracked-files=normal) ]] || fail
cd -- "$builddir"
env -i PATH=/usr/bin:/bin HOME="$builddir/home" XDG_CONFIG_HOME="$builddir/config" \
  LANG=C.UTF-8 SOURCE_DATE_EPOCH="$epoch" \
  /usr/bin/makepkg --config "$builddir/makepkg.conf" --nodeps --nocheck --noconfirm || fail
archive="$builddir/omavless-0.0.0.r$count.g${expected_sha:0:12}-1-$architecture.pkg.tar.zst"
[[ -f $archive && ! -L $archive ]] || fail
packaged_hash=$(bsdtar -xOf "$archive" usr/bin/omavless | sha256sum) || fail
[[ ${packaged_hash%% *} == "$binary_hash" ]] || fail
bsdtar -xOf "$archive" usr/share/doc/omavless/build-identity.txt \
  | cmp -s -- - "$builddir/payload/usr/share/doc/omavless/build-identity.txt" || fail
bsdtar -xOf "$archive" usr/lib/systemd/user/omavless-runtime.service \
  | cmp -s -- - "$builddir/payload/usr/lib/systemd/user/omavless-runtime.service" || fail
echo 'Local package built; no installation, service or ownership change performed.'
