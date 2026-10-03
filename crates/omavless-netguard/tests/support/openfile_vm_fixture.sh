#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Explicit developer VM-only root harness. Never installed or called by product.
set -euo pipefail
export LC_ALL=C
umask 077

[[ ${OMAVLESS_K1_OPENFILE_VM:-} == 1 && $EUID == 0 && $# == 0 ]] || exit 2
[[ $(systemd-detect-virt --vm) == kvm ]] || exit 2
probe=/run/omavless-k1-openfile-fixture/probe
unit=omavless-k1-openfile-fixture.service
[[ -f $probe && ! -L $probe && -x $probe ]] || exit 2
[[ $(stat -c '%u:%a' /run/omavless-k1-openfile-fixture) == 0:700 ]] || exit 2
[[ $(stat -c '%u:%a' "$probe") == 0:700 ]] || exit 2
[[ $(systemctl show "$unit" -p LoadState --value 2>/dev/null || true) == not-found ]] || exit 2
owned=0
cleanup() {
    if [[ $owned == 1 ]]; then
        # This exact previously absent transient unit belongs to this harness.
        systemctl stop "$unit" >/dev/null 2>&1 || true
        systemctl reset-failed "$unit" >/dev/null 2>&1 || true
    fi
}
trap cleanup EXIT

for scenario in match missing regular pid_namespace private_network; do
    properties=()
    expected=K1_OPENFILE_REFUSED
    expected_code=2
    case "$scenario" in
        match)
            properties+=(--property=OpenFile=/proc/1/ns/net:k1-host-netns:read-only)
            expected=K1_OPENFILE_DESCRIPTOR_MATCH
            expected_code=0
            ;;
        missing) ;;
        regular) properties+=(--property=OpenFile=/usr/lib/os-release:k1-host-netns:read-only) ;;
        pid_namespace) properties+=(--property=OpenFile=/proc/1/ns/pid:k1-host-netns:read-only) ;;
        private_network)
            properties+=(--property=OpenFile=/proc/1/ns/net:k1-host-netns:read-only --property=PrivateNetwork=yes)
            ;;
    esac
    [[ $(systemctl show "$unit" -p LoadState --value 2>/dev/null || true) == not-found ]] || exit 2
    owned=1
    code=0
    output=$(timeout --kill-after=2s 15s systemd-run --quiet --wait --pipe --collect \
        --unit="$unit" --property=Type=exec --property=User=root \
        --property=RuntimeMaxSec=10s --property=TimeoutStopSec=2s \
        --property=PrivateUsers=no --property=PrivatePIDs=no \
        --property=PrivateNetwork=no --property=NoNewPrivileges=yes \
        --setenv=OMAVLESS_K1_OPENFILE_FIXTURE=1 \
        "${properties[@]}" "$probe" 2>/dev/null) || code=$?
    [[ $code == "$expected_code" && $output == "$expected" ]] || {
        printf 'K1_OPENFILE_CASE_FAILED=%s\n' "$scenario"
        exit 2
    }
    # --collect must unload both successful and failed transient services.
    for attempt in {1..20}; do
        [[ $(systemctl show "$unit" -p LoadState --value 2>/dev/null || true) == not-found ]] && break
        sleep 0.1
    done
    [[ $(systemctl show "$unit" -p LoadState --value 2>/dev/null || true) == not-found ]] || exit 2
    owned=0
    printf 'K1_OPENFILE_CASE_PASS=%s\n' "$scenario"
done
printf 'K1_OPENFILE_VM_PASS\n'
