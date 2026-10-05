#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Dedicated KVM only. No installed service or network/namespace transition.
set -euo pipefail
export LC_ALL=C
umask 077
[[ ${OMAVLESS_K1_NAMESPACE_FILTER_VM:-} == 1 && $EUID == 0 && $# == 0 ]] || exit 2
[[ $(systemd-detect-virt --vm) == kvm ]] || exit 2
stage=/run/omavless-k1-typed-order-filter-fixture
probe=$stage/probe
unit=omavless-k1-typed-order-filter-fixture.service
link=/run/systemd/system/$unit
cgroup=/sys/fs/cgroup/system.slice/$unit
probe_sha=b7dc81b89045c591efd375765ddf4fc4792afedf86cbc73c18fd94d1227d7332
query_sha=de1ee66fa76d6faa53d4606d1486653ecf19a9746200fc2d997c0101a68f9eda
query_guard_sha=c4a688875037f1d990ee93f0018108ab82bceb211090f61f77c64b83a4c7cc40
# A property missing from this systemd must not look like an expected empty one.
property() {
    local reply
    reply=$(systemctl show "$unit" -p "$1") || return 2
    [[ $reply == "$1="* && $reply != *$'\n'* ]] || return 2
    printf '%s' "${reply#*=}"
}
check_probe() {
    [[ -d $stage && ! -L $stage && $(stat -c '%u:%g:%a' "$stage") == 0:0:700 ]] || return 2
    [[ $(stat -c '%d:%i' "$stage") == "$stage_identity" ]] || return 2
    [[ -f $probe && ! -L $probe && $(stat -c '%u:%g:%a:%h' "$probe") == 0:0:700:1 ]] || return 2
    [[ $(sha256sum "$probe" | cut -d ' ' -f 1) == "$probe_sha" ]] || return 2
    local caps
    caps=$(getcap "$probe") || return 2
    [[ -z $caps ]] || return 2
    [[ -f $stage/typed-properties.py && ! -L $stage/typed-properties.py && $(stat -c '%u:%g:%a:%h' "$stage/typed-properties.py") == 0:0:600:1 ]] || return 2
    [[ $(sha256sum "$stage/typed-properties.py" | cut -d ' ' -f 1) == "$query_sha" ]] || return 2
    [[ -f $stage/query-guard.py && ! -L $stage/query-guard.py && $(stat -c '%u:%g:%a:%h' "$stage/query-guard.py") == 0:0:600:1 ]] || return 2
    [[ $(sha256sum "$stage/query-guard.py" | cut -d ' ' -f 1) == "$query_guard_sha" ]] || return 2
}
check_unit() {
    [[ -f $source_unit && ! -L $source_unit && $(stat -c '%u:%g:%a:%h' "$source_unit") == 0:0:600:1 ]] || return 2
    [[ $(sha256sum "$source_unit" | cut -d ' ' -f 1) == "$digest" ]] || return 2
}
quiescent() {
    [[ $(property ActiveState) == inactive ]] || return 2
    [[ $(property MainPID) == 0 && $(property ControlPID) == 0 ]] || return 2
    local group
    group=$(property ControlGroup) || return 2
    [[ -z $group || $group == "/system.slice/$unit" ]] || return 2
    [[ ! -L $cgroup ]] || return 2
    if [[ -e $cgroup ]]; then
        [[ -d $cgroup && -f $cgroup/cgroup.events && ! -L $cgroup/cgroup.events ]] || return 2
        local events
        events=$(<"$cgroup/cgroup.events") || return 2
        [[ $'\n'"$events"$'\n' == *$'\npopulated 0\n'* ]] || return 2
    fi
}
stage_identity=$(stat -c '%d:%i' "$stage")
check_probe
[[ ! -e $link && ! -L $link ]] || exit 2
[[ $(property LoadState) == not-found ]] || exit 2
[[ ! -e $cgroup && ! -L $cgroup ]] || exit 2
owned=0
source_unit=
finish() {
    local status=$?
    trap - EXIT
    if [[ $status != 0 ]]; then
        # No stop/reset/unlink after unknown state. Preserve the exact fixture
        # for independent process/unit review; never turn cleanup into PASS.
        printf 'K1_NAMESPACE_FILTER_NONPASS retained=%s\n' "$owned" >&2
    fi
    exit "$status"
}
trap finish EXIT
for mode in control filtered; do
    case $mode in
        control) source_unit=$stage/control.service
            digest=a5f705b74081dacd086e3baa528afc4693474af0fb5da4d181e40b3b2fcfcaad
            restriction=no ;;
        filtered) source_unit=$stage/filtered.service
            digest=867c1a64eb5b8a5de25364d2493c7fa54dc2fc1838c771e210a6512db484f2f2
            restriction=yes ;;
    esac
    check_probe
    check_unit
    ln -s "$source_unit" "$link"
    owned=1
    systemctl daemon-reload
    [[ $(property LoadState) == loaded ]] || exit 2
    [[ $(property FragmentPath) == "$link" ]] || exit 2
    [[ $(property Conflicts) == shutdown.target ]] || exit 2
    expected="{ path=$probe ; argv[]=$probe ; ignore_errors=no ; start_time=[n/a] ; stop_time=[n/a] ; pid=0 ; code=(null) ; status=0/0 }"
    [[ $(property ExecStart) == "$expected" ]] || exit 2
    for name in DropInPaths Wants BindsTo PartOf Upholds OnFailure OnSuccess \
        TriggeredBy Requisite PropagatesStopTo StopPropagatedFrom \
        CapabilityBoundingSet AmbientCapabilities \
        PassEnvironment SupplementaryGroups RootDirectory RootImage \
        NetworkNamespacePath JoinsNamespaceOf; do
        value=$(property "$name") || exit 2
        [[ -z $value ]] || exit 2
    done
    # Typed empty arrays and exact unordered Requires names; no text omission
    # or manager-dependent dependency order is interpreted as authority.
    # Fixed typed query failure/uncertainty exits before any start or cleanup.
    check_probe
    python3 -I "$stage/typed-properties.py"
    [[ $(property User) == root ]] || exit 2
    [[ $(property NoNewPrivileges) == yes ]] || exit 2
    [[ $(property Delegate) == no ]] || exit 2
    [[ $(property RestrictNamespaces) == "$restriction" ]] || exit 2
    [[ $(property Environment) == "OMAVLESS_K1_NAMESPACE_FILTER_FIXTURE=$mode" ]] || exit 2
    for name in PrivateUsers PrivatePIDs PrivateNetwork; do
        [[ $(property "$name") == no ]] || exit 2
    done
    check_probe
    check_unit
    systemctl start "$unit"
    for attempt in {1..100}; do
        [[ $(property ActiveState) == inactive ]] && break
        sleep 0.1
    done
    quiescent
    [[ $(property Result) == success ]] || exit 2
    [[ $(property ExecMainStatus) == 0 ]] || exit 2
    check_probe
    check_unit
    [[ -L $link && $(readlink "$link") == "$source_unit" ]] || exit 2
    unlink "$link"
    systemctl daemon-reload
    [[ $(property LoadState) == not-found ]] || exit 2
    owned=0
    printf 'K1_NAMESPACE_FILTER_%s_PASS\n' "$mode"
done
printf 'K1_NAMESPACE_FILTER_VM_PASS\n'
