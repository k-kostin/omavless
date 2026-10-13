# SPDX-License-Identifier: MIT
"""Fixed fresh207 genuine history0 → closure-only NEW → history1 recipe.

SOURCE preparation only. History/closure decoders are comparison DATA, never
authority. No fabricated prior history, reset, retry or cleanup.
"""
import errno
import importlib.util
import json
import os
from pathlib import Path
import sys

_SPEC = importlib.util.spec_from_file_location(
    "_final_new_helpers", Path(__file__).with_name("final_new_recovery.py"))
current = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(current)
old, loss, previous = current.old, current.loss, current.previous
ACCOUNT = "ovrestore207"
HOME = Path("/home/ovrestore207")
EVIDENCE = "restore099-pc-final-history"
COMPLETE = [str(old.ELF), "developer", "restore-current", "--confirm-private-pair"]
COMPLETE_MARKER = b"t4_current_pair_completed\n"


def phase_command(phase, argv, data):
    fixed = {
        "backup": [str(old.ELF), "developer", "backup-current", "--confirm-private-export"],
        "complete": COMPLETE, "cut": current.CUT, "recover": current.RECOVER,
        "restart-stop": ["/usr/bin/systemctl", "--user", "stop", "omavless-runtime.service"],
        "restart-start": ["/usr/bin/systemctl", "--user", "start", "omavless-runtime.service"],
        "stop": ["/usr/bin/systemctl", "--user", "stop", "omavless-runtime.service"],
    }
    if phase in fixed:
        old.require(argv == fixed[phase])
        if phase in ("restart-stop", "restart-start", "stop"):
            old.require(data == b"")
        else:
            value = loss.document(data)
            old.require(set(value) == {"schema", "archive", "passphrase"}
                        and type(value["schema"]) is int and value["schema"] == 1
                        and value["archive"] == str(HOME / ".cache" / EVIDENCE / "new.ovb")
                        and isinstance(value["passphrase"], str)
                        and 12 <= len(value["passphrase"].encode()) <= 1024)
        return
    verbs = {"preset-first": "routing-preset", "import-first": "profile-import",
             "preset-second": "routing-preset", "import-second": "profile-import",
             "mode-direct": "mode", "mode-rule": "mode"}
    verb = verbs.get(phase)
    old.require(verb is not None and len(argv) == (7 if verb == "mode" else 6)
                and argv[:3] == [str(old.ELF), "plugin", verb]
                and isinstance(argv[3], str) and 0 < len(argv[3]) <= 128
                and not any(ord(character) < 32 for character in argv[3])
                and isinstance(argv[4], str) and argv[4].isascii() and argv[4].isdecimal()
                and int(argv[4]) <= 9007199254740991
                and argv[5] == "restore099-history-" + phase)
    if verb == "mode":
        old.require(data == b"" and argv[6] == phase.removeprefix("mode-"))


class Effects(loss.CapturingEffects):
    """Each fixed phase once; preserve original captures, children and codes."""
    def __init__(self, directory, run, home):
        old.require(home == HOME)
        super().__init__(directory, run, home)

    def once(self, phase, argv, data=b""):
        old.require(phase not in self.used and len(data) <= 32768)
        phase_command(phase, argv, data)
        self.used.add(phase)
        try:
            print("RESTORE099_PHASE_" + phase.upper().replace("-", "_"), flush=True)
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
            self.capture_read(err)  # Bounded diagnostic DATA, not authority.
        except BaseException:
            old.park(phase.replace("-", "_") + "_child_or_capture_unverified")
            raise
        old.require(self.codes[phase] == 0)
        return raw


def evidence_path(uid):
    parent = HOME / ".cache"
    info = parent.lstat()
    old.require(previous.stat.S_ISDIR(info.st_mode) and info.st_uid == uid
                and previous.stat.S_IMODE(info.st_mode) == 0o700 and parent.resolve() == parent)
    return parent / EVIDENCE


def inventory(config, state, expected):
    current.inventory(config, state, expected)


def terminal(closure):
    receipt = current.record(closure, 290, b"OVRCLS01", b"omavless-restore-closure-v1\0")
    current.record(receipt, 250, b"OVRFIN01", b"omavless-restore-finalization-v1\0")
    value = receipt[8:146]
    loss.decision(value, 2)
    return value


def closure_from_history(raw, uid, generation):
    ticket = current.record(raw, 386, b"OVRDON01", b"omavless-restore-disposition-complete-v1\0")
    current.record(ticket, 346, b"OVRDSP01", b"omavless-restore-disposition-candidate-v1\0")
    closure = ticket[24:-32]
    value = terminal(closure)
    old.require(int.from_bytes(ticket[8:12], "little") == uid and ticket[12:16] == bytes(4)
                and int.from_bytes(ticket[16:24], "little") == generation
                and int.from_bytes(value[9:17], "big") == generation)
    return closure


def attributes(fd):
    # Private bounded xattr comparison, never printed or used as authority.
    names = os.listxattr(fd)
    old.require(len(names) <= 32 and all(0 < len(name.encode()) <= 255 for name in names))
    result, total = {}, 0
    for name in sorted(names):
        value = os.getxattr(fd, name)
        total += len(value)
        old.require(len(value) <= 4096 and total <= 65536)
        result[name] = value
    return result


def history_pin(path, uid, held):
    item = old.Held(path, uid, 386)
    held.append(item)  # Retain before xattr reads/postchecks can fail.
    item.unchanged()
    attrs = attributes(item.fd)
    item.unchanged()
    return item, attrs


def history_unchanged(pin):
    item, attrs = pin
    item.unchanged()
    old.require(attributes(item.fd) == attrs)
    item.unchanged()


def ordinary_started(uid, run, stable):
    # ONE known Type=simple Start succeeded. Poll transport readiness only;
    # semantic Refused, manager/query failure or any different path propagates.
    deadline = old.time.monotonic() + 10
    path = str(run / "omavless/control.sock")
    while True:
        stable()
        old.require(old.time.monotonic() < deadline)
        try:
            result = old.ordinary(uid, run)
        except (FileNotFoundError, ConnectionRefusedError, TimeoutError) as error:
            stable()
            transport = (isinstance(error, FileNotFoundError)
                         and error.errno == errno.ENOENT and error.filename == path)
            transport |= isinstance(error, ConnectionRefusedError) and error.errno == errno.ECONNREFUSED
            transport |= isinstance(error, TimeoutError)
            if not transport:
                raise
            remaining = deadline - old.time.monotonic()
            old.require(remaining > 0)
            old.time.sleep(min(0.1, remaining))
            continue
        stable()
        old.require(old.time.monotonic() <= deadline)
        return result


def profile_id(raw):
    loss.old_roscom(raw)
    value = loss.document(raw)["profiles"][0].get("id")
    old.require(isinstance(value, str) and 0 < len(value) <= 128)
    return value  # Private DATA only, never argv/output.


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
    held, complete, phase = [], False, "preflight"
    try:
        inventory(config, state, ())
        original = old.ordinary(uid, run)
        original_process = old.process_identity(original[0])
        initial_pair = loss._pair(config, uid, held)
        initial = loss.empty_china(initial_pair[0].data)
        request = json.dumps({"schema": 1, "archive": str(evidence / "new.ovb"),
                              "passphrase": os.urandom(24).hex()}).encode()
        loss._private_write(evidence / "request.json", request)
        private = loss._pin(((evidence / "request.json", 32768),), uid, held)[0]
        phase = "backup"
        old.require(effects.once(phase, [str(old.ELF), "developer", "backup-current",
                    "--confirm-private-export"], private.data) == b"t4_current_backup_created\n")
        archive = loss._pin(((evidence / "new.ovb", 7 * 1024 * 1024 + 4096),), uid, held)[0]
        for phase, verb, data in (("preset-first", "routing-preset", b"roscomvpn-default\non"),
                                  ("import-first", "profile-import", b"Loss-cut fixture\n" + loss.SYNTHETIC_URI.encode())):
            owner = old.ordinary(uid, run)
            argv = [str(old.ELF), "plugin", verb, owner[1]["result"]["instanceId"],
                    str(owner[1]["revision"]), "restore099-history-" + phase]
            old.require(old.decode(effects.once(phase, argv, data)).get("ok") is True)
        first_owner = old.ordinary(uid, run)
        old.require(first_owner[0] == original[0]
                    and first_owner[1]["result"]["instanceId"] == original[1]["result"]["instanceId"]
                    and old.process_identity(first_owner[0]) == original_process)
        old1 = loss._pair(config, uid, held)
        first_id = profile_id(old1[0].data)
        first_boundary = loss._boundary(state, run, uid, held)
        inventory(config, state, ())
        phase = "complete"
        old.require(effects.once(phase, COMPLETE, private.data) == COMPLETE_MARKER)
        completed_owner = old.ordinary(uid, run)
        old.require(completed_owner[0] == first_owner[0]
                    and completed_owner[1]["result"]["instanceId"] == first_owner[1]["result"]["instanceId"]
                    and completed_owner[1]["revision"] == first_owner[1]["revision"] + 1
                    and old.process_identity(completed_owner[0]) == original_process)
        new1 = loss._pair(config, uid, held)
        loss.portable_new(new1[0].data, initial)
        old.require(new1[1].data == initial_pair[1].data)
        prior = history_pin(state / previous.HISTORY[0], uid, held)
        generation = loss.document(first_boundary[1].data)["generation"]
        prior_closure = closure_from_history(prior[0].data, uid, generation)
        current.closure_matches(prior_closure, [item.data for item in old1],
                                [item.data for item in new1], first_boundary[0].data, first_boundary[1].data)

        def first_stable():
            inventory(config, state, (previous.HISTORY[0],))
            for item in new1 + first_boundary + [private, archive]:
                item.unchanged()
            for item in old1:
                previous.retired_pin(item)
            history_unchanged(prior)
        first_stable()
        phase = "restart-stop"
        effects.once(phase, ["/usr/bin/systemctl", "--user", "stop", "omavless-runtime.service"])
        fields = old.manager(run)
        old.require(fields["MainPID"] == "0" and fields["ActiveState"] == "inactive"
                    and fields["SubState"] == "dead" and not Path(f"/proc/{completed_owner[0]}").exists())
        first_stable()
        phase = "restart-start"
        effects.once(phase, ["/usr/bin/systemctl", "--user", "start", "omavless-runtime.service"])
        restarted = ordinary_started(uid, run, first_stable)
        old.require(restarted[1]["result"]["instanceId"] != completed_owner[1]["result"]["instanceId"])
        restarted_process = old.process_identity(restarted[0])
        # Fixed disconnected mutations earn a genuinely newer Desired snapshot;
        # prior history keeps its older immutable generation/digest as DATA.
        for phase, mode in (("mode-direct", "direct"), ("mode-rule", "rule")):
            history_unchanged(prior)
            owner = old.ordinary(uid, run)
            argv = [str(old.ELF), "plugin", "mode", owner[1]["result"]["instanceId"],
                    str(owner[1]["revision"]), "restore099-history-" + phase, mode]
            old.require(old.decode(effects.once(phase, argv)).get("ok") is True)
            old.ordinary(uid, run)
            history_unchanged(prior)
        for phase, verb, data in (("preset-second", "routing-preset", b"roscomvpn-default\non"),
                                  ("import-second", "profile-import", b"Loss-cut fixture\n" + loss.SYNTHETIC_URI.encode())):
            history_unchanged(prior)
            owner = old.ordinary(uid, run)
            argv = [str(old.ELF), "plugin", verb, owner[1]["result"]["instanceId"],
                    str(owner[1]["revision"]), "restore099-history-" + phase]
            old.require(old.decode(effects.once(phase, argv, data)).get("ok") is True)
            history_unchanged(prior)
        owner = old.ordinary(uid, run)
        old.require(owner[0] == restarted[0]
                    and owner[1]["result"]["instanceId"] == restarted[1]["result"]["instanceId"]
                    and old.process_identity(owner[0]) == restarted_process)
        old2 = loss._pair(config, uid, held)
        old.require(profile_id(old2[0].data) != first_id)
        boundary = loss._boundary(state, run, uid, held)
        old.require(loss.document(boundary[0].data)["generation"]
                    > loss.document(first_boundary[0].data)["generation"])
        inventory(config, state, (previous.HISTORY[0],))
        history_unchanged(prior)
        for item in old2 + boundary + [private, archive]:
            item.unchanged()
        phase = "cut"
        current.cut_result(effects.once(phase, current.CUT, private.data))
        closure = loss._pin(((state / "restore-closure.complete", 290),), uid, held)[0]
        new2 = loss._pair(config, uid, held)
        loss.portable_new(new2[0].data, initial)
        old.require(new2[1].data == initial_pair[1].data)
        current.closure_matches(closure.data, [item.data for item in old2],
                                [item.data for item in new2], boundary[0].data, boundary[1].data)
        old.require(terminal(closure.data)[90:106] != terminal(prior_closure)[90:106])

        def second_stable():
            inventory(config, state, (previous.HISTORY[0], "restore-closure.complete"))
            history_unchanged(prior)
            for item in new2 + boundary + [private, archive, closure]:
                item.unchanged()
            for item in old2:
                previous.retired_pin(item)
            for item in first_boundary[1:]:
                item.unchanged()
        second_stable()
        previous.paused_owner(uid, run, owner, restarted_process)
        phase = "stop"
        effects.once(phase, ["/usr/bin/systemctl", "--user", "stop", "omavless-runtime.service"])
        fields = old.manager(run)
        old.require(fields["MainPID"] == "0" and fields["ActiveState"] == "inactive"
                    and fields["SubState"] == "dead" and not Path(f"/proc/{owner[0]}").exists())
        second_stable()
        phase = "recover"
        effects.once(phase, current.RECOVER, private.data)
        fresh = old.ordinary(uid, run)
        old.require(fresh[1]["result"]["instanceId"] != owner[1]["result"]["instanceId"])
        history_unchanged(prior)
        appended = history_pin(state / previous.HISTORY[1], uid, held)
        current.history_matches(appended[0].data, closure.data, uid)
        inventory(config, state, previous.HISTORY[:2])
        again = old.ordinary(uid, run)
        old.require(again[0] == fresh[0] and again[1]["result"]["instanceId"] == fresh[1]["result"]["instanceId"])
        for item in new2 + boundary + [private, archive] + first_boundary[1:]:
            item.unchanged()
        for item in old2:
            previous.retired_pin(item)
        history_unchanged(prior)
        history_unchanged(appended)
        complete = True
        try:
            print("RESTORE099_FINAL_HISTORY_NEW_ORDINARY_OFF_PASS", flush=True)
        except BaseException:
            pass
    except BaseException:
        if effects.used:
            old.park("history_" + phase.replace("-", "_") + "_unverified")
        raise
    finally:
        if complete or not effects.used:
            for item in held:
                item.close()
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
