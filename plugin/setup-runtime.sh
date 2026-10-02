#!/bin/bash
# SPDX-License-Identifier: MIT
# Explicit first-run provisioning, never called by status or plugin loading.
# No Python/Cargo, arbitrary URL, shell eval, private-store repair or VPN action.
set -euo pipefail
umask 077

setup_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
setup_locale=en
release_version=0.9.8-beta.1
package_version=0.9.8beta1

say() { if [[ "$setup_locale" == ru ]]; then printf '%s\n' "$2"; else printf '%s\n' "$1"; fi; }
native() { /usr/bin/omavless "$@"; }
native_present() { [[ -f /usr/bin/omavless && -x /usr/bin/omavless && ! -L /usr/bin/omavless ]]; }
native_target() { timeout 8 /usr/bin/omavless plugin target 2>/dev/null; }
package_info() { /usr/bin/pacman -Qp -- "$1" 2>/dev/null; }
app_archive_identity() {
  local identity
  identity=$(timeout 8 bsdtar -xOf "$1" usr/share/doc/omavless/build-identity.txt 2>/dev/null \
    | head -c 4097) || return 1
  [[ ${#identity} -le 4096 ]] || return 1
  grep -Fxq "schemaVersion=2" <<< "$identity" \
    && grep -Fxq "sourceCommit=$2" <<< "$identity" \
    && grep -Fxq "architecture=$3" <<< "$identity" \
    && grep -Fxq "productVersion=$4" <<< "$identity"
}
dns_archive_identity() {
  local receipt
  receipt=$(timeout 8 bsdtar -xOf "$1" usr/share/omavless-dns/source-receipt.json 2>/dev/null \
    | head -c 4097) || return 1
  [[ ${#receipt} -le 4096 ]] || return 1
  jq -e --arg source "$2" --arg arch "$3" \
    '.schema == 1 and .omavless_commit == $source and .architecture == $arch and .package_flavor == "release" and .broker_feature == "release-package"' \
    >/dev/null 2>&1 <<< "$receipt"
}
package_install() { /usr/bin/sudo /usr/bin/pacman -U -- "$1" "$2"; }
package_installed() { /usr/bin/pacman -Q omavless 2>/dev/null; }
dns_package_registered() { /usr/bin/pacman -Q omavless-dns >/dev/null 2>&1; }
pair_installed() {
  [[ $(/usr/bin/pacman -Q omavless-dns 2>/dev/null) == "omavless-dns $package_version-1" ]] \
    && [[ -f /usr/lib/omavless-dns/mihomo && -x /usr/lib/omavless-dns/mihomo \
          && ! -L /usr/lib/omavless-dns/mihomo \
          && -f /usr/lib/omavless-dns/omavless-dns-broker \
          && -x /usr/lib/omavless-dns/omavless-dns-broker \
          && ! -L /usr/lib/omavless-dns/omavless-dns-broker ]]
}
pair_selection_status() { timeout 8 /usr/bin/omavless dns-pair status 2>/dev/null; }
pair_selected() {
  local selected
  selected=$(pair_selection_status) || return 1
  jq -e '.schemaVersion == 1 and .scope == "local_pair_only" and .selected == true' \
    >/dev/null 2>&1 <<< "$selected"
}
user_runtime_stopped() {
  local state
  state=$(timeout 8 /usr/bin/systemctl --user show omavless-runtime.service --no-pager \
    -p LoadState -p ActiveState -p SubState -p MainPID 2>/dev/null) || return 1
  [[ "$state" == $'LoadState=loaded\nActiveState=inactive\nSubState=dead\nMainPID=0' ]]
}
fresh_user_runtime_absent() {
  local state
  state=$(timeout 8 /usr/bin/systemctl --user show omavless-runtime.service --no-pager \
    -p LoadState -p ActiveState -p SubState -p MainPID 2>/dev/null) || return 1
  [[ "$state" == $'LoadState=not-found\nActiveState=inactive\nSubState=dead\nMainPID=0' ]]
}
system_broker_idle() {
  local state
  state=$(timeout 8 /usr/bin/systemctl --system show omavless-dns-broker.service --no-pager \
    -p LoadState -p ActiveState -p SubState -p NFileDescriptorStore 2>/dev/null) || return 1
  [[ "$state" == $'LoadState=loaded\nActiveState=active\nSubState=running\nNFileDescriptorStore=0' ]]
}
system_broker_available() {
  local state
  state=$(timeout 8 /usr/bin/systemctl --system show omavless-dns-broker.service --no-pager \
    -p LoadState -p ActiveState -p SubState 2>/dev/null) || return 1
  [[ "$state" == $'LoadState=loaded\nActiveState=active\nSubState=running' ]] \
    && broker_access_for_user
}
system_broker_stopped() {
  local state
  state=$(timeout 8 /usr/bin/systemctl --system show omavless-dns-broker.service --no-pager \
    -p LoadState -p ActiveState -p SubState -p MainPID -p NFileDescriptorStore 2>/dev/null) || return 1
  [[ "$state" == $'LoadState=loaded\nActiveState=inactive\nSubState=dead\nMainPID=0\nNFileDescriptorStore=0' ]]
}
no_broker_socket() {
  [[ ! -e /run/omavless-dns/control.sock && ! -L /run/omavless-dns/control.sock ]]
}
broker_access_for_user() {
  local socket=/run/omavless-dns/control.sock
  [[ -S "$socket" && ! -L "$socket" && -w "$socket"
     && $(stat -c %u -- "$socket") == 0 ]]
}
no_managed_tun() { [[ ! -e /sys/class/net/Meta && ! -L /sys/class/net/Meta ]]; }
fresh_package_boundary() {
  # A missing executable is not proof of a fresh machine. Never let the
  # public installer repair/replace an existing or active owner implicitly.
  ! native_present && ! package_installed >/dev/null 2>&1 \
    && ! dns_package_registered && fresh_user_runtime_absent && no_managed_tun
}
enroll_uid() { /usr/bin/sudo /usr/lib/omavless-dns/omavless-dns-broker --enroll "$1"; }
start_broker() { /usr/bin/sudo /usr/bin/systemctl enable --now omavless-dns-broker.service; }
reload_units() { /usr/bin/systemctl --user daemon-reload; }
enable_runtime() { /usr/bin/systemctl --user enable omavless-runtime.service >/dev/null 2>&1; }

release_fields() {
  local app="$setup_dir/runtime-release.json" dns="$setup_dir/dns-release.json" metadata arch app_fields dns_fields app_sha app_source dns_sha dns_source
  for metadata in "$app" "$dns"; do
    [[ -f "$metadata" && ! -L "$metadata" && $(stat -c %s -- "$metadata") -le 8192 ]] || return 1
  done
  arch=$(uname -m)
  [[ "$arch" == aarch64 || "$arch" == x86_64 ]] || return 1
  # Both fixed archives are independently pinned by the reviewed frontend.
  # No remote manifest, latest tag, caller URL or executable content is accepted.
  app_fields=$(jq -er --arg arch "$arch" --arg version "$release_version" '
    select(keys == ["packages", "schemaVersion", "version"] and .schemaVersion == 1)
    | select(.version == $version and (.packages | type) == "object")
    | select((.packages | keys - ["aarch64", "x86_64"] | length) == 0)
    | .packages[$arch]
    | select(type == "object" and keys == ["sha256", "sourceCommit"])
    | select((.sha256 | type) == "string" and (.sha256 | test("^[0-9a-f]{64}$")))
    | select((.sourceCommit | type) == "string" and (.sourceCommit | test("^[0-9a-f]{40}$")))
    | [.sha256, .sourceCommit] | @tsv
  ' "$app" 2>/dev/null) || return 1
  dns_fields=$(jq -er --arg arch "$arch" --arg version "$release_version" '
    select(keys == ["packages", "schemaVersion", "version"] and .schemaVersion == 1)
    | select(.version == $version and (.packages | type) == "object")
    | select((.packages | keys - ["aarch64", "x86_64"] | length) == 0)
    | .packages[$arch]
    | select(type == "object" and keys == ["sha256", "sourceCommit"])
    | select((.sha256 | type) == "string" and (.sha256 | test("^[0-9a-f]{64}$")))
    | select((.sourceCommit | type) == "string" and (.sourceCommit | test("^[0-9a-f]{40}$")))
    | [.sha256, .sourceCommit] | @tsv
  ' "$dns" 2>/dev/null) || return 1
  IFS=$'\t' read -r app_sha app_source <<< "$app_fields"
  IFS=$'\t' read -r dns_sha dns_source <<< "$dns_fields"
  [[ -n "$app_sha" && -n "$app_source" && -n "$dns_sha" && "$app_source" == "$dns_source" ]] || return 1
  printf '%s\t%s\t%s\t%s\t%s\n' "$release_version" "$arch" "$app_sha" "$dns_sha" "$app_source"
}

setup_status() {
  if native_present; then
    local target selected
    [[ $(package_installed) == "omavless $package_version-1" ]] || { printf 'needs_attention\n'; return; }
    target=$(native_target) || { printf 'needs_attention\n'; return; }
    pair_installed || { printf 'needs_companion\n'; return; }
    case "$target" in
      rust)
        selected=$(pair_selection_status) \
          || { printf 'needs_attention\n'; return; }
        if jq -e '.schemaVersion == 1 and .scope == "local_pair_only" and .selected == true' \
          >/dev/null 2>&1 <<< "$selected"; then
          if system_broker_available; then printf 'ready\n'
          elif user_runtime_stopped && no_managed_tun && system_broker_stopped && no_broker_socket; then
            printf 'needs_broker_stopped\n'
          else printf 'needs_broker\n'; fi
        elif jq -e '.schemaVersion == 1 and .scope == "local_pair_only" and .selected == false' \
          >/dev/null 2>&1 <<< "$selected"; then
          if ! system_broker_available; then printf 'needs_broker\n'
          elif ! no_managed_tun; then printf 'needs_attention\n'
          elif user_runtime_stopped; then printf 'needs_selection\n'
          else printf 'needs_runtime_stop\n'; fi
        else
          printf 'needs_attention\n'
        fi
        ;;
      legacy) printf 'needs_activation\n' ;;
      *) printf 'needs_attention\n' ;;
    esac
  elif ! fresh_package_boundary; then
    # An existing package, unit or Meta interface is not a fresh install.
    printf 'needs_attention\n'
  elif release_fields >/dev/null; then
    printf 'needs_package\n'
  else
    printf 'release_unavailable\n'
  fi
}

# Two fixed fields, no paths, versions, native snapshots or health claims.
setup_components() {
  local state core=missing
  state=$(setup_status)
  if pair_installed; then core=present; fi
  printf '%s\t%s\n' "$state" "$core"
}

confirm() {
  local answer
  say 'Type INSTALL to continue, or press Enter to cancel:' 'Введите INSTALL для продолжения или Enter для отмены:'
  IFS= read -r answer || return 1
  [[ "$answer" == INSTALL ]]
}

confirm_dns_enrollment() {
  local answer uid
  uid=$(id -u)
  [[ "$uid" =~ ^[1-9][0-9]*$ ]] || return 1
  say "The managed DNS broker will enroll desktop UID $uid, then its system service will be enabled and started. This does not connect a VPN. Type DNS to authorize this separate administrator step; Enter cancels." \
      "DNS-брокер зарегистрирует пользователя с UID $uid, затем его системная служба будет включена и запущена. VPN не подключается. Для отдельного разрешения администратора введите DNS; Enter отменяет."
  IFS= read -r answer || return 1
  [[ "$answer" == DNS ]]
}

enroll_and_select_pair() {
  pair_installed || return 1
  user_runtime_stopped || return 1
  no_managed_tun || return 1
  confirm_dns_enrollment || return 1
  # The broker itself requires the exact empty-state package guard. Never
  # recover an unknown result by silently repeating this one-time action.
  enroll_uid "$(id -u)" || return 1
  start_broker || return 1
  system_broker_idle || return 1
  broker_access_for_user || return 1
  native dns-pair prepare-template >/dev/null 2>&1 || return 1
  native dns-pair select >/dev/null 2>&1 || return 1
  pair_selected
}

finish_pair_selection() {
  pair_installed || return 1
  system_broker_idle || return 1
  broker_access_for_user || return 1
  user_runtime_stopped || return 1
  no_managed_tun || return 1
  local answer
  say 'Only the exact bundled default DNS template can be prepared. Type SELECT to choose the installed managed pair; Enter cancels.' \
      'Можно подготовить только стандартный DNS-шаблон без изменений. Введите SELECT, чтобы выбрать установленную управляемую пару; Enter отменяет.'
  IFS= read -r answer || return 1
  [[ "$answer" == SELECT ]] || return 1
  native dns-pair prepare-template >/dev/null 2>&1 || return 1
  native dns-pair select >/dev/null 2>&1 || return 1
  pair_selected
}

restore_enrollment() {
  # Only for a preserved Rust store/selector after the old enrollment was
  # explicitly revoked. The root broker's fixed empty-state guard is the
  # authority: an existing enrollment, journal, lease or unknown state refuses.
  pair_installed || return 1
  [[ $(native_target) == rust ]] || return 1
  pair_selected || return 1
  user_runtime_stopped || return 1
  no_managed_tun || return 1
  system_broker_stopped || return 1
  no_broker_socket || return 1
  confirm_dns_enrollment || return 1
  enroll_uid "$(id -u)" || return 1
  start_broker || return 1
  system_broker_idle || return 1
  broker_access_for_user || return 1
  pair_selected
}

existing_enrollment_metadata() {
  local metadata
  # A missing enrollment must not turn an attempted service start into a
  # failed systemd unit, which would hide the separate recovery action.
  # This fixed-path root read returns only metadata, never the UID or content.
  metadata=$(/usr/bin/sudo /usr/bin/env -i PATH=/usr/bin:/bin LC_ALL=C \
    /usr/bin/stat -c '%F:%u:%a:%h' -- \
    /etc/omavless-dns/release-enrollment.json 2>/dev/null) || return 1
  [[ "$metadata" == 'regular file:0:600:1' ]]
}
start_stopped_broker_service() {
  /usr/bin/sudo /usr/bin/systemctl --system start omavless-dns-broker.service
}

start_existing_broker() {
  pair_installed || return 1
  [[ $(native_target) == rust ]] || return 1
  pair_selected || return 1
  user_runtime_stopped || return 1
  no_managed_tun || return 1
  system_broker_stopped || return 1
  no_broker_socket || return 1
  existing_enrollment_metadata || return 1
  start_stopped_broker_service || return 1
  system_broker_idle || return 1
  broker_access_for_user || return 1
  pair_selected
}

install_package() {
  local version arch app_hash dns_hash source app_package dns_package url actual
  fresh_package_boundary || return 1
  IFS=$'\t' read -r version arch app_hash dns_hash source < <(release_fields)
  [[ "$version" == "$release_version" && -n "$arch" && -n "$app_hash" && -n "$dns_hash" && -n "$source" ]] || return 1
  app_package="omavless-${package_version}-1-${arch}.pkg.tar.zst"
  dns_package="omavless-dns-${package_version}-1-${arch}.pkg.tar.zst"
  say 'Downloading the pinned OmaVLESS application and managed DNS packages…' \
      'Загрузка проверенных пакетов приложения OmaVLESS и управляемого DNS…'
  # HTTPS redirects are required by GitHub's asset delivery. Only the exact
  # pinned archive hashes authorize the downloaded payloads for pacman.
  url="https://github.com/k-kostin/omavless/releases/download/v${version}/${app_package}"
  curl --disable --fail --silent --show-error --location --max-redirs 3 \
    --proto '=https' --proto-redir '=https' --connect-timeout 15 --max-time 180 \
    --max-filesize 268435456 --output "$setup_temp/$app_package" "$url" 2>/dev/null || return 1
  actual=$(sha256sum -- "$setup_temp/$app_package")
  [[ "${actual%% *}" == "$app_hash" ]] || return 1
  [[ $(package_info "$setup_temp/$app_package") == "omavless $package_version-1" ]] || return 1
  app_archive_identity "$setup_temp/$app_package" "$source" "$arch" "$version" || return 1
  url="https://github.com/k-kostin/omavless/releases/download/v${version}/${dns_package}"
  curl --disable --fail --silent --show-error --location --max-redirs 3 \
    --proto '=https' --proto-redir '=https' --connect-timeout 15 --max-time 180 \
    --max-filesize 536870912 --output "$setup_temp/$dns_package" "$url" 2>/dev/null || return 1
  actual=$(sha256sum -- "$setup_temp/$dns_package")
  [[ "${actual%% *}" == "$dns_hash" ]] || return 1
  [[ $(package_info "$setup_temp/$dns_package") == "omavless-dns $package_version-1" ]] || return 1
  dns_archive_identity "$setup_temp/$dns_package" "$source" "$arch" || return 1
  fresh_package_boundary || return 1
  # Normal dependency and user confirmation checks. No --nodeps/--overwrite,
  # --noconfirm, package hooks added by this script, or background sudo.
  package_install "$setup_temp/$app_package" "$setup_temp/$dns_package" || return 1
  [[ $(package_installed) == "omavless $package_version-1" ]] && pair_installed
}

prepare_private_store() {
  local target config
  target=$(native_target) || return 1
  # Already activated: never reset, restart or re-enable after an explicit Quit.
  [[ "$target" != rust ]] || return 0
  [[ "$target" == legacy ]] || return 1
  # Match the canonical Rust store, which does not use XDG_CONFIG_HOME.
  config="$HOME/.config/omavless/profiles.json"
  if [[ ! -e "$config" && ! -L "$config" ]]; then
    native setup initialize >/dev/null 2>&1 || return 1
  fi
  # Existing data is validated by the canonical Rust owner; never parsed,
  # repaired, reset or overwritten by shell. Connected/enabled legacy owners,
  # malformed stores and incomplete prior transitions refuse safely.
  native store-compatibility 2>/dev/null | jq -e '.schemaVersion == 1 and .compatible == true' >/dev/null || return 1
}

prepare_application() {
  local target
  target=$(native_target) || return 1
  [[ "$target" != rust ]] || return 0
  [[ "$target" == legacy ]] || return 1
  # The candidate runtime must resolve the reviewed bundled core during
  # cutover. Activating before local pair selection fails and rolls back.
  pair_selected || return 1
  system_broker_available || return 1
  native store-compatibility 2>/dev/null | jq -e '.schemaVersion == 1 and .compatible == true' >/dev/null || return 1
  native cutover activate >/dev/null 2>&1 || return 1
  [[ $(native_target) == rust ]] || return 1
  enable_runtime || return 1
}

setup_main() {
  [[ $# -ge 1 && $# -le 2 ]] || return 2
  case "${2:-en}" in en|ru) setup_locale=${2:-en} ;; *) return 2 ;; esac
  case "$1" in
    status) [[ $# == 1 ]] || return 2; setup_status; return ;;
    components) [[ $# == 1 ]] || return 2; setup_components; return ;;
    install|finish-selection|start-broker|restore-enrollment) ;;
    *) return 2 ;;
  esac
  [[ -t 0 && -t 1 && $EUID -ne 0 ]] || return 2
  [[ ! ${OMAVLESS_HOME+x} ]] || return 2
  local state
  state=$(setup_status)
  if [[ "$1" == install && "$state" == ready ]]; then
    say 'OmaVLESS is already prepared. No changes made.' 'OmaVLESS уже настроен. Ничего не изменено.'
    return
  fi
  if [[ "$1" == finish-selection && "$state" != needs_selection ]] \
      || [[ "$1" == start-broker && "$state" != needs_broker_stopped ]] \
      || [[ "$1" == restore-enrollment && "$state" != needs_broker_stopped ]] \
      || [[ "$1" == install && "$state" != needs_package && "$state" != needs_activation ]]; then
    say 'Setup is unavailable. No changes made. Check the setup guide.' 'Установка недоступна. Ничего не изменено. Откройте руководство.'
    return 1
  fi
  if [[ "$1" == start-broker ]]; then
    say 'Start the existing DNS broker after a clean stop or package update. Enrollment, profiles and startup stay unchanged; no VPN connection is started.' \
        'Запустить существующий DNS-брокер после чистой остановки или обновления пакета. Регистрация, профили и автозапуск не меняются; VPN не подключается.'
  elif [[ "$1" == restore-enrollment ]]; then
    say 'Restore broker enrollment only after a clean removal. The selected pair and profiles stay unchanged; no VPN connection or startup is enabled.' \
        'Восстановить регистрацию DNS-брокера только после чистого удаления. Выбранная пара и профили сохранятся; VPN и автозапуск не включаются.'
  elif [[ "$1" == finish-selection ]]; then
    say 'Finish managed DNS selection while disconnected. No package or enrollment will be changed.' \
        'Завершить выбор управляемого DNS без подключения. Пакеты и регистрация пользователя не изменятся.'
  else
    say 'Set up OmaVLESS' 'Настройка OmaVLESS'
    say 'Install the matching application and managed DNS packages if missing, then prepare the private settings and services. No VPN connection will be started.' \
      'Установить недостающие пакеты приложения и управляемого DNS, затем подготовить приватные настройки и службы. VPN подключаться не будет.'
    say 'Existing profiles are preserved. Existing VPN/startup must be off before migration. Passwords go only to sudo, not this prompt.' \
      'Существующие профили сохраняются. Перед переносом отключите VPN и автоподключение. Пароль вводите только в sudo, не здесь.'
    say 'If your firewall denies incoming traffic by default, TUN may need a reviewed Meta-interface exception. Setup never changes firewall rules; read the setup guide before connecting.' \
      'Если межсетевой экран по умолчанию блокирует входящий трафик, для TUN может потребоваться проверенное исключение интерфейса Meta. Установка не меняет правила экрана; прочитайте руководство перед подключением.'
    confirm || { say 'Cancelled. No changes made.' 'Отменено. Ничего не изменено.'; return; }
  fi
  # One transaction per user runtime directory. A stale lock after a killed
  # terminal is deliberately not removed automatically on the next attempt.
  local runtime=${XDG_RUNTIME_DIR:-} setup_lock setup_temp
  [[ "$runtime" == "/run/user/$(id -u)" && -d "$runtime" && ! -L "$runtime" ]] || return 1
  [[ $(stat -c '%u:%a' "$runtime") == "$(id -u):700" ]] || return 1
  setup_lock="$runtime/omavless-first-run.lock"
  mkdir -- "$setup_lock" 2>/dev/null || return 1
  setup_temp=$(mktemp -d "$runtime/omavless-first-run.XXXXXX") || { rmdir -- "$setup_lock"; return 1; }
  # A subshell keeps the cleanup trap and private download scope local.
  (
    trap 'rm -f -- "$setup_temp/omavless-${package_version}-1-aarch64.pkg.tar.zst" "$setup_temp/omavless-${package_version}-1-x86_64.pkg.tar.zst" "$setup_temp/omavless-dns-${package_version}-1-aarch64.pkg.tar.zst" "$setup_temp/omavless-dns-${package_version}-1-x86_64.pkg.tar.zst"; rmdir -- "$setup_temp" "$setup_lock"' EXIT
    # Recheck after consent and the lock. Never replace a package that appeared
    # while the user was reading the prompt, or activate a now-unknown owner.
    [[ $(setup_status) == "$state" ]] || exit 1
    if [[ "$1" == finish-selection ]]; then
      finish_pair_selection || exit 1
    elif [[ "$1" == start-broker ]]; then
      start_existing_broker || exit 1
    elif [[ "$1" == restore-enrollment ]]; then
      restore_enrollment || exit 1
    else
      if [[ "$state" == needs_package ]]; then
        install_package || exit 1
        reload_units || exit 1
      fi
      prepare_private_store || exit 1
      selected=$(pair_selection_status) || exit 1
      if jq -e '.schemaVersion == 1 and .scope == "local_pair_only" and .selected == false' \
        >/dev/null 2>&1 <<< "$selected"; then
        if system_broker_idle; then
          # A prior enrollment may have succeeded before selection was
          # interrupted. Resume only if this UID can access the exact live
          # broker socket; never repeat or replace root enrollment blindly.
          broker_access_for_user || exit 1
          finish_pair_selection || exit 1
        else
          enroll_and_select_pair || exit 1
        fi
      else
        pair_selected || exit 1
      fi
      prepare_application || exit 1
    fi
    [[ $(setup_status) == ready ]] || exit 1
    say 'OmaVLESS is ready. Return to the plugin and press Check again.' 'OmaVLESS готов. Вернитесь в плагин и нажмите «Проверить снова».'
    say 'After connecting, omavless runtime test checks HTTPS on the current route. It is not a VPN leak test.' \
        'После подключения omavless runtime test проверяет HTTPS на текущем маршруте. Это не проверка утечки VPN.'
  ) || {
    say 'Setup did not finish. Do not repeat an unresolved authorization. Existing data was not reset; inspect the setup guide before retrying.' \
        'Настройка не завершена. Не повторяйте незавершённую авторизацию. Данные не сбрасывались; перед повтором откройте руководство.'
    return 1
  }
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  result=0
  setup_main "$@" || result=$?
  if [[ ( "${1:-}" == install || "${1:-}" == finish-selection || "${1:-}" == start-broker || "${1:-}" == restore-enrollment ) && -t 0 ]]; then
    say 'Press Enter to close.' 'Нажмите Enter, чтобы закрыть.'
    IFS= read -r _ || true
  fi
  exit "$result"
fi
