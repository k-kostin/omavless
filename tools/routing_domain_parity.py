#!/usr/bin/env python3
"""Credential-safe Python oracle for pure R3 routing semantics."""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import backend  # noqa: E402

SAFE_ID = re.compile(r"[A-Za-z0-9_.-]{1,96}\Z")


def digest(value: str) -> str:
    return hashlib.sha256(value.encode()).hexdigest()


def classify(message: str) -> str:
    for marker, code in (
        ("Unsupported custom routing match type", "unsupported_kind"),
        ("Unsupported custom routing action", "unsupported_action"),
        ("must not be empty", "empty_value"),
        ("without a scheme", "invalid_domain"),
        ("valid domain", "invalid_domain"),
        ("valid IDNA", "invalid_domain"),
        ("Domain is too long", "invalid_domain"),
        ("IP address or CIDR", "invalid_network"),
        ("one top-level rules section", "missing_rules_section"),
        ("Unsupported routing mode", "unsupported_mode"),
        ("more than one top-level mode", "multiple_modes"),
    ):
        if marker in message:
            return code
    return "internal_error"


def main(argv: list[str]) -> int:
    if argv: return 2
    try:
        envelope = json.loads(sys.stdin.buffer.read(1024 * 1024 + 1).decode("utf-8"))
        if not isinstance(envelope, dict) or set(envelope) != {"cases"}: return 1
        results = []
        for case in envelope["cases"]:
            if not isinstance(case, dict) or set(case) != {"id", "kind", "action", "value", "template", "mode"}: return 1
            if not SAFE_ID.fullmatch(case["id"]): return 1
            try:
                kind, action = case["kind"], case["action"]
                if kind not in {"domain", "suffix", "ipcidr"}:
                    raise backend.BackendError("Unsupported custom routing match type")
                if action not in {"proxy", "direct", "reject"}:
                    raise backend.BackendError("Unsupported custom routing action")
                canonical = backend.canonical_custom_rule_value(kind, case["value"])
                rule = {"kind": kind, "value": canonical, "action": action}
                matches = [line for line in case["template"].splitlines()
                           if re.match(r"^rules\s*:\s*(?:#.*)?$", line)]
                if len(matches) > 1:
                    classification = "multiple_rules_sections"
                    raise ValueError
                injected = backend.inject_custom_rules(case["template"], [rule])
                rewritten = backend.template_with_mode(injected, case["mode"])
                outcome = {"accepted": True, "classification": "accepted",
                           "canonical_fingerprint": digest(canonical),
                           "rule_fingerprint": digest(backend.custom_rule_line(rule)),
                           "template_fingerprint": digest(rewritten)}
            except backend.BackendError as error:
                outcome = {"accepted": False, "classification": classify(str(error)),
                           "canonical_fingerprint": "", "rule_fingerprint": "",
                           "template_fingerprint": ""}
            except ValueError:
                outcome = {"accepted": False, "classification": classification,
                           "canonical_fingerprint": "", "rule_fingerprint": "",
                           "template_fingerprint": ""}
            results.append({"id": case["id"], **outcome})
        sys.stdout.write(json.dumps({"cases": results}, sort_keys=True, separators=(",", ":")))
        return 0
    except Exception:
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
