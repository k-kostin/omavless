"""A VM-only acceptance entry must preserve the opt-in client after reopening."""
import configparser
from pathlib import Path
import shlex
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]


class BetaAcceptanceEntryTests(unittest.TestCase):
    def test_backup_entry_is_literal_and_explicit(self):
        parser = configparser.ConfigParser(interpolation=None)
        parser.read(ROOT / "tools/omavless-beta-backup.desktop")
        entry = parser["Desktop Entry"]
        self.assertEqual(entry["Type"], "Application")
        self.assertEqual(entry["Terminal"], "false")
        self.assertEqual(entry["StartupWMClass"], "org.omavless.beta.backup")
        self.assertIn("acceptance", entry["Name"])
        self.assertEqual(shlex.split(entry["Exec"]), [
            "/usr/bin/foot", "--app-id=org.omavless.beta.backup",
            "--title=OmaVLESS-beta-Backup", "/usr/bin/omavless", "tui",
            "--developer-private-backup",
        ])

    def test_ordinary_plugin_entry_stays_default(self):
        source = (ROOT / "plugin/Service.qml").read_text()
        self.assertIn("--app-id=org.omarchy.omavless omavless tui", source)
        self.assertNotIn("--developer-private-backup", source)

    def test_selected_beta_package_contains_its_real_client_features(self):
        script = ROOT / "packaging/release/client-features.sh"
        result = subprocess.run(["bash", str(script), "0.9.8-beta.4"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "t4-manager-actor-service\n")
        for version in ["0.8.2", "0.9.8-rc.1", "0.9.8-beta.3", "0.9.8-beta.5"]:
            result = subprocess.run(["bash", str(script), version], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")
        for version in ["invalid", "0.9.8-beta.0", "0.9.8-beta.4 extra"]:
            result = subprocess.run(["bash", str(script), version], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")

    def test_ci_uses_the_same_fixed_beta_selection(self):
        source = (ROOT / "packaging/release/build-native-ci.sh").read_text()
        self.assertIn('client_features=$(bash packaging/release/client-features.sh "$product_version")', source)
        self.assertIn('"${client_build_flags[@]}"', source)


if __name__ == "__main__":
    unittest.main()
