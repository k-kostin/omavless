"""Source-only fixture admission/package boundary; no manager or netlink calls."""
import configparser
import hashlib
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
CRATE = ROOT / "crates/omavless-netguard"
SPEC = importlib.util.spec_from_file_location("manager_private_guard", CRATE / "tests/support/manager_private_guard.py")
guard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(guard)
STAGE_SPEC = importlib.util.spec_from_file_location("manager_private_stage", CRATE / "tests/support/manager_private_stage.py")
stage_loader = importlib.util.module_from_spec(STAGE_SPEC)
STAGE_SPEC.loader.exec_module(stage_loader)


class ManagerPrivateFixture(unittest.TestCase):
    def test_acyclic_artifact_pins(self):
        files = {'guard.py': CRATE / 'tests/support/manager_private_guard.py',
                 'query-guard.py': CRATE / 'tests/support/generator_filter_guest_guard.py',
                 'fixture.service': CRATE / 'tests/fixtures/omavless-k1-manager-private-lifecycle.service'}
        for name, path in files.items():
            self.assertEqual(stage_loader.MEMBERS[name][0], hashlib.sha256(path.read_bytes()).hexdigest())
        self.assertEqual(stage_loader.MEMBERS['probe'][0], guard.PROBE_SHA)
        self.assertEqual(stage_loader.MEMBERS['query-guard.py'][0], guard.QUERY_SHA)
        self.assertEqual(stage_loader.MEMBERS['fixture.service'][0], guard.UNIT_SHA)
        self.assertEqual(stage_loader.DESTINATION, guard.STAGE)
        source = (CRATE / 'tests/support/manager_private_stage.py').read_text()
        self.assertIn("['/usr/bin/python3', '-I', '-B', str(DESTINATION / 'guard.py')]", source)
        self.assertIn("{'PATH': '/usr/bin', 'LC_ALL': 'C', 'OMAVLESS_K1_MANAGER_PRIVATE_GUARD': '1'}", source)

    def test_loader_retained_exact_fd_bytes_refuse_hash_mode_symlink_and_hardlink(self):
        # Actual ordinary uid1000 files: no privileged staging or manager calls.
        if os.getuid() != 1000 or os.getgid() != 1000:
            self.skipTest('retained loader source artifacts require uid/gid1000')
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            path = base / 'probe'
            path.write_bytes(b'fixture')
            path.chmod(0o500)
            digest = hashlib.sha256(b'fixture').hexdigest()
            fd = os.open(base, os.O_RDONLY | os.O_DIRECTORY)
            try:
                self.assertEqual(stage_loader.retained(fd, 'probe', digest, 0o500), b'fixture')
                for sha, mode in [('0' * 64, 0o500), (digest, 0o400)]:
                    with self.assertRaises(RuntimeError):
                        stage_loader.retained(fd, 'probe', sha, mode)
                (base / 'symlink').symlink_to(path)
                with self.assertRaises(OSError):
                    stage_loader.retained(fd, 'symlink', digest, 0o500)
                os.link(path, base / 'hardlink')
                with self.assertRaises(RuntimeError):
                    stage_loader.retained(fd, 'probe', digest, 0o500)
            finally:
                os.close(fd)

    def test_fixed_unit_has_no_automatic_kill_restart_or_namespace_join(self):
        parser = configparser.ConfigParser(interpolation=None, strict=True)
        parser.optionxform = str
        parser.read(CRATE / "tests/fixtures/omavless-k1-manager-private-lifecycle.service")
        self.assertEqual(parser.sections(), ["Unit", "Service"])
        self.assertEqual(dict(parser.defaults()), {})
        self.assertEqual(dict(parser["Unit"]), {
            "Description": "OmaVLESS K1 isolated manager-created lifecycle fixture",
        })
        self.assertEqual(dict(parser["Service"]), {
            "Type": "oneshot", "RemainAfterExit": "yes", "User": "root", "Group": "root",
            "SupplementaryGroups": "",
            "ExecStart": "/run/omavless-k1-manager-private-lifecycle/probe --exact kernel_observer::creator_lifecycle::manager_private::manager_private_lifecycle --ignored --nocapture --test-threads=1",
            "Environment": "OMAVLESS_K1_MANAGER_PRIVATE=1",
            "OpenFile": "/proc/1/ns/net:k1-host-netns:read-only",
            "PrivateUsers": "no", "PrivatePIDs": "no", "PrivateNetwork": "yes",
            "RestrictNamespaces": "yes", "NoNewPrivileges": "yes",
            "CapabilityBoundingSet": "CAP_NET_ADMIN", "AmbientCapabilities": "",
            "UMask": "0077", "Restart": "no", "TimeoutStartSec": "infinity",
            "TimeoutStopSec": "infinity", "RuntimeMaxSec": "infinity",
            "KillMode": "none", "SendSIGKILL": "no", "StandardInput": "null",
            "StandardOutput": "append:/run/omavless-k1-manager-private-lifecycle/native.stdout",
            "StandardError": "append:/run/omavless-k1-manager-private-lifecycle/native.stderr",
        })

    def test_native_gate_precedes_socket_and_uses_existing_creator(self):
        text = (CRATE / "src/kernel_manager_private_fixture.rs").read_text()
        body = text[text.index("fn run("):]
        self.assertLess(body.index("held.isolation.recheck()?"), body.index("FixtureCreator::open("))
        for token in ["LockedState::from_root", "RootStateStore::open_test_parent",
                      "inspect_policy_inventory", ".request(ARM", ".request(DISARM",
                      "independent.observe().is_err()", "Box::leak", "catch_unwind",
                      "std::thread::park()", "K1_MANAGER_PRIVATE_UNCERTAIN_RETAINED"]:
            self.assertIn(token, text)
        for forbidden in ["Command::", "unshare(", "setns(&self.host", "remove_dir", "remove_file", "unsafe ", "kill("]:
            self.assertNotIn(forbidden, text)
        observer = (CRATE / "src/kernel_observer.rs").read_text()
        self.assertIn('#[cfg(test)]\n#[path = "kernel_creator_lifecycle.rs"]', observer)
        self.assertIn('#[ignore = "fixed manager-owned PrivateNetwork fixture;', text)

    def test_no_package_or_product_caller(self):
        for path in (ROOT / "packaging").rglob("*"):
            # Python's generated bytecode embeds the checkout directory name;
            # it is not a packaging input or source reference to this fixture.
            if path.is_file() and "__pycache__" not in path.parts:
                self.assertNotIn(b"manager-private-lifecycle", path.read_bytes())
        self.assertNotIn("manager_private", (CRATE / "src/lib.rs").read_text())

    def test_typed_fixed_properties_refuse_omission_extra_command_fd_or_dependency(self):
        obj = object.__new__(guard.Observer)
        obj.properties = lambda names: guard.FIXED.copy()
        good = [guard.TYPED_BYTES, next(iter(guard.DEPENDENCIES))]
        for bad in [b'', guard.TYPED_BYTES.replace(b'a(sasbttttuii) 0', b'a(sasbttttuii) 1', 1),
                    guard.TYPED_BYTES.replace(b'as 0\n', b'as 1 "foreign"\n'),
                    guard.TYPED_BYTES.replace(b'(bas) false 0', b'(bas) true 0'),
                    guard.TYPED_BYTES.replace(b'"k1-host-netns" 1', b'"k1-host-netns" 9')]:
            replies = iter([bad, good[1]])
            obj.call = lambda _: next(replies)
            with self.assertRaises(RuntimeError):
                obj.fixed_properties()
        for dependencies in [b'as 0\n', b'as 2 "system.slice" "system.slice"\n',
                             b'as 3 "system.slice" "sysinit.target" "foreign.service"\n']:
            replies = iter([good[0], dependencies])
            obj.call = lambda _: next(replies)
            with self.assertRaises(RuntimeError):
                obj.fixed_properties()
        for dependencies in guard.DEPENDENCIES:
            replies = iter([good[0], dependencies])
            obj.call = lambda _: next(replies)
            obj.fixed_properties()
        for field in guard.FIXED:
            value = dict(guard.FIXED)
            del value[field]
            obj.properties = lambda names: value
            with self.assertRaises(RuntimeError):
                obj.fixed_properties()

    def test_real_wait_state_refuses_live_success_failed_and_unknown_without_retry(self):
        obj = object.__new__(guard.Observer)
        obj.recheck = lambda **_: None
        empty = []
        obj.empty_cgroup = lambda: empty.append(True)
        good = dict(ActiveState='active', SubState='exited', MainPID='0', ControlPID='0',
                    ExecMainPID='42', ExecMainCode='1', ExecMainStatus='0', Result='success', ControlGroup='')
        obj.properties = lambda _: good
        self.assertEqual(obj.wait_state(('active', 'exited'), {('activating', 'start')}), good)
        self.assertEqual(empty, [True])
        for key, value in [('MainPID', '42'), ('ControlPID', '43'), ('ExecMainCode', '2'),
                           ('ExecMainStatus', '1'), ('ExecMainPID', '0'), ('Result', 'exit-code'),
                           ('ActiveState', 'failed'), ('ControlGroup', '/foreign')]:
            bad = dict(good, **{key: value})
            obj.properties = lambda _: bad
            with self.assertRaises(RuntimeError):
                obj.wait_state(('active', 'exited'), {('activating', 'start')})

    def test_evidence_json_requires_exact_types_and_duplicate_free_fields(self):
        good = {'schema': 1, 'closed': False, 'boot': [49] * 16}
        guard.exact_json(good, good)
        for bad in [{'schema': True, 'closed': False, 'boot': [49] * 16},
                    {'schema': 1, 'closed': 0, 'boot': [49] * 16},
                    {'schema': 1, 'closed': False, 'boot': [49] * 15},
                    dict(good, extra=1)]:
            with self.assertRaises(RuntimeError):
                guard.exact_json(bad, good)
        with self.assertRaises(RuntimeError):
            guard.pairs([('same', 1), ('same', 1)])

    def test_wait_deadline_has_no_stop_or_further_observation(self):
        obj = object.__new__(guard.Observer)
        observed = []
        obj.recheck = lambda **_: observed.append('recheck')
        obj.properties = lambda _: (observed.append('show') or
            dict(ActiveState='activating', SubState='start', Result='success'))
        with patch.object(guard.time, 'monotonic', side_effect=[0, 1, 46]), \
             patch.object(guard.time, 'sleep'):
            with self.assertRaises(RuntimeError):
                obj.wait_state(('active', 'exited'), {('activating', 'start')})
        self.assertEqual(observed, ['recheck', 'show'])

    def test_actual_execute_stops_forever_at_start_or_observation_failure(self):
        # Execute the actual lifecycle, with only external observations and
        # artifact pins substituted. File operations stay in a local temp tree.
        for fault in (None, 'baseline', 'fixed', 'start', 'wait', 'native', 'stop', 'after'):
            with self.subTest(fault=fault), tempfile.TemporaryDirectory() as temp:
                stage = Path(temp) / 'stage'
                stage.mkdir()
                for name in ['guard.py', 'query-guard.py', 'fixture.service', 'probe']:
                    (stage / name).write_bytes(b'synthetic')
                link = Path(temp) / 'unit.service'
                events = []
                def step(name):
                    events.append(name)
                    if name == fault:
                        raise RuntimeError()
                snapshots = []
                def snapshot():
                    step('after' if snapshots else 'baseline')
                    snapshots.append(True)
                    return {'network': {}, 'fixed': True}
                fd = os.open(stage, os.O_RDONLY | os.O_DIRECTORY)
                ns = {'UNCERTAIN': False, 'snapshot': snapshot, 'preserve': lambda a, b: a == b}
                obj = guard.Observer(ns, [], fd)
                obj.recheck = lambda **_: None
                obj.write = lambda name, value: step(name)
                def call(argv):
                    obj.available()
                    if argv == ['/usr/bin/systemd-detect-virt', '--vm']:
                        return b'kvm\n'
                    if 'start' in argv:
                        self.assertIn('--no-block', argv)
                        step('start')
                    elif 'stop' in argv:
                        self.assertIn('--no-block', argv)
                        step('stop')
                    else:
                        step('reload')
                    return b''
                obj.call = call
                obj.properties = lambda names: ({'LoadState': 'not-found'} if list(names) == ['LoadState'] else
                    {'ActiveState': 'inactive', 'SubState': 'dead', 'MainPID': '0', 'ControlPID': '0'})
                obj.fixed_properties = lambda **_: step('fixed')
                obj.native_evidence = lambda: step('native')
                obj.wait_state = lambda *args: (step('wait') or {'synthetic': True})
                try:
                    with patch.object(guard, 'STAGE', stage), patch.object(guard, 'LINK', link), \
                         patch.object(guard, 'CGROUP', Path(temp) / 'absent-cgroup'):
                        if fault is None:
                            obj.execute()
                            self.assertIn('result.json', events)
                            self.assertFalse(link.exists())
                            self.assertLess(events.index('baseline-before.json'), events.index('start'))
                            self.assertLess(events.index('native'), events.index('stop'))
                        else:
                            with self.assertRaises(RuntimeError):
                                obj.execute()
                            count = len(events)
                            with self.assertRaises(RuntimeError):
                                obj.execute()
                            self.assertEqual(len(events), count)
                            self.assertTrue(obj.sealed)
                            self.assertNotIn('result.json', events)
                            if fault in ('baseline', 'fixed', 'start', 'wait', 'native'):
                                self.assertNotIn('stop', events)
                                self.assertNotIn('after', events)
                finally:
                    os.close(fd)


if __name__ == "__main__":
    unittest.main()
