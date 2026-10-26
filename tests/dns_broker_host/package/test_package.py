"""No package build/install, root action, host systemd query or real DNS input."""
import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import socket
import stat
import subprocess
import sys
import tarfile
import tempfile
import types
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("dns_package_stage", ROOT / "stage.py")
stage = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(stage)
sys.modules["stage"] = stage
BUILD_SPEC = importlib.util.spec_from_file_location("dns_pair_build", ROOT / "build_pair.py")
build_pair = importlib.util.module_from_spec(BUILD_SPEC)
BUILD_SPEC.loader.exec_module(build_pair)
RELEASE_SPEC = importlib.util.spec_from_file_location(
    "dns_release_stage", ROOT.parents[2] / "packaging/dns/stage.py")
release_stage = importlib.util.module_from_spec(RELEASE_SPEC)
RELEASE_SPEC.loader.exec_module(release_stage)
EMPTY = "LoadState=loaded\nActiveState=inactive\nSubState=dead\nMainPID=0\nNFileDescriptorStore=0\n"


def socket_fixture_parent():
    """Test-only counterpart of tests/support/temp.rs home_parent policy."""
    value = os.environ.get("HOME", "")
    home = Path(value)
    if not home.is_absolute() or home != Path(os.path.normpath(value)):
        raise ValueError("unsafe_test_home")
    parent = home
    for component in (None, ".cache", "ovt"):
        if component is not None:
            parent = parent / component
            try:
                parent.mkdir(mode=0o700)
            except FileExistsError:
                pass
        info = parent.lstat()
        if (not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid()
                or info.st_mode & 0o022
                or (component == "ovt" and stat.S_IMODE(info.st_mode) != 0o700)):
            raise ValueError("unsafe_test_home")
    return parent


class SocketFixtureTests(unittest.TestCase):
    def test_home_parent_refuses_unsafe_nodes_without_repair(self):
        from types import SimpleNamespace
        good = SimpleNamespace(st_uid=os.getuid(), st_mode=0o40700)
        cases = [(position, os.getuid(), mode)
                 for mode in (0o120700, 0o100700, 0o40777)
                 for position in range(3)]
        cases += [(position, os.getuid() + 1, 0o40700) for position in range(3)]
        cases.append((2, os.getuid(), 0o40755))
        for position, uid, mode in cases:
            nodes = [good, good, good]
            nodes[position] = SimpleNamespace(st_uid=uid, st_mode=mode)
            with mock.patch.dict(os.environ, HOME="/home/synthetic"), \
                 mock.patch.object(Path, "lstat", side_effect=nodes), \
                 mock.patch.object(Path, "mkdir"), \
                 mock.patch.object(Path, "chmod") as repair:
                with self.assertRaises(ValueError):
                    socket_fixture_parent()
                repair.assert_not_called()


class RustToolchainTests(unittest.TestCase):
    """Real producer selection/receipt logic; compiler subprocesses are inert."""
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="pair-toolchain-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.cargo = self.root / "selected-cargo"
        self.rustc = self.root / "selected-rustc"
        for path in (self.cargo, self.rustc):
            path.write_bytes(b"public inert executable fixture\n")
            path.chmod(0o700)
        self.versions = {
            str(self.cargo): b"cargo 1.98.0 (fixture-cargo 2026-01-01)\n",
            str(self.rustc): b"rustc 1.98.0 (fixture-rustc 2026-01-01)\n",
        }

    def version(self, arguments, **_kwargs):
        self.assertEqual(arguments[1:], ["--version"])
        return types.SimpleNamespace(returncode=0, stdout=self.versions[arguments[0]], stderr=b"")

    def test_explicit_paths_execute_and_query_the_same_pinned_tools(self):
        with mock.patch.object(build_pair.subprocess, "run", side_effect=self.version) as run, \
             mock.patch.dict(os.environ, {"RUSTC":"/wrong/rustc", "RUSTUP_TOOLCHAIN":"nightly",
                                          "RUSTUP_AUTO_INSTALL":"1"}):
            tools = build_pair.reviewed_rust(str(self.cargo), str(self.rustc))
        self.assertEqual(tools["cargo"], str(self.cargo))
        self.assertEqual(tools["rustc"], str(self.rustc))
        self.assertEqual(tools["pin"], "1.98.0")
        for call in run.call_args_list:
            self.assertEqual(call.kwargs["env"]["RUSTC"], str(self.rustc))
            self.assertEqual(call.kwargs["env"]["RUSTUP_TOOLCHAIN"], "1.98.0")
            self.assertEqual(call.kwargs["env"]["RUSTUP_AUTO_INSTALL"], "0")
            self.assertEqual(call.kwargs["env"]["CARGO_NET_OFFLINE"], "true")
            self.assertEqual(call.kwargs["timeout"], 10)
            self.assertEqual(call.kwargs["cwd"], build_pair.REPO)
        self.assertEqual([call.args[0][0] for call in run.call_args_list],
                         [str(self.cargo), str(self.rustc)])

    def test_default_path_supports_pinned_rustup_proxies_without_system_fallback(self):
        for name in ("cargo", "rustc"):
            proxy = self.root / name
            proxy.symlink_to(self.cargo)
            self.versions[str(proxy)] = f"{name} 1.98.0 (fixture 2026-01-01)\n".encode()
        with mock.patch.object(build_pair.shutil, "which", side_effect=lambda name: str(self.root / name)) as which, \
             mock.patch.object(build_pair.subprocess, "run", side_effect=self.version) as run:
            tools = build_pair.reviewed_rust()
        self.assertEqual(which.call_args_list, [mock.call("cargo"), mock.call("rustc")])
        self.assertEqual(tools["cargo"], str(self.root / "cargo"))
        self.assertEqual([call.args[0][0] for call in run.call_args_list],
                         [str(self.root / "cargo"), str(self.root / "rustc")])
        with mock.patch.object(build_pair.shutil, "which", return_value=None), \
             mock.patch.object(build_pair.subprocess, "run") as run:
            with self.assertRaises(stage.Refused):
                build_pair.reviewed_rust()
            run.assert_not_called()

    def test_relative_missing_or_nonexecutable_selector_refuses_before_query(self):
        plain = self.root / "not-executable"
        plain.write_text("public fixture")
        for bad in ("cargo", str(self.root / "absent"), str(plain), str(self.root)):
            with self.subTest(path=bad), mock.patch.object(build_pair.subprocess, "run") as run:
                with self.assertRaises(stage.Refused):
                    build_pair.reviewed_rust(bad, str(self.rustc))
                run.assert_not_called()

    def test_wrong_mixed_malformed_and_failed_versions_refuse(self):
        for tool in (self.cargo, self.rustc):
            name = "cargo" if tool == self.cargo else "rustc"
            original = self.versions[str(tool)]
            for raw in (f"{name} 1.99.1 (different 2026-01-01)\n".encode(),
                        f"{name} 1.98.0-nightly\n".encode(), b"", b"x" * 257,
                        f"{name} 1.98.0\nextra\n".encode(), b"\xff"):
                self.versions[str(tool)] = raw
                with mock.patch.object(build_pair.subprocess, "run", side_effect=self.version):
                    with self.assertRaises(stage.Refused):
                        build_pair.reviewed_rust(str(self.cargo), str(self.rustc))
            self.versions[str(tool)] = original
        for code, stderr in ((1, b""), (0, b"unexpected diagnostic")):
            result = types.SimpleNamespace(returncode=code, stdout=self.versions[str(self.cargo)], stderr=stderr)
            with mock.patch.object(build_pair.subprocess, "run", return_value=result):
                with self.assertRaises(stage.Refused):
                    build_pair.reviewed_rust(str(self.cargo), str(self.rustc))

    def test_missing_or_nonexact_committed_pin_refuses_before_query(self):
        for source in ('', '[toolchain]\nchannel="stable"\n', '[toolchain]\nchannel=198\n'):
            with mock.patch.object(Path, "read_text", return_value=source), \
                 mock.patch.object(build_pair.subprocess, "run") as run:
                with self.assertRaises(stage.Refused):
                    build_pair.reviewed_rust(str(self.cargo), str(self.rustc))
                run.assert_not_called()

    def build_fixture(self, changed_query=None):
        calls, queries = [], []
        output = self.root / "candidate"
        go = self.root / "go"
        go.write_bytes(b"public inert Go fixture")
        header = bytearray(64)
        header[:6] = b"\x7fELF\x02\x01"
        header[18:20] = (62).to_bytes(2, "little")

        def export(_repository, _revision, destination, _diagnostics=False):
            destination.mkdir()
            (destination / "LICENSE").write_text("public fixture license")
            if destination.name == "mihomo":
                (destination / "vendor").mkdir()
                (destination / "vendor/modules.txt").write_text("public fixture vendor")
            if destination.name == "omavless":
                (destination / "Cargo.lock").write_text("public fixture lock")

        def run(arguments, **kwargs):
            calls.append((arguments, kwargs))
            if arguments[:2] == [str(go), "build"]:
                Path(arguments[arguments.index("-o") + 1]).write_bytes(header)
            elif arguments[:2] == [str(self.cargo), "build"]:
                target = Path(kwargs["env"]["CARGO_TARGET_DIR"]) / "release"
                target.mkdir(parents=True)
                (target / "omavless-dns-broker").write_bytes(header)

        def subprocess_run(arguments, **kwargs):
            if arguments[:3] == [str(go), "version", "-m"]:
                return types.SimpleNamespace(returncode=0,
                    stdout=b"path\tgithub.com/metacubex/mihomo\n-tags=with_gvisor\nCGO_ENABLED=0\n", stderr=b"")
            queries.append(arguments[0])
            result = self.version(arguments, **kwargs)
            if len(queries) == changed_query:
                result.stdout = result.stdout.replace(b"fixture", b"changed")
            return result

        with contextlib.ExitStack() as stack:
            for name, value in (("export_git", export), ("command", run),
                ("git_value", lambda _repo, *args: "a" * 40 if args == ("rev-parse", "HEAD") else ""),
                ("reviewed_go", lambda *_args: "go version go1.26.8 linux/amd64")):
                stack.enter_context(mock.patch.object(build_pair, name, side_effect=value))
            stack.enter_context(mock.patch.object(stage, "outside_git_destination", return_value=output))
            stack.enter_context(mock.patch.object(build_pair.os, "uname",
                return_value=types.SimpleNamespace(sysname="Linux", machine="x86_64")))
            stack.enter_context(mock.patch.object(build_pair.subprocess, "run", side_effect=subprocess_run))
            if changed_query is not None:
                with self.assertRaises(stage.Refused):
                    build_pair.build(self.root, self.root, str(go), "x86_64", output,
                                     cargo=str(self.cargo), rustc=str(self.rustc))
                self.assertFalse(output.exists())
            else:
                receipt = build_pair.build(self.root, self.root, str(go), "x86_64", output,
                                           cargo=str(self.cargo), rustc=str(self.rustc))
                self.assertEqual(receipt["cargo_version"], self.versions[str(self.cargo)].decode().strip())
                self.assertEqual(receipt["rustc_version"], self.versions[str(self.rustc)].decode().strip())
                self.assertEqual(receipt, json.loads((output / "source-receipt.json").read_text()))
                self.assertEqual(set(receipt), stage.pair_policy("experimental")[0])
        return calls, queries

    def test_actual_builder_uses_selected_rustc_and_receipts_the_same_tools(self):
        calls, queries = self.build_fixture()
        cargo = [(args, options) for args, options in calls if "build" in args and args[0] == str(self.cargo)]
        self.assertEqual(len(cargo), 1)
        args, options = cargo[0]
        self.assertEqual(args[1:], ["build", "--release", "--locked", "--offline",
                                   "-p", "omavless-dns-broker", "--bin", "omavless-dns-broker"])
        self.assertEqual(options["env"]["RUSTC"], str(self.rustc))
        self.assertEqual(options["env"]["RUSTUP_TOOLCHAIN"], "1.98.0")
        self.assertEqual(options["env"]["RUSTUP_AUTO_INSTALL"], "0")
        self.assertEqual(queries, [str(self.cargo), str(self.rustc)] * 3)
        self.assertEqual(build_pair.source_phase(args), "broker-build")

    def test_tool_drift_before_or_after_cargo_refuses_receipt_publication(self):
        for query in (3, 5):
            with self.subTest(query=query):
                calls, _queries = self.build_fixture(changed_query=query)
                cargo_builds = [args for args, _ in calls if args[:2] == [str(self.cargo), "build"]]
                self.assertEqual(len(cargo_builds), 0 if query == 3 else 1)

    def test_wrong_compiler_refuses_before_build_prefix_or_source_export(self):
        self.versions[str(self.rustc)] = b"rustc 1.99.1 (different 2026-01-01)\n"
        with mock.patch.object(stage, "outside_git_destination", return_value=self.root / "candidate"), \
             mock.patch.object(build_pair, "git_value", side_effect=lambda _repo, *args: "a" * 40 if args == ("rev-parse", "HEAD") else ""), \
             mock.patch.object(build_pair, "reviewed_go", return_value="go version go1.26.8 linux/amd64"), \
             mock.patch.object(build_pair.os, "uname", return_value=types.SimpleNamespace(sysname="Linux", machine="x86_64")), \
             mock.patch.object(build_pair.subprocess, "run", side_effect=self.version), \
             mock.patch.object(build_pair.tempfile, "TemporaryDirectory") as scratch, \
             mock.patch.object(build_pair.tempfile, "mkdtemp") as retained, \
             mock.patch.object(build_pair, "export_git") as export:
            with self.assertRaises(stage.Refused):
                build_pair.build(self.root, self.root, "/fixture/go", "x86_64", self.root / "candidate",
                                 cargo=str(self.cargo), rustc=str(self.rustc))
            scratch.assert_not_called()
            retained.assert_not_called()
            export.assert_not_called()

    def test_phase_classification_accepts_selected_tool_without_relabeling_core(self):
        for path in ("/usr/bin/cargo", "/home/fixture/toolchain/bin/cargo", str(self.cargo)):
            args = [path, "build", "--release", "-p", "omavless-dns-broker", "--bin", "omavless-dns-broker", "--features", "release-package"]
            self.assertEqual(build_pair.source_phase(args), "broker-build")
        self.assertEqual(build_pair.source_phase(["/fixture/go", "build", "-o", "core", "."]), "core-build")
        self.assertEqual(build_pair.source_phase([str(self.cargo), "build", "-p"]), "core-build")

    def test_cli_passes_only_explicit_tool_selectors_to_the_producer(self):
        arguments = ["build_pair.py", "--mihomo-git", "/fixture/mihomo",
                     "--sing-tun-git", "/fixture/sing-tun", "--go", "/fixture/go",
                     "--arch", "x86_64", "--output", "/fixture/output",
                     "--cargo", str(self.cargo), "--rustc", str(self.rustc)]
        with mock.patch.object(build_pair.sys, "argv", arguments), \
             mock.patch.object(build_pair.sys, "stdout", io.StringIO()), \
             mock.patch.object(build_pair, "build", return_value={"architecture":"x86_64", "omavless_commit":"a" * 40}) as build:
            build_pair.main()
        build.assert_called_once_with("/fixture/mihomo", "/fixture/sing-tun", "/fixture/go",
            "x86_64", "/fixture/output", "experimental", cargo=str(self.cargo), rustc=str(self.rustc))


class PackageTests(unittest.TestCase):
    def core_build_invocation(self, flavor):
        """Reach the real producer's build call; never invoke a compiler."""
        class CoreBuildReached(Exception):
            pass

        calls = []

        def export(_repository, _revision, destination, _diagnostics=False):
            destination.mkdir()
            if destination.name == "mihomo":
                (destination / "vendor").mkdir()
                (destination / "vendor/modules.txt").write_text("synthetic vendor fixture")

        def run(arguments, **kwargs):
            if "build" in arguments:
                calls.append((arguments, kwargs))
                raise CoreBuildReached
            if kwargs.get("output"):
                kwargs["output"].write_bytes(b"synthetic test receipt\n")

        with contextlib.ExitStack() as stack:
            for name, value in (
                ("export_git", export), ("command", run),
                ("git_value", lambda _repo, *args: "a" * 40 if args == ("rev-parse", "HEAD") else ""),
                ("reviewed_go", lambda *_args: "go version go1.26.8 linux/amd64"),
                ("reviewed_rust", lambda *_args: {}),
                ("qualified_tmpdir", lambda: str(self.root)),
                ("conditional_tests_completed", lambda _raw: None),
                ("tests_completed", lambda *_args: None),
            ):
                stack.enter_context(mock.patch.object(build_pair, name, side_effect=value))
            stack.enter_context(mock.patch.object(build_pair.os, "uname",
                return_value=types.SimpleNamespace(sysname="Linux", machine="x86_64")))
            stack.enter_context(mock.patch.object(build_pair.sys, "stderr", io.StringIO()))
            with self.assertRaises(CoreBuildReached):
                build_pair.build(self.root, self.root, "/reviewed/go", "x86_64",
                                 self.root / ("output-" + flavor), flavor)
        self.assertEqual(len(calls), 1)
        return calls[0]

    def test_normal_release_core_embeds_pinned_upstream_version(self):
        arguments, options = self.core_build_invocation("release")
        self.assertEqual(arguments[:5], ["/reviewed/go", "build", "-mod=vendor",
                                        "-tags=with_gvisor", "-trimpath"])
        self.assertIn("-ldflags=-s -w -X github.com/metacubex/mihomo/constant.Version=v1.19.31",
                      arguments)
        self.assertNotIn("-buildvcs=false", arguments)
        self.assertEqual(arguments[-1], ".")
        self.assertEqual(options["cwd"].name, "mihomo")
        self.assertEqual(options["env"]["CGO_ENABLED"], "0")
        self.assertEqual(options["env"]["GOPROXY"], "off")

    def test_experimental_and_closed_qualified_core_commands_remain_unchanged(self):
        for flavor in ("experimental", "release-close", "release-close-k1"):
            with self.subTest(flavor=flavor):
                arguments, _options = self.core_build_invocation(flavor)
                self.assertIn("-ldflags=-s -w", arguments)
                self.assertFalse(any("constant.Version=" in arg for arg in arguments))
                self.assertEqual("-buildvcs=false" in arguments, flavor != "experimental")
                tags = "with_gvisor,omavless_k1_device" if flavor == "release-close-k1" else "with_gvisor"
                self.assertIn("-tags=" + tags, arguments)

    def test_four_family_requires_actual_device_test_not_zero_case_metadata(self):
        package = next(iter(build_pair.DEVICE_TESTS))
        name = next(iter(build_pair.DEVICE_TESTS[package]))
        events = [{"Action":"start","Package":package},
                  {"Action":"run","Package":package,"Test":name},
                  {"Action":"pass","Package":package,"Test":name},
                  {"Action":"pass","Package":package}]
        wire = lambda xs: b"".join(json.dumps(x).encode()+b"\n" for x in xs)
        build_pair.tests_completed(wire(events),build_pair.DEVICE_TESTS)
        for cut in range(len(events)):
            with self.assertRaises(stage.Refused):
                build_pair.tests_completed(wire(events[:cut]+events[cut+1:]),build_pair.DEVICE_TESTS)
        for bad in (events+events,list(reversed(events)),events[:1]+events[-1:]):
            with self.assertRaises(stage.Refused):
                build_pair.tests_completed(wire(bad),build_pair.DEVICE_TESTS)
        for action in ("skip","fail"):
            changed=[dict(e) for e in events];changed[2]["Action"]=action
            with self.assertRaises(stage.Refused):
                build_pair.tests_completed(wire(changed),build_pair.DEVICE_TESTS)

    def test_four_family_is_exact_and_crosses_no_legacy_or_three_family_consent(self):
        path=self.pair/"source-receipt.json";receipt=json.loads(path.read_text())
        receipt.update(schema=stage.K1_CLOSE_SCHEMA,package_flavor="release-close-k1",broker_feature="k1-managed-device",
            patch_sha256=stage.K1_CLOSE_PATCHES,conditional_close_abi=1,go_cgo=False,go_buildvcs=False,
            go_build_tags="with_gvisor,omavless_k1_device",managed_device="omavless0",
            enrollment_policy="omavless0-ipv4-development-v1",managed_device_source=stage.K1_SOURCE)
        src=self.pair/"corresponding-source.tar.xz";rebuilt=io.BytesIO()
        with tarfile.open(src,"r:xz") as old, tarfile.open(fileobj=rebuilt,mode="w:xz") as new:
            for m in old:new.addfile(m,old.extractfile(m) if m.isfile() else None)
            for name in ("hub/route/connections.go","hub/route/conditional_close_test.go","tunnel/statistic/manager.go",
                "tunnel/statistic/tracker.go","tunnel/statistic/conditional_close_test.go",
                "listener/sing_tun/system_dns_device_k1.go","listener/sing_tun/system_dns_device_legacy.go",
                "listener/sing_tun/system_dns_device_test.go"):
                body=b"public source presence fixture";m=tarfile.TarInfo("mihomo/"+name);m.size=len(body);new.addfile(m,io.BytesIO(body))
        src.write_bytes(rebuilt.getvalue());receipt["sha256"]["corresponding-source.tar.xz"]=hashlib.sha256(rebuilt.getvalue()).hexdigest()
        path.write_text(json.dumps(receipt))
        stage.reviewed_pair(self.pair,"aarch64","a"*40,"release-close-k1")
        with mock.patch.object(stage,"outside_git_destination",side_effect=lambda p:Path(p)):
            release_stage.stage(self.pair,"aarch64","a"*40,self.root/"four","k1-close-qualified")
        for flavor in ("experimental","release","release-close"):
            with self.assertRaises(stage.Refused):stage.reviewed_pair(self.pair,"aarch64","a"*40,flavor)
        for key,bad in (("go_build_tags","with_gvisor"),("broker_feature","release-package"),
            ("managed_device","Meta"),("enrollment_policy","meta-ipv4-release-v1"),
            ("managed_device_source","a"*40),("conditional_close_abi",True)):
            changed=dict(receipt);changed[key]=bad;path.write_text(json.dumps(changed))
            with self.assertRaises(stage.Refused):stage.reviewed_pair(self.pair,"aarch64","a"*40,"release-close-k1")
        self.assertEqual(build_pair.digest(build_pair.PATCHES/"mihomo-k1-device.patch"),stage.K1_PATCH)
        self.assertEqual(stage.pair_policy("release-close")[2],stage.CLOSE_PATCHES)

    def test_qualified_public_source_failure_is_fixed_phase_bounded_and_legacy_quiet(self):
        from types import SimpleNamespace
        result=SimpleNamespace(returncode=1,stderr=b"public compiler error\x1b"+b"x"*20000,stdout=b"public output")
        output=io.StringIO()
        with mock.patch.object(build_pair.subprocess,"run",return_value=result), mock.patch.object(build_pair.sys,"stderr",output):
            with self.assertRaises(stage.Refused):
                build_pair.command(["/usr/bin/cargo","build"],public_diagnostics=True)
        text=output.getvalue()
        self.assertIn("phase=broker-build",text);self.assertNotIn("\x1b",text)
        self.assertIn("diagnostic_truncated",text);self.assertLess(len(text),16550)
        output=io.StringIO()
        with mock.patch.object(build_pair.subprocess,"run",return_value=result), mock.patch.object(build_pair.sys,"stderr",output):
            with self.assertRaises(stage.Refused):build_pair.command(["/usr/bin/cargo","build"])
        self.assertEqual(output.getvalue(),"")
        with mock.patch.object(build_pair.subprocess,"run",side_effect=subprocess.TimeoutExpired([],1,stderr=b"public timeout")), mock.patch.object(build_pair.sys,"stderr",output):
            with self.assertRaises(subprocess.TimeoutExpired):
                build_pair.command(["/usr/bin/go","test"],public_diagnostics=True)
        self.assertIn("phase=dns-tests timeout",output.getvalue())

    def test_qualified_compiler_tmpdir_is_short_private_home_child_without_mutation(self):
        from types import SimpleNamespace
        uid=os.getuid()
        metadata=SimpleNamespace(st_uid=uid,st_mode=0o40700)
        with mock.patch.object(Path,"lstat",return_value=metadata), mock.patch.object(os,"mkdir") as create:
            self.assertEqual(build_pair.qualified_tmpdir({"HOME":"/home/test","TMPDIR":"/home/test/t"}),"/home/test/t")
            for path in ("/tmp/t","relative","/home/test/extra/t","/home/test/"+"x"*40):
                with self.assertRaises(stage.Refused):
                    build_pair.qualified_tmpdir({"HOME":"/home/test","TMPDIR":path})
            create.assert_not_called()
        for mode in (0o40755,0o40777,0o120700,0o100700):
            with mock.patch.object(Path,"lstat",return_value=SimpleNamespace(st_uid=uid,st_mode=mode)):
                with self.assertRaises(stage.Refused):
                    build_pair.qualified_tmpdir({"HOME":"/home/test","TMPDIR":"/home/test/t"})

    def test_distinct_close_pair_mode_never_promotes_legacy_dns_receipt(self):
        path = self.pair / "source-receipt.json"
        receipt = json.loads(path.read_text())
        receipt.update(package_flavor="release", broker_feature="release-package")
        path.write_text(json.dumps(receipt))
        with mock.patch.object(stage, "outside_git_destination", side_effect=lambda p: Path(p)):
            with self.assertRaises(stage.Refused):
                release_stage.stage(self.pair, "aarch64", "a" * 40, self.root / "legacy-release")
            with self.assertRaises(stage.Refused):
                release_stage.stage(self.pair, "aarch64", "a" * 40, self.root / "legacy-close", "close-qualified")
            receipt.update(schema=stage.CLOSE_SCHEMA, package_flavor="release-close",
                           patch_sha256=stage.CLOSE_PATCHES, conditional_close_abi=1,
                           go_cgo=False, go_buildvcs=False)
            # Source-presence fixture only, not actual compilation/attestation.
            source_path=self.pair/"corresponding-source.tar.xz"
            rebuilt=io.BytesIO()
            with tarfile.open(source_path,"r:xz") as old, tarfile.open(fileobj=rebuilt,mode="w:xz") as new:
                for member in old:
                    new.addfile(member,old.extractfile(member) if member.isfile() else None)
                for name in ("hub/route/connections.go","hub/route/conditional_close_test.go",
                             "tunnel/statistic/manager.go","tunnel/statistic/tracker.go",
                             "tunnel/statistic/conditional_close_test.go"):
                    member=tarfile.TarInfo("mihomo/"+name);body=b"public source fixture";member.size=len(body)
                    new.addfile(member,io.BytesIO(body))
            source_path.write_bytes(rebuilt.getvalue())
            receipt["sha256"]["corresponding-source.tar.xz"]=hashlib.sha256(rebuilt.getvalue()).hexdigest()
            path.write_text(json.dumps(receipt))
            with self.assertRaises(stage.Refused):
                release_stage.stage(self.pair, "aarch64", "a" * 40, self.root / "implicit-close")
            release_stage.stage(self.pair, "aarch64", "a" * 40, self.root / "qualified", "close-qualified")
        for key, bad in (("conditional_close_abi", True), ("conditional_close_abi", 2),
                         ("go_cgo", True), ("go_buildvcs", True), ("production_adoption", True)):
            altered = dict(receipt); altered[key] = bad
            path.write_text(json.dumps(altered))
            with self.assertRaises(stage.Refused):
                stage.reviewed_pair(self.pair, "aarch64", "a" * 40, "release-close")
        self.assertEqual(build_pair.digest(build_pair.PATCHES / "mihomo-conditional-close.patch"), stage.CLOSE_PATCH)
        self.assertEqual(len(build_pair.PATCH_SHA), 2)

    def test_conditional_production_test_receipt_requires_every_original_case(self):
        events = []
        for package, names in build_pair.CONDITIONAL_TESTS.items():
            events.append({"Action": "start", "Package": package})
            for name in sorted(names):
                events.extend([{"Action": "run", "Package": package, "Test": name},
                               {"Action": "pass", "Package": package, "Test": name}])
            events.append({"Action": "pass", "Package": package})
        wire = lambda xs: b"".join(json.dumps(x).encode()+b"\n" for x in xs)
        build_pair.conditional_tests_completed(wire(events))
        for cut in range(len(events)):
            with self.assertRaises(stage.Refused):
                build_pair.conditional_tests_completed(wire(events[:cut]+events[cut+1:]))
        for action in ("skip", "fail"):
            changed = [dict(e) for e in events]; changed[2]["Action"] = action
            with self.assertRaises(stage.Refused): build_pair.conditional_tests_completed(wire(changed))
        for bad in ([], events+events, events[:-1], events[1:], list(reversed(events))):
            with self.assertRaises(stage.Refused): build_pair.conditional_tests_completed(wire(bad))

    def test_native_ci_emits_the_reviewed_package_extension_on_both_arches(self):
        script_path = ROOT / "build-ci.sh"
        script = script_path.read_text()
        self.assertEqual(subprocess.run(["bash", "-n", str(script_path)],
                                       capture_output=True, check=False).returncode, 0)
        self.assertIn("PKGEXT=.pkg.tar.zst PKGDEST=", script)
        self.assertIn('"$package_name"-*.pkg.tar.zst)', script)
        self.assertIn("command -v zstd >/dev/null", script)

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="omavless-package-test-")
        self.root = Path(self.temporary.name)
        self.addCleanup(self.temporary.cleanup)
        self.binary = self.root / "reviewed-elf"
        header = bytearray(64)
        header[:6] = b"\x7fELF\x02\x01"
        header[18:20] = (183).to_bytes(2, "little")
        self.binary.write_bytes(header)
        self.pin = hashlib.sha256(header).hexdigest()
        self.pair = self.root / "pair"
        self.pair.mkdir(mode=0o700)
        source = io.BytesIO()
        with tarfile.open(fileobj=source, mode="w:xz") as archive:
            for name, content in (
                ("mihomo/go.mod", b"synthetic Go module"),
                ("mihomo/vendor/modules.txt", b"synthetic vendored modules"),
                ("mihomo/LICENSE", b"synthetic GPL fixture"),
                ("sing-tun/go.mod", b"synthetic sing-tun module"),
                ("sing-tun/LICENSE", b"synthetic GPL fixture"),
                ("omavless/Cargo.lock", b"synthetic Cargo lock"),
                ("omavless/LICENSE", b"synthetic MIT fixture"),
            ):
                member = tarfile.TarInfo(name)
                member.size = len(content)
                archive.addfile(member, io.BytesIO(content))
        files = {
            "omavless-dns-broker": bytes(header), "mihomo": bytes(header),
            "corresponding-source.tar.xz": source.getvalue(),
            "mihomo.LICENSE": b"synthetic GPL fixture",
            "sing-tun.LICENSE": b"synthetic GPL fixture",
            "omavless.LICENSE": b"synthetic MIT fixture",
        }
        for name, content in files.items():
            (self.pair / name).write_bytes(content)
        receipt = {
            "schema": 1, "architecture": "aarch64", "omavless_commit": "a" * 40,
            "mihomo_commit": build_pair.MIHOMO, "sing_tun_commit": build_pair.SING_TUN,
            "patch_sha256": build_pair.PATCH_SHA,
            "mihomo_tag": "v1.19.31", "sing_tun_tag": "v0.4.24",
            "go_version": "go version go1.26.8 linux/arm64",
            "go_build_tags": "with_gvisor", "go_dependency_mode": "vendor",
            "go_binary_sha256": "b" * 64, "cargo_lock_sha256": "c" * 64,
            "rustc_version": "rustc 1.98.1", "cargo_version": "cargo 1.98.1",
            "sha256": {name: hashlib.sha256(content).hexdigest()
                       for name, content in files.items()},
        }
        (self.pair / "source-receipt.json").write_text(json.dumps(receipt))

    def traffic_pair(self, missing=None, bad_patch=False):
        path = self.pair / "source-receipt.json"
        receipt = json.loads(path.read_text())
        archive_path = self.pair / "corresponding-source.tar.xz"
        with tarfile.open(fileobj=io.BytesIO(archive_path.read_bytes()), mode="r:xz") as old:
            entries = [(member.name, old.extractfile(member).read()) for member in old]
        extra = {
            "mihomo/hub/route/configs.go": b"synthetic configuration router",
            "mihomo/hub/route/traffic_ready.go": b"synthetic traffic projection",
            "mihomo/hub/route/traffic_ready_test.go": b"synthetic traffic tests",
            "omavless/tests/core_dns_adapter/mihomo-traffic-ready.patch":
                b"wrong patch" if bad_patch else (build_pair.PATCHES / "mihomo-traffic-ready.patch").read_bytes(),
        }
        entries = [(name, value) for name, value in entries if name not in extra]
        entries += [(name, value) for name, value in extra.items() if name != missing]
        source = io.BytesIO()
        with tarfile.open(fileobj=source, mode="w:xz") as archive:
            for name, value in entries:
                member = tarfile.TarInfo(name); member.size = len(value)
                archive.addfile(member, io.BytesIO(value))
        archive_path.write_bytes(source.getvalue())
        receipt.update(schema=stage.TRAFFIC_SCHEMA, traffic_ready_abi=1,
                       package_flavor="release", broker_feature="release-package",
                       patch_sha256=stage.TRAFFIC_PATCHES)
        receipt["sha256"]["corresponding-source.tar.xz"] = hashlib.sha256(source.getvalue()).hexdigest()
        path.write_text(json.dumps(receipt))
        return receipt

    def test_traffic_family_is_distinct_strict_and_not_close(self):
        receipt = self.traffic_pair()
        stage.reviewed_pair(self.pair, "aarch64", "a"*40, "release")
        for flavor in ("experimental", "release-close", "release-close-k1"):
            with self.assertRaises(stage.Refused):
                stage.reviewed_pair(self.pair, "aarch64", "a"*40, flavor)
        path = self.pair / "source-receipt.json"
        for key, value in (("schema",1),("traffic_ready_abi",True),("traffic_ready_abi",2),
                           ("conditional_close_abi",1),("go_build_tags","with_gvisor,omavless_k1_device"),
                           ("patch_sha256",stage.PINNED_PATCHES)):
            bad = dict(receipt); bad[key] = value; path.write_text(json.dumps(bad))
            with self.assertRaises(stage.Refused):
                stage.reviewed_pair(self.pair, "aarch64", "a"*40, "release")
        raw = json.dumps(receipt)
        path.write_text(raw.replace('"traffic_ready_abi": 1','"traffic_ready_abi": 1, "traffic_ready_abi": 1'))
        with self.assertRaises(stage.Refused):
            stage.reviewed_pair(self.pair, "aarch64", "a"*40, "release")
        self.assertEqual(stage.pair_policy("experimental")[2], build_pair.PATCH_SHA)
        self.assertEqual(stage.pair_policy("release-close")[2], stage.CLOSE_PATCHES)
        self.assertNotIn("mihomo-traffic-ready.patch", stage.CLOSE_PATCHES)

    def test_traffic_source_members_and_exact_patch_required(self):
        for missing in ("mihomo/hub/route/traffic_ready.go", "mihomo/hub/route/traffic_ready_test.go",
                        "omavless/tests/core_dns_adapter/mihomo-traffic-ready.patch"):
            self.traffic_pair(missing=missing)
            with self.assertRaises(stage.Refused):
                stage.reviewed_pair(self.pair, "aarch64", "a"*40, "release")
        self.traffic_pair(bad_patch=True)
        with self.assertRaises(stage.Refused):
            stage.reviewed_pair(self.pair, "aarch64", "a"*40, "release")
        self.assertEqual(build_pair.digest(build_pair.PATCHES / "mihomo-traffic-ready.patch"),
                         stage.TRAFFIC_PATCH)

    def test_traffic_build_requires_all_three_actual_test_completions(self):
        package = next(iter(build_pair.TRAFFIC_TESTS))
        events = [{"Action":"start","Package":package}]
        for name in sorted(build_pair.TRAFFIC_TESTS[package]):
            events += [{"Action":"run","Package":package,"Test":name},
                       {"Action":"pass","Package":package,"Test":name}]
        events.append({"Action":"pass","Package":package})
        wire = lambda rows: b"".join(json.dumps(row).encode()+b"\n" for row in rows)
        build_pair.tests_completed(wire(events),build_pair.TRAFFIC_TESTS)
        for index in range(len(events)):
            with self.assertRaises(stage.Refused):
                build_pair.tests_completed(wire(events[:index]+events[index+1:]),build_pair.TRAFFIC_TESTS)

    def render(self, output=None, **changes):
        args = {"pair": self.pair, "architecture": "aarch64", "revision": "a" * 40,
                "output": output or self.root / "staged"}
        args.update(changes)
        stage.stage(**args)
        return Path(args["output"])

    def test_stage_is_local_pinned_bounded_and_complete(self):
        destination = self.render()
        recipe = (destination / "PKGBUILD").read_text()
        manifest = json.loads((destination / "reviewed-inputs.json").read_text())
        self.assertEqual(manifest["architecture"], "aarch64")
        self.assertEqual(manifest["source_revision"], "a" * 40)
        for name, expected in manifest["sha256"].items():
            self.assertEqual(hashlib.sha256((destination / name).read_bytes()).hexdigest(), expected)
            self.assertIn(expected, recipe)
        self.assertNotIn("@", recipe)
        self.assertNotIn("SKIP", recipe)
        self.assertNotIn("://", recipe)
        self.assertEqual(destination.stat().st_mode & 0o777, 0o700)
        self.assertEqual(subprocess.run(["bash", "-n", str(destination / "PKGBUILD")],
                                       capture_output=True, check=False).returncode, 0)

    def test_release_staging_requires_release_broker_and_has_distinct_fixed_paths(self):
        with self.assertRaises(stage.Refused):
            release_stage.stage(self.pair, "aarch64", "a" * 40, self.root / "release")
        receipt_path = self.pair / "source-receipt.json"
        receipt = json.loads(receipt_path.read_text())
        # Old two-patch receipts are retained history, not new normal output.
        receipt["package_flavor"] = "release"
        receipt["broker_feature"] = "release-package"
        receipt_path.write_text(json.dumps(receipt))
        with self.assertRaises(stage.Refused):
            release_stage.stage(self.pair, "aarch64", "a" * 40, self.root / "old-release")
        self.traffic_pair()
        with self.assertRaises(stage.Refused):
            stage.stage(self.pair, "aarch64", "a" * 40, self.root / "experimental")
        destination = self.root / "release"
        release_stage.stage(self.pair, "aarch64", "a" * 40, destination)
        recipe = (destination / "PKGBUILD").read_text()
        unit = (destination / "omavless-dns-broker.service").read_text()
        hook = (destination / "omavless-dns.hook").read_text()
        script = (destination / "omavless-dns.install").read_text()
        self.assertIn("pkgname=omavless-dns\n", recipe)
        self.assertIn("pkgver=0.9.9\n", recipe)
        self.assertIn("conflicts=('omavless-dns-experimental')", recipe)
        self.assertIn("/usr/lib/omavless-dns/mihomo", recipe)
        self.assertIn("/usr/lib/omavless-dns/omavless-dns-broker", unit)
        self.assertIn("Target = omavless-dns", hook)
        self.assertIn("AbortOnFail", hook)
        self.assertIn("/usr/lib/omavless-dns/package-guard", script)
        self.assertIn("/usr/bin/setcap cap_net_bind_service,cap_net_admin,cap_net_raw=ep /usr/lib/omavless-dns/mihomo", script)
        self.assertNotIn("systemctl enable", script)
        self.assertNotIn("--enroll", script)
        self.assertNotIn("SKIP", recipe)
        self.assertNotIn("://", recipe)
        for filename in ("PKGBUILD", "omavless-dns.install"):
            self.assertEqual(subprocess.run(["bash", "-n", str(destination / filename)],
                                           capture_output=True, check=False).returncode, 0)
        manifest = json.loads((destination / "reviewed-inputs.json").read_text())
        self.assertEqual(manifest["package"], "omavless-dns")
        for name, expected in manifest["sha256"].items():
            self.assertEqual(hashlib.sha256((destination / name).read_bytes()).hexdigest(), expected)

    def test_release_package_version_requires_matching_frontend_and_runtime(self):
        for version, expected in (("0.9.0-rc.1", "0.9.0rc1"),
                                  ("0.9.0-rc.12", "0.9.0rc12"),
                                  ("0.9.5-beta.1", "0.9.5beta1"),
                                  ("0.9.5-beta.12", "0.9.5beta12"),
                                  ("0.9.0", "0.9.0")):
            cargo = f'[workspace.package]\nversion = "{version}"\n'
            manifest = json.dumps({"version": version})
            self.assertEqual(release_stage.package_version(cargo, manifest), expected)
        for cargo, manifest in (
            ('[workspace.package]\nversion = "0.9.0"\n', '{"version":"0.9.1"}'),
            ('[workspace.package]\nversion = "latest"\n', '{"version":"latest"}'),
            ('[workspace.package]\nversion = "0.9.0-rc.0"\n', '{"version":"0.9.0-rc.0"}'),
            ('[workspace.package]\nversion = "0.9.5-beta.0"\n', '{"version":"0.9.5-beta.0"}'),
            ('[workspace.package]\nversion = "0.9.5-beta.01"\n', '{"version":"0.9.5-beta.01"}'),
            ('[workspace.package]\nversion = "0.9.5-alpha.1"\n', '{"version":"0.9.5-alpha.1"}'),
            ('not toml', '{"version":"0.9.0"}'),
        ):
            with self.assertRaises(stage.Refused):
                release_stage.package_version(cargo, manifest)

    def test_offline_pair_builder_pins_committed_patches_and_local_tool(self):
        for name, expected in build_pair.PATCH_SHA.items():
            self.assertEqual(build_pair.digest(build_pair.PATCHES / name), expected)
        self.assertEqual(len(build_pair.git_value(build_pair.REPO, "rev-parse", "HEAD")), 40)
        self.assertEqual(build_pair.MIHOMO, "ab405bad5beeeac8b003bb01f60f134f6df54471")
        self.assertEqual(build_pair.SING_TUN, "b50ae28a1409c7bce8e96e6c6966cf57d8ace754")
        with self.assertRaises(stage.Refused):
            build_pair.reviewed_go("go", "x86_64")
        link = self.root / "linked-go"
        link.symlink_to("/usr/bin/go")
        with self.assertRaises(stage.Refused):
            build_pair.reviewed_go(str(link), "x86_64")
        with self.assertRaises(stage.Refused):
            build_pair.build(self.root, self.root, "/usr/bin/go", "x86_64",
                             build_pair.REPO / "candidate")

    def test_pair_builder_requires_matching_native_architecture_and_go(self):
        for architecture, go_arch in (("x86_64", "amd64"), ("aarch64", "arm64")):
            self.assertEqual(build_pair.reviewed_target(architecture, "Linux", architecture),
                             go_arch)
            with self.assertRaises(stage.Refused):
                build_pair.reviewed_target(architecture, "Linux", "other")
            with self.assertRaises(stage.Refused):
                build_pair.reviewed_target(architecture, "Darwin", architecture)
            with mock.patch.object(build_pair.subprocess, "run") as run:
                run.return_value.returncode = 0
                run.return_value.stdout = (
                    f"go version go1.26.8 linux/{go_arch}\n".encode("ascii"))
                self.assertIn(go_arch, build_pair.reviewed_go("/usr/bin/git", architecture))
                run.return_value.stdout = (
                    b"go version go1.26.8 linux/arm64\n" if go_arch == "amd64"
                    else b"go version go1.26.8 linux/amd64\n")
                with self.assertRaises(stage.Refused):
                    build_pair.reviewed_go("/usr/bin/git", architecture)
        with self.assertRaises(stage.Refused):
            build_pair.reviewed_target("unknown", "Linux", "unknown")

    def test_bad_hash_arch_revision_symlink_hardlink_existing_target_refuse(self):
        for changes in ({"architecture": "x86_64"}, {"revision": "main"}):
            with self.assertRaises(stage.Refused):
                self.render(**changes)
        link = self.root / "link"
        link.symlink_to(self.pair, target_is_directory=True)
        with self.assertRaises(stage.Refused):
            self.render(pair=link)
        (self.pair / "mihomo").unlink()
        os.link(self.binary, self.pair / "mihomo")
        with self.assertRaises(stage.Refused):
            self.render()
        (self.pair / "mihomo").unlink()
        (self.pair / "mihomo").write_bytes(self.binary.read_bytes())
        (self.pair / "mihomo").write_bytes(b"changed")
        with self.assertRaises(stage.Refused):
            self.render()
        (self.pair / "mihomo").write_bytes(self.binary.read_bytes())
        existing = self.root / "existing"
        existing.mkdir()
        with self.assertRaises(FileExistsError):
            self.render(output=existing)

    def test_pair_receipt_tamper_extra_fields_and_wrong_patches_refuse(self):
        path = self.pair / "source-receipt.json"
        original = path.read_text()
        try:
            for mutation in (
                lambda record: record.update({"private_profile": "synthetic"}),
                lambda record: record.update({"patch_sha256": {}}),
                lambda record: record["sha256"].update({"mihomo": "0" * 64}),
                lambda record: record.update({"go_version": "synthetic\nsecret"}),
            ):
                record = json.loads(original)
                mutation(record)
                path.write_text(json.dumps(record))
                with self.assertRaises(stage.Refused):
                    self.render()
        finally:
            path.write_text(original)

    def test_package_has_no_stock_core_override_activation_or_enrollment(self):
        recipe = (ROOT / "PKGBUILD.in").read_text()
        script = (ROOT / "omavless-dns-experimental.install").read_text()
        self.assertIn("/usr/lib/omavless-dns-experimental/mihomo", recipe)
        self.assertIn("corresponding-source.tar.xz", recipe)
        self.assertIn("noextract=('corresponding-source.tar.xz')", recipe)
        self.assertIn("mihomo.LICENSE", recipe)
        self.assertIn("sing-tun.LICENSE", recipe)
        self.assertIn("omavless.LICENSE", recipe)
        self.assertNotIn("$pkgdir/usr/bin/", recipe)
        self.assertNotIn("$pkgdir/etc/", recipe)
        self.assertIn("/usr/bin/setcap cap_net_bind_service,cap_net_admin,cap_net_raw=ep /usr/lib/omavless-dns-experimental/mihomo", script)
        self.assertNotRegex(script, r"(?m)^\s*(sudo|pkexec|systemctl|rm|curl|wget)\b")
        self.assertEqual(subprocess.run(["bash", "-n", str(ROOT / "omavless-dns-experimental.install")],
                                       capture_output=True, check=False).returncode, 0)

    def test_staging_refuses_normal_bare_linked_and_symlinked_git_destinations(self):
        for bare in (False, True):
            repository = self.root / ("bare" if bare else "repository")
            command = ["/usr/bin/git", "init", "--quiet"]
            if bare:
                command.append("--bare")
            subprocess.run([*command, str(repository)], check=True, capture_output=True,
                           env={"PATH": "/usr/bin:/bin", "GIT_CONFIG_NOSYSTEM": "1",
                                "GIT_CONFIG_GLOBAL": "/dev/null"})
            with self.assertRaises(stage.Refused):
                self.render(output=repository / "staged")
            self.assertFalse((repository / "staged").exists())
        linked = self.root / "linked"
        linked.mkdir()
        (linked / ".git").write_text(f"gitdir: {self.root / 'repository/.git'}\n")
        alias = self.root / "alias"
        alias.symlink_to(linked, target_is_directory=True)
        for directory in (linked, alias):
            with self.assertRaises(stage.Refused):
                self.render(output=directory / "staged")
            self.assertFalse((directory / "staged").exists())

    def test_staging_requires_absolute_destination_under_private_owned_parent(self):
        with self.assertRaises(stage.Refused):
            self.render(output=Path("relative-staged"))
        shared = self.root / "shared"
        shared.mkdir()
        shared.chmod(0o777)
        with self.assertRaises(stage.Refused):
            self.render(output=shared / "staged")
        self.assertFalse((shared / "staged").exists())

    def test_real_abort_is_pretransaction_hook_not_only_scriptlet(self):
        hook = (ROOT / "omavless-dns-experimental.hook").read_text()
        for line in ("Operation = Upgrade", "Operation = Remove", "Type = Package",
                     "Target = omavless-dns-experimental", "When = PreTransaction",
                     "Exec = /usr/lib/omavless-dns-experimental/package-guard", "AbortOnFail"):
            self.assertIn(line, hook.splitlines())

    def run_guard_fixture(self, state=EMPTY, entry=None, args=()):
        # Private source fixture substitutions only. Production has no injected
        # paths, command, UID or bus overrides. Never queries host systemd.
        runtime = self.root / "run"
        private = runtime / "omavless-dns/private"
        private.mkdir(parents=True, exist_ok=True)
        for directory in (runtime, private.parent, private):
            directory.chmod(0o700)
        if entry:
            (private / entry).write_text("synthetic")
        statefile = self.root / "properties"
        statefile.write_text(state)
        source = (ROOT / "package-guard").read_text()
        self.assertIn("[[ $# == 0 && $EUID == 0 ]]", source)
        source = source.replace("[[ $# == 0 && $EUID == 0 ]]", "[[ $# == 0 ]]")
        source = source.replace('"$owner" == 0', f'"$owner" == {os.getuid()}')
        source = source.replace("for directory in /run ", f"for directory in {runtime} ")
        source = source.replace("/run/omavless-dns", str(runtime / "omavless-dns"))
        source, replaced = re.subn(r"state=\$\(/usr/bin/env -i PATH=/usr/bin:/bin LC_ALL=C /usr/bin/timeout.*?--no-pager\)",
                                  f"state=$(/usr/bin/cat {statefile})", source, flags=re.S)
        self.assertEqual(replaced, 1)
        fixture = self.root / "guard-fixture"
        fixture.write_text(source)
        return subprocess.run(["bash", str(fixture), *args], capture_output=True,
                              timeout=5, check=False, env={"PATH": "/usr/bin:/bin"})

    def test_guard_accepts_only_proven_inactive_empty_fixture(self):
        self.assertEqual(self.run_guard_fixture().returncode, 0)
        self.assertNotEqual(self.run_guard_fixture(args=("--force",)).returncode, 0)
        for invalid in (EMPTY.replace("inactive", "active"), EMPTY.replace("inactive", "failed"),
                        EMPTY.replace("MainPID=0", "MainPID=12"),
                        EMPTY.replace("NFileDescriptorStore=0", "NFileDescriptorStore=1"),
                        EMPTY.replace("NFileDescriptorStore=0\n", ""),
                        EMPTY + "MainPID=0\n", EMPTY + "Unexpected=0\n", ""):
            result = self.run_guard_fixture(state=invalid)
            self.assertNotEqual(result.returncode, 0)
            self.assertLess(len(result.stderr), 200)
            self.assertIn(b"OmaVLESS DNS broker is not proven inactive", result.stderr)
            self.assertNotIn(b"Experimental DNS package", result.stderr)
            self.assertEqual(result.stdout, b"")

    def test_any_journal_staging_or_unknown_entry_blocks(self):
        for entry in ("lease.json", ".lease.pending", "unknown"):
            self.assertNotEqual(self.run_guard_fixture(entry=entry).returncode, 0)
            (self.root / "run/omavless-dns/private" / entry).unlink()

    def test_stale_socket_or_symlink_blocks_without_cleanup(self):
        # Only this test binds a real pathname socket. Keep all file-only
        # fixtures on their original TMPDIR policy. A deliberately long TMPDIR
        # must not affect this short, exclusively allocated HOME-backed root.
        long_tmp = self.root / ("long-tmp-" + "x" * 108)
        long_tmp.mkdir(mode=0o700)
        with mock.patch.object(tempfile, "tempdir", str(long_tmp)), \
             tempfile.TemporaryDirectory(prefix="dns-", dir=socket_fixture_parent()) as root, \
             mock.patch.object(self, "root", Path(root)):
            self.assertEqual(Path(root).parent, Path(os.environ["HOME"]) / ".cache/ovt")
            self.assertEqual(stat.S_IMODE(Path(root).lstat().st_mode), 0o700)
            self.check_stale_socket_or_symlink()
        self.assertFalse(Path(root).exists())

    def check_stale_socket_or_symlink(self):
        self.assertEqual(self.run_guard_fixture().returncode, 0)
        path = self.root / "run/omavless-dns/control.sock"
        self.assertLess(len(os.fsencode(path)), 108)
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as listener:
            listener.bind(str(path))
        self.assertNotEqual(self.run_guard_fixture().returncode, 0)
        self.assertTrue(path.is_socket())
        path.unlink()
        path.symlink_to("missing")
        self.assertNotEqual(self.run_guard_fixture().returncode, 0)
        self.assertTrue(path.is_symlink())


if __name__ == "__main__":
    unittest.main()
