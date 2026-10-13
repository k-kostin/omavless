#!/bin/bash
# SPDX-License-Identifier: MIT
# Source-only, advisory installed-byte inspection. No predecessor execution,
# package query, private state read, write, or upgrade/privilege authority.
# Public pins: v0.8.2:docs/testing/NATIVE_082_ARTIFACTS_2026-09-21.md;
# exact receipt format: producer 22e23e64 packaging/arch/build-local-package.sh.
# "qualified" identifies only the observed public bytes/source. It does not
# prove package-database ownership, committed Rust/private state, shutdown,
# broker emptiness, readiness, or exclusion after this function returns.

_omavless_predecessor_report() {
  printf '{"schemaVersion":1,"scope":"installed_predecessor_bytes","status":"%s","reason":"%s"}\n' "$1" "$2"
}

_omavless_predecessor_parents_safe() {
  local path=${1%/*} owner=$2 metadata uid mode
  while :; do
    [[ -d $path && ! -L $path ]] || return 1
    metadata=$(/usr/bin/stat -c '%u %a' -- "$path" 2>/dev/null) || return 2
    read -r uid mode <<< "$metadata"
    [[ $uid == 0 || $uid == "$owner" ]] || return 1
    [[ $mode =~ ^[0-7]{3,4}$ ]] && (( (8#$mode & 0022) == 0 )) || return 1
    [[ $path != / ]] || return 0
    path=${path%/*}
    [[ -n $path ]] || path=/
  done
}

_omavless_predecessor_file_identity() {
  local path=$1 owner=$2 mode=$3 maximum=$4 metadata uid actual_mode links size
  [[ -f $path && ! -L $path ]] || return 1
  [[ -r $path ]] || return 2
  _omavless_predecessor_parents_safe "$path" "$owner" || return $?
  metadata=$(/usr/bin/stat -c '%u %a %h %s' -- "$path" 2>/dev/null) || return 2
  read -r uid actual_mode links size <<< "$metadata"
  [[ $uid == "$owner" && $actual_mode == "$mode" && $links == 1
     && $size =~ ^[0-9]+$ && $size -gt 0 && $size -le $maximum ]] || return 1
  /usr/bin/stat -c '%d:%i:%u:%g:%a:%h:%s:%y:%z' -- "$path" 2>/dev/null || return 2
}

# Private test seam: caller-supplied fixtures/pins/owner are never accepted by
# the public entry. Same-process shell overrides are not a security boundary.
_omavless_predecessor_inspect() (
  export LC_ALL=C
  local receipt=$1 binary=$2 arch=$3 digest=$4 owner=$5 before_receipt before_binary after_receipt after_binary actual error
  if [[ ! -e $receipt && ! -L $receipt || ! -e $binary && ! -L $binary ]]; then
    _omavless_predecessor_report unavailable installed_files_unavailable; return
  fi
  before_receipt=$(_omavless_predecessor_file_identity "$receipt" "$owner" 644 4096) \
    && before_binary=$(_omavless_predecessor_file_identity "$binary" "$owner" 755 268435456) \
    || {
      error=$?
      if [[ $error == 2 ]]; then _omavless_predecessor_report unavailable inspection_unavailable
      else _omavless_predecessor_report mismatch installed_files_unsafe; fi
      return
    }
  # Byte-exact six-line schema 3: rejects duplicate/unknown fields, NULs,
  # reordered/partial receipts and unsupported development/RC identities.
  if ! printf 'schemaVersion=3\nsourceCommit=22e23e64c49b8110088b6be3f063b5e641ba0853\nbinarySha256=%s\narchitecture=%s\nprovenance=caller-supplied-prebuilt\nproductVersion=0.8.2\n' \
      "$digest" "$arch" | /usr/bin/cmp -s -- - "$receipt" 2>/dev/null; then
    _omavless_predecessor_report mismatch receipt_mismatch; return
  fi
  actual=$(/usr/bin/timeout 8 /usr/bin/sha256sum -- "$binary" 2>/dev/null) \
    || { _omavless_predecessor_report unavailable inspection_unavailable; return; }
  [[ ${actual%% *} == "$digest" ]] \
    || { _omavless_predecessor_report mismatch binary_mismatch; return; }
  after_receipt=$(_omavless_predecessor_file_identity "$receipt" "$owner" 644 4096) \
    && after_binary=$(_omavless_predecessor_file_identity "$binary" "$owner" 755 268435456) \
    && [[ $before_receipt == "$after_receipt" && $before_binary == "$after_binary" ]] \
    || {
      error=$?
      if [[ $error == 2 ]]; then _omavless_predecessor_report unavailable inspection_unavailable
      else _omavless_predecessor_report mismatch installed_files_changed; fi
      return
    }
  _omavless_predecessor_report qualified public_0_8_2
)

omavless_predecessor_status() {
  local arch digest
  [[ $# == 0 ]] || { _omavless_predecessor_report unavailable invalid_request; return; }
  arch=$(/usr/bin/uname -m 2>/dev/null) \
    || { _omavless_predecessor_report unavailable inspection_unavailable; return; }
  case "$arch" in
    x86_64) digest=3ff795316aa331463aff4e26571546433bdfd9317f348712d135be2f0a8d28a1 ;;
    aarch64) digest=62874def77df9e4f099baf74c57420a7ab1dab7b83935676828f6d445a5f357f ;;
    *) _omavless_predecessor_report unavailable unsupported_architecture; return ;;
  esac
  _omavless_predecessor_inspect /usr/share/doc/omavless/build-identity.txt /usr/bin/omavless "$arch" "$digest" 0
}
