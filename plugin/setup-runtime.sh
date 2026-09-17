#!/bin/bash
# SPDX-License-Identifier: MIT
# Explicit first-run provisioning, never called by status or plugin loading.
# No Python/Cargo, arbitrary URL, shell eval, private-store repair or VPN action.
set -euo pipefail
umask 077

setup_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
setup_locale=en

say() { if [[ "$setup_locale" == ru ]]; then printf '%s\n' "$2"; else printf '%s\n' "$1"; fi; }
native() { /usr/bin/omavless "$@"; }
native_present() { [[ -f /usr/bin/omavless && -x /usr/bin/omavless && ! -L /usr/bin/omavless ]]; }
native_target() { timeout 8 /usr/bin/omavless plugin target 2>/dev/null; }
package_info() { /usr/bin/pacman -Qp -- "$1" 2>/dev/null; }
package_install() { /usr/bin/sudo /usr/bin/pacman -U -- "$1"; }
package_installed() { /usr/bin/pacman -Q omavless 2>/dev/null; }
core_dependency_present() { /usr/bin/pacman -T mihomo >/dev/null 2>&1; }
core_binary_present() {
  local core
  core=$(command -v mihomo) || return 1
  [[ -f "$core" && -x "$core" ]]
}
no_core_process() {
  local code=0
  pgrep -x mihomo >/dev/null 2>&1 || code=$?
  [[ $code == 1 ]]
}
install_core_dependency() { omarchy pkg aur add mihomo-bin; }
reload_units() { /usr/bin/systemctl --user daemon-reload; }
enable_runtime() { /usr/bin/systemctl --user enable omavless-runtime.service >/dev/null 2>&1; }

release_fields() {
  local metadata="$setup_dir/runtime-release.json" arch
  [[ -f "$metadata" && ! -L "$metadata" && $(stat -c %s -- "$metadata") -le 8192 ]] || return 1
  arch=$(uname -m)
  [[ "$arch" == aarch64 || "$arch" == x86_64 ]] || return 1
  # Trust only pinned data shipped with the reviewed frontend. No remote
  # manifest, latest tag, caller URL or executable content is accepted.
  jq -er --arg arch "$arch" '
    select(keys == ["packages", "schemaVersion", "version"] and .schemaVersion == 1)
    | select(.version == "0.8.0" and (.packages | type) == "object")
    | select((.packages | keys - ["aarch64", "x86_64"] | length) == 0)
    | .version as $version | .packages[$arch]
    | select(type == "object" and keys == ["sha256", "sourceCommit"])
    | select((.sha256 | type) == "string" and (.sha256 | test("^[0-9a-f]{64}$")))
    | select((.sourceCommit | type) == "string" and (.sourceCommit | test("^[0-9a-f]{40}$")))
    | [$version, $arch, .sha256, .sourceCommit] | @tsv
  ' "$metadata" 2>/dev/null
}

setup_status() {
  if native_present; then
    local target
    target=$(native_target) || { printf 'needs_attention\n'; return; }
    case "$target" in
      rust) printf 'ready\n' ;;
      legacy) printf 'needs_activation\n' ;;
      *) printf 'needs_attention\n' ;;
    esac
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
  if core_binary_present; then core=present; fi
  printf '%s\t%s\n' "$state" "$core"
}

ensure_core_dependency() {
  if core_dependency_present && core_binary_present; then return 0; fi
  no_core_process || return 1
  local answer
  say 'Mihomo is required. Install mihomo-bin using “omarchy pkg aur add mihomo-bin”? This uses the AUR. Type CORE to accept, Enter to cancel.' \
      'Требуется Mihomo. Установить mihomo-bin командой «omarchy pkg aur add mihomo-bin»? Используется AUR. Введите CORE для согласия или Enter для отмены.'
  IFS= read -r answer || return 1
  [[ "$answer" == CORE ]] || return 1
  no_core_process || return 1
  install_core_dependency || return 1
  core_dependency_present && core_binary_present
}

confirm() {
  local answer
  say 'Type INSTALL to continue, or press Enter to cancel:' 'Введите INSTALL для продолжения или Enter для отмены:'
  IFS= read -r answer || return 1
  [[ "$answer" == INSTALL ]]
}

install_package() {
  local version arch checksum source package url actual
  IFS=$'\t' read -r version arch checksum source < <(release_fields)
  [[ -n "$version" && -n "$arch" && -n "$checksum" && -n "$source" ]] || return 1
  package="omavless-${version}-1-${arch}.pkg.tar.zst"
  url="https://github.com/k-kostin/omavless/releases/download/v${version}/${package}"
  say 'Downloading the pinned OmaVLESS package…' 'Загрузка проверенного пакета OmaVLESS…'
  # HTTPS redirects are required by GitHub's asset delivery. Only the exact
  # pinned archive hash authorizes the downloaded payload for pacman.
  curl --disable --fail --silent --show-error --location --max-redirs 3 \
    --proto '=https' --proto-redir '=https' --connect-timeout 15 --max-time 180 \
    --max-filesize 268435456 --output "$setup_temp/$package" "$url" 2>/dev/null || return 1
  actual=$(sha256sum -- "$setup_temp/$package")
  [[ "${actual%% *}" == "$checksum" ]] || return 1
  [[ $(package_info "$setup_temp/$package") == "omavless $version-1" ]] || return 1
  native_present && return 1
  # Respect an existing provider of the virtual 'mihomo' dependency. The
  # documented Omarchy AUR route is explicit, separate consent, not a hidden
  # download/build of our runtime. Its normal host authentication stays visible.
  ensure_core_dependency || return 1
  # Normal dependency and user confirmation checks. No --nodeps/--overwrite,
  # --noconfirm, package hooks added by this script, or background sudo.
  native_present && return 1
  package_install "$setup_temp/$package" || return 1
  [[ $(package_installed) == "omavless $version-1" ]] || return 1
}

prepare_application() {
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
    install|install-core) ;;
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
  if [[ "$1" == install-core && "$state" != ready && "$state" != needs_activation ]] \
      || [[ "$1" == install && "$state" != needs_package && "$state" != needs_activation ]]; then
    say 'Setup is unavailable. No changes made. Check the setup guide.' 'Установка недоступна. Ничего не изменено. Откройте руководство.'
    return 1
  fi
  if [[ "$1" == install-core ]]; then
    say 'Install the missing Mihomo core only. Application settings, service and VPN will not be changed.' \
        'Установить только недостающее ядро Mihomo. Настройки приложения, служба и VPN не изменятся.'
  else
    say 'Set up OmaVLESS' 'Настройка OmaVLESS'
    say 'Install the matching application if missing, prepare private settings and its user service. No VPN connection will be started.' \
      'Установить недостающее приложение, подготовить приватные настройки и пользовательскую службу. VPN подключаться не будет.'
    say 'Existing profiles are preserved. Existing VPN/startup must be off before migration. Passwords go only to sudo, not this prompt.' \
      'Существующие профили сохраняются. Перед переносом отключите VPN и автоподключение. Пароль вводите только в sudo, не здесь.'
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
    trap 'rm -f -- "$setup_temp/omavless-0.8.0-1-aarch64.pkg.tar.zst" "$setup_temp/omavless-0.8.0-1-x86_64.pkg.tar.zst"; rmdir -- "$setup_temp" "$setup_lock"' EXIT
    # Recheck after consent and the lock. Never replace a package that appeared
    # while the user was reading the prompt, or activate a now-unknown owner.
    [[ $(setup_status) == "$state" ]] || exit 1
    if [[ "$1" == install-core ]]; then
      ensure_core_dependency || exit 1
      say 'Mihomo is installed. Return to the panel and check again. TUN permission readiness is checked separately.' \
          'Mihomo установлен. Вернитесь в панель и проверьте снова. Готовность разрешений TUN проверяется отдельно.'
      exit 0
    fi
    if [[ "$state" == needs_package ]]; then
      install_package || exit 1
      reload_units || exit 1
    fi
    prepare_application || exit 1
    say 'OmaVLESS is ready. Return to the plugin and press Check again.' 'OmaVLESS готов. Вернитесь в плагин и нажмите «Проверить снова».'
  ) || {
    say 'Setup did not finish. Do not repeat an unresolved authorization. Existing data was not reset; inspect the setup guide before retrying.' \
        'Настройка не завершена. Не повторяйте незавершённую авторизацию. Данные не сбрасывались; перед повтором откройте руководство.'
    return 1
  }
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  result=0
  setup_main "$@" || result=$?
  if [[ ( "${1:-}" == install || "${1:-}" == install-core ) && -t 0 ]]; then
    say 'Press Enter to close.' 'Нажмите Enter, чтобы закрыть.'
    IFS= read -r _ || true
  fi
  exit "$result"
fi
