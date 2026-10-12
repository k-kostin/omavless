# SPDX-License-Identifier: MIT
"""Fresh fixed-account OLD recovery gate. No initialization or cleanup.

`preflight` is read-only. `positive` requires a separately provisioned empty
synthetic account and exact reviewed installed candidate. It never connects,
enrolls DNS, changes the normal user's data, or retries an uncertain effect.
"""
import hashlib
import json
import os
from pathlib import Path
import pwd
import resource
import socket
import stat
import struct
import subprocess
import sys
import tempfile
import time

# PC-only fresh scope. The ARM accounts and their retained failures are not
# transferred, resumed or cleaned by this recipe.
ACCOUNT = "ovrestore202"
HOME = Path("/home/ovrestore202")
ELF = Path("/usr/bin/omavless")
MAX_FRAME = 256 * 1024
FIXED_ACCOUNTS = {
    ACCOUNT: HOME,
    "ovrestore203": Path("/home/ovrestore203"),
    "ovrestore204": Path("/home/ovrestore204"),
}


class Refused(Exception):
    """Intentionally contains no input/path/exception text."""


def require(value):
    if not value:
        raise Refused()


def unique(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result)
        result[key] = value
    return result


def decode(raw):
    require(0 < len(raw) <= MAX_FRAME and raw.endswith(b"\n"))
    require(b"\n" not in raw[:-1])
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=unique,
                           parse_constant=lambda _: (_ for _ in ()).throw(Refused()))
    except (ValueError, UnicodeError, RecursionError):
        raise Refused() from None
    require(isinstance(value, dict))
    return value


def identity(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid,
            info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


class Held:
    """Read-only private file pin; never exports bytes in public output."""
    def __init__(self, path, uid, limit):
        self.path = Path(path)
        before = self.path.lstat()
        require(stat.S_ISREG(before.st_mode) and before.st_uid == uid
                and stat.S_IMODE(before.st_mode) == 0o600
                and before.st_nlink == 1 and before.st_size <= limit)
        self.fd = os.open(self.path, os.O_RDONLY | os.O_NOFOLLOW
                          | os.O_NONBLOCK | os.O_CLOEXEC)
        try:
            self.original = identity(before)
            self.check()
            self.data = os.pread(self.fd, limit + 1, 0)
            require(len(self.data) == before.st_size)
            self.check()
        except Exception:
            self.close()
            raise

    def check(self):
        require(identity(os.fstat(self.fd)) == self.original
                and identity(self.path.lstat()) == self.original)

    def unchanged(self):
        self.check()
        require(os.pread(self.fd, len(self.data) + 1, 0) == self.data)
        self.check()

    def close(self):
        if getattr(self, "fd", None) is not None:
            os.close(self.fd)
            self.fd = None


def account_scope(account_name=ACCOUNT, home=HOME):
    require(FIXED_ACCOUNTS.get(account_name) == home)
    account = pwd.getpwnam(account_name)
    uid = account.pw_uid
    require(uid > 0 and account.pw_dir == str(home)
            and os.getuid() == os.geteuid() == uid)
    # Fixture capacity, not a claim that the runtime's older 60-FD minimum
    # accounts for the enlarged manager/cgroup graph or supports low limits.
    soft, _ = resource.getrlimit(resource.RLIMIT_NOFILE)
    require(soft == resource.RLIM_INFINITY or soft >= 256)
    run = Path(f"/run/user/{uid}")
    require(os.environ.get("HOME") == str(home)
            and os.environ.get("XDG_RUNTIME_DIR") == str(run))
    for key in ("OMAVLESS_HOME", "OMAVLESS_MIHOMO", "LD_PRELOAD", "LD_LIBRARY_PATH",
                "XDG_CONFIG_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME"):
        require(key not in os.environ)
    for path in (home, run, run / "omavless", home / ".config/omavless",
                 home / ".local/state/omavless"):
        info = path.lstat()
        require(stat.S_ISDIR(info.st_mode) and info.st_uid == uid
                and stat.S_IMODE(info.st_mode) == 0o700
                and path.resolve() == path)
    binary = ELF.lstat()
    require(stat.S_ISREG(binary.st_mode) and binary.st_uid == 0
            and not binary.st_mode & 0o022)
    return uid, run


def send_unary(stream, frame):
    stream.sendall(json.dumps(frame, separators=(",", ":")).encode() + b"\n")
    stream.shutdown(socket.SHUT_WR)


def reply(run, uid, method, request_id):
    require(method in ("system.hello", "status.get"))
    path = run / "omavless/control.sock"
    before = path.lstat()
    require(stat.S_ISSOCK(before.st_mode) and before.st_uid == uid
            and stat.S_IMODE(before.st_mode) == 0o600)
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as stream:
        deadline = time.monotonic() + 2
        stream.settimeout(2)
        stream.connect(str(path))
        pid, peer_uid, _ = struct.unpack("3i", stream.getsockopt(
            socket.SOL_SOCKET, socket.SO_PEERCRED, struct.calcsize("3i")))
        require(pid > 0 and peer_uid == uid)
        require(identity(path.lstat()) == identity(before))
        installed = ELF.stat()
        process = Path(f"/proc/{pid}/exe").stat()
        require((installed.st_dev, installed.st_ino) == (process.st_dev, process.st_ino))
        # This is a supplementary process placement check, not manager admission.
        with open(f"/proc/{pid}/cgroup", "rb") as source:
            cgroup = source.read(8193)
        require(len(cgroup) <= 8192 and cgroup.count(b"\n") == 1)
        require(cgroup.startswith(b"0::/user.slice/")
                and f"/user-{uid}.slice/user@{uid}.service/".encode() in cgroup
                and cgroup.endswith(b"/omavless-runtime.service\n"))
        frame = {"api": "omavless.control", "version": 1, "id": request_id,
                 "method": method,
                 "params": {"versions": [1]} if method == "system.hello" else {}}
        send_unary(stream, frame)
        raw = bytearray()
        while b"\n" not in raw:
            remaining = deadline - time.monotonic()
            require(remaining > 0)
            stream.settimeout(remaining)
            chunk = stream.recv(min(4096, MAX_FRAME + 1 - len(raw)))
            require(bool(chunk))
            raw.extend(chunk)
            require(len(raw) <= MAX_FRAME)
        value = decode(bytes(raw))
        require(value.get("api") == "omavless.control" and value.get("version") == 1
                and value.get("id") == request_id and value.get("ok") is True)
        return pid, value


def off_triplet(first, status, last):
    pid, hello = first
    require(pid > 0 and status[0] == last[0] == pid)
    a, b, c = hello.get("result", {}), status[1].get("result", {}), last[1].get("result", {})
    require(all(isinstance(v, dict) for v in (a, b, c)))
    require(a.get("runtimeOwnership") is True and b.get("runtimeOwnership") is True
            and c.get("runtimeOwnership") is True)
    require(isinstance(a.get("instanceId"), str) and 0 < len(a["instanceId"]) <= 128
            and c.get("instanceId") == a["instanceId"])
    revision = hello.get("revision")
    require(type(revision) is int and 0 <= revision <= 9007199254740991
            and revision == status[1].get("revision") == last[1].get("revision"))
    require(b.get("desired") == b.get("actual") == "disconnected"
            and "transition" in b and b["transition"] is None)


def preflight():
    uid, run = account_scope()
    held = []
    try:
        config, state = HOME / ".config/omavless", HOME / ".local/state/omavless"
        for path, limit in ((config / "profiles.json", 4 * 1024 * 1024),
                            (config / "route-template.yaml", 1024 * 1024),
                            (state / "desired.json", 65536), (state / "ownership.json", 4096),
                            (run / "omavless-login.receipt", 4096)):
            held.append(Held(path, uid, limit))
        off_triplet(reply(run, uid, "system.hello", "old-preflight-1"),
                    reply(run, uid, "status.get", "old-preflight-2"),
                    reply(run, uid, "system.hello", "old-preflight-3"))
        for item in held:
            item.unchanged()
    finally:
        for item in held:
            item.close()


def no_network():
    require(not any((Path('/sys/class/net') / name).exists()
                    for name in ('Meta', 'omavless0')))
    for entry in Path('/proc').iterdir():
        if entry.name.isdecimal():
            try:
                require((entry / 'comm').read_text().strip() != 'mihomo')
            except (FileNotFoundError, ProcessLookupError):
                pass


def bounded(path, limit):
    with open(path, 'rb') as stream:
        raw = stream.read(limit + 1)
    require(len(raw) <= limit)
    return raw


def process_identity(pid):
    raw = bounded(Path(f'/proc/{pid}/stat'), 8192)
    tail = raw.rsplit(b') ', 1)[1].split()
    require(len(tail) >= 20)
    executable = Path(f'/proc/{pid}/exe').stat()
    return (pid, tail[19], executable.st_dev, executable.st_ino)


def parked_step(message=None):
    if message is not None:
        try:
            print(message, flush=True)
        except BaseException:
            pass
    try:
        time.sleep(30)
    except BaseException:
        pass


def park(phase):
    parked_step('RESTORE099_UNKNOWN_' + phase.upper() + '_PARKED')
    while True:
        parked_step()


def manager(run):
    command = ['/usr/bin/systemctl', '--user', 'show', 'omavless-runtime.service',
               '--no-pager', '--property=LoadState,ActiveState,SubState,MainPID,Job,ControlPID,FragmentPath,DropInPaths,NeedDaemonReload,RuntimeDirectoryPreserve']
    result = subprocess.run(command, capture_output=True, timeout=5, check=False,
                            env={'PATH': '/usr/bin', 'LC_ALL': 'C', 'XDG_RUNTIME_DIR': str(run)})
    require(result.returncode == 0 and len(result.stdout) <= 8192 and not result.stderr)
    fields = unique(line.split('=', 1) for line in result.stdout.decode().splitlines())
    require(fields.get('LoadState') == 'loaded' and fields.get('Job') == ''
            and fields.get('ControlPID') == '0' and fields.get('DropInPaths') == ''
            and fields.get('NeedDaemonReload') == 'no'
            and fields.get('RuntimeDirectoryPreserve') == 'yes'
            and fields.get('FragmentPath') == '/usr/lib/systemd/user/omavless-runtime.service')
    return fields


def ordinary(uid, run):
    first = reply(run, uid, 'system.hello', 'old-check-1')
    status = reply(run, uid, 'status.get', 'old-check-2')
    last = reply(run, uid, 'system.hello', 'old-check-3')
    off_triplet(first, status, last)
    fields = manager(run)
    require(fields.get('MainPID') == str(first[0])
            and fields.get('ActiveState') == 'active' and fields.get('SubState') == 'running')
    no_network()
    return first


def history_matches(raw, intent, uid, old, phase=3):
    # Comparison data only: this parser never grants recovery or Start.
    require(type(phase) is int and phase in (2, 3))
    def record(raw, size, magic, domain):
        require(len(raw) == size and raw[:8] == magic
                and hashlib.sha256(domain + raw[:-32]).digest() == raw[-32:])
        return raw[8:-32]
    ticket = record(raw, 386, b'OVRDON01', b'omavless-restore-disposition-complete-v1\0')
    record(ticket, 346, b'OVRDSP01', b'omavless-restore-disposition-candidate-v1\0')
    require(int.from_bytes(ticket[8:12], 'little') == uid and ticket[12:16] == bytes(4)
            and int.from_bytes(ticket[16:24], 'little') == int.from_bytes(intent[9:17], 'big'))
    closure = ticket[24:-32]
    receipt = record(closure, 290, b'OVRCLS01', b'omavless-restore-closure-v1\0')
    record(receipt, 250, b'OVRFIN01', b'omavless-restore-finalization-v1\0')
    terminal = receipt[8:146]
    record(terminal, 138, b'OVRDEC01', b'omavless-restore-decision-v1\0')
    require(terminal[8] == phase and terminal[9:106] == intent[9:106])
    for offset, item in zip((146, 182), old):
        require(int.from_bytes(receipt[offset:offset + 4], 'big') == len(item)
                and receipt[offset + 4:offset + 36] == hashlib.sha256(item).digest())


class Effects:
    """Finite fixture commands, private capture, no retry/timeout kill."""
    def __init__(self, directory, run, home=HOME):
        require(home in FIXED_ACCOUNTS.values())
        self.directory, self.run = directory, run
        self.home = home
        self.used = set()

    def once(self, phase, argv, data=b''):
        require(phase not in self.used and len(data) <= 32768)
        self.used.add(phase)
        print('RESTORE099_PHASE_' + phase.upper(), flush=True)
        with tempfile.TemporaryFile(dir=self.directory) as incoming, \
                open(self.directory / (phase + '.stdout'), 'xb') as out, \
                open(self.directory / (phase + '.stderr'), 'xb') as err:
            incoming.write(data)
            incoming.seek(0)
            child = subprocess.Popen(argv, stdin=incoming, stdout=out, stderr=err,
                                     env={'HOME': str(self.home), 'PATH': '/usr/bin', 'LC_ALL': 'C',
                                          'XDG_RUNTIME_DIR': str(self.run)})
            try:
                deadline = time.monotonic() + 150
                while child.poll() is None:
                    require(time.monotonic() < deadline
                            and os.fstat(out.fileno()).st_size <= MAX_FRAME
                            and os.fstat(err.fileno()).st_size <= MAX_FRAME)
                    time.sleep(0.1)
                code = child.returncode
            except BaseException:
                # Preserve the original child and all captures; operator may inspect.
                # This is never automatic recovery or permission to replay.
                park(phase)
            require(code == 0)
        raw = bounded(self.directory / (phase + '.stdout'), MAX_FRAME)
        bounded(self.directory / (phase + '.stderr'), MAX_FRAME)
        return raw


def positive():
    os.umask(0o077)
    uid, run = account_scope()
    config, state = HOME / '.config/omavless', HOME / '.local/state/omavless'
    require(not any(p.name.startswith('restore-') for p in state.iterdir()))
    current = ordinary(uid, run)
    original_process = process_identity(current[0])
    store = Held(config / 'profiles.json', uid, 4 * 1024 * 1024)
    try:
        initial = json.loads(store.data)
        require(initial.get('profiles') == [] and initial.get('subscriptions') == [])
        require(initial.get('startup', {}).get('enabled') is False)
        require(initial.get('routingPreset') in (
            'roscomvpn-default', 'china-cn-direct', 'iran-ir-direct'))
    finally:
        store.close()
    evidence = HOME / '.cache/restore099-pc-positive'
    evidence.mkdir(mode=0o700)  # Existence refuses a second run, including failed runs.
    effects = Effects(evidence, run)
    private = json.dumps({'schema': 1, 'archive': str(evidence / 'new.ovb'),
                          'passphrase': os.urandom(24).hex()}).encode()
    (evidence / 'request.json').write_bytes(private)
    effects.once('backup', [str(ELF), 'developer', 'backup-current', '--confirm-private-export'], private)
    current = ordinary(uid, run)
    # The archive contains the empty NEW store; the live OLD store gains exactly
    # one synthetic, never-connected standalone profile through the ordinary API.
    mutation = [str(ELF), 'plugin', 'profile-import', current[1]['result']['instanceId'],
                str(current[1]['revision']), 'restore099-old-fixture']
    imported = decode(effects.once('import', mutation,
        b'Recovery fixture\nvless://00000000-0000-4000-8000-000000000001@example.invalid:443?security=tls&type=tcp'))
    require(imported.get('ok') is True)
    current = ordinary(uid, run)
    held = []
    try:
        for path, limit in ((config / 'profiles.json', 4 * 1024 * 1024),
                            (config / 'route-template.yaml', 1024 * 1024),
                            (state / 'desired.json', 65536), (state / 'ownership.json', 4096),
                            (run / 'omavless-login.receipt', 4096)):
            held.append(Held(path, uid, limit))
        effects.once('pause', [str(ELF), 'developer', 'pause-current-intent',
                              '--confirm-private-intent-pause'], private)
        stage = state / 'restore-pair.pending'
        require(bounded(stage / 'old-profiles.json', 4 * 1024 * 1024) == held[0].data
                and bounded(stage / 'old-route-template.yaml', 1024 * 1024) == held[1].data
                and bounded(stage / 'new-profiles.json', 4 * 1024 * 1024) != held[0].data)
        intent = Held(state / 'restore-decision.intent', uid, 138)
        held.append(intent)
        require(len(intent.data) == 138 and intent.data[:9] == b'OVRDEC01\x01'
                and hashlib.sha256(b'omavless-restore-decision-v1\0' + intent.data[:106]).digest()
                == intent.data[106:])
        for item in held:
            item.unchanged()
        fields = manager(run)
        require(fields['MainPID'] == str(current[0]) and fields['ActiveState'] == 'active')
        require(process_identity(current[0]) == original_process)
        no_network()
        effects.once('stop', ['/usr/bin/systemctl', '--user', 'stop', 'omavless-runtime.service'])
        fields = manager(run)
        require(fields['MainPID'] == '0' and fields['ActiveState'] == 'inactive'
                and fields['SubState'] == 'dead' and not Path(f'/proc/{current[0]}').exists())
        for item in held:
            item.unchanged()
        effects.once('recover', [str(ELF), 'restore', 'recover-old', '--confirm-rollback'], private)
        fresh = ordinary(uid, run)
        require(fresh[1]['result']['instanceId'] != current[1]['result']['instanceId'])
        for item in held[:5]:
            item.unchanged()
        # Independent disposition readback, not merely the command's exit0.
        history = Held(state / 'restore-disposition.history', uid, 8192)
        held.append(history)
        history_matches(history.data, intent.data, uid, [item.data for item in held[:2]])
        require(set(p.name for p in state.iterdir() if p.name.startswith('restore-'))
                == {'restore-disposition.history'})
        require(json.loads(held[0].data).get('startup', {}).get('enabled') is False)
        again = ordinary(uid, run)  # New clients attach to the same ordinary owner.
        require(again[0] == fresh[0]
                and again[1]['result']['instanceId'] == fresh[1]['result']['instanceId'])
        history.unchanged()
        print('RESTORE099_OLD_INTENT_ORDINARY_OFF_PASS', flush=True)
    finally:
        for item in held:
            item.close()


def recover_retained():
    """Explicit fresh context for the preserved, stopped OLD/Intent fixture.

    Does not rerun Backup, Import, Pause or Stop, or remove failed evidence.
    The installed recovery consumer must earn all authority again.
    """
    os.umask(0o077)
    uid, run = account_scope()
    config, state = HOME / '.config/omavless', HOME / '.local/state/omavless'
    fields = manager(run)
    require(fields['MainPID'] == '0' and fields['ActiveState'] == 'inactive'
            and fields['SubState'] == 'dead')
    no_network()
    require(set(p.name for p in state.iterdir() if p.name.startswith('restore-'))
            == {'restore-decision.intent', 'restore-pair.pending'})
    held = []
    try:
        for path, limit in ((config / 'profiles.json', 4 * 1024 * 1024),
                            (config / 'route-template.yaml', 1024 * 1024),
                            (state / 'desired.json', 65536), (state / 'ownership.json', 4096),
                            (run / 'omavless-login.receipt', 4096),
                            (state / 'restore-decision.intent', 138)):
            held.append(Held(path, uid, limit))
        stage = state / 'restore-pair.pending'
        for leaf, limit, original in (('old-profiles.json', 4 * 1024 * 1024, held[0]),
                                      ('old-route-template.yaml', 1024 * 1024, held[1])):
            staged = Held(stage / leaf, uid, limit)
            held.append(staged)
            require(staged.data == original.data)
        require(json.loads(held[0].data).get('startup', {}).get('enabled') is False)
        evidence = HOME / '.cache/restore099-pc-phase-successor'
        evidence.mkdir(mode=0o700)
        private = Held(HOME / '.cache/restore099-pc-positive/request.json', uid, 32768)
        held.append(private)
        request = json.loads(private.data, object_pairs_hook=unique)
        require(set(request) == {'schema', 'archive', 'passphrase'}
                and type(request['schema']) is int and request['schema'] == 1
                and request['archive'] == str(HOME / '.cache/restore099-pc-positive/new.ovb')
                and isinstance(request['passphrase'], str))
        effects = Effects(evidence, run)
        effects.once('recover', [str(ELF), 'restore', 'recover-old', '--confirm-rollback'], private.data)
        fresh = ordinary(uid, run)
        for item in held[:5]:
            item.unchanged()
        history = Held(state / 'restore-disposition.history', uid, 8192)
        held.append(history)
        history_matches(history.data, held[5].data, uid, [item.data for item in held[:2]])
        require(set(p.name for p in state.iterdir() if p.name.startswith('restore-'))
                == {'restore-disposition.history'})
        again = ordinary(uid, run)
        require(again[0] == fresh[0]
                and again[1]['result']['instanceId'] == fresh[1]['result']['instanceId'])
        history.unchanged()
        print('RESTORE099_RETAINED_OLD_ORDINARY_OFF_PASS', flush=True)
    finally:
        for item in held:
            item.close()


def main(arguments):
    if arguments == ['recover-retained']:
        recover_retained()
        return
    if arguments == ['positive']:
        positive()
        return
    require(arguments == ["preflight"])
    preflight()
    print("RESTORE099_OBSERVATION_ONLY_OK")
    print("RESTORE099_NO_EFFECTS_REQUESTED")


if __name__ == "__main__":
    try:
        main(sys.argv[1:])
    except Exception:
        print("RESTORE099_OBSERVATION_REFUSED", file=sys.stderr)
        sys.exit(2)
