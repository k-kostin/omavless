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

# Advisory release delivery inventory only. Root receipts and actual payloads are
# checked without executing either app/core. This never authorizes replacement.
_omavless_pair_pins() {
  local pins
  pins="$(dirname -- "${BASH_SOURCE[0]}")/published-pairs.json"
  [[ -f $pins && ! -L $pins && $(stat -c %s -- "$pins") -le 16384 ]] || return 1
  if /usr/bin/jq -er --arg source "$1" --arg arch "$2" '
    select(keys == ["pairs", "schemaVersion"] and .schemaVersion == 1)
    | [.pairs[] | select(.sourceCommit == $source and .architecture == $arch)]
    | select(length == 1) | .[0]
    | select(keys == ["appSha", "architecture", "brokerSha", "coreSha", "dnsReceiptSha", "sourceCommit"])
    | [.appSha, .coreSha, .brokerSha, .dnsReceiptSha]
    | select(all(.[]; type == "string" and test("^[0-9a-f]{64}$"))) | @tsv' "$pins"; then return; fi
  # Target delivery pins are frontend-only. Do not put a future binary's own
  # digest into its compiled predecessor table (a circular build identity).
  pins="$(dirname -- "${BASH_SOURCE[0]}")/component-updates.json"
  [[ -f $pins && ! -L $pins && $(stat -c %s -- "$pins") -le 8192 ]] || return 1
  /usr/bin/jq -er --arg source "$1" --arg arch "$2" '
    select(keys == ["identities", "schemaVersion", "sourceCommit", "updatePair"]
      and .schemaVersion == 2 and .sourceCommit == $source and (.updatePair | type) == "boolean")
    | .identities | select(keys == ["aarch64", "x86_64"])
    | select(all(.[]; keys == ["appSha", "brokerSha", "coreSha", "dnsReceiptSha"]
        and all(.[]; type == "string" and test("^[0-9a-f]{64}$")))) | .[$arch]
    | select(keys == ["appSha", "brokerSha", "coreSha", "dnsReceiptSha"])
    | [.appSha, .coreSha, .brokerSha, .dnsReceiptSha]
    | select(all(.[]; type == "string" and test("^[0-9a-f]{64}$"))) | @tsv' "$pins"
}
_omavless_pair_delivery_inspect() (
  export LC_ALL=C
  local app_receipt=$1 app=$2 dns_receipt=$3 core=$4 broker=$5 arch=$6 target=$7 owner=$8 policy=${9:-0.9.8} version
  [[ $# == 8 || $# == 9 ]] && [[ "$policy" == 0.9.8 || "$policy" == 0.9.9 || "$policy" == public-099 ]] \
    || { printf 'unknown\n'; return; }
  local app_id binary_id dns_id core_id broker_id source digest actual dns_source core_sha broker_sha
  local pinned_app pinned_core pinned_broker pinned_receipt
  app_id=$(_omavless_predecessor_file_identity "$app_receipt" "$owner" 644 4096) \
    && binary_id=$(_omavless_predecessor_file_identity "$app" "$owner" 755 268435456) \
    || { printf 'unknown\n'; return; }
  source=$(/usr/bin/sed -n 's/^sourceCommit=//p' "$app_receipt")
  digest=$(/usr/bin/sed -n 's/^binarySha256=//p' "$app_receipt")
  [[ $source =~ ^[0-9a-f]{40}$ && $digest =~ ^[0-9a-f]{64}$ ]] \
    || { printf 'unknown\n'; return; }
  version=$policy
  # The public 0.9.9 transition recognizes its exact current target as 0.9.9
  # and only the closed released-source list below as 0.9.8 predecessors.
  # Version comes from this source relation, never a receipt's own assertion.
  if [[ "$policy" == public-099 ]]; then
    if [[ "$source" == "$target" ]]; then version=0.9.9; else version=0.9.8; fi
  fi
  # The nonpublic lane recognizes ONLY its exact current target, not public
  # predecessors relabelled with a newer package version.
  [[ "$policy" != 0.9.9 || "$source" == "$target" ]] || { printf 'unknown\n'; return; }
  case "$source" in
    "$target"|4915ae4a8f4ec6cfb469ad6b38126bd70d5a1998|e3d2245966c725ee11b505899b22a20721ff2ddd|8d02f9d7dd1a9063a82e9dcd93cb19cf4c5d8524|b65695ee0609c2a971289109975678b8da87c946|99c556e5c966e1e1dae04f2d8babd2ca2dec9375|d4d4310be6ade7e7e166f8b672cf90c7ad3e5b3b|2d54b430866fd5b2849cd815f8552a4e9e2d16a8) ;;
    *) printf 'unknown\n'; return ;;
  esac
  IFS=$'\t' read -r pinned_app pinned_core pinned_broker pinned_receipt < <(_omavless_pair_pins "$source" "$arch") \
    || { printf 'unknown\n'; return; }
  [[ "$digest" == "$pinned_app" ]] || { printf 'unknown\n'; return; }
  printf 'schemaVersion=3\nsourceCommit=%s\nbinarySha256=%s\narchitecture=%s\nprovenance=caller-supplied-prebuilt\nproductVersion=%s\n' \
    "$source" "$digest" "$arch" "$version" | /usr/bin/cmp -s -- - "$app_receipt" \
    || { printf 'unknown\n'; return; }
  actual=$(/usr/bin/timeout 8 /usr/bin/sha256sum -- "$app") \
    || { printf 'unknown\n'; return; }
  [[ ${actual%% *} == "$digest" \
     && $app_id == "$(_omavless_predecessor_file_identity "$app_receipt" "$owner" 644 4096)" \
     && $binary_id == "$(_omavless_predecessor_file_identity "$app" "$owner" 755 268435456)" ]] \
    || { printf 'unknown\n'; return; }
  if [[ ! -e $dns_receipt && ! -L $dns_receipt && ! -e $core && ! -L $core && ! -e $broker && ! -L $broker ]]; then
    printf 'missing\n'; return
  fi
  dns_id=$(_omavless_predecessor_file_identity "$dns_receipt" "$owner" 644 8192) \
    && core_id=$(_omavless_predecessor_file_identity "$core" "$owner" 755 134217728) \
    && broker_id=$(_omavless_predecessor_file_identity "$broker" "$owner" 755 33554432) \
    || { printf 'unknown\n'; return; }
  IFS=$'\t' read -r dns_source core_sha broker_sha < <(/usr/bin/jq -er --arg arch "$arch" '
    def traffic_pair:
      type == "object"
      and keys == (["schema", "architecture", "omavless_commit", "mihomo_commit", "mihomo_tag",
        "sing_tun_commit", "sing_tun_tag", "patch_sha256", "go_version", "go_binary_sha256",
        "cargo_lock_sha256", "go_build_tags", "go_dependency_mode", "rustc_version",
        "cargo_version", "sha256", "package_flavor", "broker_feature", "traffic_ready_abi"] | sort)
      and .schema == "omavless-managed-dns-traffic-ready-pair-v1"
      and ($arch == "x86_64" or $arch == "aarch64")
      and (.traffic_ready_abi | type) == "number" and .traffic_ready_abi == 1
      and (.traffic_ready_abi | tojson) == "1"
      and .mihomo_commit == "ab405bad5beeeac8b003bb01f60f134f6df54471" and .mihomo_tag == "v1.19.31"
      and .sing_tun_commit == "b50ae28a1409c7bce8e96e6c6966cf57d8ace754" and .sing_tun_tag == "v0.4.24"
      and .patch_sha256 == {
        "mihomo-dns-broker.patch": "d5ebe9d6b37f6b76599fc3c2dd25adbfb774ca0121beeb79c5768a9a08d7ff37",
        "sing-tun-descriptor.patch": "2556c82aafbeb598a817d43042cf2069c6f209433c7a506395df581b4e31e2ab",
        "mihomo-traffic-ready.patch": "e9a2f1ea78046b6f2cfafe574bc2d989a8381d57d3464a0aff2d71507424beb1"}
      and .go_build_tags == "with_gvisor" and .go_dependency_mode == "vendor"
      and (.omavless_commit | type == "string" and test("^[0-9a-f]{40}\\z") and test("[1-9a-f]"))
      and all(.go_binary_sha256, .cargo_lock_sha256;
        type == "string" and test("^[0-9a-f]{64}\\z") and test("[1-9a-f]"))
      and (.sha256 | type == "object"
        and keys == ["corresponding-source.tar.xz", "mihomo", "mihomo.LICENSE",
          "omavless-dns-broker", "omavless.LICENSE", "sing-tun.LICENSE"]
        and all(.[]; type == "string" and test("^[0-9a-f]{64}\\z") and test("[1-9a-f]")))
      and (.go_version | type == "string" and length <= 128
        and test("^go version go1\\.[0-9]+\\.[0-9]+(-[A-Za-z0-9:]+)? linux/" + (if $arch == "x86_64" then "amd64" else "arm64" end) + "\\z"))
      and (.rustc_version | type == "string" and length <= 128 and startswith("rustc ") and test("^[ -~]*\\z"))
      and (.cargo_version | type == "string" and length <= 128 and startswith("cargo ") and test("^[ -~]*\\z"));
    select((.schema == 1 or traffic_pair) and .architecture == $arch and .package_flavor == "release" and .broker_feature == "release-package")
    | [.omavless_commit, .sha256.mihomo, .sha256["omavless-dns-broker"]]
    | select(all(.[]; type == "string")) | @tsv' "$dns_receipt") \
    || { printf 'unknown\n'; return; }
  [[ $dns_source =~ ^[0-9a-f]{40}$ && $core_sha =~ ^[0-9a-f]{64}$ && $broker_sha =~ ^[0-9a-f]{64}$ ]] \
    || { printf 'unknown\n'; return; }
  # Mixed producers are diagnosis only. A same-source companion must match
  # independently recorded public bytes, not just its own mutable receipt.
  if [[ "$dns_source" == "$source" ]]; then
    actual=$(/usr/bin/timeout 8 /usr/bin/sha256sum -- "$dns_receipt") \
      || { printf 'unknown\n'; return; }
    [[ "$core_sha" == "$pinned_core" && "$broker_sha" == "$pinned_broker" && "${actual%% *}" == "$pinned_receipt" ]] \
      || { printf 'unknown\n'; return; }
  fi
  actual=$(/usr/bin/timeout 8 /usr/bin/sha256sum -- "$core") \
    || { printf 'unknown\n'; return; }
  [[ ${actual%% *} == "$core_sha" ]] || { printf 'unknown\n'; return; }
  actual=$(/usr/bin/timeout 8 /usr/bin/sha256sum -- "$broker") \
    || { printf 'unknown\n'; return; }
  [[ ${actual%% *} == "$broker_sha" \
     && $dns_id == "$(_omavless_predecessor_file_identity "$dns_receipt" "$owner" 644 8192)" \
     && $core_id == "$(_omavless_predecessor_file_identity "$core" "$owner" 755 134217728)" \
     && $broker_id == "$(_omavless_predecessor_file_identity "$broker" "$owner" 755 33554432)" \
     && $app_id == "$(_omavless_predecessor_file_identity "$app_receipt" "$owner" 644 4096)" \
     && $binary_id == "$(_omavless_predecessor_file_identity "$app" "$owner" 755 268435456)" ]] \
    || { printf 'unknown\n'; return; }
  if [[ $dns_source != "$source" ]]; then printf 'mismatch\n'
  elif [[ $source == "$target" ]]; then printf 'current\n'
  else printf 'stale\n'; fi
)

_omavless_candidate_pair_delivery_status() {
  [[ $# == 1 && $1 =~ ^[0-9a-f]{40}$ ]] || { printf 'unknown\n'; return; }
  local arch
  arch=$(/usr/bin/uname -m) || { printf 'unknown\n'; return; }
  [[ $arch == x86_64 || $arch == aarch64 ]] || { printf 'unknown\n'; return; }
  _omavless_pair_delivery_inspect /usr/share/doc/omavless/build-identity.txt /usr/bin/omavless \
    /usr/share/omavless-dns/source-receipt.json /usr/lib/omavless-dns/mihomo \
    /usr/lib/omavless-dns/omavless-dns-broker "$arch" "$1" 0 0.9.9
}

omavless_pair_delivery_status() {
  [[ ( $# == 1 || $# == 2 ) && $1 =~ ^[0-9a-f]{40}$ ]] || { printf 'unknown\n'; return; }
  local arch policy=${2:-0.9.8}
  case "$policy" in 0.9.8) ;; 0.9.9) policy=public-099 ;; *) printf 'unknown\n'; return ;; esac
  arch=$(/usr/bin/uname -m) || { printf 'unknown\n'; return; }
  [[ $arch == x86_64 || $arch == aarch64 ]] || { printf 'unknown\n'; return; }
  _omavless_pair_delivery_inspect /usr/share/doc/omavless/build-identity.txt /usr/bin/omavless \
    /usr/share/omavless-dns/source-receipt.json /usr/lib/omavless-dns/mihomo \
    /usr/lib/omavless-dns/omavless-dns-broker "$arch" "$1" 0 "$policy"
}
