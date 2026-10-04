#!/bin/bash
set -euo pipefail
set -C
umask 077
test "$#" = 0
task_stage=/home/kdk_vm/.cache/t3-encoder-unlisted-dependency-review-1
test "$(id -u)" = 1000
strict_pgrep() {
  local task_status=0
  pgrep "$@" || task_status=$?
  case "$task_status" in 0|1) return 0 ;; *) return "$task_status" ;; esac
}
test -d "$task_stage" && test ! -L "$task_stage"
test "$(stat -c '%u:%g:%a' "$task_stage")" = 1000:1000:700
python3 - "$task_stage" <<'PY'
import hashlib, os, pathlib, stat, sys
root = pathlib.Path(sys.argv[1])
pins = {
    "probe.py": "d2465f004fe7cd08f8dae27fdff5abc5d598106038a585287758f938e323d304",
    "supervisor.py": "00ce656fdacd43d3d7a7dc75e78d4f659ab431d508831e38a13f50eba4d8998d",
    "validator.py": "1f8c9dc22fa4fcdaa60aad958886109a66ebb1f215f4912ab0b5ea5871ed90c0",
    "owned.py": "8d9504cff609d703870b4b7f8bec4e4b601d8763b5f69b7df62e3b5da9106258",
    "containment.py": "2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592",
    "helpers.py": "cccc171213f4631f54d906652aeaf7954230949a7de40b2093c8ab587f86aa00",
    "copy-manifest.json": "d6ca9cd599adb573f4419f49147677f681db08fcc54a06cb0f34ed950ce19c36"
}
for path in (root, *root.parents):
    value = path.lstat()
    assert stat.S_ISDIR(value.st_mode) and value.st_uid in (0, 1000) and value.st_mode & 0o022 == 0
for name, expected in pins.items():
    path = root / name
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(fd)
        mode = 0o500 if name.endswith('.py') else 0o600
        assert stat.S_ISREG(before.st_mode) and before.st_uid == before.st_gid == 1000
        assert stat.S_IMODE(before.st_mode) == mode and before.st_nlink == 1
        assert 0 < before.st_size <= 131072 and not os.listxattr(fd)
        with os.fdopen(os.dup(fd), 'rb') as stream:
            data = stream.read(131073)
        identity = lambda s: (s.st_dev,s.st_ino,s.st_mode,s.st_uid,s.st_gid,s.st_nlink,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
        assert identity(before) == identity(os.fstat(fd)) == identity(path.lstat())
        assert len(data) == before.st_size and hashlib.sha256(data).hexdigest() == expected
    finally:
        os.close(fd)
PY
test "$(sha256sum /usr/bin/mihomo | cut -d' ' -f1)" = ba7a74ed6bbc3098930e6e172fa9b7d4ef7e42a77f7a516046d61fbeb3bfcfe6
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
# Frozen known-anchor supervision is inside supervisor.py; never GNU timeout.
if ! env -i HOME=/home/kdk_vm PATH=/usr/bin LANG=C TMPDIR="$task_stage/scratch" python3 "$task_stage/supervisor.py" --run-fixed-encoder > "$task_stage/probe.log" 2>&1; then
  printf '%s\n' ENCODER_UNLISTED_INVOCATION_NONPASS
  exit 1
fi
if ! python3 "$task_stage/validator.py" --validate-fixed-encoder; then
  printf '%s\n' ENCODER_UNLISTED_RECEIPT_NONPASS
  exit 1
fi
# Only a known completed, strictly typed successful invocation permits after-queries.
check_category() {
  if [[ "$2" == "$3" ]]; then printf 'ENCODER_UNLISTED_INVENTORY_PRESERVED_%s\n' "$1"; else printf 'ENCODER_UNLISTED_INVENTORY_CHANGED_%s\n' "$1"; exit 1; fi
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
    print('ENCODER_UNLISTED_INVENTORY_CHANGED_NETWORK_NON_TIMER_OR_UNCONFIRMED_TIMER'); sys.exit(1)
for field in sorted(confirmed):
    print('ENCODER_UNLISTED_INVENTORY_CONFIRMED_ADDRESS_COUNTDOWN_' + field.upper())
print('ENCODER_UNLISTED_INVENTORY_PRESERVED_NETWORK_ALL_NON_TIMER_FIELDS')
PY
then exit 1; fi
task_probe_survivors=$(strict_pgrep -f '^(/usr/bin/)?python3 /home/kdk_vm/.cache/t3-encoder-unlisted-dependency-review-1/(probe|supervisor)[.]py( |$)')
test -z "$task_probe_survivors" || exit 1
task_readelf_survivors=$(strict_pgrep -f '^/proc/self/fd/[0-9]+ --wide --dynamic --program-headers /proc/self/fd/[0-9]+$')
test -z "$task_readelf_survivors" || exit 1
test "$task_failed" = 0
printf '%s\n' ENCODER_UNLISTED_INVENTORY_AND_STRICT_BASELINE_PASS_NOT_COMPATIBILITY_ACCEPTANCE
# Retain staging and archive before any separately scoped cleanup.
