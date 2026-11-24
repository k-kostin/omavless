#!/bin/bash
# SPDX-License-Identifier: MIT
# Fixed sudo program, passed as immutable bash -c text before authentication.
# stdin contains exactly the two public archives, never a source pathname.
# Pacman receives root-private copies verified AFTER copying, not the cache.
set -euo pipefail
umask 077
[[ $# == 4 && $EUID -eq 0 ]] || exit 1
app_size=$1 app_hash=$2 dns_size=$3 dns_hash=$4
for size in "$app_size" "$dns_size"; do
  [[ $size =~ ^[1-9][0-9]{0,8}$ && $size -le 268435456 ]] || exit 1
done
[[ $app_hash =~ ^[0-9a-f]{64}$ && $dns_hash =~ ^[0-9a-f]{64}$ ]] || exit 1
for parent in / /var /var/cache; do
  [[ -d $parent && ! -L $parent ]] || exit 1
  [[ $(/usr/bin/stat -c '%u' -- "$parent") == 0 ]] || exit 1
  mode=$(/usr/bin/stat -c '%a' -- "$parent")
  (( (8#$mode & 0022) == 0 )) || exit 1
done
stage=$(/usr/bin/mktemp -d -p /var/cache omavless-packages.XXXXXXXX)
[[ -d $stage && ! -L $stage && $(/usr/bin/stat -c '%u:%a' -- "$stage") == 0:700 ]] || exit 1
app=$stage/omavless.pkg.tar.zst
dns=$stage/omavless-dns.pkg.tar.zst
capture_pair() {
  /usr/bin/dd bs=65536 iflag=fullblock count="${app_size}B" of="$app" conv=excl status=none || return 1
  /usr/bin/dd bs=65536 iflag=fullblock count="${dns_size}B" of="$dns" conv=excl status=none || return 1
  [[ $(/usr/bin/stat -c '%s' -- "$app") == "$app_size" \
     && $(/usr/bin/stat -c '%s' -- "$dns") == "$dns_size" ]] || return 1
  # Refuse extra bytes as well as a truncated/mutated stream.
  [[ $(/usr/bin/dd bs=1 count=1 status=none | /usr/bin/wc -c) == 0 ]] || return 1
  printf '%s  %s\n%s  %s\n' "$app_hash" "$app" "$dns_hash" "$dns" \
    | /usr/bin/sha256sum --check --status - || return 1
  /usr/bin/chmod 0400 -- "$app" "$dns" || return 1
}
status=1
if capture_pair; then
  # The archive stream is not interactive input. Authentication and pacman's
  # ordinary confirmation use the controlling terminal, without --noconfirm.
  if /usr/bin/pacman -U -- "$app" "$dns" </dev/tty; then status=0; else status=$?; fi
fi
# Only a known original result reaches here. Fatal loss retains the private
# stage for explicit reconciliation; no signal trap retries or erases it.
/usr/bin/rm -f -- "$app" "$dns"
/usr/bin/rmdir -- "$stage"
exit "$status"
