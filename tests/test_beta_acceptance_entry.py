"""A VM-only acceptance entry must preserve the opt-in client after reopening."""
import configparser
from pathlib import Path
import shlex
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


if __name__ == "__main__":
    unittest.main()
