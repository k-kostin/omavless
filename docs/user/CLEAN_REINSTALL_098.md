# Clean reinstall after an incomplete 0.9.8 setup

**Candidate procedure; not yet an instruction shipped by main.** This path was
checked on x86_64 Omarchy against the original public 0.9.8 partial installation
and the immutable `v0.9.8-fix.5` successor. Follow it only when asked by the maintainer.
It is not the planned all-stage 0.9.9 uninstaller.

Use this path if packages and DNS registration were installed, but you never
connected OmaVLESS and want to start setup again. Do not use it for a connected
VPN, Recovery required, an interrupted DNS change, an experimental package,
another account's installation or custom XDG directories. Ask for help instead.
Do not run setup or another OmaVLESS client concurrently with these steps.

This removes only our widget and `omavless`/`omavless-dns` packages. It does not
remove a separately installed Mihomo, another VPN, or firewall rules. Old private
data is moved into a private backup, not deleted. One deliberate computer reboot
is needed; save your other work before step 3.

Run each block separately in Bash. **If a command fails, stop there** and share
the step and error, not profiles, subscription links or private logs.

## 1. Remove the widget and disable application startup

If the widget is already removed, skip the first command. If the panel is usable,
use Settings → Shut down OmaVLESS first. The commands below are for the same
desktop account that ran setup, not a root login.

```bash
omarchy plugin remove kdk.omavless
```

```bash
systemctl --user disable omavless-runtime.service &&
test "$(systemctl --user show omavless-runtime.service --property=MainPID --value)" = 0
```

If the last check fails, the application has not shut down. Do not force-stop
it or proceed to reboot. Re-enable the widget and ask for help with shutdown.

## 2. Check clean DNS state and stop the idle broker

This block checks the actual journal, manager FD store, process and TUN state
**before** stopping the broker and again afterwards. It never deletes a socket,
journal, lock or enrollment. A refusal stops the block before its next action.
Enter your password only at sudo's prompt.

```bash
sudo bash <<'ROOT'
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
[[ $(sha256sum /usr/lib/systemd/system/omavless-dns-broker.service) == '126b208158e9288c475084c8bd74029e2808edf9846da514a369266b6306462d  /usr/lib/systemd/system/omavless-dns-broker.service' ]] || refuse
[[ "$(systemctl show omavless-dns-broker.service --property=ActiveState --value)" == active &&
   "$(systemctl show omavless-dns-broker.service --property=SubState --value)" == running ]] || refuse
original_pid=$(systemctl show omavless-dns-broker.service --property=MainPID --value)
[[ "$original_pid" =~ ^[1-9][0-9]*$ &&
   $(stat -c '%u' "/proc/$original_pid") == 0 &&
   $(readlink "/proc/$original_pid/exe") == /usr/lib/omavless-dns/omavless-dns-broker ]] || refuse
broker_hash=$(sha256sum "/proc/$original_pid/exe")
case "${broker_hash%% *}" in
  8caba1a57b44b2856b98b0d0213cc545905ba5a031aac144fcce2420957b0d2e|33121a6f4c4eb7b644da034fc521e221d7f292d61bdaffb837ed82c015dbe755|ba61e433d89baa3ae4fd1af5ed275f769654c19fbf6a7b22592869e617b277d5|8f250d4fce7e784477c91204579ed6f8792006713ccc895ac357e23faa03df26) ;;
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
ROOT
```

Only continue after `CLEAN_STOP_CONFIRMED`. A reboot is not a substitute for
these checks: the journal is volatile, so rebooting first would erase evidence
of an unfinished DNS operation.

## 3. Reboot once, then revoke registration and remove packages

```bash
systemctl reboot
```

After logging back in, run:

```bash
sudo /usr/lib/omavless-dns/package-guard &&
sudo /usr/lib/omavless-dns/omavless-dns-broker --revoke &&
sudo pacman -R -- omavless omavless-dns
```

Confirm removal of those two packages in pacman. The guard must pass before
registration is revoked, and revocation must happen while the broker is still
installed. Do not add force, dependency-bypass or recursive-removal flags. If
the guard or revocation refuses, stop and report it; do not delete files to make
the check pass. A missing registration needs a different continuation.

The reboot naturally clears the old stopped socket and per-login runtime
files. No manual deletion of `/run` data is part of this procedure.

## 4. Preserve old private settings separately

This is the deliberately selected **fresh settings** path, not normal updating.
It retains a backup but the new installation will not import that backup by
itself. The backup may contain private profiles; never post or upload it.

```bash
bash <<'USER'
set -euo pipefail
[[ ${XDG_CONFIG_HOME:-$HOME/.config} == "$HOME/.config" &&
   ${XDG_STATE_HOME:-$HOME/.local/state} == "$HOME/.local/state" &&
   ${XDG_CACHE_HOME:-$HOME/.cache} == "$HOME/.cache" ]] || exit 1
[[ ! -e /usr/bin/omavless && ! -L /usr/bin/omavless ]] || exit 1
runtime="/run/user/$(id -u)"
[[ ! -e "$runtime/omavless" && ! -L "$runtime/omavless" &&
   ! -e "$runtime/omavless-login.receipt" && ! -L "$runtime/omavless-login.receipt" ]] || exit 1
umask 077
parent="$HOME/.local/state/omavless-reinstall-backups"
for directory in "$HOME" "$HOME/.config" "$HOME/.local" "$HOME/.local/state" "$HOME/.cache"; do
  [[ -d "$directory" && ! -L "$directory" ]] || exit 1
  read -r owner mode < <(stat -c '%u %a' -- "$directory")
  [[ "$owner" == "$(id -u)" && "$mode" =~ ^[0-7]{3,4}$ ]] || exit 1
  (( (8#$mode & 0022) == 0 )) || exit 1
done
[[ ! -L "$parent" ]] || exit 1
mkdir -p -- "$parent"
[[ -d "$parent" && $(stat -c '%u' "$parent") == "$(id -u)" &&
   $(stat -c '%a' "$parent") == 700 ]] || exit 1
for path in "$HOME/.config/omavless" "$HOME/.local/state/omavless" "$HOME/.cache/omavless"; do
  if [[ -e "$path" || -L "$path" ]]; then
    [[ -d "$path" && ! -L "$path" && $(stat -c '%u' "$path") == "$(id -u)" ]] || exit 1
  fi
done
backup=$(mktemp -d "$parent/partial-098.XXXXXX")
[[ ! -e "$HOME/.config/omavless" ]] || mv -- "$HOME/.config/omavless" "$backup/config"
[[ ! -e "$HOME/.local/state/omavless" ]] || mv -- "$HOME/.local/state/omavless" "$backup/state"
[[ ! -e "$HOME/.cache/omavless" ]] || mv -- "$HOME/.cache/omavless" "$backup/cache"
printf 'PRIVATE_BACKUP_SAVED: %s\n' "$backup"
USER
```

If this refuses on runtime files, do not delete them or repeatedly reboot. Their
presence means the stopped baseline needs inspection.

## 5. Install the fixed candidate and complete setup

For this not-yet-main validation, install the repository **without enabling**,
select the immutable candidate, then enable it. Do not enable an old default
main between these commands. Confirm **Clone and add this plugin?** with Yes,
then answer **No** to **Enable 'kdk.omavless' now?**. The remaining commands
select and verify fix.5 before explicitly enabling it.

```bash
omarchy plugin add https://github.com/k-kostin/omavless.git &&
git -C "$HOME/.config/omarchy/plugins/kdk.omavless" checkout --detach v0.9.8-fix.5 &&
test "$(git -C "$HOME/.config/omarchy/plugins/kdk.omavless" rev-parse HEAD)" = 1e25a79796e1b7a2d9ba341f26b093c74c3c3b84 &&
omarchy plugin enable kdk.omavless right
```

Open OmaVLESS from the bar, choose **Install required components**, enter
`INSTALL`, and approve the two matching packages in the normal sudo/pacman
prompts. Approve the separate DNS step with `DNS`. This does not connect a VPN.
When the terminal finishes, close it and use the panel's terminal-closed
acknowledgement and **Check again** if shown. If onboarding says **Finish the
setup terminal first**, choose **Review component setup**, confirm **Setup
terminal and prompts are closed**, then use **Back to panel** after **Core ready**.
You should reach the ordinary disconnected panel with profile-import controls.
Complete onboarding (including your country preset if you want Routing), then
import your own profile and connect separately.

Until the successor is integrated into main, the single default-branch
`omarchy plugin add ... --enable` command is **not** this tested candidate path.
After integration, the public guide must be reconciled with actual main; never
send candidate instructions while describing them as stable/main installation.
