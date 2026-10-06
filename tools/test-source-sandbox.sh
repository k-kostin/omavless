#!/usr/bin/env bash
# Developer source tests only; no installation or live acceptance.
set -euo pipefail

if (( $# != 2 )); then
  echo 'Usage: test-source-sandbox.sh REPOSITORY PRIVATE_HOME_SCRATCH' >&2
  exit 2
fi
task_repository=$1
task_scratch=$2
task_uid=$(id -u)
(( task_uid > 0 )) || exit 2

for task_path in "$task_repository" "$task_scratch"; do
  [[ "$task_path" == /* && "$task_path" != *'/../'* && -d "$task_path" && ! -L "$task_path" ]] || exit 2
  [[ "$(realpath -- "$task_path")" == "$task_path" ]] || exit 2
done
[[ "$task_scratch" == "$HOME/.cache/"* && "$(stat -c %u -- "$task_scratch")" == "$task_uid" ]] || exit 2
[[ "$(stat -c %a -- "$task_scratch")" == 700 ]] || exit 2
[[ "$task_scratch" != "$task_repository" && "$task_scratch" != "$task_repository/"* && "$task_repository" != "$task_scratch/"* ]] || exit 2
task_contents=$(find "$task_scratch" -mindepth 1 -maxdepth 1 -printf x -quit) || exit 2
[[ -z "$task_contents" ]] || exit 2
[[ -f "$task_repository/tests/run.sh" && ! -L "$task_repository/tests/run.sh" ]] || exit 2
[[ -x /usr/bin/bwrap ]] || { echo 'bubblewrap unavailable: source sandbox not run' >&2; exit 2; }
[[ ! -L /usr/bin/bwrap && "$(stat -c %u -- /usr/bin/bwrap)" == 0 ]] || exit 2
case "$(stat -c %a -- /usr/bin/bwrap)" in 755|555) ;; *) exit 2 ;; esac
[[ -x /usr/bin/getcap ]] || exit 2
task_capabilities=$(/usr/bin/getcap /usr/bin/bwrap) || exit 2
[[ -z "$task_capabilities" ]] || exit 2

task_unset=()
# Enumerate exported variable NAMES only; never inspect or print their values.
# This blocks inherited installed/live opt-ins and reference override selection.
while IFS= read -r task_name; do
  if [[ "$task_name" == OMAVLESS_* ]]; then
    [[ "$task_name" =~ ^OMAVLESS_[A-Za-z0-9_]+$ && ${#task_name} -le 256 ]] || exit 2
    task_unset+=(--unsetenv "$task_name")
  fi
done < <(compgen -e || true)

# Ordinary trusted test code, not a sandbox for malicious code: host files
# remain read-only visible. Host network and /run sockets are isolated;
# pathname Unix sockets outside /run are not a claimed isolation boundary.
# The logical short /tmp is backed by an existing owned HOME directory;
# the primary /tmp mount, HOME, services and network are never changed.
cd -- "$task_repository"
exec /usr/bin/bwrap \
  --ro-bind / / --dev /dev --proc /proc --tmpfs /run \
  --unshare-user --unshare-pid --unshare-net --unshare-ipc --unshare-uts \
  --new-session --die-with-parent \
  --bind "$task_scratch" /tmp \
  --setenv TMPDIR /tmp --setenv PYTHONDONTWRITEBYTECODE 1 \
  --unsetenv DBUS_SESSION_BUS_ADDRESS --unsetenv SSH_AUTH_SOCK \
  --unsetenv DISPLAY --unsetenv WAYLAND_DISPLAY \
  --unsetenv LD_PRELOAD --unsetenv LD_LIBRARY_PATH \
  --unsetenv BASH_ENV --unsetenv ENV \
  --unsetenv PYTHONHOME --unsetenv PYTHONPATH --unsetenv PYTHONSTARTUP \
  --unsetenv NODE_OPTIONS \
  --unsetenv http_proxy --unsetenv HTTP_PROXY \
  --unsetenv https_proxy --unsetenv HTTPS_PROXY \
  --unsetenv ftp_proxy --unsetenv FTP_PROXY \
  --unsetenv all_proxy --unsetenv ALL_PROXY \
  --unsetenv no_proxy --unsetenv NO_PROXY \
  "${task_unset[@]}" \
  /bin/bash ./tests/run.sh < /dev/null
