# SPDX-License-Identifier: MIT
"""Fixed fresh205 closure-only OLD recipe. SOURCE preparation, VM pending.

No initialization, arbitrary path/step/command, reset, retry or cleanup.
Record decoding is comparison DATA, never native recovery authority.
"""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import sys

_SPEC = importlib.util.spec_from_file_location(
    "_loss_helpers", Path(__file__).with_name("loss_recovery.py"))
loss = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(loss)
old = loss.old
ACCOUNT = "ovrestore205"
HOME = Path("/home/ovrestore205")
EVIDENCE = "restore099-pc-final-old"
HISTORY = ("restore-disposition.history",) + tuple(
    "restore-disposition.history." + str(index) for index in range(1, 8))
SLOTS = (".restore-profiles.new", ".restore-template.new",
         ".restore-profiles.old", ".restore-template.old")
CUT = [str(old.ELF), "developer", "abort-current-after-retirement-cut",
       "--confirm-private-abort-retirement-cut"]
RECOVER = [str(old.ELF), "restore", "recover-final-old", "--confirm-disposition"]
CUT_INPUT = b'{"schema":1}'
CUT_MARKER = b"t4_current_abort_retirement_cut_reached_still_fenced\n"


def phase_command(phase, argv, data):
    fixed = {
        "backup": [str(old.ELF), "developer", "backup-current", "--confirm-private-export"],
        "pause": [str(old.ELF), "developer", "pause-current-intent", "--confirm-private-intent-pause"],
        "cut": CUT, "recover": RECOVER,
        "stop": ["/usr/bin/systemctl", "--user", "stop", "omavless-runtime.service"],
    }
    if phase in fixed:
        old.require(argv == fixed[phase])
        if phase == "cut":
            old.require(data == CUT_INPUT)
        elif phase == "stop":
            old.require(data == b"")
        return
    verb = {"preset": "routing-preset", "import": "profile-import"}.get(phase)
    old.require(verb is not None and len(argv) == 6
                and argv[:3] == [str(old.ELF), "plugin", verb]
                and isinstance(argv[3], str) and 0 < len(argv[3]) <= 128
                and not any(ord(character) < 32 for character in argv[3])
                and isinstance(argv[4], str) and argv[4].isascii() and argv[4].isdecimal()
                and int(argv[4]) <= 9007199254740991
                and argv[5] == "restore099-final-old-" + phase)


class Effects(loss.CapturingEffects):
    # Reuse reviewed capture/identity/retention mechanics, with only our fixed
    # seven commands. The original two-mode recipe is not modified or bypassed.
    def __init__(self, directory, run, home):
        old.require(home == HOME)
        super().__init__(directory, run, home)

    def once(self, phase, argv, data=b""):
        old.require(phase not in self.used and len(data) <= 32768)
        phase_command(phase, argv, data)
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
                env={"HOME": str(HOME), "PATH": "/usr/bin", "LC_ALL": "C",
                     "XDG_RUNTIME_DIR": str(self.run)})
            self.children.append(child)
            deadline = old.time.monotonic() + 150
            while child.poll() is None:
                old.require(old.time.monotonic() < deadline)
                self.capture_check(*out)
                self.capture_check(*err)
                old.time.sleep(0.1)
            self.codes[phase] = child.returncode
            raw = self.capture_read(out)
            # Native recovery emits bounded phase diagnostics on stderr even
            # on success. Retain/read them privately; they are not authority.
            self.capture_read(err)
        except BaseException:
            old.park(phase + "_child_or_capture_unverified")
            raise
        old.require(self.codes[phase] == 0)
        return raw


def inventory(config, state, expected):
    old.require({p.name for p in state.iterdir() if p.name.startswith("restore-")}
                == set(expected)
                and not any(p.name.startswith(("restore-", ".restore-")) for p in config.iterdir())
                and loss.absent(state / "routing-preset.pending.json"))
    for name in HISTORY:
        if name not in expected:
            old.require(loss.absent(state / name))
    for name in SLOTS:
        old.require(loss.absent(config / name))


def cut_result(raw):
    old.require(raw == CUT_MARKER)


def evidence_path(uid):
    parent = HOME / ".cache"
    info = parent.lstat()
    old.require(stat.S_ISDIR(info.st_mode) and info.st_uid == uid
                and stat.S_IMODE(info.st_mode) == 0o700 and parent.resolve() == parent)
    return parent / EVIDENCE


def closure_matches(raw, intent, pair):
    def record(value, size, magic, domain):
        old.require(len(value) == size and value[:8] == magic
                    and hashlib.sha256(domain + value[:-32]).digest() == value[-32:])
        return value[8:-32]
    receipt = record(raw, 290, b"OVRCLS01", b"omavless-restore-closure-v1\0")
    record(receipt, 250, b"OVRFIN01", b"omavless-restore-finalization-v1\0")
    terminal = receipt[8:146]
    loss.decision(terminal, 3)
    loss.decision(intent, 1)
    old.require(terminal[9:106] == intent[9:106] and len(pair) == 2)
    for offset, item in zip((146, 182), pair):
        old.require(int.from_bytes(receipt[offset:offset + 4], "big") == len(item)
                    and receipt[offset + 4:offset + 36] == hashlib.sha256(item).digest())


def retired_pin(item):
    # Only expected unlink metadata differs: same held inode/content/mtime,
    # nlink0; named absence is independently checked by inventory.
    current = old.identity(os.fstat(item.fd))
    old.require(current[:5] == item.original[:5] and current[5] == 0
                and current[6:8] == item.original[6:8]
                and os.pread(item.fd, len(item.data) + 1, 0) == item.data)


def paused_state(members, ready, intent, desired, owner, old_pair, initial):
    loss.stage_binding(ready, members)
    loss.decision(intent, 1)
    want, marker = loss.document(desired), loss.document(owner)
    old.require(members[:2] == old_pair and members[0] != members[2]
                and members[1] != members[3]
                and want.get("connected") is False and want.get("profileId") == ""
                and want.get("mode") == "rule" and marker.get("phase") == "rust"
                and type(want.get("generation")) is int
                and type(marker.get("generation")) is int
                and int.from_bytes(intent[9:17], "big") == marker["generation"]
                and int.from_bytes(intent[17:25], "big") == want["generation"]
                and intent[26:58] == hashlib.sha256(desired).digest()
                and intent[58:90] == hashlib.sha256(ready).digest())
    loss.portable_new(members[2], initial)


def paused_owner(uid, run, current, original):
    first = old.reply(run, uid, "system.hello", "final-old-pause-1")
    status = old.reply(run, uid, "status.get", "final-old-pause-status")
    last = old.reply(run, uid, "system.hello", "final-old-pause-2")
    # Retained/fenced owner deliberately withdraws ordinary mutation ownership.
    # This is SAME paused-holder DATA, not an ordinary runtime admission.
    old.require(first[0] == status[0] == last[0] == current[0]
                and all(item[1].get("result", {}).get("runtimeOwnership") is False
                        for item in (first, status, last))
                and status[1].get("result", {}).get("desired") == "disconnected"
                and status[1].get("result", {}).get("actual") == "disconnected"
                and "transition" in status[1].get("result", {})
                and status[1]["result"]["transition"] is None
                and first[1].get("revision") == status[1].get("revision")
                == last[1].get("revision") == current[1].get("revision")
                and first[1].get("result", {}).get("instanceId")
                == last[1].get("result", {}).get("instanceId") == current[1]["result"]["instanceId"])
    fields = old.manager(run)
    old.require(fields["MainPID"] == str(current[0]) and fields["ActiveState"] == "active"
                and old.process_identity(current[0]) == original)
    old.no_network()


def preflight():
    uid, run = old.account_scope(ACCOUNT, HOME)
    held = []
    try:
        old.require(loss.absent(evidence_path(uid)))
        config, state = HOME / ".config/omavless", HOME / ".local/state/omavless"
        inventory(config, state, ())
        pair = loss._pair(config, uid, held)
        loss._boundary(state, run, uid, held)
        loss.empty_china(pair[0].data)
        old.ordinary(uid, run)
        for item in held:
            item.unchanged()
    finally:
        for item in held:
            item.close()


def positive():
    os.umask(0o077)
    uid, run = old.account_scope(ACCOUNT, HOME)
    config, state = HOME / ".config/omavless", HOME / ".local/state/omavless"
    evidence = evidence_path(uid)
    evidence.mkdir(mode=0o700)
    effects = Effects(evidence, run, HOME)
    held, stage_fd = [], None
    complete, phase = False, "preflight"
    try:
        inventory(config, state, ())
        current = old.ordinary(uid, run)
        original = old.process_identity(current[0])
        initial_pair = loss._pair(config, uid, held)
        initial = loss.empty_china(initial_pair[0].data)
        request = json.dumps({"schema": 1, "archive": str(evidence / "new.ovb"),
                              "passphrase": os.urandom(24).hex()}).encode()
        loss._private_write(evidence / "request.json", request)
        private = loss._pin(((evidence / "request.json", 32768),), uid, held)[0]
        phase = "backup"
        effects.once(phase, [str(old.ELF), "developer", "backup-current", "--confirm-private-export"], private.data)
        archive = loss._pin(((evidence / "new.ovb", 7 * 1024 * 1024 + 4096),), uid, held)[0]
        for phase, verb, data in (("preset", "routing-preset", b"roscomvpn-default\non"),
                                  ("import", "profile-import", b"Loss-cut fixture\n" + loss.SYNTHETIC_URI.encode())):
            current = old.ordinary(uid, run)
            argv = [str(old.ELF), "plugin", verb, current[1]["result"]["instanceId"],
                    str(current[1]["revision"]), "restore099-final-old-" + phase]
            old.require(old.decode(effects.once(phase, argv, data)).get("ok") is True)
        current = old.ordinary(uid, run)
        live = loss._pair(config, uid, held)
        loss.old_roscom(live[0].data)
        boundary = loss._boundary(state, run, uid, held)
        phase = "pause"
        old.require(effects.once(phase, [str(old.ELF), "developer", "pause-current-intent",
                    "--confirm-private-intent-pause"], private.data) == b"t4_current_intent_paused\n")
        inventory(config, state, ("restore-pair.pending", "restore-decision.intent"))
        stage = state / "restore-pair.pending"
        before = stage.lstat()
        old.require(stat.S_ISDIR(before.st_mode) and before.st_uid == uid
                    and stat.S_IMODE(before.st_mode) == 0o700)
        stage_fd = os.open(stage, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        stage_id = old.identity(before)
        old.require(old.identity(os.fstat(stage_fd)) == stage_id
                    and set(os.listdir(stage_fd)) == {leaf for leaf, _ in loss.STAGED})
        staged = loss._pin(tuple((stage / leaf, limit) for leaf, limit in loss.STAGED), uid, held)
        intent = loss._pin(((state / "restore-decision.intent", 138),), uid, held)[0]
        paused_state([item.data for item in staged[:4]], staged[4].data, intent.data,
                     boundary[0].data, boundary[1].data, [item.data for item in live], initial)
        old.require(staged[3].data == initial_pair[1].data)
        for item in staged + [intent] + live + boundary + [private, archive]:
            item.unchanged()
        paused_owner(uid, run, current, original)
        phase = "cut"
        cut_result(effects.once(phase, CUT, CUT_INPUT))
        closure = loss._pin(((state / "restore-closure.complete", 290),), uid, held)[0]
        closure_matches(closure.data, intent.data, [item.data for item in live])

        def cut_unchanged():
            inventory(config, state, ("restore-closure.complete",))
            for item in live + boundary + [private, archive, closure]:
                item.unchanged()
            for item in staged + [intent]:
                retired_pin(item)
            info = os.fstat(stage_fd)
            old.require((info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid)
                        == stage_id[:5] and info.st_nlink == 0 and not os.listdir(stage_fd))
        cut_unchanged()
        paused_owner(uid, run, current, original)
        phase = "stop"
        effects.once(phase, ["/usr/bin/systemctl", "--user", "stop", "omavless-runtime.service"])
        fields = old.manager(run)
        old.require(fields["MainPID"] == "0" and fields["ActiveState"] == "inactive"
                    and fields["SubState"] == "dead" and not Path(f"/proc/{current[0]}").exists())
        cut_unchanged()
        phase = "recover"
        effects.once(phase, RECOVER, private.data)
        fresh = old.ordinary(uid, run)
        old.require(fresh[1]["result"]["instanceId"] != current[1]["result"]["instanceId"])
        for item in live + boundary + [private, archive]:
            item.unchanged()
        history = loss._pin(((state / HISTORY[0], 8192),), uid, held)[0]
        old.history_matches(history.data, intent.data, uid, [item.data for item in live], 3)
        old.require(history.data[32:322] == closure.data)
        inventory(config, state, (HISTORY[0],))
        again = old.ordinary(uid, run)
        old.require(again[0] == fresh[0] and again[1]["result"]["instanceId"] == fresh[1]["result"]["instanceId"])
        for item in live + boundary + [private, archive, history]:
            item.unchanged()
        complete = True
        try:
            print("RESTORE099_FINAL_OLD_ORDINARY_OFF_PASS", flush=True)
        except BaseException:
            pass
    except BaseException:
        if effects.used:
            old.park("final_old_" + phase + "_unverified")
        raise
    finally:
        if complete or not effects.used:
            for item in held:
                item.close()
            if stage_fd is not None:
                os.close(stage_fd)
            effects.close_completed()


def main(arguments):
    old.require(arguments in (["preflight"], ["positive"]))
    if arguments == ["positive"]:
        positive()
    else:
        preflight()
        print("RESTORE099_OBSERVATION_ONLY_OK")
        print("RESTORE099_NO_EFFECTS_REQUESTED")


if __name__ == "__main__":
    try:
        main(sys.argv[1:])
    except Exception:
        print("RESTORE099_OBSERVATION_REFUSED", file=sys.stderr)
        sys.exit(2)
