#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Dedicated KVM only. No installed service or network/namespace transition.
set -euo pipefail
export LC_ALL=C
umask 077
[[ ${OMAVLESS_K1_NAMESPACE_FILTER_VM:-} == 1 && $EUID == 0 && $# == 0 ]] || exit 2
[[ $(systemd-detect-virt --vm) == kvm ]] || exit 2
stage=/run/omavless-k1-typed-filter-fixture
probe=$stage/probe
unit=omavless-k1-typed-filter-fixture.service
link=/run/systemd/system/$unit
cgroup=/sys/fs/cgroup/system.slice/$unit
probe_sha=b7dc81b89045c591efd375765ddf4fc4792afedf86cbc73c18fd94d1227d7332
query_sha=2aef0c278a1510af8ed6d1800c963b07bcc7fbf6bbd1bf7071d9ec1986529c2e
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
            digest=b7adcf6e33b3f2f03308285c0a93ec31110a8f0e6955bf3a8376cc2ab3f4dc57
            restriction=no ;;
        filtered) source_unit=$stage/filtered.service
            digest=f3274fb882364e2ec1dc62000192642481f65ab16e650b8147e2435ae300408a
            restriction=yes ;;
    esac
    check_probe
    check_unit
    ln -s "$source_unit" "$link"
    owned=1
    systemctl daemon-reload
    [[ $(property LoadState) == loaded ]] || exit 2
    [[ $(property FragmentPath) == "$link" ]] || exit 2
    [[ $(property Requires) == 'sysinit.target system.slice' ]] || exit 2
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
    # Structured empty arrays have no reliable systemctl text representation.
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
