# SPDX-License-Identifier: MIT
"""Pure guards only: no namespace, broker, TUN, network or host operation."""
import importlib.util
import hashlib
import os
from pathlib import Path
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).parent / "composed_dns_binary/probe.py"
SPEC = importlib.util.spec_from_file_location("composed_dns_binary", SOURCE)
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)


class Guards(unittest.TestCase):
    def test_fixed_enrollment_matches_exact_release_package_policy(self):
        self.assertEqual(probe.decode(probe.RELEASE_ENROLLMENT),
                         {"schema": 1, "uid": 1000, "policy": "meta-ipv4-release-v1"})
        self.assertNotEqual(probe.decode(probe.RELEASE_ENROLLMENT)["policy"],
                            "meta-ipv4-v1")

    def test_maps_require_two_actual_fixed_ids_and_allow(self):
        valid = "0 1000 1\n1000 100000 1\n"
        probe.validate_maps(valid, valid, "allow\n")
        for uid, gid, groups in (("0 1000 1\n", valid, "allow"),
                                 (valid, "0 1000 1\n", "allow"),
                                 (valid, valid, "deny"),
                                 ("0 0 4294967295\n", valid, "allow"),
                                 (valid.replace("100000", "100001"), valid, "allow")):
            with self.assertRaises(probe.Refused):
                probe.validate_maps(uid, gid, groups)

    def test_duplicate_json_never_authorizes(self):
        with self.assertRaises(probe.Refused):
            probe.decode('{"ready":true,"ready":false}')

    def test_capability_boolean_is_not_abi_number(self):
        probe.capabilities({"abi": 1, "ready": True})
        for value in ({"abi": True, "ready": True}, {"abi": 1, "ready": 1},
                      {"abi": 1, "ready": False}, {"abi": 1, "ready": True, "extra": 1}):
            with self.assertRaises(probe.Refused):
                probe.capabilities(value)

    def test_partial_helper_line_has_real_deadline(self):
        read, write = os.pipe()
        try:
            os.write(write, b"partial")
            with os.fdopen(read, "rb", buffering=0) as stream:
                child = SimpleNamespace(stdout=stream, poll=lambda: None)
                started = time.monotonic()
                with self.assertRaises(probe.Refused):
                    probe.bounded_line(child, seconds=0.02)
                self.assertLess(time.monotonic() - started, 1)
        finally:
            os.close(write)

    def test_object_digest_symlink_fifo_and_mode(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "object"
            path.write_bytes(b"synthetic-non-ELF")
            path.chmod(0o600)
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            self.assertEqual(probe.object_bytes(path, digest), b"synthetic-non-ELF")
            with patch.object(probe.os, "listxattr", return_value=["user.unreviewed"]):
                with self.assertRaisesRegex(probe.Refused, "input_xattrs"):
                    probe.object_bytes(path, digest)
            with self.assertRaises(probe.Refused):
                probe.object_bytes(path, "0" * 64)
            path.chmod(0o666)
            with self.assertRaises(probe.Refused):
                probe.object_bytes(path, digest)
            path.chmod(0o600)
            alias = root / "alias"
            alias.symlink_to(path)
            with self.assertRaises(OSError):
                probe.object_bytes(alias, digest)
            fifo = root / "fifo"
            os.mkfifo(fifo)
            with self.assertRaises(probe.Refused):
                probe.object_bytes(fifo, digest)

    def test_namespace_guard_precedes_every_effect(self):
        original = {name: "original" for name in probe.NS}
        with patch.object(probe, "namespace", return_value="original"), \
                patch.object(probe, "command") as effect:
            with self.assertRaises(probe.Refused):
                probe.isolate(original, Path("/unused"), Path("/unused"))
            effect.assert_not_called()

    def test_group_cancellation_precedes_leader_reap(self):
        observed = []
        child = SimpleNamespace(pid=234567, wait=lambda **unused: observed.append("reap"))
        with patch.object(probe.os, "waitid", return_value=object()), \
                patch.object(probe.os, "killpg", side_effect=lambda *unused: observed.append("cancel")), \
                patch.object(probe, "live_group", return_value=[]):
            self.assertTrue(probe.supervise(child, 1))
        self.assertEqual(observed, ["cancel", "reap"])
        observed.clear()
        with patch.object(probe.os, "waitid", return_value=object()), \
                patch.object(probe.os, "killpg", side_effect=lambda *unused: observed.append("cancel")), \
                patch.object(probe, "live_group", side_effect=probe.Refused("uncertain_cleanup")):
            with self.assertRaises(probe.Refused):
                probe.supervise(child, 1)
        self.assertEqual(observed, ["cancel", "reap"])

    def test_no_directory_fd_can_escape_new_root(self):
        with tempfile.TemporaryDirectory() as directory:
            fd = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
            try:
                with self.assertRaisesRegex(probe.Refused, "outside_directory_fd"):
                    probe.no_directory_fds()
            finally:
                os.close(fd)

    def test_unexpected_usr_child_mount_refuses(self):
        probe.no_usr_submounts("1 0 0:1 / /usr ro - tmpfs tmpfs ro\n")
        with self.assertRaisesRegex(probe.Refused, "usr_child_mount"):
            probe.no_usr_submounts("1 0 0:1 / /usr/local rw - tmpfs tmpfs rw\n")

    def test_denial_unavailable_is_unknown_not_a_good_negative(self):
        child = SimpleNamespace(poll=lambda: None)
        with patch.object(probe, "controller", side_effect=ConnectionRefusedError):
            with self.assertRaisesRegex(probe.Refused, "denial_readiness_unknown"):
                probe.denied_readiness(child, [child], seconds=0.025)
        with patch.object(probe, "controller", return_value=(200, b'{"tun":{"omavless-dns-ready":true}}')):
            with self.assertRaisesRegex(probe.Refused, "denied_core_ready"):
                probe.denied_readiness(child, [child], seconds=0.025)
        with patch.object(probe, "controller", return_value=(200, b'{"tun":{"omavless-dns-ready":false}}')):
            self.assertEqual(probe.denied_readiness(child, [child], seconds=0.025), "observed_nonready_bounded")
        self.assertEqual(probe.denied_readiness(SimpleNamespace(poll=lambda: 0), [child]), "known_core_exit")

    def test_cap_exec_is_fixed_and_requires_real_postexec_readback(self):
        command = probe.cap_exec("mihomo", 1000, ["-f", "/home/core/config.yaml"])
        self.assertIn("--bounding-set=-all,+net_admin", command)
        self.assertIn("--clear-groups", command)
        self.assertIn("CapEff", command[-1])
        self.assertIn("CapAmb", command[-1])
        self.assertIn("NoNewPrivs", command[-1])
        for binary, uid in (("/usr/bin/sh", 0), ("mihomo", 2000)):
            with self.assertRaises(probe.Refused):
                probe.cap_exec(binary, uid, [])

    def test_receipts_do_not_confuse_connection_close_and_dns_release(self):
        value = dict(stored=True, received_tun=True, phase="active", tun_exists=True,
                     calls=["dns", "domains", "route"], servers=[[2, [198, 18, 0, 2]]],
                     domains=[[".", True]], route=True)
        probe.active(value)
        with self.assertRaises(probe.Refused):
            probe.clean(value, "success")
        value.update(stored=False, phase=None, tun_exists=False, calls=["dns", "revert"],
                     servers=[], domains=[], route=False,
                     notifications=["READY=1", "FDSTORE=1\nFDNAME=omavless-tun-lease\nFDPOLL=0",
                                    "BARRIER=1", "FDSTOREREMOVE=1\nFDNAME=omavless-tun-lease", "BARRIER=1"])
        probe.clean(value, "denial")
        with self.assertRaises(probe.Refused):
            probe.clean(value, "success")
        value["received_tun"] = False
        with self.assertRaises(probe.Refused):
            probe.clean(value, "denial")

    def test_absent_optin_cannot_start_any_effect(self):
        with patch.object(probe.sys, "argv", ["probe"]), \
                patch.object(probe.subprocess, "Popen") as spawn:
            with self.assertRaises(probe.Refused):
                probe.main()
            spawn.assert_not_called()


if __name__ == "__main__":
    unittest.main()
