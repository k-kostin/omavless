# SPDX-License-Identifier: MIT
"""Fixed fresh 203 MIXED / 204 COMMITTED recipes; SOURCE preparation only.

Execution needs the prior 202 OLD gate and exact pair/recipe review. No account
setup, install, journal writer, arbitrary fault selector, retry or cleanup.
Stage and history bytes below are comparison data, never effect authority.
"""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import sys

_SPEC = importlib.util.spec_from_file_location(
    "_old_recovery_helpers", Path(__file__).with_name("old_recovery.py"))
old = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(old)

SYNTHETIC_URI = (
    "vless://00000000-0000-4000-8000-000000000001@example.invalid:443"
    "?security=tls&type=tcp")
_SCENARIOS = {
    "mixed": ("ovrestore203", Path("/home/ovrestore203"),
              "restore099-pc-mixed", "cut-current-first-rename",
              "--confirm-private-first-rename-cut",
              b"t4_current_first_rename_cut_reached_still_fenced\n",
              "recover-mixed", "--confirm-rollback", 3),
    "committed": ("ovrestore204", Path("/home/ovrestore204"),
                  "restore099-pc-committed", "cut-current-terminal",
                  "--confirm-private-terminal-cut",
                  b"t4_current_terminal_cut_reached_still_fenced\n",
                  "recover-committed", "--confirm-completion", 2),
}
STORE_LIMIT = 5 * 1024 * 1024
TEMPLATE_LIMIT = 2 * 1024 * 1024
BOUNDARY = (("desired.json", 65536), ("ownership.json", 4096))
STAGED = (("old-profiles.json", STORE_LIMIT),
          ("old-route-template.yaml", TEMPLATE_LIMIT),
          ("new-profiles.json", STORE_LIMIT),
          ("new-route-template.yaml", TEMPLATE_LIMIT), ("ready.bin", 152))


class CapturingEffects(old.Effects):
    """Only this fixed recipe's once-only effects; originals stay retained.

    No command comes from arguments or data. Known exit codes stay distinct
    from uncertain child/capture observations. Neither case authorizes retry.
    """
    def __init__(self, directory, run, home):
        super().__init__(directory, run, home)
        self.files, self.children, self.codes = [], [], {}
        self.capture_fds = []

    @staticmethod
    def capture_shape(info):
        return (info.st_dev, info.st_ino, info.st_mode, info.st_uid,
                info.st_gid, info.st_nlink)

    def capture(self, phase, suffix):
        path = self.directory / (phase + suffix)
        output = os.open(path, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW
                         | os.O_CLOEXEC, 0o600)
        self.capture_fds.append(output)  # Before any postcheck or wrapper.
        original = self.capture_shape(os.fstat(output))
        old.require(stat.S_ISREG(original[2]) and original[3] == os.getuid()
                    and stat.S_IMODE(original[2]) == 0o600 and original[5] == 1)
        self.capture_check(path, output, original)
        return path, output, original

    def capture_check(self, path, output, original):
        old.require(self.capture_shape(os.fstat(output)) == original
                    and self.capture_shape(path.lstat()) == original
                    and os.fstat(output).st_size <= old.MAX_FRAME)

    def capture_read(self, capture):
        path, output, original = capture
        self.capture_check(path, output, original)
        before = old.identity(os.fstat(output))
        raw = os.pread(output, old.MAX_FRAME + 1, 0)
        old.require(len(raw) <= old.MAX_FRAME and len(raw) == before[6]
                    and old.identity(os.fstat(output)) == before
                    and old.identity(path.lstat()) == before)
        return raw

    def once(self, phase, argv, data=b""):
        old.require(phase not in self.used and phase in
                    ("backup", "preset", "import", "cut", "stop", "recover")
                    and len(data) <= 32768)
        phase_command(self.home, phase, argv)
        self.used.add(phase)
        try:
            print("RESTORE099_PHASE_" + phase.upper(), flush=True)
            incoming = old.tempfile.TemporaryFile(dir=self.directory)
            self.files.append(incoming)
            incoming.write(data)
            incoming.seek(0)
            out, err = self.capture(phase, ".stdout"), self.capture(phase, ".stderr")
            child = old.subprocess.Popen(
                argv, stdin=incoming, stdout=out[1], stderr=err[1],
                env={"HOME": str(self.home), "PATH": "/usr/bin", "LC_ALL": "C",
                     "XDG_RUNTIME_DIR": str(self.run)})
            self.children.append(child)  # Before poll/capture/clock checks.
            deadline = old.time.monotonic() + 150
            while child.poll() is None:
                old.require(old.time.monotonic() < deadline)
                self.capture_check(*out)
                self.capture_check(*err)
                old.time.sleep(0.1)
            self.codes[phase] = child.returncode
            raw = self.capture_read(out)
            self.capture_read(err)
        except BaseException:
            old.park(phase + "_child_or_capture_unverified")
            raise
        # A known completed nonzero exit is still recorded. The outer recipe
        # parks the unverified scope; it does not call that a live unknown child.
        old.require(self.codes[phase] == 0)
        return raw

    def close_completed(self):
        for output in self.files:
            output.close()
        for output in self.capture_fds:
            os.close(output)


def absent(path):
    try:
        path.lstat()
    except FileNotFoundError:
        return True
    return False


def scenario(mode):
    old.require(mode in _SCENARIOS)
    return _SCENARIOS[mode]


def commands(mode):
    selected = scenario(mode)
    return ([str(old.ELF), "developer", selected[3], selected[4]],
            [str(old.ELF), "restore", selected[6], selected[7]])


def phase_command(home, phase, argv):
    mode = next((mode for mode, entry in _SCENARIOS.items() if entry[1] == home), None)
    old.require(mode is not None)
    cut, recover = commands(mode)
    fixed = {"backup": [str(old.ELF), "developer", "backup-current",
                        "--confirm-private-export"],
             "cut": cut, "recover": recover,
             "stop": ["/usr/bin/systemctl", "--user", "stop", "omavless-runtime.service"]}
    if phase in fixed:
        old.require(argv == fixed[phase])
        return
    verb = {"preset": "routing-preset", "import": "profile-import"}.get(phase)
    old.require(verb is not None and len(argv) == 6
                and argv[:3] == [str(old.ELF), "plugin", verb]
                and isinstance(argv[3], str) and 0 < len(argv[3]) <= 128
                and not any(ord(character) < 32 for character in argv[3])
                and isinstance(argv[4], str) and argv[4].isascii() and argv[4].isdecimal()
                and int(argv[4]) <= 9007199254740991
                and argv[5] == "restore099-" + mode + "-" + phase)


def cut_result(mode, raw):
    # CLI emits this only after validating sole cutReachedStillFenced:true.
    # Neither terminal publication nor this marker means completed Restore.
    old.require(raw == scenario(mode)[5])


def document(raw):
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=old.unique,
                           parse_constant=lambda _: (_ for _ in ()).throw(old.Refused()))
    except (ValueError, UnicodeError, RecursionError):
        raise old.Refused() from None
    old.require(isinstance(value, dict))
    return value


def empty_china(raw):
    value = document(raw)
    old.require(value.get("version") == 3 and value.get("profiles") == []
                and value.get("subscriptions") == [] and value.get("activeId") == ""
                and value.get("lastId") == ""
                and value.get("routingPreset") == "china-cn-direct"
                and value.get("startup", {}).get("enabled") is False)
    return value


def old_roscom(raw):
    value = document(raw)
    profiles = value.get("profiles")
    old.require(value.get("version") == 3 and isinstance(profiles, list)
                and len(profiles) == 1 and isinstance(profiles[0], dict)
                and profiles[0].get("name") == "Loss-cut fixture"
                and profiles[0].get("uri") == SYNTHETIC_URI
                and value.get("subscriptions") == [] and value.get("activeId") == ""
                and value.get("lastId") == ""
                and value.get("routingPreset") == "roscomvpn-default"
                and value.get("startup", {}).get("enabled") is False)


def portable_new(raw, initial):
    # OpenedBackup.restore_store_off may reserialize its admitted portable copy.
    # Check the independently retained initial document, not Python JSON bytes.
    value = empty_china(raw)
    expected = dict(initial)
    expected["startup"] = dict(initial["startup"], enabled=False)
    old.require(value == expected)


def stage_binding(ready, members):
    old.require(len(ready) == 152 and ready[:8] == b"OVRPAIR1" and len(members) == 4)
    for index, member in enumerate(members):
        old.require(int.from_bytes(ready[8 + index * 4:12 + index * 4], "big")
                    == len(member)
                    and ready[24 + index * 32:56 + index * 32]
                    == hashlib.sha256(member).digest())


def decision(raw, phase):
    old.require(type(phase) is int and phase in (1, 2, 3)
                and len(raw) == 138 and raw[:8] == b"OVRDEC01" and raw[8] == phase
                and raw[25] == 1 and raw[90:106] != bytes(16)
                and hashlib.sha256(b"omavless-restore-decision-v1\0" + raw[:106]).digest()
                == raw[106:])


def cut_state(mode, members, live, intent, terminal, ready, desired, owner):
    stage_binding(ready, members)
    decision(intent, 1)
    want = document(desired)
    marker = document(owner)
    old.require(want.get("connected") is False and want.get("profileId") == ""
                and want.get("mode") == "rule" and marker.get("phase") == "rust"
                and type(want.get("generation")) is int
                and type(marker.get("generation")) is int
                and int.from_bytes(intent[9:17], "big") == marker["generation"]
                and int.from_bytes(intent[17:25], "big") == want["generation"]
                and intent[26:58] == hashlib.sha256(desired).digest()
                and intent[58:90] == hashlib.sha256(ready).digest()
                and members[0] != members[2] and members[1] != members[3])
    if mode == "mixed":
        old.require(terminal is None and live == [members[2], members[1]])
    else:
        old.require(mode == "committed" and terminal is not None
                    and live == [members[2], members[3]])
        decision(terminal, 2)
        old.require(terminal[9:106] == intent[9:106])


def _private_write(path, raw):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW
                 | os.O_CLOEXEC, 0o600)
    with os.fdopen(fd, "wb") as output:
        output.write(raw)
        output.flush()
        os.fsync(output.fileno())


def _pin(paths, uid, held):
    result = []
    for path, limit in paths:
        item = old.Held(path, uid, limit)
        held.append(item)
        result.append(item)
    return result


def _pair(config, uid, held):
    return _pin(((config / "profiles.json", STORE_LIMIT),
                 (config / "route-template.yaml", TEMPLATE_LIMIT)), uid, held)


def _boundary(state, run, uid, held):
    return _pin(tuple((state / leaf, limit) for leaf, limit in BOUNDARY)
                + ((run / "omavless-login.receipt", 4096),), uid, held)


def preflight(mode):
    selected = scenario(mode)
    uid, run = old.account_scope(selected[0], selected[1])
    held = []
    try:
        config, state = selected[1] / ".config/omavless", selected[1] / ".local/state/omavless"
        pair = _pair(config, uid, held)
        _boundary(state, run, uid, held)
        empty_china(pair[0].data)
        old.require(not any(p.name.startswith("restore-") for p in state.iterdir()))
        old.ordinary(uid, run)
        for item in held:
            item.unchanged()
    finally:
        for item in held:
            item.close()


def positive(mode):
    selected = scenario(mode)
    os.umask(0o077)
    uid, run = old.account_scope(selected[0], selected[1])
    home = selected[1]
    config, state = home / ".config/omavless", home / ".local/state/omavless"
    evidence = home / ".cache" / selected[2]
    evidence.mkdir(mode=0o700)  # Any existing/failed scope blocks this recipe.
    effects = CapturingEffects(evidence, run, home)
    held = []
    stage_fd = None
    complete = False
    phase = "preflight"
    try:
        old.require(not any(p.name.startswith("restore-") for p in state.iterdir()))
        current = old.ordinary(uid, run)
        original_process = old.process_identity(current[0])
        initial_pair = _pair(config, uid, held)
        initial = empty_china(initial_pair[0].data)
        request = json.dumps({"schema": 1, "archive": str(evidence / "new.ovb"),
                              "passphrase": os.urandom(24).hex()}).encode()
        _private_write(evidence / "request.json", request)
        private = _pin(((evidence / "request.json", 32768),), uid, held)[0]
        phase = "backup"
        effects.once(phase, [str(old.ELF), "developer", "backup-current",
                             "--confirm-private-export"], private.data)
        archive = _pin(((evidence / "new.ovb", 7 * 1024 * 1024 + 4096),), uid, held)[0]
        current = old.ordinary(uid, run)
        phase = "preset"
        mutation = [str(old.ELF), "plugin", "routing-preset",
                    current[1]["result"]["instanceId"], str(current[1]["revision"]),
                    "restore099-" + mode + "-preset"]
        old.require(old.decode(effects.once(phase, mutation, b"roscomvpn-default\non"))
                    .get("ok") is True)
        current = old.ordinary(uid, run)
        phase = "import"
        mutation = [str(old.ELF), "plugin", "profile-import",
                    current[1]["result"]["instanceId"], str(current[1]["revision"]),
                    "restore099-" + mode + "-import"]
        old.require(old.decode(effects.once(phase, mutation,
                    b"Loss-cut fixture\n" + SYNTHETIC_URI.encode())).get("ok") is True)
        current = old.ordinary(uid, run)
        original_old = _pair(config, uid, held)
        old_roscom(original_old[0].data)
        old.require(original_old[1].data != initial_pair[1].data)
        # Preset/import are finished before these unchanged boundary pins.
        boundary = _boundary(state, run, uid, held)
        cut, recover = commands(mode)
        phase = "cut"
        cut_result(mode, effects.once(phase, cut, private.data))
        stage = state / "restore-pair.pending"
        before = stage.lstat()
        old.require(stat.S_ISDIR(before.st_mode) and before.st_uid == uid
                    and stat.S_IMODE(before.st_mode) == 0o700)
        stage_fd = os.open(stage, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        stage_identity = (before.st_dev, before.st_ino, before.st_mode, before.st_uid, before.st_gid)
        expected_names = {leaf for leaf, _ in STAGED}
        old.require(set(os.listdir(stage_fd)) == expected_names)
        staged = _pin(tuple((stage / leaf, limit) for leaf, limit in STAGED), uid, held)
        members = [item.data for item in staged[:4]]
        old.require(members[:2] == [item.data for item in original_old])
        portable_new(members[2], initial)
        old.require(members[3] == initial_pair[1].data)
        intent = _pin(((state / "restore-decision.intent", 138),), uid, held)[0]
        terminal = None
        if mode == "committed":
            terminal = _pin(((state / "restore-decision.terminal", 138),), uid, held)[0]
        else:
            old.require(absent(state / "restore-decision.terminal"))
        live = _pair(config, uid, held)
        cut_state(mode, members, [item.data for item in live], intent.data,
                  terminal.data if terminal else None, staged[4].data,
                  boundary[0].data, boundary[1].data)

        def cut_unchanged():
            for item in staged + [intent] + ([terminal] if terminal else []) + live + boundary + [private, archive]:
                item.unchanged()
            for info in (os.fstat(stage_fd), stage.lstat()):
                old.require((info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid)
                            == stage_identity)
            old.require(set(os.listdir(stage_fd)) == expected_names)

        cut_unchanged()
        fields = old.manager(run)
        old.require(fields["MainPID"] == str(current[0]) and fields["ActiveState"] == "active"
                    and old.process_identity(current[0]) == original_process)
        old.no_network()
        phase = "stop"
        effects.once(phase, ["/usr/bin/systemctl", "--user", "stop", "omavless-runtime.service"])
        fields = old.manager(run)
        old.require(fields["MainPID"] == "0" and fields["ActiveState"] == "inactive"
                    and fields["SubState"] == "dead" and not Path(f"/proc/{current[0]}").exists())
        cut_unchanged()
        phase = "recover"
        effects.once(phase, recover, private.data)
        fresh = old.ordinary(uid, run)
        old.require(fresh[1]["result"]["instanceId"] != current[1]["result"]["instanceId"])
        # Rollback may replace both live inodes. Compare outcome bytes, never
        # pretend that the producer's displaced OLD live nodes stayed named.
        final_pair = _pair(config, uid, held)
        expected = members[:2] if mode == "mixed" else members[2:]
        old.require([item.data for item in final_pair] == expected)
        if mode == "mixed":
            old_roscom(final_pair[0].data)
        else:
            portable_new(final_pair[0].data, initial)
        for item in boundary + [private, archive]:
            item.unchanged()
        history = _pin(((state / "restore-disposition.history", 8192),), uid, held)[0]
        old.history_matches(history.data, intent.data, uid, expected, selected[8])
        old.require({p.name for p in state.iterdir() if p.name.startswith("restore-")}
                    == {"restore-disposition.history"})
        again = old.ordinary(uid, run)
        old.require(again[0] == fresh[0]
                    and again[1]["result"]["instanceId"] == fresh[1]["result"]["instanceId"])
        for item in final_pair + boundary + [private, archive, history]:
            item.unchanged()
        complete = True
        try:
            print("RESTORE099_" + mode.upper() + "_ORDINARY_OFF_PASS", flush=True)
        except BaseException:
            pass
    except BaseException:
        if effects.used:
            # A known nonzero exit remains recorded as such in Effects.once;
            # it is not assumed to prove no effect. Retain this unverified
            # scope/pins too. Poll/timeout/output loss keeps its original child.
            old.park(mode + "_" + phase + "_unverified")
        raise
    finally:
        # Park does not return. Only pre-effect refusal or known whole success
        # can drop these read-only pins; no uncertain-capture unwind releases.
        if complete or not effects.used:
            for item in held:
                item.close()
            if stage_fd is not None:
                os.close(stage_fd)
            effects.close_completed()


def main(arguments):
    choices = {"preflight-mixed": (preflight, "mixed"),
               "positive-mixed": (positive, "mixed"),
               "preflight-committed": (preflight, "committed"),
               "positive-committed": (positive, "committed")}
    old.require(len(arguments) == 1 and arguments[0] in choices)
    operation, mode = choices[arguments[0]]
    operation(mode)
    if operation is preflight:
        print("RESTORE099_OBSERVATION_ONLY_OK")
        print("RESTORE099_NO_EFFECTS_REQUESTED")


if __name__ == "__main__":
    try:
        main(sys.argv[1:])
    except Exception:
        print("RESTORE099_OBSERVATION_REFUSED", file=sys.stderr)
        sys.exit(2)
