#!/usr/bin/env python3
"""Stage pinned local acceptance binaries. Never downloads, builds or installs."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import tarfile


ROOT = Path(__file__).resolve().parent
ARCH = {"aarch64": 183, "x86_64": 62}
MAX_BINARY = 256 * 1024 * 1024
MAX_SOURCE = 256 * 1024 * 1024
PINNED_MIHOMO = "ab405bad5beeeac8b003bb01f60f134f6df54471"
PINNED_SING_TUN = "b50ae28a1409c7bce8e96e6c6966cf57d8ace754"
PINNED_PATCHES = {
    "mihomo-dns-broker.patch": "d5ebe9d6b37f6b76599fc3c2dd25adbfb774ca0121beeb79c5768a9a08d7ff37",
    "sing-tun-descriptor.patch": "2556c82aafbeb598a817d43042cf2069c6f209433c7a506395df581b4e31e2ab",
}
PAIR_FILES = ("mihomo", "omavless-dns-broker", "corresponding-source.tar.xz",
              "mihomo.LICENSE", "sing-tun.LICENSE", "omavless.LICENSE")
RECEIPT_KEYS = {"schema", "architecture", "omavless_commit", "mihomo_commit",
                "mihomo_tag", "sing_tun_commit", "sing_tun_tag", "patch_sha256",
                "go_version", "go_binary_sha256", "cargo_lock_sha256",
                "go_build_tags", "go_dependency_mode", "rustc_version",
                "cargo_version", "sha256"}


class Refused(ValueError):
    pass


def outside_git_destination(output):
    """Resolve symlink ancestors and reject normal, linked and bare Git trees."""
    output = Path(output)
    if not output.is_absolute() or output.name in ("", ".", ".."):
        raise Refused("Package staging requires an absolute new destination.")
    parent = output.parent.resolve(strict=True)
    metadata = parent.stat()
    if (not stat.S_ISDIR(metadata.st_mode) or metadata.st_uid != os.getuid()
            or metadata.st_mode & 0o077):
        raise Refused("Package staging requires a private owned parent.")
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


def reviewed_pair_file(directory, name, expected, maximum):
    if not isinstance(expected, str) or not re.fullmatch(r"[0-9a-f]{64}", expected):
        raise Refused("A lowercase SHA-256 pin is required for every paired input.")
    descriptor = os.open(directory / name,
                         os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, "rb") as stream:
        metadata = os.fstat(stream.fileno())
        if (not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != os.getuid()
                or metadata.st_nlink != 1 or not 1 <= metadata.st_size <= maximum):
            raise Refused("Paired input is not a bounded owned regular file.")
        content = stream.read(maximum + 1)
    if len(content) > maximum or hashlib.sha256(content).hexdigest() != expected:
        raise Refused("Paired input differs from its source receipt.")
    return content


def no_duplicate_keys(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise Refused("Duplicate source receipt key.")
        result[key] = value
    return result


def reviewed_pair(directory, architecture, revision):
    directory = Path(directory)
    metadata = directory.lstat()
    if (not directory.is_absolute() or not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid != os.getuid() or metadata.st_mode & 0o077):
        raise Refused("The source pair must be a private owned directory.")
    receipt_path = directory / "source-receipt.json"
    descriptor = os.open(receipt_path,
                         os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, "rb") as stream:
        metadata = os.fstat(stream.fileno())
        if (not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != os.getuid()
                or metadata.st_nlink != 1 or not 1 <= metadata.st_size <= 8192):
            raise Refused("The source receipt is not a bounded owned regular file.")
        raw = stream.read(8193)
    if len(raw) > 8192:
        raise Refused("The source receipt exceeds its bound.")
    try:
        receipt = json.loads(raw, object_pairs_hook=no_duplicate_keys)
    except (UnicodeError, ValueError) as error:
        raise Refused("The source receipt is invalid.") from error
    if (not isinstance(receipt, dict) or set(receipt) != RECEIPT_KEYS
            or type(receipt.get("schema")) is not int
            or receipt["schema"] != 1 or receipt.get("architecture") != architecture
            or receipt.get("omavless_commit") != revision
            or receipt.get("mihomo_commit") != PINNED_MIHOMO
            or receipt.get("mihomo_tag") != "v1.19.31"
            or receipt.get("sing_tun_commit") != PINNED_SING_TUN
            or receipt.get("sing_tun_tag") != "v0.4.24"
            or receipt.get("patch_sha256") != PINNED_PATCHES
            or receipt.get("go_build_tags") != "with_gvisor"
            or receipt.get("go_dependency_mode") != "vendor"
            or not isinstance(receipt.get("sha256"), dict)
            or set(receipt["sha256"]) != set(PAIR_FILES)
            or not isinstance(receipt.get("go_version"), str)
            or not re.fullmatch(r"go version go1\.[0-9]+\.[0-9]+(?:-[A-Za-z0-9:]+)? linux/(?:amd64|arm64)",
                                receipt["go_version"])
            or not receipt["go_version"].endswith(
                "linux/arm64" if architecture == "aarch64" else "linux/amd64")
            or not isinstance(receipt.get("go_binary_sha256"), str)
            or not re.fullmatch(r"[0-9a-f]{64}", receipt["go_binary_sha256"])
            or not isinstance(receipt.get("cargo_lock_sha256"), str)
            or not re.fullmatch(r"[0-9a-f]{64}", receipt["cargo_lock_sha256"])
            or not isinstance(receipt.get("rustc_version"), str)
            or not isinstance(receipt.get("cargo_version"), str)
            or len(receipt["rustc_version"]) > 128 or len(receipt["cargo_version"]) > 128
            or any(ord(char) < 32 or ord(char) > 126 for char in
                   receipt["rustc_version"] + receipt["cargo_version"])):
        raise Refused("The source pair does not match the fixed review contract.")
    payload = {name: reviewed_pair_file(directory, name, receipt["sha256"][name],
                                        MAX_BINARY if name in ("mihomo", "omavless-dns-broker")
                                        else MAX_SOURCE if name.endswith(".tar.xz") else 65536)
               for name in PAIR_FILES}
    for name in ("mihomo", "omavless-dns-broker"):
        content = payload[name]
        if (content[:6] != b"\x7fELF\x02\x01"
                or int.from_bytes(content[18:20], "little") != ARCH[architecture]):
            raise Refused("Source-paired binary has the wrong ELF architecture.")
    required = {"mihomo/go.mod", "mihomo/vendor/modules.txt", "mihomo/LICENSE",
                "sing-tun/go.mod", "sing-tun/LICENSE", "omavless/Cargo.lock",
                "omavless/LICENSE"}
    seen = set()
    try:
        with tarfile.open(fileobj=io.BytesIO(payload["corresponding-source.tar.xz"]),
                          mode="r:xz") as archive:
            total = 0
            for member in archive:
                path = member.name.removesuffix("/")
                parts = path.split("/")
                if (not parts or parts[0] not in {"mihomo", "sing-tun", "omavless"}
                        or any(part in {"", ".", ".."} for part in parts)
                        or not (member.isfile() or member.isdir())):
                    raise Refused("The corresponding-source archive has an unsafe entry.")
                total += member.size
                if total > 1024 * 1024 * 1024 or len(seen) >= 250000 or path in seen:
                    raise Refused("The corresponding-source archive exceeds its bounds.")
                seen.add(path)
            if not required <= seen:
                raise Refused("The corresponding-source archive is incomplete.")
            for source, separate in (("mihomo/LICENSE", "mihomo.LICENSE"),
                                     ("sing-tun/LICENSE", "sing-tun.LICENSE"),
                                     ("omavless/LICENSE", "omavless.LICENSE")):
                if not archive.getmember(source).isfile():
                    raise Refused("A source license is not a regular file.")
                with archive.extractfile(source) as stream:
                    if stream.read(65537) != payload[separate]:
                        raise Refused("A paired license differs from its source archive.")
    except (OSError, tarfile.TarError, EOFError) as error:
        raise Refused("The corresponding-source archive is invalid.") from error
    payload["source-receipt.json"] = raw
    return payload


def stage(pair, architecture, revision, output):
    if architecture not in ARCH or not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise Refused("A supported architecture and exact source revision are required.")
    output = outside_git_destination(output)
    payload = reviewed_pair(pair, architecture, revision)
    payload.update({
        "omavless-dns-broker.service": (ROOT.parent / "omavless-dns-broker.service").read_bytes(),
        "package-guard": (ROOT / "package-guard").read_bytes(),
        "omavless-dns-experimental.hook": (ROOT / "omavless-dns-experimental.hook").read_bytes(),
        "omavless-dns-experimental.install": (ROOT / "omavless-dns-experimental.install").read_bytes(),
    })
    manifest = {"schema": 2, "architecture": architecture, "source_revision": revision,
                "sha256": {name: hashlib.sha256(value).hexdigest() for name, value in payload.items()}}
    payload["reviewed-inputs.json"] = (json.dumps(manifest, sort_keys=True, indent=2) + "\n").encode()
    substitutions = {
        "ARCH": architecture,
        "BROKER_SHA": manifest["sha256"]["omavless-dns-broker"],
        "CORE_SHA": manifest["sha256"]["mihomo"],
        "SOURCE_SHA": manifest["sha256"]["corresponding-source.tar.xz"],
        "SOURCE_RECEIPT_SHA": manifest["sha256"]["source-receipt.json"],
        "MIHOMO_LICENSE_SHA": manifest["sha256"]["mihomo.LICENSE"],
        "SING_TUN_LICENSE_SHA": manifest["sha256"]["sing-tun.LICENSE"],
        "OMAVLESS_LICENSE_SHA": manifest["sha256"]["omavless.LICENSE"],
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
    for option in ("pair", "revision", "output"):
        parser.add_argument("--" + option, required=True)
    parser.add_argument("--arch", choices=tuple(ARCH), required=True)
    args = parser.parse_args()
    try:
        stage(args.pair, args.arch, args.revision, args.output)
    except (OSError, Refused):
        parser.exit(1, "Package staging refused; inspect reviewed local inputs and choose a new empty destination.\n")
    print("Pinned experimental package sources staged; nothing built or installed.")


if __name__ == "__main__":
    main()
