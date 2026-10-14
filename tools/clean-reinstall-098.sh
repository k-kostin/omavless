#!/bin/bash
# SPDX-License-Identifier: MIT
# Maintainer-assisted reset for normal, never-connected partial 0.9.8 setup.
# Manual terminal administration, NOT a runtime API or all-state uninstaller.
set -euo pipefail
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
[[ $# == 1 && ( $1 == prepare || $1 == remove ) ]] || fail 'Use: bash clean-reinstall-098.sh prepare|remove'
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
if [[ $1 == prepare ]]; then
  printf '%s\n' 'Prepare a fresh OmaVLESS installation. Use only if you never connected it.' \
    'Close other OmaVLESS clients. Other VPN applications are not changed.'
  plugin="$HOME/.config/omarchy/plugins/kdk.omavless"
  if [[ -e "$plugin" || -L "$plugin" ]]; then
    [[ -d "$plugin" && ! -L "$plugin" ]] || fail 'Unusual plugin installation; ask for help.'
    if [[ -d "$plugin/.git" ]]; then
      changes=$(git -C "$plugin" status --porcelain) || fail 'Cannot check plugin files.'
      [[ -z "$changes" ]] || fail 'Plugin has local changes; save them before removing it.'
    fi
    omarchy plugin remove kdk.omavless || fail 'Plugin removal did not finish.'
  fi
  systemctl --user disable omavless-runtime.service || fail 'Cannot disable OmaVLESS startup.'
  [[ "$(systemctl --user show omavless-runtime.service --property=MainPID --value)" == 0 ]] \
    || fail 'OmaVLESS is still running. Use Settings → Shut down OmaVLESS first.'
  # Fixed reviewed source already loaded here; never sudo an unprivileged script
  # pathname or accept caller-provided root commands, paths or account selectors.
  read -r -d '' root_prepare <<'ROOT_PREPARE' || true
set -euo pipefail
export PATH=/usr/bin:/bin LC_ALL=C
refuse() { printf '%s\n' 'STOP: clean reinstall preconditions not satisfied; ask for help.' >&2; exit 1; }
clean() {
  for directory in /run /run/omavless-dns /run/omavless-dns/private; do
    [[ -d "$directory" && ! -L "$directory" ]] || refuse
    read -r owner mode < <(stat -c '%u %a' -- "$directory")
    [[ "$owner" == 0 && "$mode" =~ ^[0-7]{3,4}$ ]] || refuse
    (( (8#$mode & 0022) == 0 )) || refuse
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
  for executable in /proc/[0-9]*/exe; do
    target=$(readlink "$executable" 2>/dev/null) || continue
    case "$target" in
      /usr/bin/omavless|'/usr/bin/omavless (deleted)'|/usr/lib/omavless-dns/mihomo|'/usr/lib/omavless-dns/mihomo (deleted)') refuse ;;
    esac
  done
}
clean
[[ "${SUDO_UID:-}" =~ ^[1-9][0-9]*$ ]] || refuse
[[ -d /etc/omavless-dns && ! -L /etc/omavless-dns &&
   $(stat -c '%u:%a' /etc/omavless-dns) == 0:700 ]] || refuse
enrollment=/etc/omavless-dns/release-enrollment.json
[[ -f "$enrollment" && ! -L "$enrollment" &&
   $(stat -c '%u:%a:%h' "$enrollment") == 0:600:1 &&
   $(stat -c '%s' "$enrollment") -le 256 ]] || refuse
jq -e --argjson uid "$SUDO_UID" '.schema == 1 and .uid == $uid and .policy == "meta-ipv4-release-v1"' "$enrollment" >/dev/null || refuse
[[ $(sha256sum /usr/lib/systemd/system/omavless-dns-broker.service) == '126b208158e9288c475084c8bd74029e2808edf9846da514a369266b6306462d  /usr/lib/systemd/system/omavless-dns-broker.service' ]] || refuse
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
printf '%s\n' 'CLEAN_STOP_CONFIRMED: save your work, then reboot once.'
ROOT_PREPARE
  sudo /usr/bin/bash -c "$root_prepare" || fail 'DNS preparation did not finish. Do not reboot or repeat; send the error to the maintainer.'
  printf '%s\n' 'Preparation finished. Save your other work and reboot once. Then run the remove command.'
else
  printf '%s\n' 'Remove the old OmaVLESS packages and delete ONLY its profiles and settings. No backup is created.'
  check_private_targets
  [[ "$(systemctl --user show omavless-runtime.service --property=MainPID --value)" == 0 ]] || fail 'OmaVLESS is still running.'
  runtime="/run/user/$(id -u)"
  [[ ! -e "$runtime/omavless" && ! -L "$runtime/omavless" &&
     ! -e "$runtime/omavless-login.receipt" && ! -L "$runtime/omavless-login.receipt" ]] \
    || fail 'Old session files remain. Follow the prepare/reboot steps; do not delete them manually.'
  # Check the intended account again before the existing fixed revocation.
  read -r -d '' root_remove <<'ROOT_REMOVE' || true
set -euo pipefail
export PATH=/usr/bin:/bin LC_ALL=C
/usr/lib/omavless-dns/package-guard
[[ "${SUDO_UID:-}" =~ ^[1-9][0-9]*$ ]]
[[ -d /etc/omavless-dns && ! -L /etc/omavless-dns &&
   $(stat -c '%u:%a' /etc/omavless-dns) == 0:700 ]]
enrollment=/etc/omavless-dns/release-enrollment.json
[[ -f "$enrollment" && ! -L "$enrollment" &&
   $(stat -c '%u:%a:%h' "$enrollment") == 0:600:1 &&
   $(stat -c '%s' "$enrollment") -le 256 ]]
jq -e --argjson uid "$SUDO_UID" '.schema == 1 and .uid == $uid and .policy == "meta-ipv4-release-v1"' "$enrollment" >/dev/null
/usr/lib/omavless-dns/omavless-dns-broker --revoke
ROOT_REMOVE
  sudo /usr/bin/bash -c "$root_remove" || fail 'DNS removal did not finish. Stop here and send the error; do not retry or delete files.'
  sudo pacman -R -- omavless omavless-dns || fail 'Package removal did not finish. Stop here and send the error.'
  [[ ! -e /usr/bin/omavless && ! -L /usr/bin/omavless ]] || fail 'The application package remains installed.'
  check_private_targets
  # Only the three independently admitted application directories, as this user.
  for directory in "$HOME/.config/omavless" "$HOME/.local/state/omavless" "$HOME/.cache/omavless"; do
    [[ ! -e "$directory" ]] || rm -r --one-file-system -- "$directory" \
      || fail 'Settings removal did not finish. Stop here and ask for help.'
  done
  printf '%s\n' 'Old OmaVLESS components and settings removed. You can now install the fixed version.'
fi
