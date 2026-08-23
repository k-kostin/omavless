#!/bin/sh
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 OmaVLESS contributors
# QML calls this stable launcher; the implementation stays testable Python.
exec python3 "$(dirname "$0")/backend.py" "$@"
