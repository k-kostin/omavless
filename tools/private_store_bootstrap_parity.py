#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Emit the credential-free canonical Python first-install store payload."""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))

import backend  # noqa: E402


def main() -> int:
    sys.stdout.write(
        json.dumps(backend.empty_store(), ensure_ascii=False, indent=2) + "\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
