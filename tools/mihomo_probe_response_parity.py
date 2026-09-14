#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Archived Python behavior, replayed from independently recorded fixtures.
# Not a runtime backend or a general-purpose parser. See tests/frozen_reference/README.md.
import sys
from frozen_reference import main

if __name__ == "__main__":
    raise SystemExit(main(__file__, sys.argv[1:]))
