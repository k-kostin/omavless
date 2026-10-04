#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Offline, unprivileged pinned-core conditional-close review; never installs."""
import argparse
from collections import Counter
import hashlib
import json
import os
import sys

_BUILDER_CODE = sys._getframe().f_code
from pathlib import Path
import subprocess
import tarfile
import tempfile

PIN = "ab405bad5beeeac8b003bb01f60f134f6df54471"
PATCH = Path(__file__).with_name("mihomo-conditional-close.patch")
MAX_ARCHIVE = 128 * 1024 * 1024
GO = "/usr/bin/go"
CONDITIONAL_TESTS = (
    "TestConditionalCloseExplicitReadiness", "TestConditionalCloseStrictRequest",
    "TestConditionalCloseReusedID", "TestConditionalCloseCannotReenroll",
    "TestConditionalCloseDelayedLeave", "TestConditionalCloseConcurrentConfirm",
    "TestConditionalCloseExhaustionAndFailure",
)


def run(args, cwd=None, env=None):
    result = subprocess.run(args, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                            capture_output=True, timeout=600, check=False)
    if result.returncode or len(result.stdout) + len(result.stderr) > 4 * 1024 * 1024:
        raise RuntimeError("Pinned core review step failed")
    return result.stdout


def git_environment():
    return {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "GIT_NO_REPLACE_OBJECTS": "1",
            "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
            "GIT_NO_LAZY_FETCH": "1", "GIT_ALLOW_PROTOCOL": "", "GIT_TERMINAL_PROMPT": "0"}


def compiler_environment(root):
    # Only explicit local caches/tooling; never inherit GOENV, GOFLAGS, CC,
    # GOEXPERIMENT, workspace or toolchain/download settings from the shell.
    home = Path(os.environ.get("HOME", ""))
    if not home.is_absolute() or not home.is_dir():
        raise RuntimeError("Local compiler home refused")
    return {"PATH": "/usr/bin:/bin", "HOME": str(home), "LC_ALL": "C",
            "TMPDIR": str(root), "GOENV": "off", "GOPROXY": "off", "GOSUMDB": "off",
            "GOWORK": "off", "GOTOOLCHAIN": "local", "GOFLAGS": "", "CGO_ENABLED": "0",
            "GOMODCACHE": str(home / "go/pkg/mod"), "GOCACHE": str(home / ".cache/go-build"),
            "CC": "/usr/bin/gcc"}


def export(repository, revision, destination):
    identity = run(["/usr/bin/git", "-C", str(repository), "rev-parse",
                    revision + "^{commit}"], env=git_environment()).strip()
    if identity != revision.encode("ascii"):
        raise RuntimeError("Composition upstream identity refused")
    # A normal git archive still consults repository-local info/attributes.
    # Share the isolated object-only export between both developer entrypoints.
    objects = Path(os.fsdecode(run(
        ["/usr/bin/git", "-C", str(repository), "rev-parse", "--path-format=absolute",
         "--git-path", "objects"], env=git_environment()).rstrip(b"\n")))
    if not objects.is_absolute() or not objects.is_dir():
        raise RuntimeError("Composition object directory refused")
    isolated = destination.parent / (destination.name + "-export.git")
    run(["/usr/bin/git", "init", "--quiet", "--bare", "--template=", str(isolated)],
        env=git_environment())
    archive_env = dict(git_environment(), GIT_OBJECT_DIRECTORY=str(objects))
    archive = destination.parent / (destination.name + ".tar")
    with archive.open("xb") as output:
        result = subprocess.run(["/usr/bin/git", "--git-dir=" + str(isolated), "archive", revision],
                                stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.DEVNULL,
                                timeout=60, env=archive_env, check=False)
    if result.returncode or archive.stat().st_size > MAX_ARCHIVE:
        raise RuntimeError("Composition archive refused")
    destination.mkdir(mode=0o700)
    with tarfile.open(archive) as members:
        members.extractall(destination, filter="data")
    archive.unlink()


def matrix_receipt(raw, tests, skips=()):
    passed, skipped = Counter(), Counter()
    try:
        for line in raw.splitlines():
            event = json.loads(line)
            if not isinstance(event, dict) or event.get("Action") == "fail":
                raise ValueError("Invalid Go matrix receipt")
            name = event.get("Test")
            if not isinstance(name, str) or "/" in name:
                continue
            if event.get("Action") == "pass":
                passed[name] += 1
            if event.get("Action") == "skip":
                skipped[name] += 1
    except (ValueError, TypeError):
        raise RuntimeError("Composition test execution receipt refused") from None
    if passed != Counter({name: 20 for name in tests}) or skipped != Counter({name: 20 for name in skips}):
        raise RuntimeError("Composition test execution receipt refused")


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
    with tempfile.TemporaryDirectory(prefix="core-close-", dir=args.scratch_parent) as scratch:
        root = Path(scratch)
        source = root / "source"
        export(args.source, PIN, source)
        run(["/usr/bin/git", "apply", "--check", str(PATCH)], cwd=source, env=git_environment())
        run(["/usr/bin/git", "apply", str(PATCH)], cwd=source, env=git_environment())
        env = compiler_environment(root)
        run([GO, "mod", "verify"], cwd=source, env=env)
        receipt = run([GO, "test", "-json", "-race", "-mod=readonly", "./tunnel/statistic", "./hub/route",
                       "-run", "TestConditional", "-count=20"], cwd=source, env=dict(env, CGO_ENABLED="1"))
        matrix_receipt(receipt, CONDITIONAL_TESTS)
        print("conditional_core_close: passed; pinned source; race matrix; no installation")
        print("go_cases=7_conditional_x20; exact_nonempty_receipts")
        print("source_sha=" + PIN)
        print("patch_sha256=" + hashlib.sha256(PATCH.read_bytes()).hexdigest())


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError, tarfile.TarError):
        raise SystemExit("conditional_core_close: refused; no installation") from None
