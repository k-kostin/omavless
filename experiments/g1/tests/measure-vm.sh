#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Synthetic G1 windows only. No daemon, profiles, service, network or sudo.
set -euo pipefail

if [[ ${OMAVLESS_G1_VM:-} != 1 ]] || ! systemd-detect-virt --vm --quiet; then
  printf '%s\n' 'Refusing outside an explicitly selected development VM' >&2
  exit 2
fi
if [[ $# -lt 2 || $# -gt 3 ]]; then
  printf '%s\n' 'Usage: measure-vm.sh direct|shell /absolute/binary [/absolute/shell-app]' >&2
  exit 2
fi
trial=$1
binary=$2
app=${3:-}
if [[ ! -x $binary || $binary != /* ]] ||
  { [[ $trial == shell ]] && [[ $app != /* || ! -d $app ]]; } ||
  { [[ $trial == direct ]] && [[ -n $app ]]; } ||
  [[ $trial != direct && $trial != shell ]]; then
  printf '%s\n' 'Invalid synthetic trial path or arguments' >&2
  exit 2
fi
for name in XDG_RUNTIME_DIR WAYLAND_DISPLAY HYPRLAND_INSTANCE_SIGNATURE; do
  if [[ -z ${!name:-} ]]; then
    printf 'Missing display environment: %s\n' "$name" >&2
    exit 2
  fi
done
for command in hyprctl jq awk getconf date; do
  command -v "$command" >/dev/null || exit 2
done

child=
cleanup() {
  if [[ -n $child ]]; then
    kill "$child" 2>/dev/null || true
    wait "$child" 2>/dev/null || true
  fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
ticks=$(getconf CLK_TCK)
printf 'trial\trun\twindow_ms\tidle_pss_kib\tidle_cpu_pct_one_core\n'
for run in 1 2 3 4 5; do
  start=$(date +%s%N)
  if [[ $trial == shell ]]; then
    "$binary" "$app" >/dev/null 2>&1 &
  else
    "$binary" >/dev/null 2>&1 &
  fi
  child=$!
  found=0
  for ((attempt=0; attempt<100; attempt++)); do
    if ! kill -0 "$child" 2>/dev/null; then
      printf 'Trial exited before a window appeared: %s run %s\n' "$trial" "$run" >&2
      exit 1
    fi
    if hyprctl clients -j | jq -e --argjson pid "$child" 'any(.[]; .pid == $pid)' >/dev/null; then
      found=1
      break
    fi
    sleep 0.05
  done
  if [[ $found != 1 ]]; then
    printf 'No trial window within the bound: %s run %s\n' "$trial" "$run" >&2
    exit 1
  fi
  appeared=$(date +%s%N)
  window_ms=$(((appeared-start)/1000000))
  sleep 20
  if [[ ! -r /proc/$child/smaps_rollup || ! -r /proc/$child/stat ]]; then
    printf 'Trial process disappeared: %s run %s\n' "$trial" "$run" >&2
    exit 1
  fi
  pss=$(awk '/^Pss:/{print $2}' "/proc/$child/smaps_rollup")
  before_ticks=$(awk '{print $14+$15}' "/proc/$child/stat")
  before_time=$(date +%s%N)
  sleep 10
  after_ticks=$(awk '{print $14+$15}' "/proc/$child/stat")
  after_time=$(date +%s%N)
  cpu=$(awk -v b="$before_ticks" -v a="$after_ticks" -v hz="$ticks" \
    -v ns="$((after_time-before_time))" 'BEGIN { printf "%.2f", 100*(a-b)*1000000000/(hz*ns) }')
  printf '%s\t%s\t%s\t%s\t%s\n' "$trial" "$run" "$window_ms" "$pss" "$cpu"
  cleanup
  child=
done
