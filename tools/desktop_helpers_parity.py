#!/usr/bin/env python3
"""Actual legacy helper-discovery oracle; only fixed public booleans/providers."""
import json
import hashlib
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import backend


def main():
    results = []
    for mask in range(8):
        available = {name for bit, name in enumerate(("zenity", "kdialog", "yad"))
                     if mask & (1 << bit)}
        with patch.object(backend.shutil, "which", side_effect=lambda name:
                          "/synthetic/" + name if name in available else None), \
             patch.object(backend, "gtk4_file_picker_available", return_value=False):
            status = backend.file_picker_status()
            helpers = backend.desktop_helper_status()
        results.append({"provider": status["provider"],
                        "editor": helpers["configEditorAvailable"]})
    # Execute the actual checked-in QML clipboard shell body against synthetic
    # fixed helpers. No real compositor, clipboard, store or credentials.
    root = Path(__file__).resolve().parents[1]
    qml = (root / "plugin/Service.qml").read_text()
    body = qml.split("readonly property string clipboardScript:", 1)[1].split(
        "readonly property string diagnosticsExportScript:", 1)[0]
    script = "".join(json.loads(line.strip().removesuffix("+").strip())
                     for line in body.splitlines() if line.strip())
    clipboard = []
    cases = json.loads((root / "tests/parity_cases/desktop-clipboard-v1.json").read_text())
    with tempfile.TemporaryDirectory(prefix="ov-clipboard-oracle-") as directory:
        path = Path(directory)
        for name in ("timeout", "grep", "head"):
            (path / name).symlink_to("/usr/bin/" + name)
        for case in cases:
            target = path / "wl-paste"
            target.write_text("#!/bin/bash\n" + case["script"] + "\n")
            target.chmod(0o700)
            result = subprocess.run(["/bin/bash", "-c", script], capture_output=True,
                                    env={**os.environ, "PATH": directory}, timeout=8)
            clipboard.append({"id": case["id"], "ok": result.returncode == 0,
                              "digest": hashlib.sha256(result.stdout).hexdigest()})
    print(json.dumps({"discovery": results, "clipboard": clipboard}))


if __name__ == "__main__":
    try:
        main()
    except Exception:
        print("Desktop helper oracle failed", file=sys.stderr)
        sys.exit(1)
