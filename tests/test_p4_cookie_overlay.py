# SPDX-License-Identifier: MIT
"""Pure source-export/event guards: no Go execution or networking in CI."""
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import time
import unittest
from unittest.mock import MagicMock, patch

spec = importlib.util.spec_from_file_location("p4_cookie_overlay", Path(__file__).parent / "fixtures/p4_awg_peer/run_cookie_overlay.py")
subject = importlib.util.module_from_spec(spec)
spec.loader.exec_module(subject)


def archive(name="device/fixture.go", payload=b"package device\n", kind=tarfile.REGTYPE):
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w") as stream:
        entry = tarfile.TarInfo(name)
        entry.type = kind
        entry.size = len(payload) if kind == tarfile.REGTYPE else 0
        stream.addfile(entry, io.BytesIO(payload) if entry.size else None)
    return output.getvalue()


class CookieOverlayGuards(unittest.TestCase):
    def test_exited_leader_keeps_cleanup_authority_for_stdio_holder(self):
        with tempfile.TemporaryDirectory() as directory:
            pidfile = Path(directory) / "public-child-pid"
            helper = "import time; time.sleep(3)"  # backstop even for a broken cleanup implementation
            leader = "import subprocess,sys; p=subprocess.Popen([sys.executable,'-c',sys.argv[2]]); open(sys.argv[1],'w').write(str(p.pid))"
            real_popen = subprocess.Popen
            owned = []
            def capture(*args, **kwargs):
                process = real_popen(*args, **kwargs)
                owned.append(process)
                return process
            with patch.object(subject.subprocess, "Popen", side_effect=capture):
                with self.assertRaises(ValueError):
                    subject.bounded_command([sys.executable, "-c", leader, str(pidfile), helper], directory, {"PATH": "/usr/bin:/bin"}, 0.25)
            self.assertEqual(len(owned), 1)
            self.assertEqual(owned[0].returncode, 0)  # naturally exited owned leader was reaped
            self.assertTrue(owned[0].stdout.closed); self.assertTrue(owned[0].stderr.closed)
            pid = int(pidfile.read_text())
            deadline = time.monotonic() + 1
            while True:
                try:
                    stat = Path(f"/proc/{pid}/stat").read_text()
                except FileNotFoundError:
                    break
                if stat.rsplit(")", 1)[1].split()[0] == "Z":
                    break  # killed; init owns grandchild reaping
                if time.monotonic() >= deadline:
                    self.fail("owned stdio holder survived group cleanup")
                time.sleep(0.01)

    def test_reaped_before_interrupt_never_signals_recycled_group(self):
        process = MagicMock(returncode=None)
        def reaped_then_interrupted(**_kwargs):
            process.returncode = 0
            raise KeyboardInterrupt
        process.wait.side_effect = reaped_then_interrupted
        selector = MagicMock()
        selector.__enter__.return_value = selector
        selector.get_map.return_value = {}
        with patch.object(subject.subprocess, "Popen", return_value=process), patch.object(subject.selectors, "DefaultSelector", return_value=selector), patch.object(subject.os, "killpg") as kill:
            with self.assertRaises(KeyboardInterrupt): subject.bounded_command(["synthetic"], "/synthetic", {}, 1)
            kill.assert_not_called(); process.poll.assert_not_called()
            process.stdout.close.assert_called_once(); process.stderr.close.assert_called_once()

    def test_clean_status_cannot_substitute_poisoned_checkout_for_export(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, exported = root / "checkout", root / "export"
            source.mkdir(); exported.mkdir()
            (source / "device").mkdir()
            (source / "device/fixture.go").write_bytes(b"poisoned checkout bytes")
            public = archive()
            def fake_git(_source, *args):
                return {"rev-parse": subject.PIN.encode(), "status": b"", "archive": public}[args[0]]
            with patch.object(subject, "git", side_effect=fake_git), patch.object(subject, "ARCHIVE_SHA256", hashlib.sha256(public).hexdigest()):
                checked = subject.verify_source(source)
                proof = subject.export_source(checked, exported)
            self.assertEqual((exported / "device/fixture.go").read_bytes(), b"package device\n")
            self.assertNotEqual((exported / "device/fixture.go").read_bytes(), (source / "device/fixture.go").read_bytes())
            (exported / "device/fixture.go").write_bytes(b"poisoned executed bytes")
            with self.assertRaises(ValueError): subject.verify_export(exported, proof)

    def test_archive_traversal_and_links_refuse_without_escape(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "export"
            target.mkdir()
            for bad in (archive("../escaped"), archive("/escaped"), archive("link", kind=tarfile.SYMTYPE), archive("fifo", kind=tarfile.FIFOTYPE)):
                with self.assertRaises(ValueError): subject.export_source(bad, target)
            self.assertEqual(list(target.iterdir()), [])
            self.assertFalse((Path(directory) / "escaped").exists())

    def test_archive_hash_is_checked_before_extraction(self):
        with patch.object(subject, "git", side_effect=[subject.PIN.encode(), b"", archive(payload=b"changed public source")]):
            with self.assertRaises(ValueError): subject.verify_source(Path("/synthetic"))

    def test_exact_events_refuse_no_tests_skips_failures_or_extra_tests(self):
        package = "github.com/amnezia-vpn/amneziawg-go/v3/device"
        names = ["TestP4CookieUnderload", *(f"TestP4CookieUnderload/trailers={trailers}/disable={disable}" for trailers in ("false", "true") for disable in ("false", "true"))]
        events = [{"Action": "pass", "Package": package, "Test": name} for name in names] + [{"Action": "pass", "Package": package}]
        encoded = lambda items: b"\n".join(json.dumps(item).encode() for item in items)
        subject.verify_events(encoded(events), 1)
        for bad in (events[:-2], events[1:], events + [{"Action": "skip", "Package": package}], events + [{"Action": "fail", "Package": package}], events + [{"Action": "pass", "Package": package, "Test": "TestUnexpected"}], events + events):
            with self.assertRaises(ValueError): subject.verify_events(encoded(bad), 1)


if __name__ == "__main__": unittest.main()
