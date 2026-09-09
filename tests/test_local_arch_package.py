"""Synthetic, offline packaging gates; never install or start a service."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class LocalArchPackageContractTests(unittest.TestCase):
    """Always run, including non-Arch CI with no makepkg installation."""

    def test_fixed_command_boundaries_and_reviewed_identity(self):
        script = (ROOT / "packaging/arch/build-local-package.sh").read_text()
        code = "\n".join(line for line in script.splitlines() if not line.lstrip().startswith("#"))
        self.assertIn('[[ $# -eq 3 && $EUID -ne 0 ]] || fail', code)
        self.assertIn('^[0-9a-f]{40}$', code)
        self.assertEqual(code.count('git -C "$repo_root" rev-parse HEAD'), 2)
        self.assertEqual(code.count('status --porcelain --untracked-files=normal'), 2)
        self.assertIn('readelf -h -- "$binary"', code)
        self.assertIn('no_symlinks "$binary"', code)
        self.assertIn('no_symlinks "$builddir"', code)
        self.assertIn('[[ $elf_type == EXEC || $elf_type == DYN ]] || fail', code)
        self.assertIn('"$script_dir/stage-payload.sh" "$builddir/payload" "$binary"', code)
        self.assertIn('env -i PATH=/usr/bin:/bin HOME="$builddir/home" XDG_CONFIG_HOME="$builddir/config"', code)
        makepkg = [line.strip() for line in code.splitlines() if '/usr/bin/makepkg ' in line]
        self.assertEqual(makepkg, ['/usr/bin/makepkg --config "$builddir/makepkg.conf" --nodeps --nocheck --noconfirm || fail'])
        self.assertNotRegex(code, r'\b(sudo|pkexec|pacman|systemctl|cargo|curl|wget)\b')
        self.assertNotRegex(code, r'(?m)^\s*(?:exec\s+)?"?\$\{?binary\}?')
        self.assertNotIn('--syncdeps', code)
        self.assertNotIn('--install', code)
        self.assertIn('provenance=caller-supplied-prebuilt', code)
        self.assertIn('[[ ${packaged_hash%% *} == "$binary_hash" ]] || fail', code)
        for name in ('build-local-package.sh', 'PKGBUILD.local.in'):
            subprocess.run(['bash', '-n', str(ROOT / 'packaging/arch' / name)], check=True)

    def test_local_recipe_has_no_build_install_or_remote_source(self):
        template = (ROOT / 'packaging/arch/PKGBUILD.local.in').read_text()
        self.assertIn("source=('payload.tar')", template)
        self.assertIn("sha256sums=('@PAYLOAD_SHA256@')", template)
        self.assertIn("options=('!strip' '!debug' 'docs')", template)
        self.assertIn('pkgver=0.0.0.r@COUNT@.g@SHORT_SHA@', template)
        self.assertIn("arch=('@ARCH@')", template)
        self.assertNotIn('SKIP', template)
        self.assertNotRegex(template, r'(?m)^\s*(?:build|prepare|check)\s*\(\)|^\s*install=')
        self.assertNotRegex(template, r'\b(sudo|pkexec|systemctl|pacman|cargo|curl|wget)\b')
        self.assertIn('cp -a --no-preserve=ownership -- "$srcdir/usr" "$pkgdir/usr"', template)


class LocalArchPackageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="omavless-package-")
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.repo = self.base / "source"
        self.repo.mkdir()
        for name in ("packaging/arch/stage-payload.sh", "packaging/arch/build-local-package.sh",
                     "packaging/arch/PKGBUILD.local.in", "packaging/arch/README.md",
                     "packaging/systemd/omavless-runtime.service", "LICENSE", "THIRD_PARTY_NOTICES.md"):
            target = self.repo / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, target)
        for args in (("init", "-q"), ("add", "."),
                     ("-c", "user.name=Synthetic", "-c", "user.email=synthetic@example.invalid", "commit", "-qm", "fixture")):
            subprocess.run(["git", "-C", str(self.repo), *args], check=True, capture_output=True)
        self.sha = subprocess.check_output(["git", "-C", str(self.repo), "rev-parse", "HEAD"], text=True).strip()
        self.build = self.base / "build"
        self.build.mkdir()

    def invoke(self, binary="/usr/bin/true", sha=None, build=None):
        return subprocess.run(["bash", str(self.repo / "packaging/arch/build-local-package.sh"),
                               str(build or self.build), str(binary), sha or self.sha],
                              capture_output=True, text=True, timeout=120)

    def test_refuses_wrong_identity_dirty_checkout_unsafe_paths_without_payload(self):
        self.assertNotEqual(self.invoke(sha="0" * 40).returncode, 0)
        self.assertFalse(any(self.build.iterdir()))
        (self.repo / "unreviewed").write_text("synthetic")
        self.assertNotEqual(self.invoke().returncode, 0)
        (self.repo / "unreviewed").unlink()
        link = self.base / "binary-link"
        link.symlink_to("/usr/bin/true")
        self.assertNotEqual(self.invoke(binary=link).returncode, 0)
        self.assertNotEqual(self.invoke(build=self.repo).returncode, 0)
        not_elf = self.base / "not-elf"
        marker = self.base / "must-not-execute"
        not_elf.write_text(f"#!/bin/sh\ntouch '{marker}'\n")
        not_elf.chmod(0o700)
        self.assertNotEqual(self.invoke(binary=not_elf).returncode, 0)
        self.assertFalse(marker.exists())
        self.assertFalse(any(self.build.iterdir()))

    def test_root_refusal_precedes_all_staging(self):
        if os.geteuid() != 0:
            self.skipTest("root refusal executes only in root CI; exact guard checked portably")
        result = self.invoke()
        self.assertEqual(result.returncode, 2)
        self.assertIn("build refused or failed", result.stderr)
        self.assertFalse(any(self.build.iterdir()))

    def test_offline_real_makepkg_payload_identity_and_no_activation_hooks(self):
        if os.geteuid() == 0:
            self.skipTest("makepkg deliberately refuses root")
        if not all(shutil.which(tool) for tool in ("makepkg", "fakeroot", "bsdtar", "readelf", "zstd")):
            self.skipTest("Arch packaging tools unavailable")
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr[-2000:])
        packages = list(self.build.glob("omavless-*.pkg.tar.zst"))
        self.assertEqual(len(packages), 1)
        archive = packages[0]
        def content(member):
            return subprocess.check_output(["bsdtar", "-xOf", str(archive), member])
        listing = subprocess.check_output(["bsdtar", "-tf", str(archive)], text=True).splitlines()
        self.assertNotIn(".INSTALL", listing)
        self.assertFalse(any("backend.py" in path or path.startswith("etc/") for path in listing))
        binary_hash = hashlib.sha256(Path("/usr/bin/true").read_bytes()).hexdigest()
        self.assertEqual(hashlib.sha256(content("usr/bin/omavless")).hexdigest(), binary_hash)
        identity = content("usr/share/doc/omavless/build-identity.txt").decode()
        self.assertIn(f"sourceCommit={self.sha}\n", identity)
        self.assertIn(f"binarySha256={binary_hash}\n", identity)
        self.assertIn("provenance=caller-supplied-prebuilt\n", identity)
        self.assertEqual(content("usr/lib/systemd/user/omavless-runtime.service"),
                         (ROOT / "packaging/systemd/omavless-runtime.service").read_bytes())
        metadata = content(".PKGINFO").decode()
        self.assertIn("pkgname = omavless\n", metadata)
        self.assertIn("pkgver = 0.0.0.r1.g", metadata)
        self.assertIn("depend = mihomo\n", metadata)
        self.assertNotIn("depend = python", metadata)
        self.assertEqual(subprocess.check_output(["git", "-C", str(self.repo), "status", "--porcelain"]), b"")


if __name__ == "__main__":
    unittest.main()
