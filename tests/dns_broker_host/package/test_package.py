"""No package build/install, root action, host systemd query or real DNS input."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("dns_package_stage", ROOT / "stage.py")
stage = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(stage)
EMPTY = "LoadState=loaded\nActiveState=inactive\nSubState=dead\nMainPID=0\nNFileDescriptorStore=0\n"


class PackageTests(unittest.TestCase):
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

    def render(self, output=None, **changes):
        args = {"broker": self.binary, "core": self.binary, "broker_sha": self.pin,
                "core_sha": self.pin, "architecture": "aarch64", "revision": "a" * 40,
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

    def test_bad_hash_arch_revision_symlink_hardlink_existing_target_refuse(self):
        for changes in ({"broker_sha": "0" * 64}, {"core_sha": "SKIP"},
                        {"architecture": "x86_64"}, {"revision": "main"}):
            with self.assertRaises(stage.Refused):
                self.render(**changes)
        link = self.root / "link"
        link.symlink_to(self.binary)
        with self.assertRaises(OSError):
            self.render(broker=link)
        os.link(self.binary, self.root / "hardlink")
        with self.assertRaises(stage.Refused):
            self.render()
        (self.root / "hardlink").unlink()
        existing = self.root / "existing"
        existing.mkdir()
        with self.assertRaises(FileExistsError):
            self.render(output=existing)

    def test_package_has_no_stock_core_override_activation_or_enrollment(self):
        recipe = (ROOT / "PKGBUILD.in").read_text()
        script = (ROOT / "omavless-dns-experimental.install").read_text()
        self.assertIn("/usr/lib/omavless-dns-experimental/mihomo", recipe)
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
            self.assertEqual(result.stdout, b"")

    def test_any_journal_staging_or_unknown_entry_blocks(self):
        for entry in ("lease.json", ".lease.pending", "unknown"):
            self.assertNotEqual(self.run_guard_fixture(entry=entry).returncode, 0)
            (self.root / "run/omavless-dns/private" / entry).unlink()


if __name__ == "__main__":
    unittest.main()
