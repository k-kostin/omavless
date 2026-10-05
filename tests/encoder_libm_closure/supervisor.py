#!/usr/bin/python3
"""Fixed encoder capture child; first unknown/nonzero is terminal."""
import hashlib
import os
from pathlib import Path
import stat
import sys
import types

STAGE = Path('/home/kdk_vm/.cache/t3-encoder-libm-closure-review-1')
PINS = {
    'owned.py': '3ff7973c23d750fc8f1706120116def70d4615ac1d45a75fd61d31373dd17a39',
    'probe.py': 'cc478a798b78954c675c32fd6a0b3ea71c707041a02926fd120293a166eafa62',
    'containment.py': '2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592',
    'helpers.py': 'cccc171213f4631f54d906652aeaf7954230949a7de40b2093c8ab587f86aa00',
    'copy-manifest.json': 'd6ca9cd599adb573f4419f49147677f681db08fcc54a06cb0f34ed950ce19c36',
}


def require(value):
    if not value:
        raise RuntimeError('fixed_encoder_supervision_refused')


def run():
    require(Path(__file__) == STAGE/'supervisor.py' and sys.argv[1:] == ['--run-fixed-encoder']
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
    base = types.ModuleType('fixed_encoder_containment')
    base.__file__ = str(STAGE/'containment.py')
    exec(compile(raw['containment.py'],base.__file__,'exec'),base.__dict__)
    owned = types.ModuleType('fixed_encoder_owned')
    owned.__file__ = str(STAGE/'owned.py')
    exec(compile(raw['owned.py'],owned.__file__,'exec'),owned.__dict__)
    with (STAGE/'result.json').open('xb') as output, (STAGE/'child.stderr').open('xb') as error:
        child = base.OwnedProcess(['/usr/bin/python3',str(STAGE/'probe.py'),'--capture-fixed-encoder'],
                                  stdin=base.subprocess.DEVNULL,stdout=output,stderr=error,
                                  env={'HOME':'/home/kdk_vm','PATH':'/usr/bin','LANG':'C'},
                                  start_new_session=True,preexec_fn=base.limits)
        owned.settle(base,child,140)
        require(type(child.returncode) is int and child.returncode == 0)


if __name__ == '__main__':
    os.umask(0o077)
    try:
        run()
    except BaseException:
        os.write(2,b'ENCODER_LIBM_CLOSURE_SUPERVISION_NONPASS\n')
        raise SystemExit(1) from None
