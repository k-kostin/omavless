#!/usr/bin/env python3
"""Synthetic editor oracle: exercise actual editor seeding without a GUI."""
import hashlib
import json
import sys
import tempfile
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import backend


def main():
    raw = sys.stdin.buffer.read(8 * 1024 * 1024 + 1)
    if len(raw) > 8 * 1024 * 1024:
        raise ValueError
    cases = json.loads(raw)
    if not isinstance(cases, list) or len(cases) > 256:
        raise ValueError
    results = []
    with tempfile.TemporaryDirectory(prefix="ov-editor-oracle-") as directory:
        paths = SimpleNamespace(runtime=Path(directory))
        for case in cases:
            captured = []
            def editor(command, **_kwargs):
                filename = next(part.removeprefix("--filename=") for part in command
                                if part.startswith("--filename="))
                target = Path(filename)
                if target.parent != paths.runtime or target.stat().st_mode & 0o777 != 0o600:
                    raise ValueError
                captured.append(target.read_text())
                return SimpleNamespace(returncode=1, stdout="")
            store = backend.validate_store(case["store"])
            name = backend.profile_by_id(store, case["id"])["name"]
            with patch.object(backend, "load_store", return_value=store), \
                 patch.object(backend.shutil, "which", return_value="/synthetic/zenity"), \
                 patch.object(backend, "ensure_runtime"), patch.object(backend, "run", side_effect=editor):
                result = backend.edit_profile(paths, case["id"], name, "")
            if result != 3 or len(captured) != 1 or list(paths.runtime.iterdir()):
                raise ValueError
            # The frontend appends the editor newline; v1 returns the raw link.
            if not captured[0].endswith("\n"):
                raise ValueError
            payload = {"name":name, "input":captured[0][:-1]}
            canonical = json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
            results.append(hashlib.sha256(canonical).hexdigest())
    print(json.dumps(results))


if __name__ == "__main__":
    try:
        main()
    except Exception:
        print("Profile editor oracle failed", file=sys.stderr)
        sys.exit(1)
