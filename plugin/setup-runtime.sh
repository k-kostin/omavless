#!/bin/bash
# SPDX-License-Identifier: MIT
# Explicit first-run provisioning, never called by status or plugin loading.
# No Python/Cargo, arbitrary URL, shell eval, private-store repair or VPN action.
set -euo pipefail
umask 077

setup_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
setup_locale=en
setup_failure_reported=false
release_version=0.9.8
package_version=0.9.8
# Corrected validation delivery; the original v0.9.8 assets stay immutable.
# A fixed repository/tag plus exact archive pins, never caller-supplied URLs.
readonly release_tag=v0.9.8-fix.8

say() { if [[ "$setup_locale" == ru ]]; then printf '%s\n' "$2"; else printf '%s\n' "$1"; fi; }
native() { /usr/bin/omavless "$@"; }
native_quiet() { native "$@" >/dev/null 2>&1; }
# Fixed public stage identifiers only; never print command arguments, private
# native output or rejected input. Reporting does not authorize retry/recovery.
setup_step() {
  local stage="$1"
  shift
  case "$stage" in
    runtime-directory|setup-lock|temporary-directory|setup-state|package-install|upgrade-artifacts|upgrade-operation|unit-reload|private-settings|private-initialization|store-validation|dns-pair-status|dns-preconditions|dns-consent|dns-enrollment|dns-broker-start|dns-broker-readiness|dns-socket-access|dns-template|dns-selection|dns-selection-confirmation|broker-enrollment-check|application-preconditions|application-activation|runtime-enablement|completion) ;;
    *) return 2 ;;
  esac
  if "$@"; then return 0; fi
  if [[ "$setup_failure_reported" == false ]]; then
    setup_failure_reported=true
    say "Setup stopped at stage: $stage." "Настройка остановлена на этапе: $stage."
  fi
  return 1
}
store_compatible() {
  native store-compatibility 2>/dev/null \
    | jq -e '.schemaVersion == 1 and .compatible == true' >/dev/null
}
setup_runtime_directory_safe() {
  local runtime="$1"
  [[ "$runtime" == "/run/user/$(id -u)" && -d "$runtime" && ! -L "$runtime" ]] \
    && [[ $(stat -c '%u:%a' "$runtime") == "$(id -u):700" ]]
}
native_present() { [[ -f /usr/bin/omavless && -x /usr/bin/omavless && ! -L /usr/bin/omavless ]]; }
native_target() { timeout 8 /usr/bin/omavless plugin target 2>/dev/null; }
package_info() { /usr/bin/pacman -Qp -- "$1" 2>/dev/null; }
app_archive_identity() {
  local identity
  identity=$(timeout 8 bsdtar -xOf "$1" usr/share/doc/omavless/build-identity.txt 2>/dev/null \
    | head -c 4097) || return 1
  [[ ${#identity} -le 4096 ]] || return 1
  grep -Fxq "schemaVersion=3" <<< "$identity" \
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
pending_dns_boundary() {
  local selected target
  pair_installed && user_runtime_stopped && no_managed_tun \
    && system_broker_stopped && no_broker_socket || return 1
  target=$(native_target) || return 1
  [[ "$target" == rust || "$target" == legacy ]] || return 1
  selected=$(pair_selection_status) || return 1
  jq -e '.schemaVersion == 1 and .scope == "local_pair_only" and .selected == false' \
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
runtime_start_available() { timeout 8 /usr/bin/omavless app can-start >/dev/null 2>&1; }
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
# The release unit uses Type=notify for initial readiness. Later fixed status
# queries may still be transiently unavailable. Observe only fixed facts after
# the original successful start; never
# repeat enrollment/start or turn a readiness timeout into cleanup/recovery.
# The five-second window also retains each existing bounded query's timeout.
wait_broker_fact() {
  [[ $# == 1 ]] || return 2
  local fact="$1" setup_broker_deadline=$((SECONDS + 5))
  case "$fact" in idle|socket) ;; *) return 2 ;; esac
  while (( SECONDS < setup_broker_deadline )); do
    if [[ "$fact" == idle ]]; then
      if system_broker_idle && (( SECONDS < setup_broker_deadline )); then return 0; fi
    elif system_broker_idle && broker_access_for_user \
        && (( SECONDS < setup_broker_deadline )); then
      return 0
    fi
    (( SECONDS < setup_broker_deadline )) || break
    sleep 0.1 || return 1
  done
  return 1
}
await_broker_ready() {
  [[ $# == 0 ]] || return 2
  setup_step dns-broker-readiness wait_broker_fact idle || return 1
  setup_step dns-socket-access wait_broker_fact socket
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
    local target selected installed
    installed=$(package_installed) || { printf 'needs_attention\n'; return; }
    if [[ "$installed" != "omavless $package_version-1" ]]; then
      # Navigation/diagnosis only, NOT updater admission or byte provenance.
      # The prior public Rust application cannot answer managed-DNS commands.
      if [[ "$installed" == 'omavless 0.8.2-1' ]] && [[ $(native_target) == rust ]]; then
        printf 'needs_update\n'
      else printf 'needs_attention\n'; fi
      return
    fi
    target=$(native_target) || { printf 'needs_attention\n'; return; }
    if [[ "$target" == rust ]]; then
      local delivery
      delivery=$(installed_pair_delivery)
      case "$delivery" in
        # Delivery freshness is advisory. Keep connection and stopped-runtime
        # continuation available for a verified compatible older pair.
        stale) ;;
        missing)
          if pair_update_available; then printf 'needs_pair_install\n'; else printf 'needs_companion\n'; fi
          return ;;
        mismatch) printf 'needs_pair_mismatch\n'; return ;;
        current) ;;
        *) printf 'needs_attention\n'; return ;;
      esac
    fi
    pair_installed || { printf 'needs_companion\n'; return; }
    case "$target" in
      rust)
        selected=$(pair_selection_status) \
          || { printf 'needs_attention\n'; return; }
        if jq -e '.schemaVersion == 1 and .scope == "local_pair_only" and .selected == true' \
          >/dev/null 2>&1 <<< "$selected"; then
          # Preserve the existing stopped-broker repair path. Starting the
          # user runtime first would make that stricter path unavailable.
          if ! system_broker_available; then
            if user_runtime_stopped && no_managed_tun && system_broker_stopped && no_broker_socket; then
              printf 'needs_broker_stopped\n'
            else printf 'needs_broker\n'; fi
          elif user_runtime_stopped; then
            if runtime_start_available; then
              printf 'needs_runtime_start\n'; return
            fi
            printf 'needs_attention\n'; return
          else
            printf 'ready\n'
          fi
        elif jq -e '.schemaVersion == 1 and .scope == "local_pair_only" and .selected == false' \
          >/dev/null 2>&1 <<< "$selected"; then
          if pending_dns_boundary; then printf 'needs_dns_setup\n'
          elif ! system_broker_available; then printf 'needs_broker\n'
          elif ! no_managed_tun; then printf 'needs_attention\n'
          elif user_runtime_stopped; then printf 'needs_selection\n'
          else printf 'needs_runtime_stop\n'; fi
        else
          printf 'needs_attention\n'
        fi
        ;;
      legacy)
        if pending_dns_boundary; then printf 'needs_dns_setup\n'
        else printf 'needs_activation\n'; fi ;;
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

installed_pair_delivery() {
  local version arch app_hash dns_hash source delivery
  IFS=$'\t' read -r version arch app_hash dns_hash source < <(release_fields)
  [[ "$version" == "$release_version" && "$source" =~ ^[0-9a-f]{40}$ ]] \
    || { printf 'unknown\n'; return; }
  source "$setup_dir/setup-predecessor.sh"
  delivery=$(omavless_pair_delivery_status "$source")
  # Absence of payloads is not absence of a package registration.
  if [[ "$delivery" == missing ]] && dns_package_registered; then delivery=unknown; fi
  printf '%s\n' "$delivery"
}

pair_update_available() {
  local version arch app_hash dns_hash source metadata="$setup_dir/component-updates.json"
  [[ -f "$metadata" && ! -L "$metadata" && $(stat -c %s -- "$metadata") -le 8192 ]] || return 1
  IFS=$'\t' read -r version arch app_hash dns_hash source < <(release_fields)
  [[ "$source" =~ ^[0-9a-f]{40}$ ]] || return 1
  jq -e --arg source "$source" --arg arch "$arch" '
    select(keys == ["identities", "schemaVersion", "sourceCommit", "updatePair"]
      and .schemaVersion == 2 and .sourceCommit == $source and .updatePair == true)
    | .identities | select(keys == ["aarch64", "x86_64"])
    | select(all(.[]; keys == ["appSha", "brokerSha", "coreSha", "dnsReceiptSha"]
        and all(.[]; type == "string" and test("^[0-9a-f]{64}$")))) | .[$arch]
    | select(keys == ["appSha", "brokerSha", "coreSha", "dnsReceiptSha"])
    | [.appSha, .coreSha, .brokerSha, .dnsReceiptSha]
    | all(.[]; type == "string" and test("^[0-9a-f]{64}$"))' "$metadata" >/dev/null 2>&1
}

# Fixed enums only. Freshness is independent of operational readiness.
setup_components() {
  local state core=missing delivery=current
  state=$(setup_status)
  if pair_installed; then core=present; fi
  if [[ $(package_installed) == "omavless $package_version-1" ]] && [[ $(native_target) == rust ]] \
      && [[ $(installed_pair_delivery) == stale ]]; then
    if pair_update_available; then delivery=stale; else delivery=stale_manual; fi
  fi
  if [[ "$delivery" == current ]]; then printf '%s\t%s\n' "$state" "$core"
  else printf '%s\t%s\t%s\n' "$state" "$core" "$delivery"; fi
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
  say "Allow the OmaVLESS DNS helper for your account (UID $uid)." \
      "Разрешить DNS-помощник OmaVLESS для вашей учётной записи (UID $uid)."
  say 'Start its service now and automatically when the computer boots. The VPN stays off.' \
      'Запустить его службу сейчас и автоматически при включении компьютера. VPN останется выключенным.'
  say 'Type DNS to confirm this separate administrator step; Enter cancels.' \
      'Введите DNS для подтверждения этого отдельного шага с правами администратора; Enter — отмена.'
  IFS= read -r answer || return 1
  [[ "$answer" == DNS ]]
}

enroll_and_select_pair() {
  pair_installed || return 1
  user_runtime_stopped || return 1
  no_managed_tun || return 1
  setup_step dns-consent confirm_dns_enrollment || return 1
  # The broker itself requires the exact empty-state package guard. Never
  # recover an unknown result by silently repeating this one-time action.
  setup_step dns-enrollment enroll_uid "$(id -u)" || return 1
  setup_step dns-broker-start start_broker || return 1
  await_broker_ready || return 1
  setup_step dns-template native_quiet dns-pair prepare-template || return 1
  setup_step dns-selection native_quiet dns-pair select || return 1
  setup_step dns-selection pair_selected
}

finish_pair_selection() {
  pair_installed || return 1
  system_broker_idle || return 1
  broker_access_for_user || return 1
  user_runtime_stopped || return 1
  no_managed_tun || return 1
  local answer
  say 'Use the installed VPN core and DNS helper with the default settings. Custom DNS settings will not be overwritten.' \
      'Использовать установленное ядро VPN и DNS-помощник со стандартными настройками. Ваши собственные настройки DNS не перезаписываются.'
  say 'Type SELECT to confirm; Enter cancels.' \
      'Введите SELECT для подтверждения; Enter — отмена.'
  IFS= read -r answer || return 1
  [[ "$answer" == SELECT ]] || return 1
  setup_step dns-template native_quiet dns-pair prepare-template || return 1
  setup_step dns-selection native_quiet dns-pair select || return 1
  setup_step dns-selection pair_selected
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
  setup_step dns-consent confirm_dns_enrollment || return 1
  setup_step dns-enrollment enroll_uid "$(id -u)" || return 1
  setup_step dns-broker-start start_broker || return 1
  await_broker_ready || return 1
  setup_step dns-selection pair_selected
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
  setup_step broker-enrollment-check existing_enrollment_metadata || return 1
  setup_step dns-broker-start start_stopped_broker_service || return 1
  await_broker_ready || return 1
  setup_step dns-selection pair_selected
}

dns_choice_prompt() {
  say 'Choose how to finish setting up the DNS helper:' 'Выберите, как завершить настройку DNS-помощника:'
  say '' ''
  say '  FIRST — first-time setup: the OmaVLESS DNS helper has not been authorized yet.' \
    '  FIRST — первая настройка: вы ещё не разрешали работу DNS-помощника OmaVLESS.'
  say '          You will confirm this separately. A sudo password may be needed.' \
    '          Это нужно будет подтвердить отдельно. Может потребоваться пароль sudo.'
  say '' ''
  say '  START — the DNS helper was already set up successfully for your account on this computer.' \
    '  START — DNS-помощник на этом компьютере уже успешно настроен для вашей учётной записи.'
  say '          Reuse that setup without changing it.' \
    '          Использовать эту настройку без изменений.'
  say '' ''
  say 'Press Enter to cancel. No VPN will connect; saved profiles are kept.' \
    'Enter — отмена. VPN не подключится; сохранённые профили не изменятся.'
  say 'If an earlier attempt had an unknown result, inspect it before choosing either option.' \
    'Если результат прошлой попытки неизвестен, сначала проверьте его, не повторяйте настройку.'
  say 'Type FIRST or START, then press Enter:' 'Введите FIRST или START и нажмите Enter:'
}

resume_pending_dns() {
  pending_dns_boundary || return 1
  local choice
  dns_choice_prompt
  IFS= read -r choice || return 1
  [[ "$choice" == START || "$choice" == FIRST ]] || return 1
  pending_dns_boundary || return 1
  setup_step private-settings prepare_private_store || return 1
  if [[ "$choice" == START ]]; then
    # Failure/absence here NEVER means permission to enroll. The user must
    # explicitly choose FIRST; the root broker then earns its empty-state guard.
    setup_step broker-enrollment-check existing_enrollment_metadata || return 1
    setup_step dns-broker-start start_stopped_broker_service || return 1
    setup_step dns-broker-readiness await_broker_ready || return 1
    setup_step dns-selection-confirmation finish_pair_selection || return 1
  else
    setup_step dns-preconditions enroll_and_select_pair || return 1
  fi
  setup_step application-preconditions prepare_application
}

# Optional frontend refresh after a completed package-only update. Never part
# of native updater authority, automatic installation or failure recovery.
offer_shell_reload() {
  [[ -t 0 && -t 1 ]] || return 0
  say '' ''
  say 'The packages are updated. The running shell may still show the old plugin.' \
    'Пакеты обновлены. Работающая оболочка может ещё показывать старый плагин.'
  say 'Optional: reload the entire Omarchy shell so it reads the new plugin.' \
    'Можно перезагрузить всю оболочку Omarchy, чтобы загрузить новый плагин.'
  say 'The bar and panels will briefly disappear; other shell plugins also reload.' \
    'Бар и панели ненадолго исчезнут; другие плагины оболочки тоже перезагрузятся.'
  say 'First finish all setup and OS authorization prompts. This is not VPN recovery.' \
    'Сначала завершите все запросы установки и авторизации ОС. Это не восстановление VPN.'
  say 'Type RELOAD to reload now, or press Enter to leave the shell unchanged:' \
    'Введите RELOAD для перезагрузки сейчас или Enter, чтобы ничего не менять:'
  local choice
  IFS= read -r -n 33 choice || return 0
  [[ "$choice" == RELOAD ]] || return 0
  # Re-earn the current stopped setup facts after the user's decision. Do not
  # restart during a newly started runtime, completed DNS setup or uncertainty.
  local runtime=${XDG_RUNTIME_DIR:-}
  if ! setup_runtime_directory_safe "$runtime" \
      || [[ -e "$runtime/omavless-first-run.lock" || -L "$runtime/omavless-first-run.lock" ]] \
      || [[ $(setup_status) != needs_dns_setup ]] || ! user_runtime_stopped; then
    say 'Reload skipped: setup state changed. Check the plugin; do not repeat UPGRADE.' \
      'Перезагрузка пропущена: состояние изменилось. Проверьте плагин; не повторяйте UPGRADE.'
    return 0
  fi
  if omarchy restart shell; then
    say 'Shell reloaded.' 'Оболочка перезагружена.'
    return_to_panel
    say 'Then choose "Set up DNS helper".' 'Затем выберите «Настроить DNS-помощник».'
  else
    say 'Shell reload did not complete. Packages remain updated; do not repeat UPGRADE.' \
      'Перезагрузка оболочки не завершилась. Пакеты обновлены; не повторяйте UPGRADE.'
    say 'After resolving the shell problem, run: omarchy restart shell' \
      'После устранения проблемы оболочки выполните: omarchy restart shell'
  fi
}

obtain_install_archives() {
  [[ $# == 5 ]] || return 2
  upgrade_download "$1" "$2" "$5" && upgrade_download "$3" "$4" "$5"
}

install_package() {
  local version arch app_hash dns_hash source app_package dns_package cache
  fresh_package_boundary || return 1
  IFS=$'\t' read -r version arch app_hash dns_hash source < <(release_fields)
  [[ "$version" == "$release_version" && -n "$arch" && -n "$app_hash" && -n "$dns_hash" && -n "$source" ]] || return 1
  app_package="omavless-${package_version}-1-${arch}.pkg.tar.zst"
  dns_package="omavless-dns-${package_version}-1-${arch}.pkg.tar.zst"
  cache=$(setup_release_cache "$app_hash" "$dns_hash") || return 1
  say 'Checking the application, VPN core and DNS helper packages…' 'Проверка пакетов приложения, ядра VPN и DNS-помощника…'
  # Cache presence is not trust. Reuse only the same fixed reviewed pins and
  # authenticated regular archives as upgrade; a poisoned entry refuses.
  obtain_install_archives "$app_package" "$app_hash" "$dns_package" "$dns_hash" "$cache" || return 1
  [[ $(package_info "$cache/$app_package") == "omavless $package_version-1" \
     && $(package_info "$cache/$dns_package") == "omavless-dns $package_version-1" ]] || return 1
  app_archive_identity "$cache/$app_package" "$source" "$arch" "$version" \
    && dns_archive_identity "$cache/$dns_package" "$source" "$arch" || return 1
  fresh_package_boundary || return 1
  upgrade_archive_safe "$cache/$app_package" "$app_hash" \
    && upgrade_archive_safe "$cache/$dns_package" "$dns_hash" || return 1
  # Normal dependency and user confirmation checks. No --nodeps/--overwrite,
  # --noconfirm, package hooks added by this script, or background sudo.
  package_install "$cache/$app_package" "$cache/$dns_package" || return 1
  [[ $(package_installed) == "omavless $package_version-1" ]] && pair_installed
}

# The frontend authenticates exact release pins. The extracted candidate then
# independently qualifies the predecessor/private boundary, retains original
# owner/migration leases and verifies both archives plus its own ELF. Metadata
# is closed DATA, never caller-supplied commands or an authorization token.
upgrade_cache_safe() {
  [[ $# == 1 ]] || return 2
  local path="$1" metadata
  [[ -d "$path" && ! -L "$path" ]] || return 1
  metadata=$(stat -c '%u:%a' -- "$path") || return 1
  [[ "$metadata" == "$(id -u):700" ]] || return 1
  _omavless_predecessor_parents_safe "$path/entry" "$(id -u)"
}
upgrade_archive_safe() {
  [[ $# == 2 ]] || return 2
  local metadata actual
  [[ -f "$1" && ! -L "$1" ]] || return 1
  metadata=$(stat -c '%u:%a:%h:%s' -- "$1") || return 1
  [[ "$metadata" =~ ^$(id -u):600:1:([0-9]+)$ ]] || return 1
  (( BASH_REMATCH[1] > 0 && BASH_REMATCH[1] <= 268435456 )) || return 1
  actual=$(timeout 15 sha256sum -- "$1") || return 1
  [[ "${actual%% *}" == "$2" ]]
}
upgrade_download() (
  local name="$1" hash="$2" cache="$3" actual candidate
  if [[ -e "$cache/$name" || -L "$cache/$name" ]]; then
    upgrade_archive_safe "$cache/$name" "$hash"; return
  fi
  candidate=$(mktemp "$cache/.download.XXXXXX") || return 1
  trap 'rm -f -- "$candidate"' EXIT
  curl --disable --fail --silent --show-error --location --max-redirs 3 \
    --proto '=https' --proto-redir '=https' --connect-timeout 15 --max-time 180 \
    --max-filesize 268435456 --output "$candidate" \
    "https://github.com/k-kostin/omavless/releases/download/${release_tag}/${name}" 2>/dev/null || return 1
  actual=$(sha256sum -- "$candidate") || return 1
  [[ "${actual%% *}" == "$hash" ]] || return 1
  # Never replace an artifact appearing during the download.
  # Publish completed verified bytes atomically on the same filesystem. A
  # failed download never reserves the final name. link() does not replace an
  # artifact appearing concurrently; unlink our own temporary before recheck.
  ln -- "$candidate" "$cache/$name" 2>/dev/null || return 1
  rm -f -- "$candidate" || return 1
  upgrade_archive_safe "$cache/$name" "$hash"
)
setup_release_cache() {
  [[ $# == 2 && "$1" =~ ^[0-9a-f]{64}$ && "$2" =~ ^[0-9a-f]{64}$ ]] || return 2
  local cache="$HOME/.cache/omavless/upgrade082/$1.$2" parent="$HOME/.cache"
  source "$setup_dir/setup-predecessor.sh"
  # This shared directory is still selected only by the reviewed pair pins.
  # The parent checker is advisory filesystem validation, not predecessor or
  # updater admission. Fresh install never runs the predecessor inspector.
  if [[ ! -e "$parent" && ! -L "$parent" ]]; then
    _omavless_predecessor_parents_safe "$parent" "$(id -u)" || return 1
    mkdir -m 700 -- "$parent" || return 1
  fi
  _omavless_predecessor_parents_safe "$parent/entry" "$(id -u)" || return 1
  if [[ ! -e "$HOME/.cache/omavless" && ! -L "$HOME/.cache/omavless" ]]; then
    mkdir -m 700 -- "$HOME/.cache/omavless" || return 1
  fi
  [[ -d "$HOME/.cache/omavless" && ! -L "$HOME/.cache/omavless" \
     && $(stat -c %u -- "$HOME/.cache/omavless") == "$(id -u)" ]] || return 1
  _omavless_predecessor_parents_safe "$HOME/.cache/omavless/entry" "$(id -u)" || return 1
  if [[ ! -e "$HOME/.cache/omavless/upgrade082" && ! -L "$HOME/.cache/omavless/upgrade082" ]]; then
    mkdir -m 700 -- "$HOME/.cache/omavless/upgrade082" || return 1
  fi
  upgrade_cache_safe "$HOME/.cache/omavless/upgrade082" || return 1
  if [[ ! -e "$cache" && ! -L "$cache" ]]; then mkdir -m 700 -- "$cache" || return 1; fi
  upgrade_cache_safe "$cache" || return 1
  printf '%s\n' "$cache"
}
upgrade_package() {
  local version arch app_hash dns_hash source app_name dns_name cache receipt binary_hash actual predecessor metadata
  local operation=${1:-upgrade082}
  [[ "$operation" == upgrade082 || "$operation" == update-pair ]] || return 2
  # Advisory public-byte inspection earns no private or replacement authority.
  source "$setup_dir/setup-predecessor.sh"
  if [[ "$operation" == upgrade082 ]]; then
    predecessor=$(omavless_predecessor_status) || return 1
    jq -e '.schemaVersion == 1 and .status == "qualified" and .reason == "public_0_8_2"' \
      >/dev/null <<< "$predecessor" || return 1
  else
    pair_update_available || return 1
    case "$(installed_pair_delivery)" in stale|missing) ;; *) return 1 ;; esac
  fi
  IFS=$'\t' read -r version arch app_hash dns_hash source < <(release_fields)
  [[ "$version" == "$release_version" && -n "$arch" && -n "$app_hash" && -n "$dns_hash" && -n "$source" ]] || return 1
  cache=$(setup_release_cache "$app_hash" "$dns_hash") || return 1
  app_name="omavless-${package_version}-1-${arch}.pkg.tar.zst"
  dns_name="omavless-dns-${package_version}-1-${arch}.pkg.tar.zst"
  say 'Checking the application, VPN core and DNS helper packages…' 'Проверка пакетов приложения, ядра VPN и DNS-помощника…'
  upgrade_download "$app_name" "$app_hash" "$cache" \
    && upgrade_download "$dns_name" "$dns_hash" "$cache" || return 1
  [[ $(package_info "$cache/$app_name") == "omavless $package_version-1" \
     && $(package_info "$cache/$dns_name") == "omavless-dns $package_version-1" ]] || return 1
  app_archive_identity "$cache/$app_name" "$source" "$arch" "$version" \
    && dns_archive_identity "$cache/$dns_name" "$source" "$arch" || return 1
  receipt=$(timeout 8 bsdtar -xOf "$cache/$app_name" usr/share/doc/omavless/build-identity.txt \
    | head -c 4097) || return 1
  [[ ${#receipt} -le 4096 ]] || return 1
  binary_hash=$(sed -n 's/^binarySha256=//p' <<< "$receipt")
  [[ "$binary_hash" =~ ^[0-9a-f]{64}$ ]] || return 1
  timeout 15 bsdtar -xOf "$cache/$app_name" usr/bin/omavless > "$setup_temp/upgrade-helper" || return 1
  actual=$(sha256sum -- "$setup_temp/upgrade-helper") || return 1
  [[ "${actual%% *}" == "$binary_hash" ]] || return 1
  chmod 700 -- "$setup_temp/upgrade-helper" || return 1
  upgrade_archive_safe "$cache/$app_name" "$app_hash" \
    && upgrade_archive_safe "$cache/$dns_name" "$dns_hash" || return 1
  metadata=$(jq -cn --arg app "$app_hash" --arg dns "$dns_hash" --arg source "$source" \
    --arg version "$version" --arg arch "$arch" \
    '{schemaVersion:1,appSHA:$app,dnsSHA:$dns,sourceCommit:$source,version:$version,arch:$arch}') || return 1
  printf '%s\n' "$metadata" | OMAVLESS_LOCALE="$setup_locale" "$setup_temp/upgrade-helper" setup "$operation"
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
    setup_step private-initialization native_quiet setup initialize || return 1
  fi
  # Existing data is validated by the canonical Rust owner; never parsed,
  # repaired, reset or overwritten by shell. Connected/enabled legacy owners,
  # malformed stores and incomplete prior transitions refuse safely.
  setup_step store-validation store_compatible || return 1
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
  setup_step store-validation store_compatible || return 1
  setup_step application-activation native_quiet cutover activate || return 1
  [[ $(native_target) == rust ]] || return 1
  setup_step runtime-enablement enable_runtime || return 1
}

return_to_panel() {
  say 'Finish or cancel any open system prompts, then close this terminal.' \
      'Завершите или отмените открытые запросы системы, затем закройте этот терминал.'
  say 'In the panel, confirm "Terminal and system prompts are closed" if shown, then choose "Check again".' \
      'В панели подтвердите «Терминал и запросы системы закрыты», если эта кнопка показана, затем выберите «Проверить снова».'
}

setup_completion() {
  local completed
  completed=$(setup_status) || return 1
  case "$completed" in
    ready) say 'OmaVLESS is ready. This setup did not connect a VPN.' 'OmaVLESS готов. Эта настройка не подключала VPN.'; return_to_panel ;;
    needs_runtime_start)
      say 'Setup is complete. This setup did not connect a VPN.' 'Настройка завершена. Эта настройка не подключала VPN.'
      return_to_panel
      say 'Then choose "Start OmaVLESS".' 'Затем выберите «Запустить OmaVLESS».' ;;
    *) return 1 ;;
  esac
}

setup_main() {
  [[ $# -ge 1 && $# -le 2 ]] || return 2
  case "${2:-en}" in en|ru) setup_locale=${2:-en} ;; *) return 2 ;; esac
  case "$1" in
    status) [[ $# == 1 ]] || return 2; setup_status; return ;;
    components) [[ $# == 1 ]] || return 2; setup_components; return ;;
    install|upgrade|update-pair|finish-selection|start-broker|restore-enrollment|resume-dns) ;;
    *) return 2 ;;
  esac
  [[ -t 0 && -t 1 && $EUID -ne 0 ]] || return 2
  [[ ! ${OMAVLESS_HOME+x} ]] || return 2
  setup_failure_reported=false
  local state
  state=$(setup_status)
  if [[ "$1" == install && "$state" == ready ]]; then
    say 'OmaVLESS is already prepared. No changes made.' 'OmaVLESS уже настроен. Ничего не изменено.'
    return
  fi
  if [[ "$1" == upgrade && "$state" != needs_update ]] \
      || [[ "$1" == update-pair && "$state" != needs_pair_install && ! ( "$state" =~ ^(ready|needs_broker_stopped|needs_runtime_start|needs_selection|needs_dns_setup|needs_runtime_stop)$ && $(installed_pair_delivery) == stale ) ]] \
      || [[ "$1" == finish-selection && "$state" != needs_selection ]] \
      || [[ "$1" == start-broker && "$state" != needs_broker_stopped ]] \
      || [[ "$1" == restore-enrollment && "$state" != needs_broker_stopped ]] \
      || [[ "$1" == resume-dns && "$state" != needs_dns_setup ]] \
      || [[ "$1" == install && "$state" != needs_package && "$state" != needs_activation ]]; then
    say 'Setup is unavailable. No changes made. Check the setup guide.' 'Установка недоступна. Ничего не изменено. Откройте руководство.'
    return 1
  fi
  say 'The OmaVLESS DNS helper sets up website address lookup for VPN connections.' \
      'DNS-помощник OmaVLESS настраивает поиск адресов сайтов для VPN-подключений.'
  if [[ "$1" == update-pair ]]; then
    say 'Update OmaVLESS, its VPN core and DNS helper. Your profiles and settings will be kept.' \
      'Обновление OmaVLESS, ядра VPN и DNS-помощника. Профили и настройки сохранятся.'
    if ! user_runtime_stopped; then
      say '1. Turn off VPN login autoconnect and the VPN connection. In Settings, choose Shut down OmaVLESS. This closes the widget too.' \
      '1. Выключите автоподключение VPN, отключитесь, затем Настройки → Выключить OmaVLESS. Виджет тоже закроется.'
      say '2. To reopen ONLY the widget, run in your desktop terminal:' \
        '2. Чтобы снова открыть ТОЛЬКО виджет, выполните в обычном терминале:'
      printf '\n  omarchy plugin enable kdk.omavless\n\n'
      say '3. Open the panel and choose Update components… again. Keep the application stopped until the update finishes.' \
        '3. Откройте панель и снова выберите «Обновить компоненты…». До завершения обновления не запускайте приложение.'
      # Never fetch/authorize effects until normal Quit earns a stopped owner.
      say 'OmaVLESS is still running or its shutdown could not be confirmed. Nothing was downloaded or changed.' \
        'OmaVLESS работает или его выключение не удалось подтвердить. Ничего не скачано и не изменено.'
      say 'Close this terminal and follow steps 1–3. If that does not help, open "Setup help" in the panel.' \
        'Закройте терминал и выполните шаги 1–3. Если не помогло, откройте «Помощь» в панели.'
      return 0
    fi
    say 'Follow the preparation and installation prompts below. Enter your password only at sudo’s prompt.' \
      'Следуйте запросам подготовки и установки ниже. Пароль вводите только по запросу sudo.'
    say 'If a step fails, stop and open "Setup help" in the panel.' \
      'При ошибке остановитесь и откройте «Помощь» в панели.'
    say 'The update will not start OmaVLESS or connect a VPN.' \
      'Обновление не запускает OmaVLESS и не подключает VPN.'
  elif [[ "$1" == upgrade ]]; then
    say 'Update OmaVLESS 0.8.2 and install the VPN core and DNS helper. Profiles will be kept; the application stays stopped.' \
      'Обновить OmaVLESS 0.8.2 и установить ядро VPN и DNS-помощник. Профили сохранятся; приложение останется выключенным.'
    say 'Type UPGRADE when asked. Setting up the DNS helper is a separate next step.' \
      'Введите UPGRADE, когда появится запрос. Настройка DNS-помощника — отдельный следующий шаг.'
  elif [[ "$1" == resume-dns ]]; then
    say 'Finish setting up the DNS helper. The application is stopped; no VPN will connect.' \
      'Завершение настройки DNS-помощника. Приложение остановлено; VPN не подключится.'
  elif [[ "$1" == start-broker ]]; then
    say 'Start the DNS helper after an update or a completed shutdown. Existing access, profiles and startup stay unchanged; VPN stays off.' \
        'Запустить DNS-помощник после обновления или завершённого выключения. Доступ, профили и автозапуск не меняются; VPN остаётся выключенным.'
  elif [[ "$1" == restore-enrollment ]]; then
    say 'Restore helper access only if a clean reinstall removed it. This is not a retry for a failed Start. Profiles and startup preferences stay unchanged; VPN stays off.' \
        'Восстановить доступ к помощнику можно только если он был удалён при чистой переустановке. Это не повтор неудачного запуска. Профили и автозапуск не меняются; VPN остаётся выключенным.'
  elif [[ "$1" == finish-selection ]]; then
    say 'Finish setting up the installed VPN core and DNS helper. No packages or existing access will be changed; VPN stays off.' \
        'Завершить настройку установленного ядра VPN и DNS-помощника. Пакеты и существующий доступ не меняются; VPN остаётся выключенным.'
  else
    say 'Set up OmaVLESS' 'Настройка OmaVLESS'
    say 'Install missing OmaVLESS components and complete setup. VPN will stay off.' \
      'Установка недостающих компонентов OmaVLESS и завершение настройки. VPN останется выключенным.'
    say 'Before setup, turn off other VPN connections to avoid conflicts.' \
      'Перед настройкой отключите другие VPN-подключения, чтобы избежать конфликтов.'
    say 'Profiles are kept. For an existing installation, turn off its VPN and VPN login autoconnect before setup.' \
        'Профили сохраняются. Для существующей установки сначала отключите её VPN и автоподключение при входе.'
    say 'Enter your password only when sudo asks for it, not at these confirmation prompts.' \
        'Вводите пароль только по запросу sudo, не вместо слов подтверждения.'
    say 'Firewall rules will not change. If you use a restrictive firewall, read "Setup help" before connecting.' \
      'Правила межсетевого экрана не меняются. Если он ограничивает трафик, прочитайте «Помощь» перед подключением.'
    confirm || { say 'Cancelled. No changes made.' 'Отменено. Ничего не изменено.'; return; }
  fi
  # One transaction per user runtime directory. A stale lock after a killed
  # terminal is deliberately not removed automatically on the next attempt.
  local runtime=${XDG_RUNTIME_DIR:-} setup_lock setup_temp
  setup_step runtime-directory setup_runtime_directory_safe "$runtime" || return 1
  setup_lock="$runtime/omavless-first-run.lock"
  create_setup_lock() { mkdir -- "$setup_lock" 2>/dev/null; }
  create_setup_temp() { setup_temp=$(mktemp -d "$runtime/omavless-first-run.XXXXXX"); }
  setup_step setup-lock create_setup_lock || return 1
  setup_step temporary-directory create_setup_temp || { rmdir -- "$setup_lock"; return 1; }
  # A subshell keeps the cleanup trap and private download scope local.
  (
    trap 'rm -f -- "$setup_temp/upgrade-helper" "$setup_temp/omavless-${package_version}-1-aarch64.pkg.tar.zst" "$setup_temp/omavless-${package_version}-1-x86_64.pkg.tar.zst" "$setup_temp/omavless-dns-${package_version}-1-aarch64.pkg.tar.zst" "$setup_temp/omavless-dns-${package_version}-1-x86_64.pkg.tar.zst"; rmdir -- "$setup_temp" "$setup_lock"' EXIT
    # Recheck after consent and the lock. Never replace a package that appeared
    # while the user was reading the prompt, or activate a now-unknown owner.
    setup_state_unchanged() { [[ $(setup_status) == "$state" ]]; }
    setup_step setup-state setup_state_unchanged || exit 1
    if [[ "$1" == upgrade || "$1" == update-pair ]]; then
      local operation=upgrade082
      [[ "$1" != update-pair ]] || operation=update-pair
      setup_step upgrade-operation upgrade_package "$operation" || exit 1
      setup_step unit-reload reload_units || exit 1
      local next
      next=$(setup_status)
      [[ "$next" == needs_dns_setup || ( "$1" == update-pair && "$next" == needs_broker_stopped ) ]] \
        || { setup_step completion false; exit 1; }
      if [[ "$next" == needs_broker_stopped ]]; then
        say 'Packages updated. Existing helper access is kept. The update did not start the application or VPN.' \
          'Пакеты обновлены. Существующий доступ к помощнику сохранён. Обновление не запускало приложение или VPN.'
        return_to_panel
        say 'Then choose "Start DNS helper", followed by "Start OmaVLESS". Do not authorize the helper again.' \
          'Затем выберите «Запустить DNS-помощник», а после него — «Запустить OmaVLESS». Повторно разрешать работу помощника не нужно.'
      else
        say 'Packages updated. The update did not start the application or VPN.' \
          'Пакеты обновлены. Обновление не запускало приложение или VPN.'
        return_to_panel
        say 'Then choose "Set up DNS helper" and follow the next action shown in the panel.' \
          'Затем выберите «Настроить DNS-помощник» и следуйте следующему действию в панели.'
      fi
      exit 0
    elif [[ "$1" == resume-dns ]]; then
      setup_step dns-preconditions resume_pending_dns || exit 1
    elif [[ "$1" == finish-selection ]]; then
      setup_step dns-selection-confirmation finish_pair_selection || exit 1
    elif [[ "$1" == start-broker ]]; then
      setup_step dns-preconditions start_existing_broker || exit 1
    elif [[ "$1" == restore-enrollment ]]; then
      setup_step dns-preconditions restore_enrollment || exit 1
    else
      if [[ "$state" == needs_package ]]; then
        setup_step package-install install_package || exit 1
        setup_step unit-reload reload_units || exit 1
      fi
      setup_step private-settings prepare_private_store || exit 1
      selected=$(pair_selection_status) || { setup_step dns-pair-status false; exit 1; }
      if jq -e '.schemaVersion == 1 and .scope == "local_pair_only" and .selected == false' \
        >/dev/null 2>&1 <<< "$selected"; then
        if system_broker_idle; then
          # A prior enrollment may have succeeded before selection was
          # interrupted. Resume only if this UID can access the exact live
          # broker socket; never repeat or replace root enrollment blindly.
          setup_step dns-socket-access broker_access_for_user || exit 1
          setup_step dns-selection-confirmation finish_pair_selection || exit 1
        else
          setup_step dns-preconditions enroll_and_select_pair || exit 1
        fi
      else
        setup_step dns-selection pair_selected || exit 1
      fi
      setup_step application-preconditions prepare_application || exit 1
    fi
    setup_step completion setup_completion || exit 1
  ) || {
    say 'Setup stopped at the step shown above. Some earlier steps may have succeeded; your data was not reset.' \
        'Настройка остановилась на указанном выше этапе. Предыдущие этапы могли завершиться; ваши данные не сбрасывались.'
    return_to_panel
    say 'Do not repeat a step with an unknown result. Open "Setup help" in the panel if the remaining step is unclear.' \
        'Не повторяйте шаг, если его результат неизвестен. Если следующий шаг непонятен, откройте «Помощь» в панели.'
    return 1
  }
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  result=0
  setup_main "$@" || result=$?
  if [[ "${1:-}" == upgrade && "$result" == 0 && -t 0 ]]; then
    offer_shell_reload
  fi
  if [[ ( "${1:-}" == install || "${1:-}" == upgrade || "${1:-}" == update-pair || "${1:-}" == finish-selection || "${1:-}" == start-broker || "${1:-}" == restore-enrollment || "${1:-}" == resume-dns ) && -t 0 ]]; then
    say 'Press Enter to close.' 'Нажмите Enter, чтобы закрыть.'
    IFS= read -r _ || true
  fi
  exit "$result"
fi
