#!/bin/bash
set -euo pipefail
set -o noclobber
umask 077
test "${1:-}" = 1
task_stage=/home/kdk_vm/.cache/t3-alpm-files-retained-fd-review-1
test "$(id -u)" = 1000
strict_pgrep() {
  local task_status=0
  pgrep "$@" || task_status=$?
  case "$task_status" in 0|1) return 0 ;; *) return "$task_status" ;; esac
}
test -d "$task_stage"
test ! -L "$task_stage"
test "$(stat -c %u:%g "$task_stage")" = 1000:1000
test "$(stat -c %a "$task_stage")" = 700
test "$(sha256sum "$task_stage/probe.py" | cut -d' ' -f1)" = 5c696390ccdfdc536a77650cb0d85d5b7f4b817bc7b84eedba22166e914752d5
test "$(sha256sum "$task_stage/supervisor.py" | cut -d' ' -f1)" = 37e1e7fa4e1d393bd6df8e80a2c06623015afe6c0fa8dd5fc823919196ad80a6
test "$(sha256sum "$task_stage/containment.py" | cut -d' ' -f1)" = 2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592
test "$(sha256sum /usr/bin/mihomo | cut -d' ' -f1)" = ba7a74ed6bbc3098930e6e172fa9b7d4ef7e42a77f7a516046d61fbeb3bfcfe6
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
if ! env -i HOME=/home/kdk_vm PATH=/usr/bin LANG=C /usr/bin/python3 "$task_stage/supervisor.py" --run-metadata-child > "$task_stage/supervisor.log" 2>&1; then
  printf '%s\n' ALPM_SHAPE_DIAGNOSTIC_NONPASS
  # Failure can mean unknown child ownership. Retain before snapshots and stop:
  # no result reads, baseline queries, quiescence checks, signals or cleanup.
  exit 1
fi
if ! python3 - "$task_stage/result.json" <<'PY'
import json, pathlib, sys
value = json.loads(pathlib.Path(sys.argv[1]).read_text())
owned = json.loads(pathlib.Path(sys.argv[1]).with_name('supervisor-receipt.json').read_text())
assert owned['schema'] == 'alpm-metadata-owned-child-v1'
assert owned['outcome'] == 'KNOWN_COMPLETED' and owned['returncode'] == 0
assert value['schema'] == 'public-alpm-files-shape-v1'
assert value['outcome'] == 'OBSERVED_ALPM_FILELIST_SHAPE' and 'cleanup_reason' not in value
assert all(value[key] is False for key in ('readelf_executed','candidate_elf_executed','allowlist_adoption'))
assert value['shape']['legacy_exactly_one_files_marker'] is False
assert set(value['original_open_fd']) == {'device','inode','size','uid','gid','mode','nlink','mtime_ns','ctime_ns','sha256'}
assert set(value['package']) == {'name','version'}
print('ALPM_SHAPE_METADATA_OBSERVED_NOT_PARSER_OR_ELF_ADMISSION')
PY
then task_failed=1; fi
check_category() {
  if [[ "$2" == "$3" ]]; then printf 'ALPM_SHAPE_PRESERVED_%s\n' "$1"; else printf 'ALPM_SHAPE_CHANGED_%s\n' "$1"; task_failed=1; fi
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
    print('ALPM_SHAPE_CHANGED_NETWORK_NON_TIMER_OR_UNCONFIRMED_TIMER'); sys.exit(1)
for field in sorted(confirmed):
    print('ALPM_SHAPE_CONFIRMED_ADDRESS_COUNTDOWN_' + field.upper())
print('ALPM_SHAPE_PRESERVED_NETWORK_ALL_NON_TIMER_FIELDS')
PY
then task_failed=1; fi
task_probe_survivors=$(strict_pgrep -f '^(/usr/bin/)?python3 /home/kdk_vm/.cache/t3-alpm-files-retained-fd-review-1/(probe|supervisor)[.]py( |$)')
test -z "$task_probe_survivors" || task_failed=1
test "$task_failed" = 0
printf '%s\n' ALPM_SHAPE_METADATA_AND_STRICT_BASELINE_PRESERVED_NOT_ACCEPTANCE
# Retain staging and archive before any separately scoped cleanup.
