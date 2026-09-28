#!/usr/bin/env python3
"""Stage an offline, unpublished production-name DNS companion candidate.

This never installs, enrolls, enables a unit, connects, or publishes anything.
The caller must independently review the pinned source and build provenance.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tests/dns_broker_host/package"))
import stage as fixture  # noqa: E402 - shared strict offline pair validation

HERE = Path(__file__).resolve().parent


def package_version(cargo_text, manifest_text):
    try:
        cargo_version = tomllib.loads(cargo_text)["workspace"]["package"]["version"]
        frontend_version = json.loads(manifest_text)["version"]
    except (KeyError, TypeError, ValueError) as error:
        raise fixture.Refused("Product version metadata is invalid.") from error
    if (not isinstance(cargo_version, str) or cargo_version != frontend_version
            or not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:-rc\.[1-9][0-9]*)?",
                                cargo_version)):
        raise fixture.Refused("Companion version does not match the frontend and runtime.")
    return cargo_version.replace("-rc.", "rc")


def stage(pair, architecture, revision, output):
    if architecture not in fixture.ARCH or not fixture.re.fullmatch(r"[0-9a-f]{40}", revision):
        raise fixture.Refused("A supported architecture and exact source revision are required.")
    output = fixture.outside_git_destination(output)
    payload = fixture.reviewed_pair(pair, architecture, revision, "release")
    unit = (ROOT / "tests/dns_broker_host/omavless-dns-broker.service").read_bytes()
    old = b"ExecStart=/usr/lib/omavless/omavless-dns-broker --serve"
    if unit.count(old) != 1:
        raise fixture.Refused("The reviewed broker unit changed.")
    unit = unit.replace(old, b"ExecStart=/usr/lib/omavless-dns/omavless-dns-broker --serve")
    unit = unit.replace(b"# REVIEW-ONLY opt-in candidate. Not installed or enabled by normal packages.\n",
                        b"# Installation alone does not enroll, enable, or start this service.\n")
    payload.update({
        "omavless-dns-broker.service": unit,
        "package-guard": (ROOT / "tests/dns_broker_host/package/package-guard").read_bytes(),
        "omavless-dns.hook": (HERE / "omavless-dns.hook").read_bytes(),
        "omavless-dns.install": (HERE / "omavless-dns.install").read_bytes(),
    })
    manifest = {
        "schema": 3, "package": "omavless-dns", "architecture": architecture,
        "source_revision": revision,
        "sha256": {name: hashlib.sha256(value).hexdigest() for name, value in payload.items()},
    }
    payload["reviewed-inputs.json"] = (json.dumps(manifest, sort_keys=True, indent=2) + "\n").encode()
    values = {
        "ARCH": architecture,
        "VERSION": package_version((ROOT / "Cargo.toml").read_text(encoding="utf-8"),
                                   (ROOT / "manifest.json").read_text(encoding="utf-8")),
        "BROKER_SHA": manifest["sha256"]["omavless-dns-broker"],
        "CORE_SHA": manifest["sha256"]["mihomo"],
        "UNIT_SHA": manifest["sha256"]["omavless-dns-broker.service"],
        "GUARD_SHA": manifest["sha256"]["package-guard"],
        "HOOK_SHA": manifest["sha256"]["omavless-dns.hook"],
        "INSTALL_SHA": manifest["sha256"]["omavless-dns.install"],
        "MANIFEST_SHA": hashlib.sha256(payload["reviewed-inputs.json"]).hexdigest(),
        "SOURCE_RECEIPT_SHA": manifest["sha256"]["source-receipt.json"],
        "SOURCE_SHA": manifest["sha256"]["corresponding-source.tar.xz"],
        "MIHOMO_LICENSE_SHA": manifest["sha256"]["mihomo.LICENSE"],
        "SING_TUN_LICENSE_SHA": manifest["sha256"]["sing-tun.LICENSE"],
        "OMAVLESS_LICENSE_SHA": manifest["sha256"]["omavless.LICENSE"],
    }
    recipe = (HERE / "PKGBUILD.in").read_text(encoding="utf-8")
    for key, value in values.items():
        marker = f"@{key}@"
        if marker not in recipe:
            raise fixture.Refused("Package template does not match its fixed schema.")
        recipe = recipe.replace(marker, value)
    if "@" in recipe:
        raise fixture.Refused("Package template has unresolved fields.")
    output.mkdir(mode=0o700)
    for name, content in payload.items():
        path = output / name
        with path.open("xb") as stream:
            stream.write(content)
        path.chmod(0o600)
    with (output / "PKGBUILD").open("x", encoding="utf-8") as stream:
        stream.write(recipe)
    (output / "PKGBUILD").chmod(0o600)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for option in ("pair", "revision", "output"):
        parser.add_argument("--" + option, required=True)
    parser.add_argument("--arch", choices=tuple(fixture.ARCH), required=True)
    args = parser.parse_args()
    try:
        stage(args.pair, args.arch, args.revision, args.output)
    except (OSError, fixture.Refused):
        parser.exit(1, "DNS companion staging refused; nothing installed or activated.\n")
    print("Unpublished DNS companion sources staged; nothing built or installed.")


if __name__ == "__main__":
    main()
