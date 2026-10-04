#!/usr/bin/python3
"""Fixed encoder capture child; first unknown/nonzero is terminal."""
import hashlib
import os
from pathlib import Path
import stat
import sys
import types

STAGE = Path('/home/kdk_vm/.cache/t3-six-library-static-closure-review-1')
PINS = {
    'owned.py': '473547131f72ac768b168370fb31f551b64a428520828ca47e200cd46885eefa',
    'probe.py': '50c3ecb552fe9232d0bb8803d12e1f4eb618d7328cdda32c9a95aa4aed176fff',
    'containment.py': '2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592',
    'helpers.py': 'cccc171213f4631f54d906652aeaf7954230949a7de40b2093c8ab587f86aa00',
    'copy-manifest.json': 'b914dece6cb3c58f74bb4cdea8b19ade7b3b032e1b12d112d7524a2c47ca6c87',
}


def require(value):
    if not value:
        raise RuntimeError('fixed_six_library_supervision_refused')


def start_capture(base, owned, output, error):
    child = base.OwnedProcess(['/usr/bin/python3',str(STAGE/'probe.py'),'--capture-fixed-six-libraries'],
                              stdin=base.subprocess.DEVNULL,stdout=output,stderr=error,
                              env={'HOME':'/home/kdk_vm','PATH':'/usr/bin','LANG':'C',
                                   'TMPDIR':str(STAGE/'scratch')},
                              start_new_session=True,preexec_fn=base.limits)
    owned.settle(base,child,140)
    require(type(child.returncode) is int and child.returncode == 0)


def run():
    require(Path(__file__) == STAGE/'supervisor.py' and sys.argv[1:] == ['--run-fixed-six-libraries']
            and os.getuid() == os.geteuid() == 1000)
    for parent in (STAGE, *STAGE.parents):
        s = parent.lstat()
        require(stat.S_ISDIR(s.st_mode) and s.st_uid in (0,1000) and not s.st_mode & 0o022)
    require(stat.S_IMODE(STAGE.lstat().st_mode) == 0o700)
    raw = {}
    for name,pin in PINS.items():
        fd = os.open(STAGE/name, os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
        try:
            before = os.fstat(fd)
            require(stat.S_ISREG(before.st_mode) and before.st_uid == before.st_gid == 1000
                    and before.st_nlink == 1 and 0 < before.st_size <= 131072
                    and stat.S_IMODE(before.st_mode) == (0o600 if name.endswith('.json') else 0o500)
                    and not os.listxattr(fd))
            data = os.pread(fd,131073,0)
            fields = lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_uid,s.st_gid,s.st_nlink,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
            require(len(data) == before.st_size and hashlib.sha256(data).hexdigest() == pin
                    and fields(before) == fields(os.fstat(fd)) == fields((STAGE/name).lstat()))
            raw[name] = data
        finally:
            os.close(fd)
    base = types.ModuleType('fixed_six_library_containment')
    base.__file__ = str(STAGE/'containment.py')
    exec(compile(raw['containment.py'],base.__file__,'exec'),base.__dict__)
    owned = types.ModuleType('fixed_six_library_owned')
    owned.__file__ = str(STAGE/'owned.py')
    exec(compile(raw['owned.py'],owned.__file__,'exec'),owned.__dict__)
    with (STAGE/'result.json').open('xb') as output, (STAGE/'child.stderr').open('xb') as error:
        start_capture(base,owned,output,error)


if __name__ == '__main__':
    os.umask(0o077)
    try:
        run()
    except BaseException:
        os.write(2,b'SIX_LIBRARY_STATIC_CLOSURE_SUPERVISION_NONPASS\n')
        raise SystemExit(1) from None
