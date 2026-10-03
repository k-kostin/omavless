#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Dedicated KVM only. No installed service or network/namespace transition.
set -euo pipefail
export LC_ALL=C
umask 077
[[ ${OMAVLESS_K1_NAMESPACE_FILTER_VM:-} == 1 && $EUID == 0 && $# == 0 ]] || exit 2
[[ $(systemd-detect-virt --vm) == kvm ]] || exit 2
stage=/run/omavless-k1-namespace-filter-fixture
probe=$stage/probe
unit=omavless-k1-namespace-filter-fixture.service
link=/run/systemd/system/$unit
[[ -d $stage && ! -L $stage && $(stat -c '%u:%a' "$stage") == 0:700 ]] || exit 2
[[ -f $probe && ! -L $probe && $(stat -c '%u:%a' "$probe") == 0:700 ]] || exit 2
[[ ! -e $link && ! -L $link ]] || exit 2
[[ $(systemctl show "$unit" -p LoadState --value) == not-found ]] || exit 2
owned=0
started=0
source_unit=
cleanup() {
    if [[ $owned == 1 ]]; then
        if [[ $started == 1 ]]; then
            systemctl stop "$unit" >/dev/null 2>&1 || true
            systemctl reset-failed "$unit" >/dev/null 2>&1 || true
        fi
        if [[ -L $link && $(readlink "$link") == "$source_unit" ]]; then
            unlink "$link"
        fi
        systemctl daemon-reload
    fi
}
trap cleanup EXIT
for mode in control filtered; do
    case $mode in
        control) source_unit=$stage/control.service
            digest=1f24f558019a60923531f4ad673642e2878f2b77391e60bc03ac5d0cea48e7b8
            restriction=no ;;
        filtered) source_unit=$stage/filtered.service
            digest=8648d11b754e287fb8e6d64e5ba0b9e2bf2f4adb6b968c116d011ec57aca5f0c
            restriction=yes ;;
    esac
    [[ -f $source_unit && ! -L $source_unit && $(stat -c '%u:%a' "$source_unit") == 0:600 ]] || exit 2
    [[ $(sha256sum "$source_unit" | cut -d ' ' -f 1) == "$digest" ]] || exit 2
    ln -s "$source_unit" "$link"
    owned=1
    systemctl daemon-reload
    [[ $(systemctl show "$unit" -p LoadState --value) == loaded ]] || exit 2
    [[ $(systemctl show "$unit" -p FragmentPath --value) == "$link" ]] || exit 2
    [[ $(systemctl show "$unit" -p Requires --value) == 'sysinit.target system.slice' ]] || exit 2
    [[ $(systemctl show "$unit" -p Conflicts --value) == shutdown.target ]] || exit 2
    expected="{ path=$probe ; argv[]=$probe ; ignore_errors=no ; start_time=[n/a] ; stop_time=[n/a] ; pid=0 ; code=(null) ; status=0/0 }"
    [[ $(systemctl show "$unit" -p ExecStart --value) == "$expected" ]] || exit 2
    for property in DropInPaths Wants BindsTo PartOf Upholds OnFailure OnSuccess \
        TriggeredBy Requisite PropagatesStopTo StopPropagatedFrom \
        ExecCondition ExecStartPre ExecStartPost ExecReload ExecStop ExecStopPost \
        CapabilityBoundingSet AmbientCapabilities SystemCallFilter; do
        [[ -z $(systemctl show "$unit" -p "$property" --value) ]] || exit 2
    done
    [[ $(systemctl show "$unit" -p User --value) == root ]] || exit 2
    [[ $(systemctl show "$unit" -p NoNewPrivileges --value) == yes ]] || exit 2
    [[ $(systemctl show "$unit" -p RestrictNamespaces --value) == "$restriction" ]] || exit 2
    [[ $(systemctl show "$unit" -p Environment --value) == "OMAVLESS_K1_NAMESPACE_FILTER_FIXTURE=$mode" ]] || exit 2
    for property in PrivateUsers PrivatePIDs PrivateNetwork; do
        [[ $(systemctl show "$unit" -p "$property" --value) == no ]] || exit 2
    done
    started=1
    systemctl start "$unit"
    for attempt in {1..100}; do
        [[ $(systemctl show "$unit" -p ActiveState --value) == inactive ]] && break
        sleep 0.1
    done
    [[ $(systemctl show "$unit" -p ActiveState --value) == inactive ]] || exit 2
    [[ $(systemctl show "$unit" -p Result --value) == success ]] || exit 2
    [[ $(systemctl show "$unit" -p ExecMainStatus --value) == 0 ]] || exit 2
    [[ -L $link && $(readlink "$link") == "$source_unit" ]] || exit 2
    unlink "$link"
    systemctl daemon-reload
    [[ $(systemctl show "$unit" -p LoadState --value) == not-found ]] || exit 2
    owned=0
    started=0
    printf 'K1_NAMESPACE_FILTER_%s_PASS\n' "$mode"
done
printf 'K1_NAMESPACE_FILTER_VM_PASS\n'
