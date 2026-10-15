#!/bin/bash
# SPDX-License-Identifier: MIT
# Explicit terminal removal, independent of the widget/native setup UI.
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
  [[ ${#mounts} -le 524288 ]] || fail 'The mount list exceeds the removal limit. No folders were deleted.'
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

say() { printf '%s\n' "$@"; }
prepare_maintenance_folders() {
  local directory owner mode
  folder="$HOME/.cache/omavless-maintenance/uninstall"
  for directory in "$HOME/.cache/omavless-maintenance" "$folder"; do
    [[ -e "$directory" || -L "$directory" ]] || mkdir -- "$directory"
    [[ -d "$directory" && ! -L "$directory" ]] || fail 'The removal folder is redirected. Its target was left unchanged.'
    read -r owner mode < <(stat -c '%u %a' -- "$directory")
    [[ "$owner" == "$(id -u)" && "$mode" =~ ^[0-7]{3,4}$ ]] && (( (8#$mode & 0022) == 0 )) \
      || fail 'The removal folder has unsafe ownership or permissions. Nothing was deleted.'
    [[ "$directory" != "$folder" || "$mode" == 700 ]] \
      || fail 'The private removal folder must have mode 700. Nothing was deleted.'
  done
}
check_authorizations() {
  local file comm uid args
  [[ ! -e /var/lib/pacman/db.lck ]] \
    || fail 'The package manager is busy. Let its current transaction finish, then run this command again. Do not delete its lock.'
  for file in /proc/[0-9]*/comm; do
    [[ -r "$file" ]] || continue
    comm=$(<"$file") || continue
    case "$comm" in
      sudo|sudoedit)
        [[ -r ${file%/comm}/cmdline ]] || continue
        args=$(head -c 65536 "${file%/comm}/cmdline" | tr '\0' '\n') || continue
        [[ "$args" != *'# OmaVLESS removal attempt '* ]] \
          || fail 'An earlier removal password prompt is still open. Finish or cancel it, then run this command again.' ;;
      resolvectl|pkexec)
        uid=$(awk '/^Uid:/ {print $2}' "${file%/comm}/status") || continue
        [[ "$uid" != "$(id -u)" ]] \
          || fail 'A system authorization command is still open. Finish or cancel its prompt, then run this command again.' ;;
    esac
  done
}
process_birth() {
  local line fields
  [[ "$1" =~ ^[1-9][0-9]*$ ]] || return 1
  [[ -e /proc/$1 ]] || return 2
  line=$(<"/proc/$1/stat") || return 1
  read -ra fields <<< "${line##*) }"
  [[ ${fields[19]:-} =~ ^[0-9]+$ ]] || return 1
  printf '%s\n' "${fields[19]}"
}
operator_gone() {
  local pid birth actual status
  IFS=: read -r pid birth <<< "$previous_operator"
  if actual=$(process_birth "$pid"); then [[ "$actual" != "$birth" ]]; else
    status=$?; [[ "$status" == 2 ]]
  fi
}
safe_file() {
  [[ -f "$1" && ! -L "$1" && $(stat -c '%u:%a:%h' -- "$1") == "$(id -u):600:1" ]]
}
stage_resume_helper() {
  local original=$1 digest actual temporary destination
  [[ -f "$original" && $(stat -c '%s' -- "$original") -le 65536 ]] \
    || fail 'Cannot retain this removal command. Download the official helper again.'
  digest=$(sha256sum -- "$original"); digest=${digest%% *}
  [[ "$digest" =~ ^[a-f0-9]{64}$ ]] || fail 'Cannot identify this removal command.'
  destination="$folder/uninstall-$digest.sh"
  if [[ ! -e "$destination" && ! -L "$destination" ]]; then
    temporary=$(mktemp "$folder/.helper.XXXXXXXX") || fail 'Free space in your home folder before continuing.'
    cp -- "$original" "$temporary"
    chmod 600 -- "$temporary"
    actual=$(sha256sum -- "$temporary")
    [[ ${actual%% *} == "$digest" ]] || fail 'The helper changed while being saved. Download it again; no removal step was submitted.'
    mv -T --no-clobber -- "$temporary" "$destination"
    [[ ! -e "$temporary" ]] || unlink -- "$temporary"
  fi
  safe_file "$destination" || fail 'The saved removal command has unexpected permissions. It was not overwritten.'
  actual=$(sha256sum -- "$destination")
  [[ ${actual%% *} == "$digest" ]] || fail 'The saved removal command was modified. It was not run or overwritten.'
  sync -f -- "$folder" || fail 'The continuation command could not be saved. Free space before continuing.'
  helper_path=$destination
}
read_progress() {
  phase=none; masks='[]'; recorded_lock=; lock_token=; previous_operator=; prepared_boot=$boot_id
  if [[ ! -e "$record" && ! -L "$record" ]]; then return; fi
  safe_file "$record" && [[ $(stat -c '%s' -- "$record") -le 2048 ]] \
    || fail 'The removal record has unexpected permissions. Restore its original private file before continuing.'
  record_bytes=$(<"$record")
  [[ $(printf '%s' "$record_bytes" | jq -c .) == "$record_bytes" ]] \
    || fail 'The removal record is damaged. Keep it unchanged; automatic continuation is unavailable for a damaged record.'
  printf '%s' "$record_bytes" | jq -e --argjson uid "$(id -u)" '
    keys == ["attempt","boot","lock","lockToken","masks","operator","phase","schema","uid"] and
    .schema == 2 and .uid == $uid and (.attempt | test("^[a-f0-9]{32}$")) and
    (.boot | test("^[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}$")) and
    (.operator | test("^[1-9][0-9]*:[0-9]+$")) and
    (.lock | test("^([0-9]+:[0-9]+:[0-9]+:700)?$")) and
    (.lockToken | test("^([a-f0-9]{32})?$")) and
    (.lock == "" or (.lock | endswith(":" + ($uid|tostring) + ":700"))) and
    ((.lock == "") == (.lockToken == "")) and
    (.phase == "preparing" or .phase == "prepared" or .phase == "removing") and
    (.masks | type == "array" and length <= 2 and
      all(.[]; (keys == ["identity","unit"] or
          (keys == ["identity","unit","witness"] and (.witness | test("^u?[a-f0-9]{32}$")))) and
        (.unit == "omavless-runtime.service" or .unit == "omavless-login-prepare.service") and
        (.identity | test("^[0-9]+:[0-9]+$"))) and
      ([.[].unit] | unique | length) == length)
  ' >/dev/null || fail 'This removal record has an unsupported format. Do not delete system files to bypass it.'
  phase=$(jq -r .phase <<< "$record_bytes")
  masks=$(jq -c .masks <<< "$record_bytes")
  recorded_lock=$(jq -r .lock <<< "$record_bytes")
  lock_token=$(jq -r .lockToken <<< "$record_bytes")
  previous_operator=$(jq -r .operator <<< "$record_bytes")
  prepared_boot=$(jq -r .boot <<< "$record_bytes")
}
save_progress() {
  local next=$1 temporary
  if [[ "$phase" == none ]]; then
    [[ ! -e "$record" && ! -L "$record" ]] || fail 'Another removal created a record. Run this command again.'
  else
    safe_file "$record" && [[ $(<"$record") == "$record_bytes" ]] \
      || fail 'The removal record changed. No further step was submitted.'
  fi
  temporary=$(mktemp "$folder/.progress.XXXXXXXX") || fail 'Cannot save progress. Free space in your home folder, then run this command again.'
  jq -cn --argjson uid "$(id -u)" --arg attempt "$attempt" --arg boot "$prepared_boot" \
    --arg operator "$operator" --arg lock "$recorded_lock" --arg lockToken "$lock_token" --arg phase "$next" --argjson masks "$masks" \
    '{schema:2,uid:$uid,attempt:$attempt,boot:$boot,operator:$operator,lock:$lock,lockToken:$lockToken,phase:$phase,masks:$masks}' > "$temporary"
  if [[ "$phase" == none ]]; then
    ln -- "$temporary" "$record" || fail 'Another removal started. Run this command again.'
    unlink -- "$temporary"
  else
    mv -T -- "$temporary" "$record"
  fi
  sync -f -- "$folder" || fail 'Progress could not be saved safely. Free space before continuing.'
  read_progress
}
confirm_removal() {
  say '' 'Remove OmaVLESS' '' \
    'OmaVLESS and its saved profiles and settings will be deleted. No backup is made.' \
    'Other VPN clients and a separately installed Mihomo stay installed.' '' \
    'Type REMOVE to continue, or press Enter to cancel:'
  local answer
  IFS= read -r answer || return 1
  [[ "$answer" == REMOVE ]]
}
restart_offer() {
  say '' 'One restart is needed to finish removal.' \
    'Save your work. Restarting interrupts all current connections.' \
    'If VPN cleanup was interrupted, protection is not guaranteed while waiting.' \
    'After logging back in, run this same command:' "bash \"$helper_path\"" '' \
    'Type REBOOT to restart now, or press Enter to restart later:'
  local answer
  IFS= read -r answer || answer=
  if [[ "$answer" == REBOOT ]]; then
    systemctl reboot || fail 'Restart from your desktop menu, then run the command above.'
  else say 'Restart from your desktop when ready, then run the command above.'; fi
}
take_setup_lock() {
  local temporary
  if [[ -e "$setup_lock" || -L "$setup_lock" ]]; then
    [[ -n "$recorded_lock" && -d "$setup_lock" && ! -L "$setup_lock" &&
       $(stat -c '%d:%i:%u:%a' -- "$setup_lock") == "$recorded_lock" ]] \
      || fail 'A setup may still be running. Finish or cancel its terminal and authorization prompts. If no setup is running and all prompts are closed, save your work and restart the computer, then run this command again. Do not delete the setup lock.'
    [[ -z $(find "$setup_lock" -mindepth 1 -maxdepth 1 -print -quit) ]] \
      || fail 'The earlier setup directory is not empty. Nothing in it was deleted.'
    # The original parent is gone and the root barrier already reconciled its
    # critical section/pending authorization. Reuse its exact empty directory.
    return
  fi
  if [[ -n "$lock_token" ]]; then
    temporary="$runtime/.omavless-uninstall-$lock_token"
    if [[ -d "$temporary" && ! -L "$temporary" && $(stat -c '%d:%i:%u:%a' -- "$temporary") == "$recorded_lock" ]]; then
      mv -T --no-clobber -- "$temporary" "$setup_lock"
      [[ -d "$setup_lock" && ! -L "$setup_lock" && $(stat -c '%d:%i:%u:%a' -- "$setup_lock") == "$recorded_lock" ]] \
        || fail 'Another setup acquired the exclusion. Finish it before continuing.'
      return
    fi
  fi
  lock_token=$attempt
  temporary="$runtime/.omavless-uninstall-$lock_token"
  mkdir -- "$temporary" || fail 'Cannot create a private removal exclusion.'
  recorded_lock=$(stat -c '%d:%i:%u:%a' -- "$temporary")
  # Record identity BEFORE atomic publication; an earlier crash can leave only
  # an unpublished private directory, never an unrecorded shared setup lock.
  save_progress "$phase"
  mv -T --no-clobber -- "$temporary" "$setup_lock"
  [[ -d "$setup_lock" && ! -L "$setup_lock" && $(stat -c '%d:%i:%u:%a' -- "$setup_lock") == "$recorded_lock" ]] \
    || fail 'Another setup acquired the exclusion. Finish it before continuing.'
}
release_setup_lock() {
  [[ -e "$setup_lock" || -L "$setup_lock" ]] || return 0
  [[ -n ${recorded_lock:-} && -d "$setup_lock" && ! -L "$setup_lock" &&
     $(stat -c '%d:%i:%u:%a' -- "$setup_lock") == "$recorded_lock" ]] \
    || fail 'The setup exclusion changed. Removal progress was kept; no unrelated directory was deleted.'
  rmdir -- "$setup_lock" || fail 'The setup exclusion is not empty. Removal progress was kept; run the same command after the other setup finishes.'
}
mask_link_matches() {
  [[ -L "$1" && $(readlink -- "$1") == /dev/null &&
     $(stat -c '%u:%i:%h' -- "$1") == "$(id -u):$2:$3" ]]
}
mask_witness_path() {
  # Fixed locations only. New adjacent witnesses work even when .cache is on
  # a separate filesystem; the old unprefixed token retains dd compatibility.
  if [[ "$1" == u* ]]; then
    printf '%s/.omavless-uninstall-%s-%s.link\n' "$HOME/.config/systemd/user" "${1#u}" "$2"
  else
    printf '%s/mask-%s%s\n' "$folder" "${1:+$1-}" "$2"
  fi
}
mask_future_start() {
  local unit destination temporary identity expected witness
  for directory in "$HOME/.config/systemd" "$HOME/.config/systemd/user"; do
    if [[ ! -e "$directory" && ! -L "$directory" ]]; then mkdir -- "$directory"; fi
    [[ -d "$directory" && ! -L "$directory" && $(stat -c '%u' -- "$directory") == "$(id -u)" ]] \
      || fail 'Your user-service folder is unusual. Restore its normal ownership before continuing.'
    read -r owner mode < <(stat -c '%u %a' -- "$directory")
    (( (8#$mode & 0022) == 0 )) || fail 'Your user-service folder is writable by another account.'
  done
  for unit in omavless-runtime.service omavless-login-prepare.service; do
    destination="$HOME/.config/systemd/user/$unit"
    expected=$(jq -r --arg unit "$unit" '.[] | select(.unit == $unit) | .identity' <<< "$masks")
    witness=$(jq -r --arg unit "$unit" '.[] | select(.unit == $unit) | .witness // ""' <<< "$masks")
    temporary=$(mask_witness_path "$witness" "$unit")
    if [[ -e "$destination" || -L "$destination" ]]; then
      [[ -L "$destination" && $(readlink -- "$destination") == /dev/null ]] \
        || fail "A custom $unit override blocks removal. Move that override out of the user-service folder, then run this command again."
      if [[ -n "$expected" ]]; then
        if [[ -n "$witness" ]]; then
          mask_link_matches "$destination" "${expected#*:}" 2 && mask_link_matches "$temporary" "${expected#*:}" 2 &&
            [[ $(stat -c '%d:%i' -- "$destination") == $(stat -c '%d:%i' -- "$temporary") ]] \
            || fail 'A removal-created service mask or its saved witness was replaced. No other service file was changed.'
        else
          [[ $(stat -c '%d:%i' -- "$destination") == "$expected" ]] || fail 'A removal-created service mask was replaced. No other service file was changed.'
        fi
      fi
      continue
    fi
    if [[ -n "$expected" ]]; then
      mask_link_matches "$temporary" "${expected#*:}" 1 || fail 'A saved service mask is missing or changed. No new mask was created.'
      [[ -n "$witness" || $(stat -c '%d:%i' -- "$temporary") == "$expected" ]] \
        || fail 'The older saved service mask changed. No new mask was created.'
    else
      witness=u$attempt
      temporary=$(mask_witness_path "$witness" "$unit")
      [[ ! -e "$temporary" && ! -L "$temporary" ]] || fail 'An unrecognized temporary service mask exists. It was left unchanged.'
      ln -s -- /dev/null "$temporary"
      identity=$(stat -c '%d:%i' -- "$temporary")
      sync -f -- "${temporary%/*}" || fail 'The saved service mask could not be made durable. Free space before continuing.'
      masks=$(jq -c --arg unit "$unit" --arg identity "$identity" --arg witness "$witness" '. + [{unit:$unit,identity:$identity,witness:$witness}]' <<< "$masks")
      save_progress "$phase"
    fi
    # Publish only after exact mask identity was durably recorded; no overwrite.
    ln -P -T -- "$temporary" "$destination"
    # Keep a private hardlink witness across reboot. st_dev alone is not a
    # persistent identity (Btrfs may assign another device number next boot).
    sync -f -- "${destination%/*}" || fail 'Startup changes could not be saved. Free space before continuing; removal progress is kept.'
  done
  # Existing-pair retries must earn durability too, including a previous
  # publication whose sync failed with .cache on another filesystem.
  sync -f -- "$HOME/.config/systemd/user" || fail 'Startup changes could not be saved. Free space before continuing; removal progress is kept.'
  systemctl --user daemon-reload
  for unit in omavless-runtime.service omavless-login-prepare.service; do
    [[ $(systemctl --user show "$unit" --property=LoadState --value) == masked ]] \
      || fail 'OmaVLESS startup is not yet blocked. Run the same removal command again before restarting.'
  done
}
check_user_units() {
  local unit loaded fragment dropins reload expected digest
  for unit in omavless-runtime.service omavless-login-prepare.service; do
    loaded=$(systemctl --user show "$unit" --property=LoadState --value) || return 1
    [[ "$loaded" != not-found ]] || continue
    dropins=$(systemctl --user show "$unit" --property=DropInPaths --value)
    reload=$(systemctl --user show "$unit" --property=NeedDaemonReload --value)
    [[ -z "$dropins" && "$reload" == no ]] \
      || fail "The $unit service has overrides or an unfinished reload. Restore its packaged configuration before continuing."
    if [[ "$loaded" == masked ]]; then
      [[ -L "$HOME/.config/systemd/user/$unit" && $(readlink -- "$HOME/.config/systemd/user/$unit") == /dev/null ]] \
        || fail 'An unfamiliar service mask is active. No service configuration was changed.'
      continue
    fi
    fragment=$(systemctl --user show "$unit" --property=FragmentPath --value)
    [[ "$loaded" == loaded && "$fragment" == "/usr/lib/systemd/user/$unit" &&
       -f "$fragment" && ! -L "$fragment" && $(stat -c '%u:%a:%h' -- "$fragment") == 0:644:1 ]] \
      || fail "The $unit service is not the supported packaged service. Restore its official unit before continuing."
    case "$unit" in
      omavless-runtime.service) expected=438df1a83ff636b662c2422c522124ccdc1a37c195dc4076834ec0f522cb250f ;;
      omavless-login-prepare.service) expected=15d2b5405938cc36346e4c73a8cd2394cca30bf366ac53a63365dad37054b209 ;;
    esac
    digest=$(sha256sum -- "$fragment")
    [[ ${digest%% *} == "$expected" ]] || fail "The $unit service was modified. Restore its official unit before continuing."
  done
}
check_legacy_selection() {
  local environment line manager_path= directory resolved found=false
  [[ -z ${OMAVLESS_MIHOMO:-} && ! -e "$HOME/.local/bin/mihomo" && ! -L "$HOME/.local/bin/mihomo" ]] \
    || fail 'A custom Mihomo override is configured. Remove that override from OmaVLESS before continuing; the separate core was left installed.'
  environment=$(systemctl --user show-environment) || fail 'Cannot read the application service environment.'
  [[ ${#environment} -le 262144 ]] || fail 'The service environment exceeds the removal limit.'
  while IFS= read -r line; do
    case "$line" in
      OMAVLESS_MIHOMO=*) [[ "$line" == OMAVLESS_MIHOMO= ]] || fail 'The application service selects a custom Mihomo. Clear its OMAVLESS_MIHOMO override before continuing.' ;;
      PATH=*) manager_path=${line#PATH=} ;;
    esac
  done <<< "$environment"
  [[ "$manager_path" =~ ^[a-zA-Z0-9_./:+-]+$ ]] || fail 'The application service has an unrecognized executable search path. Restore its normal desktop PATH before continuing.'
  local directories
  IFS=: read -ra directories <<< "$manager_path"
  for directory in "${directories[@]}"; do
    [[ "$directory" == /* ]] || fail 'A relative executable search path is configured for the application.'
    if [[ -x "$directory/mihomo" && -f "$directory/mihomo" ]]; then
      resolved=$(realpath -e -- "$directory/mihomo") || return 1
      [[ "$resolved" == /usr/bin/mihomo ]] || fail 'The application service selects a custom core. Restore its packaged Mihomo selection before continuing.'
      found=true
      break
    fi
  done
  # A genuinely missing core is a supported partial installation.
  [[ "$found" == true || ( ! -e /usr/bin/mihomo && ! -L /usr/bin/mihomo ) ]] \
    || fail 'The packaged core is not selected by the application service PATH.'
}
check_widget() {
  local plugin="$HOME/.config/omarchy/plugins/kdk.omavless" changes
  if [[ -e "$plugin" || -L "$plugin" ]]; then
    [[ -d "$plugin" && ! -L "$plugin" ]] || fail 'The widget folder is redirected. Its target was left unchanged.'
    if [[ -e "$plugin/.git" || -L "$plugin/.git" ]]; then
      changes=$(git -C "$plugin" status --porcelain) \
        || fail 'Cannot check the installed widget for development edits. Its files were left unchanged.'
      [[ -z "$changes" ]] \
        || fail 'The installed widget has development edits. Save those edits outside the installed widget, then run removal again.'
    fi
  fi
}
remove_widget() {
  local plugin="$HOME/.config/omarchy/plugins/kdk.omavless"
  check_widget
  if [[ -e "$plugin" || -L "$plugin" ]]; then
    LC_ALL=C.UTF-8 omarchy plugin remove kdk.omavless || fail 'Widget removal was cancelled or failed. Run this same command to continue.'
  fi
  [[ ! -e "$plugin" && ! -L "$plugin" ]] || fail 'The widget is still installed. Finish its removal prompt, then run this command again.'
}
clear_created_masks() {
  local unit destination expected temporary witness links destination_present temporary_present
  while IFS=$'\t' read -r unit expected witness; do
    destination="$HOME/.config/systemd/user/$unit"
    temporary=$(mask_witness_path "$witness" "$unit")
    destination_present=false; temporary_present=false; links=0
    if [[ -e "$destination" || -L "$destination" ]]; then destination_present=true; ((links+=1)); fi
    if [[ -e "$temporary" || -L "$temporary" ]]; then temporary_present=true; ((links+=1)); fi
    if [[ "$witness" != '' && "$destination_present" == true && "$temporary_present" != true ]]; then
      fail 'The saved service-mask witness is missing. Removal progress is kept; no service file was changed.'
    fi
    # Validate the complete pair before either unlink; after an interruption a
    # lone witness is expected, but a lone new-format destination is not.
    if [[ "$links" == 2 ]]; then
      [[ $(stat -c '%d:%i' -- "$destination") == $(stat -c '%d:%i' -- "$temporary") ]] \
        || fail 'A saved service mask was replaced. No service file was changed.'
    fi
    for path in "$destination" "$temporary"; do
      if [[ -e "$path" || -L "$path" ]]; then
        mask_link_matches "$path" "${expected#*:}" "$links" \
          || fail 'A service mask changed. It was left untouched; removal progress is saved.'
        if [[ -z "$witness" && $(stat -c '%d:%i' -- "$path") != "$expected" ]]; then
          # Compatibility with the unreleased b4 receipt, after this invocation
          # freshly proved packages/resources absent. Ignore only old st_dev.
          [[ ${mask_cleanup_admitted:-false} == true && "$phase" == removing && "$prepared_boot" != "$boot_id" && "$links" == 1 ]] \
            || fail 'The older service mask identity changed. It was left untouched; removal progress is saved.'
        fi
      fi
    done
    # Destination first: a crash may leave only its private retained witness.
    if [[ "$destination_present" == true ]]; then unlink -- "$destination"; fi
    if [[ "$temporary_present" == true ]]; then
      mask_link_matches "$temporary" "${expected#*:}" 1 || fail 'The remaining service-mask witness changed. Removal progress is kept.'
      unlink -- "$temporary"
    fi
  done < <(jq -r '.[] | [.unit,.identity,(.witness // "")] | @tsv' <<< "$masks")
  if [[ -d "$HOME/.config/systemd/user" ]]; then
    sync -f -- "$HOME/.config/systemd/user" || fail 'Service-mask cleanup could not be saved. Free space, then run the same command to finish.'
  fi
  systemctl --user daemon-reload
}
clear_runtime_enablement() {
  local parent path target owner mode
  for parent in "$HOME/.config/systemd" "$HOME/.config/systemd/user"; do
    if [[ -e "$parent" || -L "$parent" ]]; then
      [[ -d "$parent" && ! -L "$parent" ]] || fail 'A user-service folder is redirected. It was left unchanged.'
      read -r owner mode < <(stat -c '%u %a' -- "$parent")
      [[ "$owner" == "$(id -u)" && "$mode" =~ ^[0-7]{3,4}$ ]] && (( (8#$mode & 0022) == 0 )) \
        || fail 'A user-service folder has unsafe permissions. It was left unchanged.'
    fi
  done
  shopt -s nullglob
  for path in "$HOME"/.config/systemd/user/*.wants/omavless-runtime.service \
              "$HOME"/.config/systemd/user/*.requires/omavless-runtime.service; do
    parent=${path%/*}
    [[ -d "$parent" && ! -L "$parent" ]] || fail 'A user-service dependency folder is redirected; it was left unchanged.'
    read -r owner mode < <(stat -c '%u %a' -- "$parent")
    [[ "$owner" == "$(id -u)" && "$mode" =~ ^[0-7]{3,4}$ ]] && (( (8#$mode & 0022) == 0 )) \
      || fail 'A user-service dependency folder has unsafe permissions; it was left unchanged.'
    [[ -L "$path" && $(stat -c '%u' -- "$path") == "$(id -u)" ]] \
      || fail 'A custom runtime dependency is not a normal enablement link; it was left unchanged.'
    target=$(readlink -- "$path")
    [[ "$target" == /usr/lib/systemd/user/omavless-runtime.service || "$target" == ../omavless-runtime.service ]] \
      || fail 'A runtime dependency points to another program. Its target and link were left unchanged.'
    unlink -- "$path"
  done
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
  # Omarchy queries Quickshell: Qt emits a non-UTF-8 warning on stdout under
  # plain C, corrupting the JSON even when the command succeeds. Keep the
  # parent/root byte-oriented checks in C, and the desktop children in UTF-8.
  registry=$(LC_ALL=C.UTF-8 timeout 8 omarchy plugin list --json) || return 1
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
  # Native Quit also calls Omarchy to disable/verify the widget; the existing
  # installed binary inherits this child-only locale for those subprocesses.
  if [[ ${guided:-false} != true || ! -t 2 ]]; then LC_ALL=C.UTF-8 /usr/bin/omavless "$@"; return; fi
  local output child result frame=0
  output=$(mktemp "$stage_directory/.shutdown.XXXXXXXX") || return 1
  LC_ALL=C.UTF-8 /usr/bin/omavless "$@" </dev/null > "$output" &
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
  local pid state fence instance revision outcome deadline
  pid=$(runtime_pid) || return 1
  if [[ "$pid" == 0 ]]; then
    say 'OmaVLESS is already stopped.'
    return
  fi
  state=$(plugin_state) || return 1
  case "$state" in
    enabled)
      fence=$(hello_fence) || return 1
      IFS=$'\t' read -r instance revision <<< "$fence"
      outcome=$(native_shutdown plugin quit "$instance" "$revision" "clean-reinstall-$$") \
        || return 1
      printf '%s' "$outcome" | jq -e '.schemaVersion == 1 and .shutdown == true and .pluginDisabled == true' >/dev/null \
        || return 1
      ;;
    disabled|absent)
      # Existing fixed consumer captures/authenticates its own original fence.
      # Never use this as a fallback after an uncertain Quit result.
      outcome=$(native_shutdown plugin watch-removal) \
        || return 1
      printf '%s' "$outcome" | jq -e '.schemaVersion == 1 and .runtimeStopped == true' >/dev/null \
        || return 1
      ;;
    *) return 1 ;;
  esac
  # A watcher may coalesce with the original QML unload consumer. Its successful
  # no-op is not proof of shutdown: wait only for independent fixed unit facts.
  deadline=$((SECONDS + 65))
  while :; do
    pid=$(runtime_pid) || return 1
    if [[ "$pid" == 0 && $(systemctl --user show omavless-runtime.service --property=ActiveState --value) == inactive ]]; then
      printf '%s\n' 'OmaVLESS closed. Continuing removal preparation.'
      return
    fi
    (( SECONDS < deadline )) || return 1
    sleep 0.2
  done
}
check_supported_installation() {
  local app dns digest
  app=$(pacman -Q omavless 2>/dev/null) || app=absent
  dns=$(pacman -Q omavless-dns 2>/dev/null) || dns=absent
  case "$app:$dns" in
    'omavless 0.8.2-1:absent'|'omavless 0.9.8-1:omavless-dns 0.9.8-1'|'omavless 0.9.8-1:absent'|'absent:omavless-dns 0.9.8-1'|'absent:absent') ;;
    *) fail 'The installed package version is not supported by this remover. Restore an official 0.8.2 or 0.9.8 package before continuing.' ;;
  esac
  installed_public_app=$app
  installed_public_dns=$dns
  if [[ "$app" != absent ]]; then
    [[ -f /usr/bin/omavless && ! -L /usr/bin/omavless && $(stat -c '%u:%a' /usr/bin/omavless) == 0:755 ]] \
      || fail 'The registered application file is missing or changed. Restore its official package before continuing.'
    digest=$(sha256sum /usr/bin/omavless) || fail 'Cannot identify the application.'
    # Public ELF identities, not just a development build's package label.
    # 0.8.2, original 0.9.8 and corrected public deliveries through fix.7.
    case "${digest%% *}" in
      3ff795316aa331463aff4e26571546433bdfd9317f348712d135be2f0a8d28a1|62874def77df9e4f099baf74c57420a7ab1dab7b83935676828f6d445a5f357f|219479595f4b59e07a9639d703e939125d464165f7a5f426ec523ce5d952600b|42a0f2550567c21cf8201a13d8797124237edfb71f1ee67a93d33fb6852753ee|3e5e8a5aca3d48d1bada328471b5078eb96a5796a1c44a6ba03a289def050141|5930244698e7fb1c6fe7518af2b093e7c2f5dcb5c4400dd845588553e0c7e2fd|118a12989f737e7c5f7ee167d10f6487ab6ca16ba4ed34f8dae71e813a384bd4|bf7787ee90bdbb0e2de03ecdd8bebb4b01810ad9607eaaecbfbf5ff7eeeaa2b4|b8cc106248953abcc3b3475efa7cec362778b5ae918b6866c1e8febdd3c44ff9|897710505a8fae9783bb8130c0e0d98133144e2b8e9ae68d169ad7a413999990|e1d320dc3347e2bdb4f45a576c34c42098a73e54e00f698010e5424cf97808b6|07295f1694bffb0ce234a8ce4164e03b3ddad2e88ef25a98f36a9e3fd6be6b73|7c7b0f0c350b8903353d2fed3c8effb693aaaca0c271279b85efa51dfebb8881|6949d9473a714e36824e93437c61aff1064c619b9960e8a9b31bb49834176004) ;;
      *) fail 'The application is a development or modified build. Restore its official package before continuing; no development checkout was deleted.' ;;
    esac
  else
    [[ ! -e /usr/bin/omavless && ! -L /usr/bin/omavless ]] \
      || fail 'An unregistered application file remains. Have its installer remove that file before continuing; it was left unchanged.'
  fi
  printf '%s\n' "Application: $app" "DNS helper (website address lookup) and bundled core: $dns"
}

[[ $# == 0 ]] || fail 'Run this removal helper without arguments.'
[[ $EUID != 0 ]] || fail 'Run as your desktop user, not with sudo. Password prompts appear only when needed.'
check_account
[[ ${XDG_CONFIG_HOME:-$HOME/.config} == "$HOME/.config" &&
   ${XDG_STATE_HOME:-$HOME/.local/state} == "$HOME/.local/state" &&
   ${XDG_CACHE_HOME:-$HOME/.cache} == "$HOME/.cache" ]] \
  || fail 'Custom XDG settings paths are configured. This helper removes only the normal OmaVLESS folders; custom folders were left unchanged.'
for directory in "$HOME" "$HOME/.config" "$HOME/.local" "$HOME/.local/state" "$HOME/.cache"; do
  [[ -d "$directory" && ! -L "$directory" ]] || fail 'A settings parent is missing or redirected. Restore the normal desktop folders first.'
  read -r owner mode < <(stat -c '%u %a' -- "$directory")
  [[ "$owner" == "$(id -u)" && "$mode" =~ ^[0-7]{3,4}$ ]] && (( (8#$mode & 0022) == 0 )) \
    || fail 'A settings parent has unsafe ownership or permissions. No settings were deleted.'
done
prepare_maintenance_folders
stage_resume_helper "$(realpath -e -- "${BASH_SOURCE[0]}")"
boot_id=$(</proc/sys/kernel/random/boot_id)
attempt=$(tr -d '-' </proc/sys/kernel/random/uuid)
[[ "$attempt" =~ ^[a-f0-9]{32}$ && "$boot_id" =~ ^[a-f0-9-]{36}$ ]] || fail 'Cannot identify this boot/removal attempt.'
operator="$$:$(process_birth "$$")"
record="$folder/progress.json"
lease="$folder/operation.lock"
if [[ ! -e "$lease" && ! -L "$lease" ]]; then (set -o noclobber; : > "$lease") || true; fi
safe_file "$lease" || fail 'The removal lock is not a private regular file.'
exec {lease_fd}<> "$lease"
[[ $(stat -c '%d:%i' -- "$lease") == $(stat -Lc '%d:%i' -- "/proc/$$/fd/$lease_fd") ]] || fail 'The removal lock changed.'
flock -n "$lease_fd" || fail 'Another removal is still running. Return to its terminal.'
runtime="/run/user/$(id -u)"
[[ -d "$runtime" && ! -L "$runtime" && $(stat -c '%u:%a' -- "$runtime") == "$(id -u):700" ]] \
  || fail 'Run this command inside your normal desktop session.'
setup_lock="$runtime/omavless-first-run.lock"
guided=true
spinner='|/-\'
stage_directory=$folder
read_progress
entry_phase=$phase
prior_operator=$previous_operator
if [[ "$phase" != none && "$prepared_boot" == "$boot_id" ]]; then
  operator_gone || fail 'The earlier removal is still open. Finish or cancel it in its original terminal.'
fi
if [[ "$phase" == prepared && "$prepared_boot" == "$boot_id" ]]; then restart_offer; exit 0; fi
check_authorizations
check_private_targets
check_supported_installation
check_user_units
if [[ "$installed_public_app" == 'omavless 0.8.2-1' ]]; then check_legacy_selection; fi
confirm_removal || { say 'Cancelled.'; exit 0; }
if [[ "$phase" == none ]]; then prepared_boot=$boot_id; phase=none; save_progress preparing; else save_progress "$phase"; fi

read -r -d '' root_checks <<'ROOT_CHECKS' || true
set -euo pipefail
export PATH=/usr/bin:/bin LC_ALL=C
refuse() { printf 'Cannot continue: %s\n' "${blocker:-a removal check failed. No forced cleanup was attempted}" >&2; exit 1; }
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
empty_runtime_journal() {
  local path owner mode
  for path in /run /run/omavless-dns /run/omavless-dns/private; do
    if [[ -e "$path" || -L "$path" ]]; then
      [[ -d "$path" && ! -L "$path" ]] || refuse
      read -r owner mode < <(stat -c '%u %a' "$path")
      [[ "$owner" == 0 && "$mode" =~ ^[0-7]{3,4}$ ]] && (( (8#$mode & 0022) == 0 )) || refuse
      [[ "$path" != /run/omavless-dns/private || "$mode" == 700 ]] || refuse
    fi
  done
  shopt -s nullglob dotglob
  entries=(/run/omavless-dns/private/*)
  [[ ${#entries[@]} == 0 ]] || refuse
}
check_packaged_file() {
  local package=$1 file=$2 version directory expected hash
  [[ -f "$file" && ! -L "$file" && $(stat -c '%u:%a:%h' "$file") == 0:755:1 ]] || refuse
  [[ $(pacman -Qqo "$file") == "$package" ]] || refuse
  version=$(pacman -Q "$package") || refuse
  [[ "$version" == "$package "* ]] || refuse
  version=${version#* }
  [[ "$version" =~ ^[a-zA-Z0-9.+_:-]+$ ]] || refuse
  directory="/var/lib/pacman/local/$package-$version"
  for path in /var /var/lib /var/lib/pacman /var/lib/pacman/local "$directory"; do
    [[ -d "$path" && ! -L "$path" ]] || refuse
    read -r owner mode < <(stat -c '%u %a' "$path")
    [[ "$owner" == 0 && "$mode" =~ ^[0-7]{3,4}$ ]] && (( (8#$mode & 0022) == 0 )) || refuse
  done
  [[ -f "$directory/mtree" && ! -L "$directory/mtree" && $(stat -c '%u:%h' "$directory/mtree") == 0:1 ]] || refuse
  read -r owner mode < <(stat -c '%u %a' "$directory/mtree")
  [[ "$mode" =~ ^[0-7]{3,4}$ ]] && (( (8#$mode & 0022) == 0 )) || refuse
  expected=$(gzip -cd -- "$directory/mtree" | awk -v wanted=".$file" '
    $1 == wanted { count++; for (i=2;i<=NF;i++) if ($i ~ /^sha256digest=/) {sub(/^sha256digest=/,"",$i); hash=$i} }
    END { if (count != 1 || length(hash) != 64) exit 1; print hash }') || refuse
  [[ "$expected" =~ ^[a-f0-9]{64}$ ]] || refuse
  hash=$(sha256sum "$file") || refuse
  [[ ${hash%% *} == "$expected" ]] || refuse
}
network_empty() {
  local inventory family
  blocker='the VPN interface is still present. Finish the requested restart before continuing'
  inventory=$(timeout 8 ip -json link show | head -c 1048577) || refuse
  [[ ${#inventory} -le 1048576 ]] || refuse
  printf '%s' "$inventory" | jq -e 'type == "array" and all(.[]; (.ifname | type) == "string" and .ifname != "Meta" and .ifname != "omavless0")' >/dev/null || refuse
  blocker='the system resolver is unavailable or still exposes the VPN link. Let networking finish starting, then run removal again'
  inventory=$(timeout 8 resolvectl --json=short status | head -c 1048577) || refuse
  [[ ${#inventory} -le 1048576 ]] || refuse
  printf '%s' "$inventory" | jq -e 'type == "array" and length > 0 and all(.[]; type == "object" and
    (if has("ifname") then ((.ifname | type) == "string" and (.ifindex | type) == "number" and .ifname != "Meta" and .ifname != "omavless0")
     else ((.resolvConfMode | type) == "string" and (.fallbackServers | type) == "array" and (.scopes | type) == "array") end))' >/dev/null || refuse
  for family in -4 -6; do
    blocker='VPN routing rules remain (table 2022). Another network service may be using them; stop that service normally before continuing. No rules were deleted'
    inventory=$(timeout 8 ip -json "$family" rule show | head -c 1048577) || refuse
    [[ ${#inventory} -le 1048576 ]] || refuse
    printf '%s' "$inventory" | jq -e 'type == "array" and all(.[]; type == "object" and .table != 2022 and .table != "2022" and .iif != "Meta" and .oif != "Meta" and .iif != "omavless0" and .oif != "omavless0")' >/dev/null || refuse
    blocker='a route still refers to the VPN interface/table. Finish its normal network cleanup before continuing; no routes were deleted'
    inventory=$(timeout 8 ip -json "$family" route show table all | head -c 1048577) || refuse
    [[ ${#inventory} -le 1048576 ]] || refuse
    printf '%s' "$inventory" | jq -e 'type == "array" and all(.[]; type == "object" and .dev != "Meta" and .dev != "omavless0" and .table != 2022 and .table != "2022")' >/dev/null || refuse
  done
}
ROOT_CHECKS

read -r -d '' root_custody <<'ROOT_CUSTODY' || true
# Root takes its own lifetime custody before interpreting user navigation data.
umask 077
blocker='the temporary removal lock has unexpected ownership or permissions. If no installation or removal is running, save your work and restart the computer, then run the saved command again'
lease=/run/omavless-uninstall.lock
if [[ ! -e "$lease" && ! -L "$lease" ]]; then (set -o noclobber; : > "$lease") || true; fi
[[ -f "$lease" && ! -L "$lease" && $(stat -c '%u:%a:%h' "$lease") == 0:600:1 ]] || refuse
exec {root_lease}<> "$lease"
[[ $(stat -c '%d:%i' "$lease") == $(stat -Lc '%d:%i' "/proc/$$/fd/$root_lease") ]] || refuse
flock -n "$root_lease" || { printf '%s\n' 'An earlier removal step is still running. Wait for it to finish, then run the same command.' >&2; exit 1; }
blocker='the removal record changed or is no longer present. Close this old prompt and rerun the saved removal command from your desktop account'
[[ ${SUDO_UID:-} =~ ^[1-9][0-9]*$ ]] || refuse
account=$(getent passwd "$SUDO_UID") || refuse
IFS=: read -r name password uid gid description desktop_home shell <<< "$account"
[[ "$uid" == "$SUDO_UID" && "$desktop_home" == /* && "$desktop_home" != / ]] || refuse
record="$desktop_home/.cache/omavless-maintenance/uninstall/progress.json"
for path in "$desktop_home" "$desktop_home/.cache" "$desktop_home/.cache/omavless-maintenance" "$desktop_home/.cache/omavless-maintenance/uninstall"; do
  [[ -d "$path" && ! -L "$path" ]] || refuse
  read -r owner mode < <(stat -c '%u %a' "$path")
  [[ "$owner" == "$SUDO_UID" && "$mode" =~ ^[0-7]{3,4}$ ]] && (( (8#$mode & 0022) == 0 )) || refuse
done
[[ -f "$record" && ! -L "$record" && $(stat -c '%u:%a:%h' "$record") == "$SUDO_UID:600:1" && $(stat -c '%s' "$record") -le 2048 ]] || refuse
current=$(<"$record")
[[ $(printf '%s' "$current" | jq -c .) == "$current" ]] || refuse
printf '%s' "$current" | jq -e --argjson uid "$SUDO_UID" --arg attempt "$removal_attempt" --arg phase "$removal_phase" --arg boot "$removal_boot" \
  '.schema == 2 and .uid == $uid and .attempt == $attempt and .phase == $phase and .boot == $boot' >/dev/null || refuse
[[ $(</proc/sys/kernel/random/boot_id) == "$invocation_boot" ]] || refuse
# A pending old sudo has not taken root's lock yet. Observe it explicitly, not
# merely a dead frontend; the token fence also denies delayed old execution.
blocker='the process list could not be checked completely. Let other system commands finish, then run the saved removal command again'
for file in /proc/[0-9]*/cmdline; do
  pid=${file#/proc/}; pid=${pid%/cmdline}
  if [[ ! -r "$file" ]]; then
    [[ ! -d "${file%/cmdline}" ]] && continue
    refuse
  fi
  # Keep the reader's status separate from the NUL-to-newline transformer.
  # A process may exit between the readable check and read. That is ordinary
  # churn, but a live unreadable process or failed transformation is not absence.
  # The final numeric line also preserves captured trailing argument newlines.
  args=$({ read_status=0; timeout 5 head -c 65537 -- "$file" 2>/dev/null || read_status=$?;
    printf '\n%d' "$read_status"; } | tr '\0' '\n') || refuse
  read_status=${args##*$'\n'}
  args=${args%$'\n'*}
  [[ "$read_status" =~ ^[0-9]+$ && ${#args} -le 65536 ]] || refuse
  if [[ "$args" == *'# OmaVLESS removal attempt '* && "$args" != *"# OmaVLESS removal attempt $removal_attempt"* ]]; then
    printf '%s\n' 'An earlier removal authorization is still open. Finish or cancel that prompt, then run the same command.' >&2
    exit 1
  fi
  if [[ "$read_status" != 0 ]]; then
    [[ ! -d "${file%/cmdline}" ]] && continue
    refuse
  fi
done
ROOT_CUSTODY

run_root() {
  local body=$1 source
  # Values are fixed-schema public fence fields, never shell/private input.
  [[ "$attempt" =~ ^[a-f0-9]{32}$ && "$phase" =~ ^(preparing|prepared|removing)$ && "$prepared_boot" =~ ^[a-f0-9-]{36}$ ]] || fail 'Invalid removal progress.'
  source="# OmaVLESS removal attempt $attempt"$'\n'"readonly removal_attempt=$attempt removal_phase=$phase removal_boot=$prepared_boot invocation_boot=$boot_id"$'\n'"$root_checks"$'\n'"$root_custody"$'\n'"$body"
  sudo /usr/bin/bash -c "$source"
}

# Cross the root barrier BEFORE reclaiming our old interoperability exclusion
# or any user-side deletion; an already-admitted root worker may have finished.
run_root ':' || fail 'The previous step or authorization has not settled. Follow the message above, then run this command again.'
previous_operator=$prior_operator
take_setup_lock

if [[ "$phase" == preparing && "$prepared_boot" != "$boot_id" ]]; then
  # A crash may have preceded suppression. A new boot alone cannot promote
  # PREPARING to completed preparation; reconcile and earn suppression now.
  prepared_boot=$boot_id
  save_progress preparing
fi

skip_restart=false
if [[ "$installed_public_app" == absent && "$installed_public_dns" == absent ]]; then
  # Widget-only/already-removed cases earn a fresh clean gate rather than an
  # unnecessary reboot. No receipt alone can select this mutation lane.
  run_root $'blocker="an old OmaVLESS process or network resource is still active; finish its normal shutdown first"\nempty_host\nnetwork_empty\nblocker="DNS helper files or its service remain without a registered package. Restore the official DNS package before continuing"\ndns_absent' \
    || fail 'Remaining network state was not clean. Follow the specific message above; no settings were deleted.'
  skip_restart=true
fi

if [[ "$prepared_boot" == "$boot_id" && "$skip_restart" != true ]]; then
  # A fresh invocation tries normal native cleanup once. An interrupted one
  # never resends an uncertain Quit: explicit restart recovery replaces it.
  if [[ "$entry_phase" == none ]]; then
    if [[ $(runtime_pid) != 0 ]]; then
      say '[1/3] Closing OmaVLESS...'
      if ! shutdown_running_app; then
        say 'Closing was not confirmed. No packages were removed.' \
          'Run this same command again to continue with restart recovery.'
        exit 1
      fi
    fi
  else say 'Continuing an interrupted removal. Checking the remaining steps...'; fi
  mask_future_start
  read -r -d '' root_prepare <<'ROOT_PREPARE' || true
if ! pacman -Q omavless-dns >/dev/null 2>&1; then
  # Reboot recovery retains the current legacy owner; do not pretend its
  # disappearance already happened or call empty_host before the reboot.
  blocker='DNS helper files remain without their registered package. Restore the official DNS package before continuing'
  for path in /usr/lib/omavless-dns /run/omavless-dns /usr/lib/systemd/system/omavless-dns-broker.service; do
    [[ ! -e "$path" && ! -L "$path" ]] || refuse
  done
  empty_enrollment_directory
  if pacman -Q omavless >/dev/null 2>&1; then
    blocker='the separate legacy Mihomo core is not the installed stock package. Restore its packaged binary before continuing'
    if pacman -Q mihomo-bin >/dev/null 2>&1; then check_packaged_file mihomo-bin /usr/bin/mihomo;
    elif pacman -Q mihomo >/dev/null 2>&1; then check_packaged_file mihomo /usr/bin/mihomo;
    elif [[ -e /usr/bin/mihomo || -L /usr/bin/mihomo ]]; then refuse; fi
  fi
  printf '%s\n' 'No DNS helper is installed.'
  exit 0
fi
blocker='the installed DNS helper files were changed. Restore the official DNS package before continuing'
check_dns_programs
blocker='the bundled core differs from its registered package. Restore the official DNS package before continuing'
check_packaged_file omavless-dns /usr/lib/omavless-dns/mihomo
blocker='DNS helper access belongs to another account or is incomplete. Use the account that configured OmaVLESS; no other account was changed'
check_enrollment
if [[ "$enrolled" != true ]]; then
  blocker='the unconfigured DNS helper still retains a session. Its ownership cannot be confirmed for this account; no network state was changed'
  [[ $(systemctl show omavless-dns-broker.service --property=MainPID --value) == 0 &&
     $(systemctl show omavless-dns-broker.service --property=NFileDescriptorStore --value) == 0 &&
     $(systemctl show omavless-dns-broker.service --property=ActiveState --value) == inactive &&
     ! -e /sys/class/net/Meta && ! -e /sys/class/net/omavless0 ]] || refuse
  empty_runtime_journal
fi
blocker='the DNS helper service has overrides or an unfinished reload. Restore its packaged service configuration and complete the reload before continuing'
[[ -z $(systemctl show omavless-dns-broker.service --property=DropInPaths --value) &&
   $(systemctl show omavless-dns-broker.service --property=FragmentPath --value) == /usr/lib/systemd/system/omavless-dns-broker.service &&
   $(systemctl show omavless-dns-broker.service --property=NeedDaemonReload --value) == no &&
   -z $(systemctl show omavless-dns-broker.service --property=Job --value) ]] || refuse
# Deliberate reboot recovery: retain current owner/journal/socket/FD, suppress
# only future boot activation. This is NOT a claim of original DNS rollback.
systemctl disable omavless-dns-broker.service
[[ $(systemctl show omavless-dns-broker.service --property=UnitFileState --value) == disabled ]] || refuse
printf '%s\n' 'OmaVLESS will not start again at the next login.'
ROOT_PREPARE
  say '[2/3] Preparing restart...'
  run_root "$root_prepare" || fail 'Restart preparation did not finish. Run this same command to inspect and continue; do not delete network files.'
  save_progress prepared
  release_setup_lock
  restart_offer
  exit 0
fi

# A different kernel epoch is a declared recovery boundary, not evidence that
# a previously interrupted Disconnect or DNS rollback succeeded.
[[ $(runtime_pid) == 0 ]] || fail 'OmaVLESS started again after reboot. Close it with Shut down OmaVLESS before continuing; no packages were removed.'
[[ ! -e "$runtime/omavless" && ! -L "$runtime/omavless" &&
   ! -e "$runtime/omavless-login.receipt" && ! -L "$runtime/omavless-login.receipt" ]] \
  || fail 'OmaVLESS created new session state after reboot. Another start is active; finish it before continuing.'
save_progress removing
# Reject redirected/development widgets before package effects, but leave QML
# loaded until the installed native binary has actually been removed. Otherwise
# its unload observer treats this intentionally stopped/masked reboot state as
# an unverified Quit and emits a false critical warning. All root/network and
# package guards still precede widget removal; masks/exclusion remain held.
check_widget
read -r -d '' root_remove <<'ROOT_REMOVE' || true
blocker='an OmaVLESS process or tunnel is still active. Finish its normal shutdown before continuing'
empty_host
network_empty
blocker='DNS helper files or its service remain without a registered package. Restore the official DNS package before continuing'
if ! dns_absent; then
  blocker='the installed DNS helper files were changed. Restore its official package before continuing'
  check_dns_programs
  blocker='the DNS helper restarted or retained session resources after reboot. Finish its normal shutdown before continuing; no session files were deleted'
  /usr/lib/omavless-dns/package-guard
  blocker='DNS helper access belongs to another account. Use the enrolled desktop account; no enrollment was changed'
  check_enrollment
  if [[ "$enrolled" == true ]]; then /usr/lib/omavless-dns/omavless-dns-broker --revoke; fi
fi
remove_empty_enrollment_directory
packages=()
if pacman -Q omavless >/dev/null 2>&1; then packages+=(omavless); fi
if pacman -Q omavless-dns >/dev/null 2>&1; then packages+=(omavless-dns); fi
if [[ ${#packages[@]} -gt 0 ]]; then
  # Root retains its original flock across the entire ordinary transaction,
  # even if the unprivileged frontend/terminal disappears.
  pacman -R -- "${packages[@]}"
fi
! pacman -Q omavless >/dev/null 2>&1
! pacman -Q omavless-dns >/dev/null 2>&1
empty_host
network_empty
ROOT_REMOVE
say '[3/3] Removing OmaVLESS...'
run_root "$root_remove" || fail 'Removal is not complete. Progress is saved; follow the specific message above, then run this command again.'
[[ ! -e /usr/bin/omavless && ! -L /usr/bin/omavless ]] || fail 'The application file remains. The installed package must be removed before deleting settings.'
remove_widget
check_private_targets
[[ -d "$setup_lock" && ! -L "$setup_lock" && $(stat -c '%d:%i:%u:%a' -- "$setup_lock") == "$recorded_lock" ]] \
  || fail 'The setup exclusion changed. Settings were left unchanged; run the same command to inspect the remaining state.'
for directory in "$HOME/.config/omavless" "$HOME/.local/state/omavless" "$HOME/.cache/omavless"; do
  [[ ! -e "$directory" ]] || rm -r --one-file-system -- "$directory" \
    || fail 'Some OmaVLESS settings could not be deleted. Fix their file permissions, then run this same command to finish.'
done
clear_runtime_enablement
mask_cleanup_admitted=true
clear_created_masks
safe_file "$record" && [[ $(<"$record") == "$record_bytes" ]] || fail 'Components were removed but the progress record changed. No unrelated file was deleted.'
# Release the owned shared exclusion before removing its last identity receipt.
# A crash here can leave a receipt with an absent directory, never an orphaned
# shared lock that permanently blocks setup and cannot be reconciled.
release_setup_lock
unlink -- "$record"
sync -f -- "$folder" || fail 'Removal finished but progress cleanup could not be saved. Free space before installing again.'
say '' 'OmaVLESS and its saved profiles and settings have been removed.' \
  'Other VPN clients and a separately installed Mihomo were kept.' '' \
  'To install OmaVLESS again:' 'omarchy plugin add https://github.com/k-kostin/omavless.git --enable'
