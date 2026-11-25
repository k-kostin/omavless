#!/bin/bash
# SPDX-License-Identifier: MIT
# Fixed sudo program, captured and authenticated before authentication.
# Preparation consumes archive bytes only; installation starts with terminal
# stdin so sudo's normal PTY relay owns authentication and pacman's confirmation.
set -euo pipefail
umask 077
[[ $# -ge 1 && $EUID -eq 0 ]] || exit 1
caller_uid=${OMAVLESS_INSTALL_UID:-}
[[ $caller_uid =~ ^(0|[1-9][0-9]{0,9})$ ]] || exit 1
action=$1
shift
case "$action:$#" in
  prepare:4) ;;
  install:5|discard:5)
    token=$1
    shift
    [[ $token =~ ^omavless-packages\.[A-Za-z0-9]{8}$ ]] || exit 1
    ;;
  *) exit 1 ;;
esac
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
if [[ $action == prepare ]]; then
  stage=$(/usr/bin/mktemp -d -p /var/cache omavless-packages.XXXXXXXX)
else
  stage=/var/cache/$token
fi
[[ -d $stage && ! -L $stage && $(/usr/bin/stat -c '%u:%a' -- "$stage") == 0:700 ]] || exit 1
app=$stage/omavless.pkg.tar.zst
dns=$stage/omavless-dns.pkg.tar.zst
ready=$stage/ready
claimed=$stage/claimed
expected=$(printf '1\n%s\n%s:%s\n%s:%s\n' "$caller_uid" "$app_size" "$app_hash" "$dns_size" "$dns_hash")
capture_pair() {
  /usr/bin/dd bs=65536 iflag=fullblock count="${app_size}B" of="$app" conv=excl status=none || return 1
  /usr/bin/dd bs=65536 iflag=fullblock count="${dns_size}B" of="$dns" conv=excl status=none || return 1
  [[ $(/usr/bin/stat -c '%s' -- "$app") == "$app_size" \
     && $(/usr/bin/stat -c '%s' -- "$dns") == "$dns_size" ]] || return 1
  local extra_count
  extra_count=$(/usr/bin/dd bs=1 count=1 status=none | /usr/bin/wc -c) || return 1
  [[ $extra_count == 0 ]] || return 1
  printf '%s  %s\n%s  %s\n' "$app_hash" "$app" "$dns_hash" "$dns" \
    | /usr/bin/sha256sum --check --status - || return 1
  /usr/bin/chmod 0400 -- "$app" "$dns" || return 1
}
verify_pair() {
  local member
  for member in "$app" "$dns"; do
    [[ -f $member && ! -L $member \
       && $(/usr/bin/stat -c '%u:%a:%h' -- "$member") == 0:400:1 ]] || return 1
  done
  [[ $(/usr/bin/stat -c '%s' -- "$app") == "$app_size" \
     && $(/usr/bin/stat -c '%s' -- "$dns") == "$dns_size" ]] || return 1
  printf '%s  %s\n%s  %s\n' "$app_hash" "$app" "$dns_hash" "$dns" \
    | /usr/bin/sha256sum --check --status -
}
status=1
if [[ $action == prepare ]]; then
  if capture_pair; then
    # Publish readiness last. UID comes from sudo, not an archive argument.
    printf '%s\n' "$expected" >"$stage/.ready-pending"
    /usr/bin/chmod 0400 -- "$stage/.ready-pending"
    /usr/bin/mv -T -- "$stage/.ready-pending" "$ready"
    printf '%s\n' "${stage##*/}"
    exit 0
  fi
else
  [[ -f $ready && ! -L $ready \
     && $(/usr/bin/stat -c '%u:%a:%h' -- "$ready") == 0:400:1 \
     && $(/usr/bin/stat -c '%s' -- "$ready") == $(( ${#expected} + 1 )) \
     && $(/usr/bin/head -c 257 -- "$ready") == "$expected" ]] || exit 1
  [[ ! -e $claimed && ! -L $claimed ]] || exit 1
  # One-way claim: the source name disappears atomically. A delayed loser
  # cannot re-acquire it, even after known completion or incomplete cleanup.
  /usr/bin/mv -T -- "$ready" "$claimed" || exit 1
  if [[ $action == discard ]]; then
    status=0
  elif [[ -t 0 ]] && verify_pair; then
    if /usr/bin/pacman -U -- "$app" "$dns"; then status=0; else status=$?; fi
  fi
fi
# Only a known original result reaches here. Fatal loss retains the stage and
# its claim; no signal/EXIT trap retries or erases an uncertain transaction.
cleanup_status=0
/usr/bin/rm -f -- "$app" "$dns" "$ready" "$claimed" || cleanup_status=1
/usr/bin/rmdir -- "$stage" || cleanup_status=1
if [[ $cleanup_status != 0 ]]; then
  printf '%s\n' 'Package staging cleanup is incomplete; do not repeat installation.' >&2
  if [[ $action != install ]]; then status=1; fi
fi
exit "$status"
