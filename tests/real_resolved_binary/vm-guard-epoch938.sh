#!/bin/bash
set -euo pipefail
test "${1:-}" = 1
task_stage=/home/kdk_vm/.cache/t3-real-resolved-review-1
test "$(id -u)" = 1000
strict_pgrep() {
  local task_status=0
  pgrep "$@" || task_status=$?
  case "$task_status" in 0|1) return 0 ;; *) return "$task_status" ;; esac
}
test "$(stat -c %a "$task_stage")" = 700
test "$(sha256sum "$task_stage/probe.py" | cut -d' ' -f1)" = 2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592
test "$(sha256sum "$task_stage/host.rs" | cut -d' ' -f1)" = b5aaccb001f6c6773b241ff6e2de7cadab5a06bcc5149f66168ce28e9d3e784c
test "$(sha256sum "$task_stage/observer.rs" | cut -d' ' -f1)" = 507c8ab6c99cd8afe49ed581bcddbb12063a05294daaaf3bec8ed1ae07286161
test "$(sha256sum "$task_stage/guest-inventory.json" | cut -d' ' -f1)" = 4f1b92aeaff78f5f376576fc1da722cf9077735ff5c36d8674ff92581342ada4
test "$(sha256sum "$task_stage/host-fixture" | cut -d' ' -f1)" = fbd19fc83f5d6548ff8f1fe89d4d66ab9032d85cb551a2364f5719a5cad234f7
test "$(sha256sum "$task_stage/bundle/developer-manifest.json" | cut -d' ' -f1)" = 39fa6ae1e39b47e511e7a794cadff3322df9b445f04902dcc0c0d1bcd013b532
test "$(sha256sum "$task_stage/bundle/mihomo" | cut -d' ' -f1)" = 3b1da75d3c9fd8440216f9c256c6c59da812faae88debc936f3c72fef9724544
test "$(sha256sum "$task_stage/bundle/omavless-dns-broker" | cut -d' ' -f1)" = 6126e5b159eb7996cbf8ac6bdb212be3d7b4b12b1809e09e74c19dbc1394001e
test "$(sha256sum /usr/bin/mihomo | cut -d' ' -f1)" = ba7a74ed6bbc3098930e6e172fa9b7d4ef7e42a77f7a516046d61fbeb3bfcfe6
task_file_caps=$(getcap "$task_stage/host-fixture" "$task_stage/bundle/mihomo" "$task_stage/bundle/omavless-dns-broker" /usr/bin/mihomo)
test -z "$task_file_caps"
mkdir -m700 "$task_stage/scratch"
task_private_before=$(sha256sum /home/kdk_vm/.config/omavless/profiles.json /home/kdk_vm/.config/omavless/route-template.yaml /home/kdk_vm/.local/state/omavless/desired.json /home/kdk_vm/.local/state/omavless/ownership.json | sha256sum)
canonical_epoch() {
  python3 - <<'PY'
from pathlib import Path
import json, os
boot = Path('/proc/sys/kernel/random/boot_id').read_text().strip()
fields = Path('/proc/938/stat').read_text().rsplit(')', 1)[1].split()
assert boot == '9cdd6950-1655-495b-a52e-0f8f14200d19'
assert fields[0] != 'Z' and int(fields[19]) == 1901
loaded, installed = os.stat('/proc/938/exe'), os.stat('/usr/bin/omavless')
assert (loaded.st_dev, loaded.st_ino) == (installed.st_dev, installed.st_ino) == (31, 292400)
print(json.dumps({'boot_id': boot, 'pid': 938, 'starttime_ticks': 1901,
                  'executable_device': loaded.st_dev, 'executable_inode': loaded.st_ino}, sort_keys=True))
PY
}
task_epoch_before=$(canonical_epoch)
task_service_before=$(systemctl --user show omavless-runtime.service -p ActiveState -p SubState -p MainPID)
test "$(systemctl --user show omavless-runtime.service -p ActiveState --value)" = active
test "$(systemctl --user show omavless-runtime.service -p SubState --value)" = running
test "$(systemctl --user show omavless-runtime.service -p MainPID --value)" = 938
test "$(sha256sum /proc/938/exe | cut -d' ' -f1)" = 76abb574f611c11c2513436ac4be48d2fb07a69ba35f56127c93c21c6205921c
test "$(sha256sum /usr/bin/omavless | cut -d' ' -f1)" = 76abb574f611c11c2513436ac4be48d2fb07a69ba35f56127c93c21c6205921c
task_executable_before=$(sha256sum /proc/938/exe)
task_namespace_before=$(stat -Lc '%d:%i' /proc/self/ns/net)
test "$task_namespace_before" = 5:4026531833
task_cores_before=$(strict_pgrep -x mihomo)
task_tuns_before=$(find -L /sys/class/net/* -maxdepth 1 -name tun_flags -print | sort)
task_resolver_before=$(resolvectl status --no-pager | sha256sum)
task_resolvconf_before=$(sha256sum /etc/resolv.conf)
snapshot_network() {
  ip -j address show > "$task_stage/address-$1.json"
  ip -j route show table all > "$task_stage/route-$1.json"
  ip -j rule show > "$task_stage/rule-$1.json"
  ip -6 -j route show table all > "$task_stage/route6-$1.json"
  ip -6 -j rule show > "$task_stage/rule6-$1.json"
}
snapshot_network before
task_failed=0
if ! timeout --signal=TERM --kill-after=5 300 env -i HOME=/home/kdk_vm PATH=/usr/bin LANG=C TMPDIR="$task_stage/scratch" python3 "$task_stage/probe.py" --run --ack-disposable-vm --bundle "$task_stage/bundle" --fixture "$task_stage/host-fixture" --fixture-sha fbd19fc83f5d6548ff8f1fe89d4d66ab9032d85cb551a2364f5719a5cad234f7 --launcher-sha 2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592 --host-source-sha b5aaccb001f6c6773b241ff6e2de7cadab5a06bcc5149f66168ce28e9d3e784c --observer-source-sha 507c8ab6c99cd8afe49ed581bcddbb12063a05294daaaf3bec8ed1ae07286161 --scratch "$task_stage/scratch" > "$task_stage/probe.log" 2>&1; then
  printf '%s\n' REAL_RESOLVED_EXECUTION_NONPASS
  task_failed=1
fi
if ! python3 - "$task_stage/scratch/real-resolved-gate/results.json" <<'PY'
import json, pathlib, sys
value = json.loads(pathlib.Path(sys.argv[1]).read_text())
assert value['schema'] == 'composed-binary-real-resolved-modeled-manager-v1'
assert value['manifest_sha256'] == '39fa6ae1e39b47e511e7a794cadff3322df9b445f04902dcc0c0d1bcd013b532'
assert value['core_sha256'] == '3b1da75d3c9fd8440216f9c256c6c59da812faae88debc936f3c72fef9724544'
assert value['broker_sha256'] == '6126e5b159eb7996cbf8ac6bdb212be3d7b4b12b1809e09e74c19dbc1394001e'
assert value['fixture_sha256'] == 'fbd19fc83f5d6548ff8f1fe89d4d66ab9032d85cb551a2364f5719a5cad234f7'
assert value['normal_activation'] is False and value['installed_attestation'] is False
assert value['manager_authority'] == 'modeled'
assert value['resolver_authority'] == 'actual_private_daemon'
assert value['inventory_sha256'] == '4f1b92aeaff78f5f376576fc1da722cf9077735ff5c36d8674ff92581342ada4'
assert value['resolver_sha256'] == 'feb36cd417f4e6a065222be0f88ce25933244bc22a9f5dd54030f0fa6ac5cfcd'
assert value['source_sha256'] == {
    'probe.py': '2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592',
    'host.rs': 'b5aaccb001f6c6773b241ff6e2de7cadab5a06bcc5149f66168ce28e9d3e784c',
    'observer.rs': '507c8ab6c99cd8afe49ed581bcddbb12063a05294daaaf3bec8ed1ae07286161'}
rows = value['results']
assert len(rows) == 4 and [row['case'] for row in rows] == ['success', 'denial', 'revert-denial', 'owner-loss']
assert all(row['outcome'] == 'MEASURED' and 'cleanup_reason' not in row for row in rows)
import hashlib
for row in rows:
    receipt = row['receipt']
    assert receipt['received_tun'] is True and receipt['notify_alive'] is True
    assert receipt['observer_finalized'] is True and receipt['monitor_alive'] is False
    assert row['monitor_final_sha256'] == hashlib.sha256(json.dumps(receipt, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    assert set(row['config_sha256']) == {'bus', 'resolved', 'core'}
    if row['case'] in ('success', 'denial'):
        assert receipt['stored'] is False and receipt['phase'] is None and receipt['tun_exists'] is False
        assert receipt['reset_while_held'] is True and receipt['unrelated_preserved'] is True
    else:
        assert receipt['stored'] is True and receipt['phase'] == 'quarantined' and receipt['tun_exists'] is True
        assert receipt['reset_while_held'] is False and row['recovery_verified'] is False
        assert row['namespace_teardown_only'] is True
print('REAL_RESOLVED_ACTUAL_DAEMON_MODELED_MANAGER_4_MEASURED_NOT_INSTALLED_ACCEPTANCE')
PY
then task_failed=1; fi
check_category() {
  if [[ "$2" == "$3" ]]; then printf 'REAL_RESOLVED_PRESERVED_%s\n' "$1"; else printf 'REAL_RESOLVED_CHANGED_%s\n' "$1"; task_failed=1; fi
}
task_private_after=$(sha256sum /home/kdk_vm/.config/omavless/profiles.json /home/kdk_vm/.config/omavless/route-template.yaml /home/kdk_vm/.local/state/omavless/desired.json /home/kdk_vm/.local/state/omavless/ownership.json | sha256sum)
task_service_after=$(systemctl --user show omavless-runtime.service -p ActiveState -p SubState -p MainPID)
task_executable_after=$(sha256sum /proc/938/exe)
task_namespace_after=$(stat -Lc '%d:%i' /proc/self/ns/net)
task_cores_after=$(strict_pgrep -x mihomo)
task_tuns_after=$(find -L /sys/class/net/* -maxdepth 1 -name tun_flags -print | sort)
task_resolver_after=$(resolvectl status --no-pager | sha256sum)
task_resolvconf_after=$(sha256sum /etc/resolv.conf)
task_epoch_after=$(canonical_epoch)
check_category CANONICAL_EPOCH "$task_epoch_before" "$task_epoch_after"
check_category PRIVATE_FILES "$task_private_before" "$task_private_after"
check_category USER_SERVICE "$task_service_before" "$task_service_after"
check_category EXECUTABLE "$task_executable_before" "$task_executable_after"
check_category NAMESPACE "$task_namespace_before" "$task_namespace_after"
check_category CORE_INVENTORY "$task_cores_before" "$task_cores_after"
check_category TUN_INVENTORY "$task_tuns_before" "$task_tuns_after"
check_category RESOLVER "$task_resolver_before" "$task_resolver_after"
check_category RESOLVCONF "$task_resolvconf_before" "$task_resolvconf_after"
snapshot_network after
if ! python3 - "$task_stage" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
timers = {'valid_life_time', 'preferred_life_time'}
confirmed, other = set(), False
def compare(before, after, path):
    global other
    if type(before) is not type(after):
        other = True; return
    if isinstance(before, dict):
        if before.keys() != after.keys():
            other = True; return
        for key in before:
            compare(before[key], after[key], (*path, key))
    elif isinstance(before, list):
        if len(before) != len(after):
            other = True; return
        for index, (left, right) in enumerate(zip(before, after)):
            compare(left, right, (*path, index))
    elif before != after:
        if (len(path) == 5 and path[0] == 'address' and type(path[1]) is int
                and path[2] == 'addr_info' and type(path[3]) is int and path[4] in timers
                and type(before) is int and 0 <= after <= before):
            confirmed.add(path[4])
        else:
            other = True
for collection in ('address', 'route', 'rule', 'route6', 'rule6'):
    compare(json.loads((root / f'{collection}-before.json').read_text()),
            json.loads((root / f'{collection}-after.json').read_text()), (collection,))
if other:
    print('REAL_RESOLVED_CHANGED_NETWORK_NON_TIMER_OR_UNCONFIRMED_TIMER'); sys.exit(1)
for field in sorted(confirmed):
    print('REAL_RESOLVED_CONFIRMED_ADDRESS_COUNTDOWN_' + field.upper())
print('REAL_RESOLVED_PRESERVED_NETWORK_ALL_NON_TIMER_FIELDS')
PY
then task_failed=1; fi
for task_case in success denial revert-denial owner-loss; do
  test -d "$task_stage/scratch/real-resolved-gate/$task_case-root"
  task_root_survivors=$(find "$task_stage/scratch/real-resolved-gate/$task_case-root" -mindepth 1 -print -quit)
  test -z "$task_root_survivors" || task_failed=1
done
task_artifact_survivors=$(strict_pgrep -f '^/artifacts/(mihomo|omavless-dns-broker|host-fixture)( |$)')
task_probe_survivors=$(strict_pgrep -f '^(/usr/bin/)?python3 /home/kdk_vm/.cache/t3-real-resolved-review-1/(probe.py|scratch/real-resolved-gate/inputs/probe.py)( |$)')
test -z "$task_artifact_survivors" || task_failed=1
test -z "$task_probe_survivors" || task_failed=1
test "$task_failed" = 0
printf '%s\n' REAL_RESOLVED_VM_MOCK_HOST_AND_STRICT_BASELINE_PASS
# No material cleanup here. Hash/archive and independently retain receipts first;
# stop on any refusal. Remove only validated own staging after separate inspection.
