"""Offline screenshot staging uses committed public metadata, never a live store."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class MarketplaceVisualStagingTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="omavless-visual-source-")
        self.addCleanup(self.temp.cleanup)
        self.repo = Path(self.temp.name)
        (self.repo / "plugin").mkdir()
        (self.repo / "plugin/Panel.qml").write_text("// synthetic committed QML\n")
        self.manifest = {"version": "0.9.8-beta.1", "author": "kdk"}
        (self.repo / "manifest.json").write_text(json.dumps(self.manifest))
        for args in (("init", "-q"), ("add", "."),
                     ("-c", "user.name=Synthetic", "-c", "user.email=synthetic@example.invalid",
                      "commit", "-qm", "fixture")):
            subprocess.run(["git", "-C", str(self.repo), *args], check=True, capture_output=True)

    def stage(self):
        return subprocess.run(["bash", str(ROOT / "tests/marketplace-visual/prepare.sh"),
                               str(self.repo)], capture_output=True, text=True, timeout=30)

    def test_committed_manifest_is_available_to_unmodified_product_qml(self):
        result = self.stage()
        self.assertEqual(result.returncode, 0, result.stderr)
        capture = Path(result.stdout.strip())
        self.assertEqual(capture.parent, Path.home() / ".cache" / "ovt")
        self.assertTrue(capture.name.startswith("omavless-marketplace."))
        self.assertTrue(capture.is_dir() and not capture.is_symlink())
        self.addCleanup(shutil.rmtree, capture)
        self.assertEqual(json.loads((capture / "manifest.json").read_text()), self.manifest)
        self.assertEqual((capture / "plugin/Panel.qml").read_bytes(),
                         (self.repo / "plugin/Panel.qml").read_bytes())

    def test_dirty_public_manifest_refuses_before_staging(self):
        (self.repo / "manifest.json").write_text('{"version":"uncommitted"}')
        result = self.stage()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "")

    def test_unsafe_capture_parent_refuses_without_permission_repair(self):
        for kind in ("writable-home", "symlink-cache", "writable-cache", "public-ovt"):
            with self.subTest(kind=kind):
                home = self.repo / kind
                home.mkdir(mode=0o700)
                cache = home / ".cache"
                if kind == "writable-home":
                    home.chmod(0o777)
                    watched = home
                elif kind == "symlink-cache":
                    target = self.repo / "cache-target"
                    target.mkdir(mode=0o700)
                    cache.symlink_to(target, target_is_directory=True)
                    watched = target
                else:
                    cache.mkdir(mode=0o700)
                    watched = cache
                    if kind == "writable-cache":
                        cache.chmod(0o777)
                    else:
                        watched = cache / "ovt"
                        watched.mkdir(mode=0o755)
                before = watched.stat().st_mode
                result = subprocess.run(
                    ["bash", str(ROOT / "tests/marketplace-visual/prepare.sh"), str(self.repo)],
                    env={**os.environ, "HOME": str(home)}, capture_output=True, text=True, timeout=30)
                self.assertEqual(result.returncode, 64, result.stderr)
                self.assertEqual(result.stdout, "")
                self.assertEqual(watched.stat().st_mode, before)
                self.assertFalse(list(home.rglob("omavless-marketplace.*")))


if __name__ == "__main__":
    unittest.main()
