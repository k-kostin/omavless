#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Offline, unprivileged pinned-core conditional-close review; never installs."""
import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile

PIN = "ab405bad5beeeac8b003bb01f60f134f6df54471"
PATCH = Path(__file__).with_name("mihomo-conditional-close.patch")


def run(args, cwd=None, env=None):
    result = subprocess.run(args, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                            capture_output=True, timeout=600, check=False)
    if result.returncode or len(result.stdout) + len(result.stderr) > 4 * 1024 * 1024:
        raise RuntimeError("Pinned core review step failed")
    return result.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--scratch-parent", type=Path, required=True)
    args = parser.parse_args()
    for path in (args.source, args.scratch_parent):
        if not path.is_absolute() or path.is_symlink() or not path.is_dir():
            raise RuntimeError("Review requires existing absolute source and scratch directories")
    st = args.scratch_parent.stat()
    if st.st_uid != os.getuid() or st.st_mode & 0o077:
        raise RuntimeError("Review scratch parent must be private")
    if run(["git", "-C", str(args.source), "rev-parse", PIN + "^{commit}"]).strip().decode() != PIN:
        raise RuntimeError("Pinned upstream unavailable")
    with tempfile.TemporaryDirectory(prefix="core-close-", dir=args.scratch_parent) as scratch:
        root = Path(scratch)
        archive = root / "source.tar"
        # git archive ignores all worktree modifications, using only exact objects.
        with archive.open("wb") as output:
            result = subprocess.run(["git", "-C", str(args.source), "archive", PIN],
                                    stdin=subprocess.DEVNULL, stdout=output,
                                    stderr=subprocess.DEVNULL, timeout=60, check=False)
        if result.returncode or archive.stat().st_size > 128 * 1024 * 1024:
            raise RuntimeError("Pinned archive unavailable or oversized")
        source = root / "source"
        source.mkdir(mode=0o700)
        with tarfile.open(archive) as members:
            members.extractall(source, filter="data")
        run(["git", "apply", "--check", str(PATCH)], cwd=source)
        run(["git", "apply", str(PATCH)], cwd=source)
        env = dict(os.environ, TMPDIR=str(root), GOPROXY="off", GOSUMDB="off")
        run(["go", "test", "-race", "-mod=readonly", "./tunnel/statistic", "./hub/route",
             "-run", "TestConditional", "-count=20"], cwd=source, env=env)
        print("conditional_core_close: passed; pinned source; race matrix; no installation")
        print("source_sha=" + PIN)
        print("patch_sha256=" + hashlib.sha256(PATCH.read_bytes()).hexdigest())


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError, tarfile.TarError):
        raise SystemExit("conditional_core_close: refused; no installation") from None
