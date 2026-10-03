# SPDX-License-Identifier: MIT
"""Pure/private-files guards. Never invokes namespaces, daemons or host tools."""
import importlib.util
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET

ROOT = Path(__file__).parent / "real_resolved_binary"
SPEC = importlib.util.spec_from_file_location("real_resolved_binary", ROOT / "probe.py")
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)


class Guards(unittest.TestCase):
    def setUp(self):
        probe.UNSETTLED.clear()

    def tearDown(self):
        probe.UNSETTLED.clear()

    def test_unknown_child_destructor_has_no_hidden_poll(self):
        child = object.__new__(probe.OwnedProcess)
        with patch.object(probe.OwnedProcess, "_internal_poll") as poll, \
             patch.object(probe.os, "waitpid") as reap:
            child.__del__()
            poll.assert_not_called()
            reap.assert_not_called()

    def test_first_unknown_wait_never_retries_signals_or_reaps(self):
        child = SimpleNamespace(pid=42, returncode=None)
        for error in (ChildProcessError(), OSError(), KeyboardInterrupt()):
            probe.UNSETTLED.clear()
            with patch.object(probe.os, "waitid", side_effect=[error, None]) as observe, \
                 patch.object(probe.os, "waitpid") as reap, \
                 patch.object(probe.os, "killpg") as signal_group, \
                 patch.object(probe.os, "kill") as signal_child:
                with self.assertRaises(probe.Refused):
                    probe.supervise(child, 1)
                with self.assertRaises(probe.Refused):
                    probe.stop(child)
                self.assertEqual(observe.call_count, 1)
                reap.assert_not_called()
                signal_group.assert_not_called()
                signal_child.assert_not_called()

    def test_final_reap_unknown_cannot_be_manufactured_zero(self):
        exited = SimpleNamespace(si_pid=42, si_code=probe.os.CLD_EXITED, si_status=0)
        for result in (ChildProcessError(), OSError(), (0, 0), (41, 0), (42, 256), (42, 0x7f)):
            probe.UNSETTLED.clear()
            child = SimpleNamespace(pid=42, returncode=None)
            with patch.object(probe.os, "waitid", return_value=exited) as observe, \
                 patch.object(probe.os, "waitpid", side_effect=[result, (42, 0)]) as reap, \
                 patch.object(probe.os, "killpg") as signal_group, \
                 patch.object(probe, "live_group", return_value=[]):
                with self.assertRaises(probe.Refused):
                    probe.supervise(child, 1)
                with self.assertRaises(probe.Refused):
                    probe.supervise(child, 1)
                self.assertEqual(observe.call_count, 1)
                self.assertEqual(reap.call_count, 1)
                self.assertEqual(signal_group.call_count, 1)
                self.assertIsNone(child.returncode)

    def test_exact_raw_status_and_nonreaped_anchor(self):
        child = SimpleNamespace(pid=42, returncode=None)
        exited = SimpleNamespace(si_pid=42, si_code=probe.os.CLD_EXITED, si_status=7)
        with patch.object(probe.os, "waitid", return_value=exited), \
             patch.object(probe.os, "waitpid", return_value=(42, 7 << 8)) as reap, \
             patch.object(probe.os, "killpg") as signal_group, \
             patch.object(probe, "live_group", return_value=[]):
            self.assertTrue(probe.supervise(child, 1))
            self.assertEqual(child.returncode, 7)
            reap.assert_called_once_with(42, probe.os.WNOHANG)
            signal_group.assert_called_once_with(42, probe.signal.SIGKILL)

    def test_cancellation_rechecks_anchor_once_and_unknown_is_terminal(self):
        child = SimpleNamespace(pid=42, returncode=None)
        for error in (ChildProcessError(), OSError(), KeyboardInterrupt()):
            probe.UNSETTLED.clear()
            with patch.object(probe, "wait_child", side_effect=KeyboardInterrupt()), \
                 patch.object(probe.os, "waitid", side_effect=[error, None]) as observe, \
                 patch.object(probe.os, "waitpid") as reap, \
                 patch.object(probe.os, "killpg") as signal_group:
                with self.assertRaises(probe.Refused):
                    probe.supervise(child, 1)
                self.assertEqual(observe.call_count, 1)
                reap.assert_not_called()
                signal_group.assert_not_called()

    def test_mapped_inode_replacement_refuses_even_with_identical_bytes(self):
        child = SimpleNamespace(pid=42, returncode=None)
        name = "/usr/lib/synthetic.so"
        inventory = {"elfs": {name: {"resolved_path": name, "sha256": "0" * 64}}}
        replaced = SimpleNamespace(st_mode=probe.stat.S_IFREG | 0o644,
                                   st_dev=probe.os.makedev(8, 1), st_ino=101, st_size=10)
        with patch.object(probe, "child_status", return_value=None), \
             patch.object(probe.Path, "read_text", return_value="1-2 r-xp 0 08:01 100 " + name + "\n"), \
             patch.object(probe.os, "open", return_value=99), \
             patch.object(probe.os, "fstat", return_value=replaced), \
             patch.object(probe.os, "close"), \
             patch.object(probe.hashlib, "file_digest") as digest:
            with self.assertRaisesRegex(probe.Refused, "mapped_object_replaced"):
                probe.verify_loaded(child, inventory)
            digest.assert_not_called()

    def test_wrong_proc_topology_refuses_before_group_inventory(self):
        with patch.object(probe.os, "readlink", return_value="999"), \
             patch.object(probe.os, "getpid", return_value=1), \
             patch.object(probe.Path, "iterdir") as inventory:
            with self.assertRaisesRegex(probe.Refused, "proc_pid_namespace_mismatch"):
                probe.live_group(42)
            inventory.assert_not_called()

    def test_bootstrap_timeout_stops_all_later_helpers_without_group_claim(self):
        child = SimpleNamespace(pid=42, returncode=None)
        with patch.object(probe, "BOOTSTRAP_DIRECT_ONLY", True), \
             patch.object(probe, "OwnedProcess", return_value=child) as spawn, \
             patch.object(probe, "wait_child", return_value=None), \
             patch.object(probe, "live_group") as inventory, \
             patch.object(probe.os, "waitpid") as reap, \
             patch.object(probe.os, "killpg") as signal_group:
            with self.assertRaisesRegex(probe.Refused, "namespace_containment_only"):
                probe.command(["/usr/bin/ip", "-j", "link"])
            with self.assertRaisesRegex(probe.Refused, "owned_state_quarantined"):
                probe.command(["/usr/bin/mount", "--make-rprivate", "/"])
            self.assertEqual(spawn.call_count, 1)
            inventory.assert_not_called()
            reap.assert_not_called()
            signal_group.assert_not_called()

    def test_positive_clean_receipt_uses_actual_newline_notifications(self):
        value = {"stored": False, "received_tun": True, "phase": None, "tun_exists": False,
                 "reset_while_held": True, "owner_pinned": True, "unrelated_preserved": True,
                 "observation": {"servers": [], "extended": [], "domains": [], "route": False},
                 "effects": [{"method": "SetLinkDNS", "outcome": "org.freedesktop.DBus.Error.AccessDenied"},
                             {"method": "RevertLink", "outcome": "settled_success"}],
                 "notifications": ["READY=1", "FDSTORE=1\nFDNAME=omavless-tun-lease\nFDPOLL=0",
                    "BARRIER=1", "FDSTOREREMOVE=1\nFDNAME=omavless-tun-lease", "BARRIER=1"]}
        probe.clean(value, "denial")
        value["notifications"] = [line.replace("\n", "\\n") for line in value["notifications"]]
        with self.assertRaisesRegex(probe.Refused, "notification_sequence"):
            probe.clean(value, "denial")

    def test_external_guard_network_comparator_counterexamples(self):
        source = (ROOT / "vm-guard.sh").read_text()
        fragment = source[source.index("timers = "):source.index("for collection in (")]
        def compare(before, after):
            scope = {}
            exec(compile(fragment, "fixed-network-comparator", "exec"), scope)
            scope["compare"](before, after, ("address",))
            return scope["other"]
        baseline = [{"ifname": "eth0", "addr_info": [
            {"family": "inet6", "local": "2001:db8::1", "prefixlen": 64,
             "valid_life_time": 100, "preferred_life_time": 100}]}]
        decreased = json.loads(json.dumps(baseline))
        decreased[0]["addr_info"][0]["valid_life_time"] = 99
        self.assertFalse(compare(baseline, decreased))
        for field, changed in (("valid_life_time", 101), ("valid_life_time", "99"),
                               ("preferred_life_time", -1), ("prefixlen", 63),
                               ("local", "2001:db8::2"), ("family", "inet")):
            candidate = json.loads(json.dumps(baseline))
            candidate[0]["addr_info"][0][field] = changed
            self.assertTrue(compare(baseline, candidate))
        self.assertTrue(compare(baseline, []))
        self.assertTrue(compare(baseline, baseline + baseline))
        self.assertIn("'route6', 'rule6'", source)
        self.assertEqual(source.count("check_category "), 8)

    def test_new_epoch_guard_refuses_boot_pid_reuse_and_replaced_executable(self):
        source = (ROOT / "vm-guard-epoch938.sh").read_text()
        fragment = source[source.index("from pathlib import Path"):source.index("\nPY\n}")]
        boot = "9cdd6950-1655-495b-a52e-0f8f14200d19"
        for actual_boot, ticks, inode, passed in ((boot, 1901, 292400, True),
                ("other-boot", 1901, 292400, False), (boot, 1902, 292400, False),
                (boot, 1901, 292401, False)):
            fields = ["S"] + ["0"] * 18 + [str(ticks)]
            with patch.object(probe.Path, "read_text", side_effect=[actual_boot,
                    "938 (synthetic) " + " ".join(fields)]), \
                 patch.object(probe.os, "stat", return_value=SimpleNamespace(st_dev=31, st_ino=inode)), \
                 patch("builtins.print"):
                if passed:
                    exec(compile(fragment, "fixed-epoch-guard", "exec"), {})
                else:
                    with self.assertRaises(AssertionError):
                        exec(compile(fragment, "fixed-epoch-guard", "exec"), {})

    def test_exact_three_maps_and_no_subordinate_alias(self):
        valid = "0 1000 1\n974 100001 1\n1000 100000 1\n"
        probe.validate_maps(valid, valid, "allow\n")
        for altered in (valid.replace("974 100001 1\n", ""),
                        valid.replace("974 100001", "974 100000"),
                        valid.replace("100000 1", "100000 2"), "0 0 4294967295\n"):
            with self.assertRaises(probe.Refused):
                probe.validate_maps(altered, valid, "allow")
            with self.assertRaises(probe.Refused):
                probe.validate_maps(valid, altered, "allow")
        with self.assertRaises(probe.Refused):
            probe.validate_maps(valid, valid, "deny")

    def test_insufficient_or_additional_grants_refuse(self):
        with patch.object(probe.Path, "read_text", return_value="kdk_vm:100000:65536\n"):
            probe.verify_subordinates()
        for content in ("", "kdk_vm:100000:1\n", "kdk_vm:100000:65536\nkdk_vm:200000:1\n"):
            with patch.object(probe.Path, "read_text", return_value=content):
                with self.assertRaises(probe.Refused):
                    probe.verify_subordinates()

    def test_bus_ownership_and_denial_are_separate(self):
        for case in probe.CASES:
            root = ET.fromstring(probe.bus_config(case))
            self.assertEqual(root.findtext("auth"), "EXTERNAL")
            self.assertEqual(root.findtext("listen"), "unix:path=/run/dbus/system_bus_socket")
            self.assertFalse(root.findall("servicedir") + root.findall("include") + root.findall("includedir"))
            owned = {p.attrib["user"]: [a.attrib["own"] for a in p.findall("allow") if "own" in a.attrib]
                     for p in root.findall("policy") if "user" in p.attrib}
            self.assertEqual(owned, {"root": ["org.freedesktop.systemd1"],
                                     "systemd-resolve": ["org.freedesktop.resolve1"]})
            denies = [d.attrib["send_member"] for p in root.findall("policy") for d in p.findall("deny")
                      if "send_member" in d.attrib]
            self.assertEqual(denies, {"denial": ["SetLinkDNS"], "revert-denial": ["RevertLink"]}.get(case, []))
        with self.assertRaises(probe.Refused):
            probe.bus_config("host")

    def test_caps_never_flow_from_resolver_into_broker_or_core(self):
        self.assertEqual(probe.CAP, 0x1000)
        self.assertEqual(probe.RESOLVER_CAPS, 0x2500)
        resolver = probe.resolved_exec()
        self.assertIn("--reuid=974", resolver)
        for name, uid in (("mihomo", 1000), ("omavless-dns-broker", 0), ("host-fixture", 0)):
            argv = probe.cap_exec(name, uid, [])
            self.assertIn("--bounding-set=-all,+net_admin", argv)
            self.assertNotIn("--bounding-set=-all,+setpcap,+net_bind_service,+net_raw", argv)

    def test_postexec_rejects_gained_lost_caps_or_wrong_uid(self):
        child = SimpleNamespace(pid=42, returncode=None)
        def status(uid, caps):
            return "Uid: " + " ".join([str(uid)]*4) + "\nGid: " + " ".join([str(uid)]*4) + "\nNoNewPrivs: 1\n" + "".join(k+": "+format(caps,"x")+"\n" for k in ("CapEff","CapPrm","CapInh","CapBnd","CapAmb"))
        with patch.object(probe, "child_status", return_value=None), patch.object(probe.Path, "read_text", return_value=status(974, 0x2500)):
            probe.verify_child(child, 974, 0x2500)
        for uid, caps in ((0, 0x2500), (974, 0x3500), (974, 0x500), (1000, 0x2500)):
            with patch.object(probe, "child_status", return_value=None), patch.object(probe.Path, "read_text", return_value=status(uid, caps)):
                with self.assertRaises(probe.Refused):
                    probe.verify_child(child, 974, 0x2500)

    def test_unknown_loaded_module_refuses_before_broker_start(self):
        child = SimpleNamespace(pid=42, returncode=None)
        with patch.object(probe, "child_status", return_value=None), patch.object(probe.Path, "read_text", return_value="1-2 r-xp 0 00:00 1 /usr/lib/unreviewed.so\n"):
            with self.assertRaisesRegex(probe.Refused, "unapproved_loaded_elf"):
                probe.verify_loaded(child, {"elfs": {}})

    def test_config_has_no_stub_multicast_or_upstream(self):
        rows = dict(line.split("=", 1) for line in probe.RESOLVED_CONFIG.splitlines() if "=" in line)
        for name in ("DNS", "FallbackDNS", "Domains", "DNSStubListenerExtra"):
            self.assertEqual(rows[name], "")
        for name in ("LLMNR", "MulticastDNS", "DNSStubListener", "DNSSEC", "DNSOverTLS"):
            self.assertEqual(rows[name], "no")

    def test_absence_is_not_reset_or_cleanup(self):
        value = {"stored": False, "received_tun": True, "phase": None, "tun_exists": False,
                 "reset_while_held": False, "owner_pinned": True, "unrelated_preserved": True,
                 "observation": {"servers": [], "extended": [], "domains": [], "route": False},
                 "effects": [{"method": "SetLinkDNS", "outcome": "org.freedesktop.DBus.Error.AccessDenied"},
                             {"method": "RevertLink", "outcome": "settled_success"}]}
        with self.assertRaisesRegex(probe.Refused, "clean_receipt"):
            probe.clean(value, "denial")

    def test_original_frozen_pair_never_relabelled(self):
        inventory = json.loads((ROOT / "guest-inventory.json").read_text())
        self.assertEqual(inventory["elfs"]["/usr/lib/systemd/systemd-resolved"]["sha256"],
                         "feb36cd417f4e6a065222be0f88ce25933244bc22a9f5dd54030f0fa6ac5cfcd")
        self.assertEqual(probe.BROKER, "6126e5b159eb7996cbf8ac6bdb212be3d7b4b12b1809e09e74c19dbc1394001e")
        self.assertEqual(probe.CORE, "3b1da75d3c9fd8440216f9c256c6c59da812faae88debc936f3c72fef9724544")


if __name__ == "__main__":
    unittest.main()
