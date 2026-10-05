"""Execute the real shell runner against isolated fake manager/tool processes.

Only fixed path/root/binary-pin substitutions are applied to a private copy.
No system manager, privilege, namespace or network operation is invoked.
"""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SUPPORT = ROOT / "crates/omavless-netguard/tests"
RUNNER = SUPPORT / "support/namespace_filter_vm_fixture.sh"

MOCK = r'''
import json, os, pathlib, sys
base = pathlib.Path(os.environ["CASE_ROOT"])
case = os.environ["CASE_KIND"]
name = pathlib.Path(sys.argv[0]).name
args = sys.argv[1:]
stage = base / "stage"
link = base / "unit-link"
state = base / "started"
mode = "filtered" if link.is_symlink() and pathlib.Path(os.readlink(link)).name == "filtered.service" else "control"
if name == "systemd-detect-virt":
    print("kvm")
elif name == "getcap":
    if case == "caps": print("probe cap_sys_admin=ep")
elif name == "stat":
    fmt = args[1]
    path = pathlib.Path(args[2])
    if fmt == "%u:%g:%a": print("0:0:700")
    elif fmt == "%u:%g:%a:%h":
        modebits = "700" if path.name == "probe" else "600"
        print("0:0:" + modebits + (":2" if case == "hardlink" and path.name == "probe" else ":1"))
    elif fmt == "%d:%i":
        item = path.stat(); print(str(item.st_dev) + ":" + str(item.st_ino))
    else: sys.exit(19)
elif name == "systemctl":
    with (base / "calls").open("a") as log: log.write(json.dumps(args) + "\n")
    if args[0] == "daemon-reload": sys.exit(0)
    if args[0] == "start":
        state.write_text(mode)
        if case == "start-failure": sys.exit(1)
        if case == "second-start-failure" and mode == "filtered": sys.exit(1)
        if case == "replacement":
            link.unlink(); link.symlink_to(base / "foreign")
        if case == "binary-replacement": (stage / "probe").write_bytes(b"replaced")
        if case == "populated":
            group = base / "cgroup"; group.mkdir(); (group / "cgroup.events").write_text("populated 1\n")
        sys.exit(0)
    if args[0] != "show": sys.exit(29)
    prop = args[args.index("-p") + 1]
    values = {
        "LoadState": "loaded" if link.is_symlink() else "not-found",
        "FragmentPath": str(link), "Requires": "sysinit.target system.slice",
        "Conflicts": "shutdown.target", "User": "root", "NoNewPrivileges": "yes",
        "Delegate": "no", "RestrictNamespaces": "yes" if mode == "filtered" else "no",
        "Environment": "OMAVLESS_K1_NAMESPACE_FILTER_FIXTURE=" + mode,
        "PrivateUsers": "no", "PrivatePIDs": "no", "PrivateNetwork": "no",
        "ActiveState": "inactive", "MainPID": "0", "ControlPID": "0",
        "Result": "success", "ExecMainStatus": "0",
        "ExecStart": "{ path=" + str(stage / "probe") + " ; argv[]=" + str(stage / "probe") + " ; ignore_errors=no ; start_time=[n/a] ; stop_time=[n/a] ; pid=0 ; code=(null) ; status=0/0 }",
    }
    if case == "unknown-property" and prop == "RootImage": sys.exit(0)
    if case == "query-failure" and state.exists() and prop == "MainPID": sys.exit(1)
    if case == "live-pid" and state.exists(): values["MainPID"] = "12345"
    if case == "extra-environment": values["PassEnvironment"] = "UNEXPECTED"
    if case.startswith("property-drift:"): values[case.split(":", 1)[1]] = "UNEXPECTED"
    if case == "missing-exec": values["ExecStart"] = ""
    if case == "unknown-filter": values["RestrictNamespaces"] = "unexpected"
    print(prop + "=" + values.get(prop, ""))
else: sys.exit(39)
'''


class NamespaceFilterRunnerTests(unittest.TestCase):
    def execute(self, kind):
        with tempfile.TemporaryDirectory(prefix="ov-filter-mock-") as temp:
            base = Path(temp)
            stage = base / "stage"
            stage.mkdir(mode=0o700)
            probe = stage / "probe"
            probe.write_bytes(b"synthetic bytes never executed")
            probe.chmod(0o700)
            for mode, suffix in [("control", "control"), ("filtered", "fixture")]:
                data = (SUPPORT / f"fixtures/omavless-k1-namespace-filter-{suffix}.service").read_bytes()
                (stage / f"{mode}.service").write_bytes(data)
                (stage / f"{mode}.service").chmod(0o600)
            source = RUNNER.read_text()
            changes = {
                " && $EUID == 0": "",
                "stage=/run/omavless-k1-namespace-filter-fixture": f"stage={stage}",
                "link=/run/systemd/system/$unit": f"link={base / 'unit-link'}",
                "cgroup=/sys/fs/cgroup/system.slice/$unit": f"cgroup={base / 'cgroup'}",
                "probe_sha=b7dc81b89045c591efd375765ddf4fc4792afedf86cbc73c18fd94d1227d7332":
                    "probe_sha=" + hashlib.sha256(probe.read_bytes()).hexdigest(),
            }
            for old, new in changes.items():
                self.assertEqual(source.count(old), 1)
                source = source.replace(old, new)
            script = base / "runner.sh"
            script.write_text(source)
            commands = base / "bin"
            commands.mkdir()
            for name in ["systemctl", "stat", "getcap", "systemd-detect-virt"]:
                path = commands / name
                path.write_text(f"#!{sys.executable}\n" + MOCK)
                path.chmod(0o700)
            # No real systemctl/sudo exists in this child's PATH.
            for name in ["sha256sum", "cut", "ln", "readlink", "unlink", "sleep"]:
                (commands / name).symlink_to(shutil.which(name))
            env = {"PATH": str(commands), "CASE_ROOT": str(base), "CASE_KIND": kind,
                   "OMAVLESS_K1_NAMESPACE_FILTER_VM": "1"}
            result = subprocess.run([shutil.which("bash"), str(script)], env=env,
                                    capture_output=True, text=True, timeout=15)
            link = base / "unit-link"
            calls = (base / "calls").read_text() if (base / "calls").exists() else ""
            self.assertNotIn('"stop"', calls)
            self.assertNotIn('"reset-failed"', calls)
            self.assertTrue(stage.exists())
            return result, link.is_symlink(), calls

    def test_complete_control_and_filtered_success(self):
        result, retained, calls = self.execute("success")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(retained)
        self.assertEqual(calls.count('["start",'), 2)
        self.assertIn("K1_NAMESPACE_FILTER_control_PASS", result.stdout)
        self.assertIn("K1_NAMESPACE_FILTER_filtered_PASS", result.stdout)
        self.assertIn("K1_NAMESPACE_FILTER_VM_PASS", result.stdout)

    def test_uncertainty_retains_unit_and_never_stops_or_claims_success(self):
        for kind in ["start-failure", "second-start-failure", "replacement",
                     "binary-replacement", "populated", "query-failure", "live-pid",
                     "unknown-property", "extra-environment", "missing-exec", "unknown-filter",
                     *["property-drift:" + name for name in ["EnvironmentFiles", "SupplementaryGroups",
                         "RootDirectory", "RootImage", "NetworkNamespacePath", "JoinsNamespaceOf"]]]:
            with self.subTest(kind=kind):
                result, retained, calls = self.execute(kind)
                self.assertNotEqual(result.returncode, 0)
                self.assertTrue(retained)
                self.assertNotIn("K1_NAMESPACE_FILTER_VM_PASS", result.stdout)
                self.assertIn("K1_NAMESPACE_FILTER_NONPASS retained=1", result.stderr)

    def test_unsafe_executable_refuses_before_unit_creation(self):
        for kind in ["caps", "hardlink"]:
            with self.subTest(kind=kind):
                result, retained, calls = self.execute(kind)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(retained)
                self.assertNotIn('["start",', calls)


if __name__ == "__main__":
    unittest.main()
