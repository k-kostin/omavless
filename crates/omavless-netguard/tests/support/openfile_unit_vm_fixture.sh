#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Developer-only, bounded systemd unit launch in the dedicated KVM VM.
# This never installs/enables NetGuard or changes firewall/network state.
set -euo pipefail
export LC_ALL=C
umask 077

[[ ${OMAVLESS_K1_OPENFILE_UNIT_VM:-} == 1 && $EUID == 0 && $# == 0 ]] || exit 2
[[ $(systemd-detect-virt --vm) == kvm ]] || exit 2

fixture_dir=/run/omavless-k1-openfile-fixture
probe=$fixture_dir/probe
source_unit=$fixture_dir/unit.service
unit=omavless-k1-openfile-fixture.service
linked_unit=/run/systemd/system/$unit
expected_unit_sha=753c11ee3f6bbfb9f259097f6455ba9fb5d185045d94c6f6f2c937b496bca6ab

[[ -f $probe && ! -L $probe && -x $probe ]] || exit 2
[[ -f $source_unit && ! -L $source_unit ]] || exit 2
[[ $(stat -c '%u:%a' "$fixture_dir") == 0:700 ]] || exit 2
[[ $(stat -c '%u:%a' "$probe") == 0:700 ]] || exit 2
[[ $(stat -c '%u:%a' "$source_unit") == 0:600 ]] || exit 2
[[ $(sha256sum "$source_unit" | cut -d ' ' -f 1) == "$expected_unit_sha" ]] || exit 2
[[ ! -e $linked_unit && ! -L $linked_unit ]] || exit 2
[[ $(systemctl show "$unit" -p LoadState --value 2>/dev/null || true) == not-found ]] || exit 2

owned=0
start_attempted=0
cleanup() {
    if [[ $owned == 1 ]]; then
        # Only the exact symlink exclusively created by this runner is removed.
        # Do not send even a stop request before effective-unit validation:
        # a rejected drop-in could propagate it to an unrelated unit.
        if [[ $start_attempted == 1 ]]; then
            systemctl stop "$unit" >/dev/null 2>&1 || true
            systemctl reset-failed "$unit" >/dev/null 2>&1 || true
        fi
        if [[ -L $linked_unit && $(readlink "$linked_unit") == "$source_unit" ]]; then
            unlink "$linked_unit" || true
        fi
        systemctl daemon-reload >/dev/null 2>&1 || true
    fi
}
trap cleanup EXIT

ln -s "$source_unit" "$linked_unit"
owned=1
systemctl daemon-reload
[[ $(systemctl show "$unit" -p LoadState --value) == loaded ]] || exit 2
[[ $(systemctl show "$unit" -p FragmentPath --value) == "$linked_unit" ]] || exit 2
# A verified fragment alone is insufficient: manager-wide/hierarchical drop-ins
# or .wants/.requires links can add privileged commands or other units.
[[ -z $(systemctl show "$unit" -p DropInPaths --value) ]] || exit 2
[[ $(systemctl show "$unit" -p Requires --value) == 'sysinit.target system.slice' ]] || exit 2
[[ $(systemctl show "$unit" -p Conflicts --value) == shutdown.target ]] || exit 2
expected_exec="{ path=$probe ; argv[]=$probe ; ignore_errors=no ; start_time=[n/a] ; stop_time=[n/a] ; pid=0 ; code=(null) ; status=0/0 }"
[[ $(systemctl show "$unit" -p ExecStart --value) == "$expected_exec" ]] || exit 2
for property in DropInPaths Wants BindsTo PartOf Upholds OnFailure OnSuccess \
    TriggeredBy Requisite PropagatesStopTo StopPropagatedFrom \
    ExecCondition ExecStartPre ExecStartPost ExecReload ExecStop ExecStopPost; do
    [[ -z $(systemctl show "$unit" -p "$property" --value) ]] || exit 2
done
start_attempted=1
systemctl start "$unit"

# A successful systemctl start may precede the short-lived probe's exit.
for attempt in {1..100}; do
    [[ $(systemctl show "$unit" -p ActiveState --value) == inactive ]] && break
    sleep 0.1
done
[[ $(systemctl show "$unit" -p ActiveState --value) == inactive ]] || exit 2
[[ $(systemctl show "$unit" -p Result --value) == success ]] || exit 2
[[ $(systemctl show "$unit" -p ExecMainStatus --value) == 0 ]] || exit 2
[[ -L $linked_unit && $(readlink "$linked_unit") == "$source_unit" ]] || exit 2

systemctl reset-failed "$unit" >/dev/null 2>&1 || true
unlink "$linked_unit"
systemctl daemon-reload
[[ $(systemctl show "$unit" -p LoadState --value 2>/dev/null || true) == not-found ]] || exit 2
owned=0
printf 'K1_OPENFILE_UNIT_VM_PASS\n'
