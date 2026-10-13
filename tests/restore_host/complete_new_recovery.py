# SPDX-License-Identifier: MIT
"""Fixed fresh209 Committed all-three Complete NEW recipe. SOURCE, VM pending.

No stage/Intent/nonce custody is claimed: reconstructed identity is comparison
DATA only. Only the native producer and consumer authenticate the archive.
No initialization, arbitrary selector, retry, reset or cleanup.
"""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import sys

_SPEC = importlib.util.spec_from_file_location(
    "_complete_shared_helpers", Path(__file__).with_name("final_new_recovery.py"))
shared = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(shared)
previous, loss, old = shared.previous, shared.loss, shared.old
ACCOUNT = "ovrestore209"
HOME = Path("/home/ovrestore209")
EVIDENCE = "restore099-pc-complete-new"
CUT = [str(old.ELF), "developer", "cut-current-committed-complete",
       "--confirm-private-committed-complete-cut"]
RECOVER = [str(old.ELF), "restore", "recover-complete-new", "--confirm-disposition"]
CUT_MARKER = b"t4_current_committed_complete_cut_reached_still_fenced\n"


def phase_command(phase, argv, data):
    fixed = {
        "backup": [str(old.ELF), "developer", "backup-current", "--confirm-private-export"],
        "cut": CUT, "recover": RECOVER,
        "stop": ["/usr/bin/systemctl", "--user", "stop", "omavless-runtime.service"],
    }
    if phase in fixed:
        old.require(argv == fixed[phase])
        if phase == "stop":
            old.require(data == b"")
        else:
            request = loss.document(data)
            old.require(set(request) == {"schema", "archive", "passphrase"}
                        and type(request["schema"]) is int and request["schema"] == 1
                        and request["archive"] == str(HOME / ".cache" / EVIDENCE / "new.ovb")
                        and isinstance(request["passphrase"], str)
                        and 12 <= len(request["passphrase"].encode()) <= 1024)
        return
    verb = {"preset": "routing-preset", "import": "profile-import"}.get(phase)
    old.require(verb is not None and len(argv) == 6
                and argv[:3] == [str(old.ELF), "plugin", verb]
                and isinstance(argv[3], str) and 0 < len(argv[3]) <= 128
                and not any(ord(character) < 32 for character in argv[3])
                and isinstance(argv[4], str) and argv[4].isascii() and argv[4].isdecimal()
                and int(argv[4]) <= 9007199254740991
                and argv[5] == "restore099-complete-new-" + phase)


class Effects(loss.CapturingEffects):
    """Reviewed private bounded capture, with this recipe's six fixed phases."""
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
            self.capture_read(err)  # Retained diagnostic DATA, not authority.
        except BaseException:
            old.park(phase + "_child_or_capture_unverified")
            raise
        old.require(self.codes[phase] == 0)
        return raw


def evidence_path(uid):
    parent = HOME / ".cache"
    info = parent.lstat()
    old.require(stat.S_ISDIR(info.st_mode) and info.st_uid == uid
                and stat.S_IMODE(info.st_mode) == 0o700 and parent.resolve() == parent)
    return parent / EVIDENCE


def inventory(config, state, expected):
    reserved = ("restore-", ".restore-")
    old.require({p.name for p in state.iterdir() if p.name.startswith(reserved)} == set(expected)
                and not any(p.name.startswith(reserved) for p in config.iterdir())
                and loss.absent(state / "routing-preset.pending.json"))
    for name in previous.HISTORY:
        if name not in expected:
            old.require(loss.absent(state / name))
    for name in previous.SLOTS:
        old.require(loss.absent(config / name))


def cut_result(raw):
    old.require(raw == CUT_MARKER)


TICKET = "restore-disposition.pending"
COMPLETE = "restore-disposition.complete"


def attributes(fd):
    names = os.listxattr(fd)
    old.require(len(names) <= 32 and all(0 < len(name.encode()) <= 255 for name in names))
    result, total = {}, 0
    for name in sorted(names):
        value = os.getxattr(fd, name)
        total += len(value)
        old.require(len(value) <= 4096 and total <= 65536)
        result[name] = value
    return result


def ticket_matches(raw, closure, desired, owner, uid):
    # Exact source-model comparison DATA, never authentication or permission.
    shared.record(raw, 346, b"OVRDSP01", b"omavless-restore-disposition-candidate-v1\0")
    receipt = shared.record(closure, 290, b"OVRCLS01", b"omavless-restore-closure-v1\0")
    shared.record(receipt, 250, b"OVRFIN01", b"omavless-restore-finalization-v1\0")
    terminal = receipt[8:146]
    loss.decision(terminal, 2)
    want, marker = loss.document(desired), loss.document(owner)
    old.require(type(marker.get("generation")) is int and marker.get("phase") == "rust"
                and type(want.get("generation")) is int and want.get("connected") is False
                and want.get("profileId") == "" and want.get("mode") == "rule"
                and int.from_bytes(raw[8:12], "little") == uid and raw[12:16] == bytes(4)
                and int.from_bytes(raw[16:24], "little") == marker["generation"]
                and raw[24:-32] == closure
                and int.from_bytes(terminal[9:17], "big") == marker["generation"]
                and int.from_bytes(terminal[17:25], "big") == want["generation"]
                and terminal[26:58] == hashlib.sha256(desired).digest())


def pinned_unchanged(item, attrs):
    item.unchanged()
    old.require(attributes(item.fd) == attrs)
    item.unchanged()


def complete_matches(raw, ticket):
    # Existing E1's nested ticket is DATA, not publication or recovery authority.
    nested = shared.record(raw, 386, b"OVRDON01",
                           b"omavless-restore-disposition-complete-v1\0")
    old.require(nested == ticket)


def renamed_complete(item, history, attrs):
    # Exactly this successful consumer's own rename: SAME original E1 FD,
    # nlink1, all metadata/content preserved except nondecreasing ctime.
    # Do not refresh Held.original, call retired_pin(E1), or recapture history.
    current = old.identity(os.fstat(item.fd))
    old.require(current[:8] == item.original[:8] and current[5] == 1
                and current[8] >= item.original[8]
                and loss.absent(item.path)
                and old.identity(history.lstat()) == current
                and os.pread(item.fd, len(item.data) + 1, 0) == item.data
                and attributes(item.fd) == attrs)
    old.require(old.identity(os.fstat(item.fd)) == current
                and old.identity(history.lstat()) == current)
    return current


def renamed_unchanged(item, history, stamp, attrs):
    # Subsequent reopen observes the SAME post-rename stamp, not another refresh.
    old.require(loss.absent(item.path)
                and old.identity(os.fstat(item.fd)) == stamp
                and old.identity(history.lstat()) == stamp
                and os.pread(item.fd, len(item.data) + 1, 0) == item.data
                and attributes(item.fd) == attrs)
    old.require(old.identity(os.fstat(item.fd)) == stamp
                and old.identity(history.lstat()) == stamp)


def preflight():
    uid, run = old.account_scope(ACCOUNT, HOME)
    held = []
    try:
        old.require(loss.absent(evidence_path(uid)))
        config, state = HOME / ".config/omavless", HOME / ".local/state/omavless"
        inventory(config, state, ())
        pair = loss._pair(config, uid, held)
        loss._boundary(state, run, uid, held)  # Packaged Login MUST exist.
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
                    str(current[1]["revision"]), "restore099-complete-new-" + phase]
            old.require(old.decode(effects.once(phase, argv, data)).get("ok") is True)
        current = old.ordinary(uid, run)
        old_pair = loss._pair(config, uid, held)
        loss.old_roscom(old_pair[0].data)
        old.require(old_pair[1].data != initial_pair[1].data)
        boundary = loss._boundary(state, run, uid, held)
        inventory(config, state, ())
        old.require(old.process_identity(current[0]) == original)
        for item in old_pair + boundary + [private, archive]:
            item.unchanged()
        phase = "cut"
        cut_result(effects.once(phase, CUT, private.data))
        closure = loss._pin(((state / "restore-closure.complete", 290),), uid, held)[0]
        ticket = loss._pin(((state / TICKET, 346),), uid, held)[0]
        existing_complete = loss._pin(((state / COMPLETE, 386),), uid, held)[0]
        ticket_attrs, closure_attrs = attributes(ticket.fd), attributes(closure.fd)
        complete_attrs = attributes(existing_complete.fd)
        ticket.unchanged()
        closure.unchanged()
        existing_complete.unchanged()
        new_pair = loss._pair(config, uid, held)
        loss.portable_new(new_pair[0].data, initial)
        old.require(new_pair[1].data == initial_pair[1].data)
        shared.closure_matches(closure.data, [item.data for item in old_pair],
                        [item.data for item in new_pair], boundary[0].data, boundary[1].data)
        ticket_matches(ticket.data, closure.data, boundary[0].data, boundary[1].data, uid)
        complete_matches(existing_complete.data, ticket.data)

        def cut_unchanged():
            inventory(config, state, ("restore-closure.complete", TICKET, COMPLETE))
            for item in new_pair + boundary + [private, archive]:
                item.unchanged()
            pinned_unchanged(ticket, ticket_attrs)
            pinned_unchanged(closure, closure_attrs)
            pinned_unchanged(existing_complete, complete_attrs)
            for item in old_pair:
                previous.retired_pin(item)
        cut_unchanged()
        previous.paused_owner(uid, run, current, original)
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
        for item in new_pair + boundary + [private, archive]:
            item.unchanged()
        history = state / previous.HISTORY[0]
        rename_stamp = renamed_complete(existing_complete, history, complete_attrs)
        shared.history_matches(existing_complete.data, closure.data, uid)
        complete_matches(existing_complete.data, ticket.data)
        previous.retired_pin(ticket)
        previous.retired_pin(closure)
        old.require(attributes(ticket.fd) == ticket_attrs and attributes(closure.fd) == closure_attrs)
        inventory(config, state, (previous.HISTORY[0],))
        again = old.ordinary(uid, run)
        old.require(again[0] == fresh[0] and again[1]["result"]["instanceId"] == fresh[1]["result"]["instanceId"])
        for item in new_pair + boundary + [private, archive]:
            item.unchanged()
        renamed_unchanged(existing_complete, history, rename_stamp, complete_attrs)
        for item in old_pair:
            previous.retired_pin(item)
        previous.retired_pin(ticket)
        previous.retired_pin(closure)
        old.require(attributes(ticket.fd) == ticket_attrs and attributes(closure.fd) == closure_attrs)
        complete = True
        try:
            print("RESTORE099_COMPLETE_NEW_ORDINARY_OFF_PASS", flush=True)
        except BaseException:
            pass
    except BaseException:
        if effects.used:
            old.park("complete_new_" + phase + "_unverified")
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
