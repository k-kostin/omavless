#!/bin/bash
# SPDX-License-Identifier: MIT
# Explicit unpublished0.9.9 path. No remote downloads or caller-provided pins.
set -euo pipefail
umask 077
candidate_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
source "$candidate_dir/setup-channel.sh"
source "$candidate_dir/setup-runtime.sh"
readonly release_version=0.9.9 package_version=0.9.9

candidate_selected() { [[ $# == 0 && $(setup_channel) == offline-candidate ]]; }
pair_update_available() { return 1; }
installed_pair_delivery() {
  local version arch app_hash dns_hash producer
  IFS=$'\t' read -r version arch app_hash dns_hash producer < <(release_fields) || return 1
  [[ "$version" == 0.9.9 && "$producer" =~ ^[0-9a-f]{40}$ ]] || return 1
  source "$setup_dir/setup-predecessor.sh"
  _omavless_candidate_pair_delivery_status "$producer"
}
candidate_status() {
  local state
  state=$(setup_status) || return 1
  case "$state" in
    needs_update|needs_pair_update|needs_pair_update_manual|needs_pair_install) printf 'needs_attention\n' ;;
    *) printf '%s\n' "$state" ;;
  esac
}
setup_components() {
  local state core=missing
  state=$(candidate_status) || return 1
  if pair_installed; then core=present; fi
  # No unsupported public-predecessor update advisory in this fresh-only lane.
  printf '%s\t%s\toffline-candidate\n' "$state" "$core"
}

candidate_input_directory() {
  local directory=$1
  [[ ${#directory} -gt 1 && ${#directory} -le 4096 && "$directory" == /* \
     && "$directory" != *[[:cntrl:]]* && -d "$directory" && ! -L "$directory" ]] || return 1
  [[ $(realpath -e -- "$directory") == "$directory" ]] || return 1
  [[ $(stat -c '%u:%a' -- "$directory") == "$(id -u):700" ]] || return 1
  _omavless_predecessor_parents_safe "$directory/entry" "$(id -u)"
}
candidate_file_stamp() { stat -c '%d:%i:%u:%g:%a:%h:%s:%y:%z' -- "$1"; }
obtain_install_archives() (
  # Both complete archives are authenticated before any decompression or copy.
  [[ $# == 5 ]] || return 2
  local app_name=$1 app_hash=$2 dns_name=$3 dns_hash=$4 cache=$5
  local version arch expected_app expected_dns producer directory app_before dns_before temporary='' name hash before
  candidate_selected || return 1
  IFS=$'\t' read -r version arch expected_app expected_dns producer < <(release_fields) || return 1
  [[ "$version" == 0.9.9 && "$app_hash" == "$expected_app" && "$dns_hash" == "$expected_dns" \
     && "$app_name" == "omavless-0.9.9-1-$arch.pkg.tar.zst" \
     && "$dns_name" == "omavless-dns-0.9.9-1-$arch.pkg.tar.zst" \
     && "$cache" == "$HOME/.cache/omavless/upgrade082/$app_hash.$dns_hash" ]] || return 1
  source "$setup_dir/setup-predecessor.sh"
  upgrade_cache_safe "$cache" || return 1
  # Poisoned/redirected existing entries refuse; never replace them.
  for name in "$app_name" "$dns_name"; do
    hash=$app_hash; [[ "$name" != "$dns_name" ]] || hash=$dns_hash
    if [[ -e "$cache/$name" || -L "$cache/$name" ]]; then
      upgrade_archive_safe "$cache/$name" "$hash" || return 1
    fi
  done
  if upgrade_archive_safe "$cache/$app_name" "$app_hash" \
      && upgrade_archive_safe "$cache/$dns_name" "$dns_hash"; then return 0; fi
  say 'Offline development candidate 0.9.9. Nothing will be downloaded.' \
      'Offline-кандидат разработки 0.9.9. Ничего не будет скачано.'
  say 'Enter the absolute path of your private archive folder (mode 700, both package files mode 600). Enter cancels:' \
      'Введите абсолютный путь к личному каталогу архивов (права 700, оба пакета 600). Enter — отмена:'
  IFS= read -r -n 4097 directory || return 1
  candidate_input_directory "$directory" || return 1
  upgrade_archive_safe "$directory/$app_name" "$app_hash" \
    && upgrade_archive_safe "$directory/$dns_name" "$dns_hash" || return 1
  app_before=$(candidate_file_stamp "$directory/$app_name") || return 1
  dns_before=$(candidate_file_stamp "$directory/$dns_name") || return 1
  trap 'if [[ -n "$temporary" ]]; then rm -f -- "$temporary"; fi' EXIT
  for name in "$app_name" "$dns_name"; do
    hash=$app_hash; before=$app_before
    if [[ "$name" == "$dns_name" ]]; then hash=$dns_hash; before=$dns_before; fi
    [[ $(candidate_file_stamp "$directory/$name") == "$before" ]] || return 1
    if [[ -e "$cache/$name" || -L "$cache/$name" ]]; then
      upgrade_archive_safe "$cache/$name" "$hash" || return 1
      continue
    fi
    temporary=$(mktemp "$cache/.offline.XXXXXX") || return 1
    # Bounded copy even if a same-user source changes after validation.
    timeout 30 head -c 268435457 -- "$directory/$name" > "$temporary" || return 1
    upgrade_archive_safe "$temporary" "$hash" \
      && [[ $(candidate_file_stamp "$directory/$name") == "$before" ]] || return 1
    ln -- "$temporary" "$cache/$name" || return 1
    rm -f -- "$temporary" || return 1
    temporary=''
  done
  [[ $(candidate_file_stamp "$directory/$app_name") == "$app_before" \
     && $(candidate_file_stamp "$directory/$dns_name") == "$dns_before" ]] || return 1
  upgrade_archive_safe "$cache/$app_name" "$app_hash" \
    && upgrade_archive_safe "$cache/$dns_name" "$dns_hash"
)
# Defensive refusal: the candidate must never fall through to a public URL,
# even if a future shared caller accidentally selects a download operation.
upgrade_download() { return 1; }

candidate_main() {
  candidate_selected || return 2
  [[ $# -ge 1 && $# -le 2 ]] || return 2
  case "$1" in
    status) [[ $# == 1 ]] || return 2; candidate_status; return ;;
    components) [[ $# == 1 ]] || return 2; setup_components; return ;;
    install|resume-dns|finish-selection|start-broker)
      case "${2:-en}" in en|ru) setup_locale=${2:-en} ;; *) return 2 ;; esac
      say 'Unpublished offline development candidate 0.9.9. This is not the public release installer.' \
          'Неопубликованный offline-кандидат разработки 0.9.9. Это не установщик публичного релиза.' ;;
    *) return 2 ;;
  esac
  setup_main "$@"
}
if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  result=0
  candidate_main "$@" || result=$?
  case "${1:-}" in
    install|resume-dns|finish-selection|start-broker)
      if [[ -t 0 ]]; then say 'Press Enter to close.' 'Нажмите Enter, чтобы закрыть.'; IFS= read -r _ || true; fi ;;
  esac
  exit "$result"
fi
