#!/bin/bash
# SPDX-License-Identifier: MIT
# Guided removal of supported public 0.8.2 / 0.9.8 installations.
# Manual terminal administration, NOT a runtime API or all-state uninstaller.
set -euo pipefail
umask 077
export PATH=/usr/bin:/bin LC_ALL=C
fail() { printf '%s\n' "Stopped: $*" >&2; exit 1; }
check_account() {
  account=$(getent passwd "$(id -u)") || fail 'Cannot identify your desktop account.'
  IFS=: read -r name password account_uid group description canonical_home shell <<< "$account"
  [[ "$account_uid" == "$(id -u)" && "$HOME" == "$canonical_home" && "$HOME" != / ]] \
    || fail 'Run from your normal desktop account and home directory.'
}
check_private_targets() {
  mounts=$(findmnt --json --list --output TARGET) || fail 'Cannot check mounted directories.'
  [[ ${#mounts} -le 524288 ]] || fail 'Mount inventory is too large; ask for help.'
  for directory in "$HOME/.config/omavless" "$HOME/.local/state/omavless" "$HOME/.cache/omavless"; do
    jq -e --arg directory "$directory" '
      .filesystems | type == "array" and length <= 4096 and
      all(.[]; (.target | type) == "string") and
      all(.[]; (.target != $directory and (.target | startswith($directory + "/") | not)))
    ' <<< "$mounts" >/dev/null || fail 'OmaVLESS settings include a mounted directory; nothing will be deleted.'
    if [[ -e "$directory" || -L "$directory" ]]; then
      [[ -d "$directory" && ! -L "$directory" && $(stat -c '%u' -- "$directory") == "$(id -u)" ]] \
        || fail 'Unexpected OmaVLESS settings directory; nothing will be deleted.'
      outsiders=$(find "$directory" -xdev ! -uid "$(id -u)" -print -quit) || fail 'Cannot check settings ownership.'
      [[ -z "$outsiders" ]] || fail 'Settings contain another owner’s files; nothing will be deleted.'
    fi
  done
}
confirm_reset() {
  if [[ ${guided:-false} == true ]]; then
    printf '%s\n' '' 'Remove OmaVLESS' '' \
      'This removes OmaVLESS and its saved profiles and settings. No backup is made.' \
      'Other VPN clients and a separately installed Mihomo stay installed.'
    if [[ "$lane" == never_connected ]]; then
      printf '%s\n' '' 'Use this path only if OmaVLESS has never connected on this installation.' \
        'If you have used it before, press Enter and ask for help instead.'
    elif [[ ${1:-prepare} == prepare ]]; then
      printf '%s\n' 'Its current VPN connection will be disconnected and the application closed.'
    fi
    printf '%s\n' '' 'Type REMOVE to continue, or press Enter to cancel:'
    local answer
    IFS= read -r answer || return 1
    [[ "$answer" == REMOVE ]]
    return
  fi
  printf '%s\n' 'Remove OmaVLESS and start again' '' \
    'Use this reset only if this installation has never connected a VPN.' \
    'This removes the OmaVLESS plugin, application, bundled VPN core and DNS helper.' \
    'Its saved VPN profiles, subscriptions and settings will also be deleted.' \
    'Other VPN applications and a separately installed Mihomo are kept.' '' \
    'Type REMOVE to continue, or press Enter to cancel:'
  local answer
  IFS= read -r answer || return 1
  [[ "$answer" == REMOVE ]]
}
guided_operation() {
  case "$stage" in
    none) printf '%s\n' prepare ;;
    prepared)
      if [[ "$prepared_boot" == "$boot_id" ]]; then printf '%s\n' restart; else printf '%s\n' remove; fi ;;
    *) fail 'A previous removal stopped before completion. Send its error to support; do not repeat it.' ;;
  esac
}
guided_storage() {
  stage_directory="$HOME/.cache/omavless-maintenance/reinstall-098"
  for directory in "$HOME/.cache/omavless-maintenance" "$stage_directory"; do
    if [[ ! -e "$directory" && ! -L "$directory" ]]; then mkdir -- "$directory" || fail 'Cannot create the removal folder.'; fi
    [[ -d "$directory" && ! -L "$directory" ]] || fail 'The removal folder is unusual. Ask for help.'
    read -r owner mode < <(stat -c '%u %a' -- "$directory")
    [[ "$owner" == "$(id -u)" && "$mode" =~ ^[0-7]{3,4}$ ]] && (( (8#$mode & 0022) == 0 )) \
      || fail 'The removal folder is writable by another account. Ask for help.'
  done
  stage_file="$stage_directory/session.json"
  helper_path=$(realpath -e -- "${BASH_SOURCE[0]}") || fail 'Cannot identify the removal helper.'
  helper_hash=$(sha256sum -- "$helper_path") || fail 'Cannot check the removal helper.'
  helper_hash=${helper_hash%% *}
  boot_id=$(</proc/sys/kernel/random/boot_id)
  [[ "$boot_id" =~ ^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$ ]] || fail 'Cannot check this computer restart.'
}
read_stage() {
  stage=none
  if [[ ! -e "$stage_file" && ! -L "$stage_file" ]]; then return; fi
  [[ -f "$stage_file" && ! -L "$stage_file" && $(stat -c '%u:%a:%h' -- "$stage_file") == "$(id -u):600:1" &&
     $(stat -c '%s' -- "$stage_file") -le 512 ]] || fail 'A previous removal record needs inspection. Do not repeat removal.'
  stage_bytes=$(<"$stage_file")
  [[ $(printf '%s' "$stage_bytes" | jq -c .) == "$stage_bytes" ]] \
    || fail 'A previous removal record is damaged. Ask for help; do not repeat removal.'
  printf '%s' "$stage_bytes" | jq -e --argjson uid "$(id -u)" --arg helper "$helper_hash" '
    keys == ["boot", "helper", "lane", "phase", "schema", "uid"] and .schema == 1 and .uid == $uid and
    .helper == $helper and (.lane == "never_connected" or .lane == "live_managed098") and
    (.phase == "preparing" or .phase == "prepared" or .phase == "removing") and
    (.boot | type == "string" and test("^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$"))
  ' >/dev/null || fail 'A previous removal record does not match this helper. Ask for help; do not repeat removal.'
  stage=$(printf '%s' "$stage_bytes" | jq -r .phase)
  prepared_boot=$(printf '%s' "$stage_bytes" | jq -r .boot)
  lane=$(printf '%s' "$stage_bytes" | jq -r .lane)
}
write_stage() {
  local next=$1 temporary
  case "$stage:$next" in
    none:preparing|preparing:prepared|prepared:removing) ;;
    *) fail 'The removal phase is not valid. Stop here and ask for help.' ;;
  esac
  # This same-account record selects a screen only. It never bypasses the fresh
  # package, service, enrollment, root guard or original shutdown checks.
  if [[ "$stage" == none ]]; then
    [[ "$next" == preparing && ! -e "$stage_file" && ! -L "$stage_file" ]] \
      || fail 'The removal record changed. Stop here and ask for help.'
  else
    [[ -f "$stage_file" && ! -L "$stage_file" && $(stat -c '%u:%a:%h' -- "$stage_file") == "$(id -u):600:1" &&
       $(<"$stage_file") == "$stage_bytes" ]] || fail 'The removal record changed. Stop here and ask for help.'
  fi
  temporary=$(mktemp "$stage_directory/.session.XXXXXXXX") || fail 'Cannot save removal progress. Stop here and ask for help.'
  jq -cn --argjson uid "$(id -u)" --arg helper "$helper_hash" --arg boot "$boot_id" --arg lane "$lane" --arg phase "$next" \
    '{schema:1,uid:$uid,helper:$helper,boot:$boot,lane:$lane,phase:$phase}' > "$temporary" \
    || fail 'Cannot save removal progress. Stop here and ask for help.'
  if [[ "$stage" == none ]]; then
    ln -- "$temporary" "$stage_file" || fail 'Cannot save removal progress. Stop here and ask for help.'
    unlink -- "$temporary"
  else
    mv -T -- "$temporary" "$stage_file" || fail 'Cannot save removal progress. Stop here and ask for help.'
  fi
  sync -f -- "$stage_directory" || fail 'Cannot safely save removal progress. Stop here and ask for help.'
  read_stage
}
offer_restart() {
  printf '%s\n' '' 'Preparation is complete. One restart is needed before deleting the old components.' \
    'Save your other work. After logging back in, run this same command:' \
    "bash \"$helper_path\"" '' \
    'Type REBOOT to restart now, or press Enter to restart later:'
  local answer
  IFS= read -r answer || answer=
  if [[ "$answer" == REBOOT ]]; then
    systemctl reboot || fail 'The restart was not confirmed. Use the desktop restart menu, then run the command above.'
  else
    printf '%s\n' 'Nothing else will run now. Restart from your desktop when ready, then use the same command.'
  fi
}
runtime_pid() {
  local pid
  pid=$(systemctl --user show omavless-runtime.service --property=MainPID --value) || return 1
  [[ "$pid" =~ ^(0|[1-9][0-9]*)$ ]] || return 1
  printf '%s\n' "$pid"
}
runtime_disabled() {
  local load enabled
  load=$(systemctl --user show omavless-runtime.service --property=LoadState --value) || return 1
  case "$load" in
    not-found) return 0 ;;
    loaded)
      enabled=$(systemctl --user show omavless-runtime.service --property=UnitFileState --value) || return 1
      [[ "$enabled" == disabled ]] ;;
    *) return 1 ;;
  esac
}
plugin_state() {
  local registry
  registry=$(timeout 8 omarchy plugin list --json) || return 1
  [[ ${#registry} -le 262144 ]] || return 1
  printf '%s' "$registry" | jq -er '
    select(type == "array" and length <= 1024
      and all(.[]; type == "object" and (.id | type) == "string" and (.enabled | type) == "boolean"))
    | map(select(.id == "kdk.omavless"))
    | if length == 0 then "absent" elif length == 1 then
        if .[0].enabled then "enabled" else "disabled" end
      else error("ambiguous plugin") end'
}
hello_fence() {
  local hello
  hello=$(timeout 8 /usr/bin/omavless hello) || return 1
  [[ ${#hello} -le 65536 ]] || return 1
  # Only non-secret protocol fence fields reach argv. Full Quit independently
  # authenticates the service peer and verifies this exact original revision.
  printf '%s' "$hello" | jq -er '
    select(.ok == true and .result.version == 1 and .result.runtimeOwnership == true
      and (.result.instanceId | type == "string" and length > 0 and length <= 128
        and test("^[!-~]+$"))
      and (.revision | type == "number" and floor == . and . >= 0 and . <= 9007199254740991))
    | [.result.instanceId, (.revision | tostring)] | @tsv'
}
native_shutdown() {
  if [[ ${guided:-false} != true || ! -t 2 ]]; then /usr/bin/omavless "$@"; return; fi
  local output child result frame=0
  output=$(mktemp "$stage_directory/.shutdown.XXXXXXXX") || return 1
  /usr/bin/omavless "$@" </dev/null > "$output" &
  child=$!
  # Observe this shell's original job, not a reusable process ID. No timeout,
  # signal, retry or success follows from the animation.
  while jobs -pr | grep -qx -- "$child"; do
    printf '\r[%s] Closing OmaVLESS...' "${spinner:frame++%4:1}" >&2
    sleep 0.15
  done
  if wait "$child"; then result=0; else result=$?; fi
  printf '\r                                      \r' >&2
  if [[ $(stat -c '%s' -- "$output") -le 65536 ]]; then
    head -c 65536 -- "$output"
  else
    result=1
  fi
  unlink -- "$output"
  return "$result"
}
shutdown_running_app() {
  local pid answer state fence instance revision outcome deadline
  pid=$(runtime_pid) || fail 'Cannot check whether OmaVLESS is running. Nothing was removed.'
  if [[ "$pid" == 0 ]]; then
    [[ ${lane:-never_connected} != live_managed098 ]] \
      || fail 'The running application changed before shutdown. Stop here and ask for help.'
    printf '%s\n' 'OmaVLESS is already stopped.'
    return
  fi
  if [[ ${guided:-false} == true ]]; then
    printf '%s\n' '[1/3] Closing OmaVLESS...'
  else
    printf '%s\n' 'OmaVLESS is running. To remove it, first close the application.' \
    'This will disconnect its VPN if connected. Other VPN clients are not stopped.' \
    'Type STOP to close OmaVLESS, or press Enter to cancel:'
  IFS= read -r answer || fail 'Cancelled. Nothing was removed.'
  [[ "$answer" == STOP ]] || fail 'Cancelled. Nothing was removed.'
  fi
  state=$(plugin_state) || fail 'Cannot check the plugin. Nothing was removed.'
  case "$state" in
    enabled)
      fence=$(hello_fence) || fail 'Cannot safely close OmaVLESS. Nothing was removed; send this message to support.'
      IFS=$'\t' read -r instance revision <<< "$fence"
      release_lock=false
      outcome=$(native_shutdown plugin quit "$instance" "$revision" "clean-reinstall-$$") \
        || fail 'OmaVLESS did not confirm shutdown. Stop here; do not repeat or force-close it.'
      printf '%s' "$outcome" | jq -e '.schemaVersion == 1 and .shutdown == true and .pluginDisabled == true' >/dev/null \
        || fail 'Shutdown could not be confirmed. Stop here and send this message to support.'
      earned_live_shutdown=true
      ;;
    disabled|absent)
      # Existing fixed consumer captures/authenticates its own original fence.
      # Never use this as a fallback after an uncertain Quit result.
      release_lock=false
      outcome=$(native_shutdown plugin watch-removal) \
        || fail 'OmaVLESS did not confirm shutdown. Stop here; do not repeat or force-close it.'
      printf '%s' "$outcome" | jq -e '.schemaVersion == 1 and (.runtimeStopped | type) == "boolean"' >/dev/null \
        || fail 'Shutdown could not be confirmed. Stop here and send this message to support.'
      if printf '%s' "$outcome" | jq -e '.runtimeStopped == true' >/dev/null; then earned_live_shutdown=true; fi
      ;;
    *) fail 'Cannot safely close OmaVLESS. Nothing was removed.' ;;
  esac
  if [[ ${lane:-never_connected} == live_managed098 && ${earned_live_shutdown:-false} != true ]]; then
    fail 'The application did not confirm its own clean shutdown. Nothing else will be removed; ask for help.'
  fi
  # A watcher may coalesce with the original QML unload consumer. Its successful
  # no-op is not proof of shutdown: wait only for independent fixed unit facts.
  deadline=$((SECONDS + 65))
  while :; do
    pid=$(runtime_pid) || fail 'Cannot verify shutdown. Stop here and ask for help.'
    if [[ "$pid" == 0 && $(systemctl --user show omavless-runtime.service --property=ActiveState --value) == inactive ]]; then
      printf '%s\n' 'OmaVLESS closed. Continuing removal preparation.'
      return
    fi
    (( SECONDS < deadline )) || fail 'Shutdown is still unconfirmed. Stop here; do not force-close the process.'
    sleep 0.2
  done
}
check_supported_installation() {
  local app dns digest
  app=$(pacman -Q omavless 2>/dev/null) || app=absent
  dns=$(pacman -Q omavless-dns 2>/dev/null) || dns=absent
  case "$app:$dns" in
    'omavless 0.8.2-1:absent'|'omavless 0.9.8-1:omavless-dns 0.9.8-1'|'omavless 0.9.8-1:absent'|'absent:omavless-dns 0.9.8-1'|'absent:absent') ;;
    *) fail 'This installation is not a supported public release. Nothing was removed; ask for help.' ;;
  esac
  installed_public_app=$app
  installed_public_dns=$dns
  if [[ "$app" != absent ]]; then
    [[ -f /usr/bin/omavless && ! -L /usr/bin/omavless && $(stat -c '%u:%a' /usr/bin/omavless) == 0:755 ]] \
      || fail 'The application file is missing or unusual. Nothing was removed; ask for help.'
    digest=$(sha256sum /usr/bin/omavless) || fail 'Cannot identify the application.'
    # Public ELF identities, not just a development build's package label.
    # 0.8.2, original 0.9.8 and corrected public deliveries through fix.7.
    case "${digest%% *}" in
      3ff795316aa331463aff4e26571546433bdfd9317f348712d135be2f0a8d28a1|62874def77df9e4f099baf74c57420a7ab1dab7b83935676828f6d445a5f357f|219479595f4b59e07a9639d703e939125d464165f7a5f426ec523ce5d952600b|42a0f2550567c21cf8201a13d8797124237edfb71f1ee67a93d33fb6852753ee|3e5e8a5aca3d48d1bada328471b5078eb96a5796a1c44a6ba03a289def050141|5930244698e7fb1c6fe7518af2b093e7c2f5dcb5c4400dd845588553e0c7e2fd|118a12989f737e7c5f7ee167d10f6487ab6ca16ba4ed34f8dae71e813a384bd4|bf7787ee90bdbb0e2de03ecdd8bebb4b01810ad9607eaaecbfbf5ff7eeeaa2b4|b8cc106248953abcc3b3475efa7cec362778b5ae918b6866c1e8febdd3c44ff9|897710505a8fae9783bb8130c0e0d98133144e2b8e9ae68d169ad7a413999990|e1d320dc3347e2bdb4f45a576c34c42098a73e54e00f698010e5424cf97808b6|07295f1694bffb0ce234a8ce4164e03b3ddad2e88ef25a98f36a9e3fd6be6b73|7c7b0f0c350b8903353d2fed3c8effb693aaaca0c271279b85efa51dfebb8881|6949d9473a714e36824e93437c61aff1064c619b9960e8a9b31bb49834176004) ;;
      *) fail 'An experimental or modified application is installed. Nothing was removed; ask for help.' ;;
    esac
  else
    [[ ! -e /usr/bin/omavless && ! -L /usr/bin/omavless ]] \
      || fail 'An application file remains without its package. Nothing was removed; ask for help.'
  fi
  printf '%s\n' "Application: $app" "DNS helper (website address lookup) and bundled core: $dns"
}
# CLI entry point
guided=false
spinner='|/-\'
[[ $# == 0 || ( $# == 1 && ( $1 == prepare || $1 == remove ) ) ]] || fail 'Run this helper without arguments to remove OmaVLESS.'
[[ $# != 0 ]] || guided=true
[[ $EUID != 0 ]] || fail 'Run as your desktop user, not as root.'
check_account
[[ ${XDG_CONFIG_HOME:-$HOME/.config} == "$HOME/.config" &&
   ${XDG_STATE_HOME:-$HOME/.local/state} == "$HOME/.local/state" &&
   ${XDG_CACHE_HOME:-$HOME/.cache} == "$HOME/.cache" ]] || fail 'Custom settings paths need maintainer help.'
for directory in "$HOME" "$HOME/.config" "$HOME/.local" "$HOME/.local/state" "$HOME/.cache"; do
  [[ -d "$directory" && ! -L "$directory" ]] || fail 'Unexpected settings directory.'
  read -r owner mode < <(stat -c '%u %a' -- "$directory")
  [[ "$owner" == "$(id -u)" && "$mode" =~ ^[0-7]{3,4}$ ]] || fail 'Unexpected settings owner.'
  (( (8#$mode & 0022) == 0 )) || fail 'Settings directory is writable by another user.'
done
runtime="/run/user/$(id -u)"
[[ -d "$runtime" && ! -L "$runtime" && $(stat -c '%u:%a' "$runtime") == "$(id -u):700" ]] \
  || fail 'Cannot verify the desktop session. Nothing was removed.'
if [[ "$guided" == true ]]; then
  guided_storage
  read_stage
  operation=$(guided_operation) || exit 1
  if [[ "$operation" == restart ]]; then offer_restart; exit 0; fi
  set -- "$operation"
fi
setup_lock="$runtime/omavless-first-run.lock"
mkdir -- "$setup_lock" 2>/dev/null \
  || fail 'A setup or removal operation may still be open. Finish it first; nothing was removed.'
lock_identity=$(stat -c '%d:%i:%u:%a' "$setup_lock")
release_lock=true
cleanup_lock() {
  if [[ "$release_lock" == true && -d "$setup_lock" && ! -L "$setup_lock" &&
        $(stat -c '%d:%i:%u:%a' "$setup_lock") == "$lock_identity" ]]; then
    rmdir -- "$setup_lock" || true
  elif [[ "$release_lock" != true ]]; then
    printf '%s\n' 'Removal paused after an unconfirmed step. Do not repeat it; send the error to support.' >&2
  fi
}
trap cleanup_lock EXIT
# Root receives only this reviewed in-memory fixed source. No downloaded/user
# pathname, caller-selected account, unit or executable is accepted by root.
read -r -d '' root_checks <<'ROOT_CHECKS' || true
set -euo pipefail
export PATH=/usr/bin:/bin LC_ALL=C
refuse() { printf '%s\n' 'Stopped: cannot verify safe removal of the DNS helper. No forced cleanup was attempted; send this message to support.' >&2; exit 1; }
empty_host() {
  [[ ! -e /sys/class/net/Meta && ! -L /sys/class/net/Meta && ! -e /sys/class/net/omavless0 && ! -L /sys/class/net/omavless0 ]] || refuse
  for executable in /proc/[0-9]*/exe; do
    target=$(readlink "$executable" 2>/dev/null) || continue
    case "$target" in
      /usr/bin/omavless|'/usr/bin/omavless (deleted)'|/usr/lib/omavless-dns/mihomo|'/usr/lib/omavless-dns/mihomo (deleted)') refuse ;;
    esac
  done
}
check_dns_programs() {
  for path in /usr /usr/lib /usr/lib/omavless-dns /usr/lib/systemd /usr/lib/systemd/system; do
    [[ -d "$path" && ! -L "$path" ]] || refuse
    read -r owner mode < <(stat -c '%u %a' "$path")
    [[ "$owner" == 0 && "$mode" =~ ^[0-7]{3,4}$ ]] && (( (8#$mode & 0022) == 0 )) || refuse
  done
  for path in /usr/lib/omavless-dns/package-guard /usr/lib/omavless-dns/omavless-dns-broker; do
    [[ -f "$path" && ! -L "$path" && $(stat -c '%u:%a:%h' "$path") == 0:755:1 ]] || refuse
  done
  [[ $(sha256sum /usr/lib/omavless-dns/package-guard) == '624c08b458afd1dd48f6a16ef36e80d97027ee1062e0a7c8fa4ef0239fa89b03  /usr/lib/omavless-dns/package-guard' ]] || refuse
  hash=$(sha256sum /usr/lib/omavless-dns/omavless-dns-broker)
  case "${hash%% *}" in
    8caba1a57b44b2856b98b0d0213cc545905ba5a031aac144fcce2420957b0d2e|33121a6f4c4eb7b644da034fc521e221d7f292d61bdaffb837ed82c015dbe755|ba61e433d89baa3ae4fd1af5ed275f769654c19fbf6a7b22592869e617b277d5|8f250d4fce7e784477c91204579ed6f8792006713ccc895ac357e23faa03df26|1855ed920a4b064c811ed14134b5ae14e3c5ece0a26695a84afc1ccfd711dc93|c764c30ba24844f6972e2b19c600c068c92682d9fe1e1bd94de5896356f93c5e) ;;
    *) refuse ;;
  esac
  [[ -f /usr/lib/systemd/system/omavless-dns-broker.service && ! -L /usr/lib/systemd/system/omavless-dns-broker.service &&
     $(stat -c '%u:%a:%h' /usr/lib/systemd/system/omavless-dns-broker.service) == 0:644:1 &&
     $(sha256sum /usr/lib/systemd/system/omavless-dns-broker.service) == '126b208158e9288c475084c8bd74029e2808edf9846da514a369266b6306462d  /usr/lib/systemd/system/omavless-dns-broker.service' ]] || refuse
}
dns_absent() {
  if pacman -Q omavless-dns >/dev/null 2>&1; then return 1; fi
  for path in /usr/lib/omavless-dns /run/omavless-dns /usr/lib/systemd/system/omavless-dns-broker.service; do
    [[ ! -e "$path" && ! -L "$path" ]] || refuse
  done
  [[ $(systemctl show omavless-dns-broker.service --property=LoadState --value) == not-found &&
     $(systemctl show omavless-dns-broker.service --property=MainPID --value) == 0 ]] || refuse
  empty_enrollment_directory
  empty_host
}
empty_enrollment_directory() {
  if [[ -e /etc/omavless-dns || -L /etc/omavless-dns ]]; then
    [[ -d /etc/omavless-dns && ! -L /etc/omavless-dns && $(stat -c '%u:%a' /etc/omavless-dns) == 0:700 ]] || refuse
    shopt -s nullglob dotglob
    entries=(/etc/omavless-dns/*)
    [[ ${#entries[@]} == 0 ]] || refuse
  fi
}
remove_empty_enrollment_directory() {
  empty_enrollment_directory
  if [[ -d /etc/omavless-dns ]]; then rmdir -- /etc/omavless-dns || refuse; fi
}
check_enrollment() {
  [[ "${SUDO_UID:-}" =~ ^[1-9][0-9]*$ ]] || refuse
  enrollment=/etc/omavless-dns/release-enrollment.json
  if [[ -e /etc/omavless-dns || -L /etc/omavless-dns ]]; then
    [[ -d /etc/omavless-dns && ! -L /etc/omavless-dns && $(stat -c '%u:%a' /etc/omavless-dns) == 0:700 ]] || refuse
    shopt -s nullglob dotglob
    for entry in /etc/omavless-dns/*; do
      [[ "$entry" == "$enrollment" ]] || refuse
    done
  fi
  enrolled=false
  if [[ -e "$enrollment" || -L "$enrollment" ]]; then
    [[ -f "$enrollment" && ! -L "$enrollment" && $(stat -c '%u:%a:%h' "$enrollment") == 0:600:1 && $(stat -c '%s' "$enrollment") -le 256 ]] || refuse
    jq -e --argjson uid "$SUDO_UID" 'keys == ["policy", "schema", "uid"] and .schema == 1 and .uid == $uid and .policy == "meta-ipv4-release-v1"' "$enrollment" >/dev/null || refuse
    enrolled=true
  fi
}
ROOT_CHECKS
if [[ $1 == prepare ]]; then
  check_private_targets
  check_supported_installation
  if [[ "$guided" == true ]]; then
    lane=never_connected
    initial_pid=$(runtime_pid) || fail 'Cannot check the running application. Nothing was removed.'
    if [[ "$installed_public_app" == 'omavless 0.9.8-1' && "$installed_public_dns" == 'omavless-dns 0.9.8-1' && "$initial_pid" != 0 ]]; then
      lane=live_managed098
    fi
  fi
  confirm_reset prepare || fail 'Cancelled. Nothing was removed.'
  if [[ "$guided" == true ]]; then write_stage preparing; fi
  shutdown_running_app
  if [[ ${lane:-never_connected} == live_managed098 && ${earned_live_shutdown:-false} != true ]]; then
    fail 'The running application changed before shutdown. Stop here and ask for help.'
  fi
  plugin="$HOME/.config/omarchy/plugins/kdk.omavless"
  if [[ -e "$plugin" || -L "$plugin" ]]; then
    [[ -d "$plugin" && ! -L "$plugin" ]] || fail 'Unusual plugin installation; ask for help.'
    if [[ -d "$plugin/.git" ]]; then
      changes=$(git -C "$plugin" status --porcelain) || fail 'Cannot check plugin files.'
      [[ -z "$changes" ]] || fail 'Plugin has local changes; save them before removing it.'
    fi
    release_lock=false
    omarchy plugin remove kdk.omavless || fail 'Plugin removal did not finish.'
  fi
  if [[ $(systemctl --user show omavless-runtime.service --property=LoadState --value) == loaded ]]; then
    release_lock=false
    systemctl --user disable omavless-runtime.service || fail 'Cannot disable OmaVLESS startup.'
  fi
  runtime_disabled || fail 'OmaVLESS startup was not confirmed disabled. Stop here and ask for help.'
  [[ "$(systemctl --user show omavless-runtime.service --property=MainPID --value)" == 0 ]] \
    || fail 'OmaVLESS started again while preparing removal. Stop here and ask for help.'
  # Fixed reviewed source already loaded here; never sudo an unprivileged script
  # pathname or accept caller-provided root commands, paths or account selectors.
  read -r -d '' root_prepare <<'ROOT_PREPARE' || true
if dns_absent; then
  printf '%s\n' 'No DNS helper is installed. Preparation is complete.'
  exit 0
fi
check_dns_programs
clean() {
  for directory in /run /run/omavless-dns /run/omavless-dns/private; do
    [[ ! -L "$directory" ]] || refuse
    if [[ -e "$directory" ]]; then
      [[ -d "$directory" ]] || refuse
      read -r owner mode < <(stat -c '%u %a' -- "$directory")
      [[ "$owner" == 0 && "$mode" =~ ^[0-7]{3,4}$ ]] || refuse
      (( (8#$mode & 0022) == 0 )) || refuse
    fi
  done
  shopt -s nullglob dotglob
  entries=(/run/omavless-dns/private/*)
  [[ ${#entries[@]} == 0 ]] || refuse
  [[ ! -e /sys/class/net/Meta && ! -e /sys/class/net/omavless0 ]] || refuse
  [[ "$(systemctl show omavless-dns-broker.service --property=LoadState --value)" == loaded ]] || refuse
  [[ "$(systemctl show omavless-dns-broker.service --property=NFileDescriptorStore --value)" == 0 ]] || refuse
  [[ -z "$(systemctl show omavless-dns-broker.service --property=DropInPaths --value)" ]] || refuse
  [[ "$(systemctl show omavless-dns-broker.service --property=FragmentPath --value)" == /usr/lib/systemd/system/omavless-dns-broker.service ]] || refuse
  [[ "$(systemctl show omavless-dns-broker.service --property=NeedDaemonReload --value)" == no ]] || refuse
  [[ -z "$(systemctl show omavless-dns-broker.service --property=Job --value)" ]] || refuse
  empty_host
}
clean
check_enrollment
[[ $(sha256sum /usr/lib/systemd/system/omavless-dns-broker.service) == '126b208158e9288c475084c8bd74029e2808edf9846da514a369266b6306462d  /usr/lib/systemd/system/omavless-dns-broker.service' ]] || refuse
if [[ $(systemctl show omavless-dns-broker.service --property=ActiveState --value) == inactive ]]; then
  [[ $(systemctl show omavless-dns-broker.service --property=SubState --value) == dead &&
     $(systemctl show omavless-dns-broker.service --property=MainPID --value) == 0 ]] || refuse
  /usr/lib/omavless-dns/package-guard
  systemctl disable omavless-dns-broker.service
  printf '%s\n' 'DNS helper is already stopped. Preparation is complete.'
  exit 0
fi
[[ "$enrolled" == true ]] || refuse
[[ "$(systemctl show omavless-dns-broker.service --property=ActiveState --value)" == active &&
   "$(systemctl show omavless-dns-broker.service --property=SubState --value)" == running ]] || refuse
original_pid=$(systemctl show omavless-dns-broker.service --property=MainPID --value)
[[ "$original_pid" =~ ^[1-9][0-9]*$ &&
   $(stat -c '%u' "/proc/$original_pid") == 0 &&
   $(readlink "/proc/$original_pid/exe") == /usr/lib/omavless-dns/omavless-dns-broker ]] || refuse
broker_hash=$(sha256sum "/proc/$original_pid/exe")
case "${broker_hash%% *}" in
  8caba1a57b44b2856b98b0d0213cc545905ba5a031aac144fcce2420957b0d2e|33121a6f4c4eb7b644da034fc521e221d7f292d61bdaffb837ed82c015dbe755|ba61e433d89baa3ae4fd1af5ed275f769654c19fbf6a7b22592869e617b277d5|8f250d4fce7e784477c91204579ed6f8792006713ccc895ac357e23faa03df26|1855ed920a4b064c811ed14134b5ae14e3c5ece0a26695a84afc1ccfd711dc93|c764c30ba24844f6972e2b19c600c068c92682d9fe1e1bd94de5896356f93c5e) ;;
  *) refuse ;;
esac
systemctl disable --now omavless-dns-broker.service
[[ "$(systemctl show omavless-dns-broker.service --property=ActiveState --value)" == inactive ]] || refuse
[[ "$(systemctl show omavless-dns-broker.service --property=SubState --value)" == dead ]] || refuse
[[ "$(systemctl show omavless-dns-broker.service --property=MainPID --value)" == 0 ]] || refuse
[[ "$(systemctl show omavless-dns-broker.service --property=Result --value)" == success ]] || refuse
[[ ! -e "/proc/$original_pid" ]] || refuse
exit_code=$(systemctl show omavless-dns-broker.service --property=ExecMainCode --value)
exit_status=$(systemctl show omavless-dns-broker.service --property=ExecMainStatus --value)
[[ "$exit_code:$exit_status" == 0:0 || "$exit_code:$exit_status" == 1:0 || "$exit_code:$exit_status" == 2:15 ]] || refuse
clean
printf '%s\n' 'DNS helper stopped safely.'
ROOT_PREPARE
  root_prepare="$root_checks"$'\n'"$root_prepare"
  release_lock=false
  if [[ "$guided" == true ]]; then printf '%s\n' '[2/3] Preparing removal...'; fi
  sudo /usr/bin/bash -c "$root_prepare" || fail 'DNS preparation did not finish. Do not reboot or repeat; send the error to the maintainer.'
  if [[ "$guided" == true ]]; then write_stage prepared; fi
  release_lock=true
  if [[ "$guided" == true ]]; then
    cleanup_lock
    trap - EXIT
    offer_restart
    exit 0
  fi
  printf '%s\n' 'Preparation finished.' \
    'Save your other work, then restart your computer once: systemctl reboot' \
    'After logging back in, finish removal with:' \
    "bash \"$HOME/.cache/omavless-maintenance/reinstall-098/clean-reinstall-098.sh\" remove"
else
  if [[ "$guided" != true ]]; then
    printf '%s\n' 'Remove the old OmaVLESS packages and delete ONLY its profiles and settings. No backup is created.'
  fi
  check_private_targets
  check_supported_installation
  plugin="$HOME/.config/omarchy/plugins/kdk.omavless"
  [[ ! -e "$plugin" && ! -L "$plugin" ]] || fail 'First run the prepare command to remove the plugin, then restart once.'
  runtime_disabled \
    || fail 'OmaVLESS startup is still enabled. First run the prepare command.'
  confirm_reset remove || fail 'Cancelled. Nothing was removed.'
  [[ "$(systemctl --user show omavless-runtime.service --property=MainPID --value)" == 0 ]] || fail 'OmaVLESS is still running.'
  runtime="/run/user/$(id -u)"
  [[ ! -e "$runtime/omavless" && ! -L "$runtime/omavless" &&
     ! -e "$runtime/omavless-login.receipt" && ! -L "$runtime/omavless-login.receipt" ]] \
    || fail 'Old session files remain. Follow the prepare/reboot steps; do not delete them manually.'
  # Check the intended account again before the existing fixed revocation.
  read -r -d '' root_remove <<'ROOT_REMOVE' || true
if dns_absent; then remove_empty_enrollment_directory; exit 0; fi
check_dns_programs
/usr/lib/omavless-dns/package-guard
empty_host
check_enrollment
if [[ "$enrolled" == true ]]; then
  /usr/lib/omavless-dns/omavless-dns-broker --revoke
else
  printf '%s\n' 'DNS helper was not configured for an account; no access needs removal.'
fi
remove_empty_enrollment_directory
ROOT_REMOVE
  root_remove="$root_checks"$'\n'"$root_remove"
  release_lock=false
  if [[ "$guided" == true ]]; then write_stage removing; printf '%s\n' '[3/3] Removing OmaVLESS components and settings...'; fi
  sudo /usr/bin/bash -c "$root_remove" || fail 'DNS removal did not finish. Stop here and send the error; do not retry or delete files.'
  packages=()
  if pacman -Q omavless >/dev/null 2>&1; then packages+=(omavless); fi
  if pacman -Q omavless-dns >/dev/null 2>&1; then packages+=(omavless-dns); fi
  if [[ ${#packages[@]} -gt 0 ]]; then
    sudo pacman -R -- "${packages[@]}" || fail 'Package removal did not finish. Stop here and send the error.'
  else
    printf '%s\n' 'Application packages are already absent.'
  fi
  [[ ! -e /usr/bin/omavless && ! -L /usr/bin/omavless ]] || fail 'The application package remains installed.'
  check_private_targets
  [[ -d "$setup_lock" && ! -L "$setup_lock" && $(stat -c '%d:%i:%u:%a' "$setup_lock") == "$lock_identity" ]] \
    || fail 'The removal session changed. No settings were deleted; ask for help.'
  # Only the three independently admitted application directories, as this user.
  for directory in "$HOME/.config/omavless" "$HOME/.local/state/omavless" "$HOME/.cache/omavless"; do
    [[ ! -e "$directory" ]] || rm -r --one-file-system -- "$directory" \
      || fail 'Settings removal did not finish. Stop here and ask for help.'
  done
  if [[ "$guided" == true ]]; then
    [[ -f "$stage_file" && ! -L "$stage_file" && $(stat -c '%u:%a:%h' -- "$stage_file") == "$(id -u):600:1" &&
       $(<"$stage_file") == "$stage_bytes" ]] || fail 'Components were removed, but the progress record changed. Ask for help.'
    unlink -- "$stage_file" || fail 'Components were removed, but the progress record could not be cleared. Ask for help.'
    sync -f -- "$stage_directory" || fail 'Components were removed, but progress cleanup was not confirmed. Ask for help.'
  fi
  release_lock=true
  printf '%s\n' 'Old OmaVLESS components and settings removed. You can now install the fixed version.'
  if [[ "$guided" == true ]]; then
    printf '%s\n' '' 'To install OmaVLESS again:' \
      'omarchy plugin add https://github.com/k-kostin/omavless.git --enable'
  fi
fi
