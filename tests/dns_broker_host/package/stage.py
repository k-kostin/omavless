#!/usr/bin/env python3
"""Stage pinned local acceptance binaries. Never downloads, builds or installs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess


ROOT = Path(__file__).resolve().parent
ARCH = {"aarch64": 183, "x86_64": 62}
MAX_BINARY = 256 * 1024 * 1024


class Refused(ValueError):
    pass


def outside_git_destination(output):
    """Resolve symlink ancestors and reject normal, linked and bare Git trees."""
    output = Path(output)
    parent = output.parent.resolve(strict=True)
    # Do not inherit GIT_DIR/GIT_WORK_TREE, global includes or discovery limits.
    try:
        result = subprocess.run(
            ["/usr/bin/git", "--no-optional-locks", "-C", str(parent),
             "rev-parse", "--absolute-git-dir"],
            stdin=subprocess.DEVNULL, capture_output=True, timeout=10, check=False,
            env={"PATH": "/usr/bin:/bin", "LC_ALL": "C",
                 "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
                 "GIT_DISCOVERY_ACROSS_FILESYSTEM": "1"},
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise Refused("Outside-Git destination could not be verified.") from error
    if result.returncode != 128 or not result.stderr.startswith(b"fatal: not a git repository"):
        raise Refused("Package staging requires a destination outside Git.")
    return parent / output.name


def reviewed_binary(path, expected, architecture):
    if not re.fullmatch(r"[0-9a-f]{64}", expected):
        raise Refused("A lowercase SHA-256 pin is required.")
    descriptor = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, "rb") as stream:
        metadata = os.fstat(stream.fileno())
        if (not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != os.getuid()
                or metadata.st_nlink != 1 or not 20 <= metadata.st_size <= MAX_BINARY):
            raise Refused("Reviewed input must be a bounded owned regular binary.")
        content = stream.read(MAX_BINARY + 1)
    if len(content) > MAX_BINARY or hashlib.sha256(content).hexdigest() != expected:
        raise Refused("Reviewed binary does not match its supplied pin.")
    if (content[:6] != b"\x7fELF\x02\x01"
            or int.from_bytes(content[18:20], "little") != ARCH[architecture]):
        raise Refused("Reviewed binary is not native ELF64 for the selected architecture.")
    return content


def stage(broker, core, broker_sha, core_sha, architecture, revision, output):
    if architecture not in ARCH or not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise Refused("A supported architecture and exact source revision are required.")
    output = outside_git_destination(output)
    payload = {
        "omavless-dns-broker": reviewed_binary(broker, broker_sha, architecture),
        "mihomo": reviewed_binary(core, core_sha, architecture),
        "omavless-dns-broker.service": (ROOT.parent / "omavless-dns-broker.service").read_bytes(),
        "package-guard": (ROOT / "package-guard").read_bytes(),
        "omavless-dns-experimental.hook": (ROOT / "omavless-dns-experimental.hook").read_bytes(),
        "omavless-dns-experimental.install": (ROOT / "omavless-dns-experimental.install").read_bytes(),
    }
    manifest = {"schema": 1, "architecture": architecture, "source_revision": revision,
                "sha256": {name: hashlib.sha256(value).hexdigest() for name, value in payload.items()}}
    payload["reviewed-inputs.json"] = (json.dumps(manifest, sort_keys=True, indent=2) + "\n").encode()
    substitutions = {
        "ARCH": architecture, "BROKER_SHA": broker_sha, "CORE_SHA": core_sha,
        "UNIT_SHA": manifest["sha256"]["omavless-dns-broker.service"],
        "GUARD_SHA": manifest["sha256"]["package-guard"],
        "HOOK_SHA": manifest["sha256"]["omavless-dns-experimental.hook"],
        "INSTALL_SHA": manifest["sha256"]["omavless-dns-experimental.install"],
        "MANIFEST_SHA": hashlib.sha256(payload["reviewed-inputs.json"]).hexdigest(),
    }
    recipe = (ROOT / "PKGBUILD.in").read_text(encoding="utf-8")
    for key, value in substitutions.items():
        if f"@{key}@" not in recipe:
            raise Refused("Package template does not match its fixed schema.")
        recipe = recipe.replace(f"@{key}@", value)
    if "@" in recipe:
        raise Refused("Package template has unresolved fields.")
    output.mkdir(mode=0o700)  # Existing directories/symlinks are never reused.
    for name, content in payload.items():
        path = output / name
        with path.open("xb") as stream:
            stream.write(content)
        path.chmod(0o600)
    # Recipe appears last: failed staging is not a complete build directory.
    with (output / "PKGBUILD").open("x", encoding="utf-8") as stream:
        stream.write(recipe)
    (output / "PKGBUILD").chmod(0o600)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for option in ("broker", "core", "broker-sha", "core-sha", "revision", "output"):
        parser.add_argument("--" + option, required=True)
    parser.add_argument("--arch", choices=tuple(ARCH), required=True)
    args = parser.parse_args()
    try:
        stage(args.broker, args.core, args.broker_sha, args.core_sha,
              args.arch, args.revision, args.output)
    except (OSError, Refused):
        parser.exit(1, "Package staging refused; inspect reviewed local inputs and choose a new empty destination.\n")
    print("Pinned experimental package sources staged; nothing built or installed.")


if __name__ == "__main__":
    main()
