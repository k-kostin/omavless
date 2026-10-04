#!/usr/bin/python3
"""Fixed read-only eight-property fixture; no caller-selected IPC or output."""
import hashlib
import os
from pathlib import Path
import stat
import sys

STAGE = Path('/run/omavless-k1-typed-order-filter-fixture')
ORIGINAL_SHA = 'c4a688875037f1d990ee93f0018108ab82bceb211090f61f77c64b83a4c7cc40'
PROPERTIES = ('ExecCondition', 'ExecStartPre', 'ExecStartPost', 'ExecReload',
              'ExecStop', 'ExecStopPost', 'EnvironmentFiles', 'SystemCallFilter')
ARGV = ['/usr/bin/busctl', '--system', 'get-property', 'org.freedesktop.systemd1',
        '/org/freedesktop/systemd1/unit/omavless_2dk1_2dtyped_2dorder_2dfilter_2dfixture_2eservice',
        'org.freedesktop.systemd1.Service', *PROPERTIES]
EXPECTED = (('a(sasbttttuii) 0\n' * 6) + 'a(sb) 0\n(bas) false 0\n').encode('ascii')
REQUIRES_ARGV = [*ARGV[:5], 'org.freedesktop.systemd1.Unit', 'Requires']
REQUIRES_EXPECTED = {
    b'as 2 "sysinit.target" "system.slice"\n',
    b'as 2 "system.slice" "sysinit.target"\n',
}


def require(value):
    if not value:
        raise RuntimeError()


def load_original(data):
    require(hashlib.sha256(data).hexdigest() == ORIGINAL_SHA)
    namespace = {'__name__': 'fixed_property_query_not_main',
                 '__file__': str(STAGE / 'query-guard.py')}
    exec(compile(data, '<hash-pinned-query-guard>', 'exec'), namespace)
    namespace['STAGE'] = STAGE
    return namespace


def fixed_query(namespace):
    require(not namespace['UNCERTAIN'])
    data = namespace['command'](list(ARGV))
    require(not namespace['UNCERTAIN'] and data == EXPECTED)
    # Dependency order is not manager semantics. Require exactly the two
    # approved names, typed count/signature and byte framing, with no extras.
    dependencies = namespace['command'](list(REQUIRES_ARGV))
    require(not namespace['UNCERTAIN'] and dependencies in REQUIRES_EXPECTED)


def main():
    require(os.geteuid() == 0 and len(sys.argv) == 1)
    directory = STAGE.lstat()
    require(stat.S_ISDIR(directory.st_mode) and directory.st_uid == directory.st_gid == 0
            and stat.S_IMODE(directory.st_mode) == 0o700)
    fd = os.open(STAGE / 'query-guard.py', os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    with os.fdopen(fd, 'rb') as stream:
        before = os.fstat(stream.fileno())
        require(stat.S_ISREG(before.st_mode) and before.st_uid == before.st_gid == 0
                and stat.S_IMODE(before.st_mode) == 0o600 and before.st_nlink == 1
                and 0 < before.st_size < 256 * 1024)
        data = stream.read(256 * 1024)
        after = os.fstat(stream.fileno())
        fields = ('st_dev', 'st_ino', 'st_uid', 'st_gid', 'st_mode', 'st_nlink',
                  'st_size', 'st_mtime_ns', 'st_ctime_ns')
        require(len(data) == before.st_size
                and all(getattr(before, f) == getattr(after, f) for f in fields))
    final_directory = STAGE.lstat()
    require((directory.st_dev, directory.st_ino, directory.st_mode, directory.st_uid, directory.st_gid)
            == (final_directory.st_dev, final_directory.st_ino, final_directory.st_mode,
                final_directory.st_uid, final_directory.st_gid))
    fixed_query(load_original(data))


if __name__ == '__main__':
    os.umask(0o077)
    try:
        main()
    except BaseException:
        # Never expose property values, errors or paths, and never retry.
        print('K1_TYPED_PROPERTIES_REFUSED_RETAINED', file=sys.stderr)
        sys.exit(2)
