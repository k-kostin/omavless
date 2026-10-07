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
        self.assertIn('[[ ( $# -eq 3 || ( $# -eq 4 && ( $4 == --candidate || $4 == --stable ) ) || ( $# -eq 5 && $4 == --product-image-witness && -n $5 ) ) && $EUID -ne 0 ]] || fail', code)
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
        self.assertIn('pkgver=@VERSION@', template)
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
                     "Cargo.toml",
                     "packaging/systemd/omavless-runtime.service",
                     "packaging/systemd/omavless-login-prepare.service", "LICENSE", "THIRD_PARTY_NOTICES.md"):
            target = self.repo / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, target)
        target = self.repo / "packaging/systemd/omavless-image-witness.service"
        shutil.copyfile(ROOT / "packaging/systemd/omavless-image-witness.service", target)
        # Keep RC/development test inputs independent of the current product version.
        (self.repo / 'Cargo.toml').write_text('[workspace.package]\nversion = "0.8.0-rc.1"\n')
        for args in (("init", "-q"), ("add", "."),
                     ("-c", "user.name=Synthetic", "-c", "user.email=synthetic@example.invalid", "commit", "-qm", "fixture")):
            subprocess.run(["git", "-C", str(self.repo), *args], check=True, capture_output=True)
        self.sha = subprocess.check_output(["git", "-C", str(self.repo), "rev-parse", "HEAD"], text=True).strip()
        self.build = self.base / "build"
        self.build.mkdir()

    def invoke(self, binary="/usr/bin/true", sha=None, build=None, candidate=False, stable=False, product_helper=None):
        return subprocess.run(["bash", str(self.repo / "packaging/arch/build-local-package.sh"),
                               str(build or self.build), str(binary), sha or self.sha,
                               *(["--product-image-witness", str(product_helper)] if product_helper else ["--stable"] if stable else ["--candidate"] if candidate else [])],
                              capture_output=True, text=True, timeout=120)

    def test_product_staging_is_exclusive_opt_in_and_has_no_enrollment_or_activation(self):
        plain = self.base / "plain"
        product = self.base / "product"
        plain.mkdir(); product.mkdir()
        script = self.repo / "packaging/arch/stage-payload.sh"
        subprocess.run(["bash", str(script), str(plain), "/usr/bin/true"], check=True)
        subprocess.run(["bash", str(script), str(product), "/usr/bin/true", "--product-image-witness", "/usr/bin/true"], check=True)
        self.assertFalse((plain / "usr/lib/omavless-image").exists())
        self.assertEqual((product / "usr/lib/omavless-image/omavless-image-witness").read_bytes(), Path("/usr/bin/true").read_bytes())
        for node in ("omavless-runtime.service", "omavless-login-prepare.service"):
            self.assertEqual((plain / "usr/lib/systemd/user" / node).read_bytes(), (product / "usr/lib/systemd/user" / node).read_bytes())
        self.assertFalse((product / "var").exists())
        self.assertFalse((product / "etc").exists())
        helper_unit = (product / "usr/lib/systemd/system/omavless-image-witness.service").read_text()
        self.assertIn("AmbientCapabilities=CAP_SYS_PTRACE", helper_unit)
        self.assertIn("CapabilityBoundingSet=CAP_SYS_PTRACE", helper_unit)
        self.assertIn("RuntimeDirectoryMode=0755", helper_unit)
        self.assertIn("Restart=no", helper_unit)
        self.assertEqual(helper_unit.count("TasksMax=2\n"), 1)
        self.assertNotIn("TasksMax=1\n", helper_unit)
        self.assertIn("LimitNOFILE=64\n", helper_unit)
        self.assertIn("NoNewPrivileges=yes\n", helper_unit)
        self.assertIn("PrivateUsers=no\n", helper_unit)
        self.assertIn("PrivateNetwork=no\n", helper_unit)
        self.assertIn("RestrictNamespaces=yes\n", helper_unit)
        self.assertNotIn("[Install]", helper_unit)
        self.assertNotIn("CAP_NET_ADMIN", helper_unit)

    def test_product_stage_refuses_helper_symlink_and_unknown_option_before_writes(self):
        link = self.base / "helper-link"
        link.symlink_to("/usr/bin/true")
        script = self.repo / "packaging/arch/stage-payload.sh"
        for flag, helper in [("--product-image-witness", link), ("--arbitrary", Path("/usr/bin/true"))]:
            result = subprocess.run(["bash", str(script), str(self.build), "/usr/bin/true", flag, str(helper)], capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(any(self.build.iterdir()))

    def test_explicit_product_empty_helper_refuses_stage_and_builder_without_writes(self):
        for arguments in (["bash", str(self.repo / "packaging/arch/stage-payload.sh"), str(self.build), "/usr/bin/true", "--product-image-witness", ""],
                          ["bash", str(self.repo / "packaging/arch/build-local-package.sh"), str(self.build), "/usr/bin/true", self.sha, "--product-image-witness", ""]):
            result = subprocess.run(arguments, capture_output=True)
            self.assertEqual(result.returncode, 2)
            self.assertFalse(any(self.build.iterdir()))

    def test_offline_product_makepkg_records_both_elf_hashes_without_hooks(self):
        if os.geteuid() == 0 or not all(shutil.which(t) for t in ("makepkg", "fakeroot", "bsdtar", "readelf", "zstd")):
            self.skipTest("non-root Arch packaging tools required")
        result = self.invoke(product_helper="/usr/bin/true")
        self.assertEqual(result.returncode, 0, result.stderr[-2000:])
        archive, = self.build.glob("omavless-*.pkg.tar.zst")
        identity = subprocess.check_output(["bsdtar", "-xOf", str(archive), "usr/share/doc/omavless/build-identity.txt"], text=True)
        self.assertIn("schemaVersion=4\n", identity)
        self.assertIn("helperSha256=" + hashlib.sha256(Path("/usr/bin/true").read_bytes()).hexdigest(), identity)
        listing = subprocess.check_output(["bsdtar", "-tf", str(archive)], text=True).splitlines()
        self.assertNotIn(".INSTALL", listing)
        self.assertFalse(any(path.startswith(("var/", "etc/")) for path in listing))
        metadata = subprocess.check_output(["bsdtar", "-xOf", str(archive), ".PKGINFO"], text=True)
        self.assertIn("depend = omavless-dns\n", metadata)
        packaged_unit = subprocess.check_output(["bsdtar", "-xOf", str(archive),
            "usr/lib/systemd/system/omavless-image-witness.service"])
        self.assertEqual(packaged_unit, (ROOT / "packaging/systemd/omavless-image-witness.service").read_bytes())
        self.assertEqual(packaged_unit.count(b"TasksMax=2\n"), 1)

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

    def test_candidate_refuses_stable_version_before_payload(self):
        cargo = self.repo / 'Cargo.toml'
        cargo.write_text('[workspace.package]\nversion = "0.8.0"\n')
        subprocess.run(['git', '-C', str(self.repo), 'add', '.'], check=True, capture_output=True)
        subprocess.run(['git', '-C', str(self.repo), '-c', 'user.name=Synthetic',
                        '-c', 'user.email=synthetic@example.invalid', 'commit', '-qm', 'stable'],
                       check=True, capture_output=True)
        sha = subprocess.check_output(['git', '-C', str(self.repo), 'rev-parse', 'HEAD'], text=True).strip()
        self.assertNotEqual(self.invoke(sha=sha, candidate=True).returncode, 0)
        self.assertFalse(any(self.build.iterdir()))

    def test_stable_refuses_rc_version_before_payload(self):
        self.assertNotEqual(self.invoke(stable=True).returncode, 0)
        self.assertFalse(any(self.build.iterdir()))

    def test_offline_candidate_makepkg_version(self):
        if os.geteuid() == 0 or not all(shutil.which(t) for t in ('makepkg', 'fakeroot', 'bsdtar', 'readelf', 'zstd')):
            self.skipTest('non-root Arch packaging tools required')
        result = self.invoke(candidate=True)
        self.assertEqual(result.returncode, 0, result.stderr[-2000:])
        archive, = self.build.glob('omavless-0.8.0rc1-1-*.pkg.tar.zst')
        metadata = subprocess.check_output(['bsdtar', '-xOf', str(archive), '.PKGINFO'], text=True)
        self.assertIn('pkgver = 0.8.0rc1-1\n', metadata)
        self.assertIn('depend = omavless-dns=0.8.0rc1-1\n', metadata)
        self.assertNotIn('depend = mihomo\n', metadata)
        if shutil.which('vercmp'):
            self.assertLess(int(subprocess.check_output(['vercmp', '0.8.0rc1', '0.8.0'])), 0)

    def test_offline_beta_makepkg_version_dependency_and_ordering(self):
        if os.geteuid() == 0 or not all(shutil.which(t) for t in ('makepkg', 'fakeroot', 'bsdtar', 'readelf', 'zstd')):
            self.skipTest('non-root Arch packaging tools required')
        (self.repo / 'Cargo.toml').write_text('[workspace.package]\nversion = "0.9.5-beta.1"\n')
        subprocess.run(['git', '-C', str(self.repo), 'add', '.'], check=True, capture_output=True)
        subprocess.run(['git', '-C', str(self.repo), '-c', 'user.name=Synthetic',
                        '-c', 'user.email=synthetic@example.invalid', 'commit', '-qm', 'beta'],
                       check=True, capture_output=True)
        self.sha = subprocess.check_output(['git', '-C', str(self.repo), 'rev-parse', 'HEAD'], text=True).strip()
        result = self.invoke(candidate=True)
        self.assertEqual(result.returncode, 0, result.stderr[-2000:])
        archive, = self.build.glob('omavless-0.9.5beta1-1-*.pkg.tar.zst')
        metadata = subprocess.check_output(['bsdtar', '-xOf', str(archive), '.PKGINFO'], text=True)
        self.assertIn('pkgver = 0.9.5beta1-1\n', metadata)
        self.assertIn('depend = omavless-dns=0.9.5beta1-1\n', metadata)
        self.assertNotIn('depend = mihomo\n', metadata)
        if shutil.which('vercmp'):
            for later in ('0.9.5beta2', '0.9.5rc1', '0.9.5'):
                self.assertLess(int(subprocess.check_output(['vercmp', '0.9.5beta1', later])), 0)

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
        self.assertEqual(content("usr/lib/systemd/user/omavless-login-prepare.service"),
                         (ROOT / "packaging/systemd/omavless-login-prepare.service").read_bytes())
        metadata = content(".PKGINFO").decode()
        self.assertIn("pkgname = omavless\n", metadata)
        self.assertIn("pkgver = 0.0.0.r1.g", metadata)
        self.assertIn("depend = mihomo\n", metadata)
        self.assertIn("depend = bubblewrap\n", metadata)
        self.assertNotIn("depend = python", metadata)
        self.assertEqual(subprocess.check_output(["git", "-C", str(self.repo), "status", "--porcelain"]), b"")


if __name__ == "__main__":
    unittest.main()
