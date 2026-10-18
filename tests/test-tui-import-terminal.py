#!/usr/bin/env python3
"""Owned synthetic PTY only. No native daemon, private store or provider access."""
import argparse
import fcntl
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import tempfile
import termios
import time


def check(binary, locale, confirm):
    with tempfile.TemporaryDirectory(prefix="tui-import-") as temporary:
        receipt = Path(temporary) / "original-calls"
        master, slave = pty.openpty()
        original = termios.tcgetattr(slave)
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 70, 0, 0))
        child = subprocess.Popen([str(binary), "bundle", str(receipt)], stdin=slave,
            stdout=slave, stderr=slave, start_new_session=True,
            env={**os.environ, "HOME": temporary, "TERM": "xterm-256color", "OMAVLESS_LOCALE": locale})
        output = bytearray()

        def drain():
            if select.select([master], [], [], 0.05)[0]:
                output.extend(os.read(master, 65536))
                assert len(output) <= 2 * 1024 * 1024, "synthetic terminal output bound"

        def wait(ready):
            deadline = time.monotonic() + 6
            while not ready() and time.monotonic() < deadline:
                drain()
            assert ready(), "synthetic terminal phase not reached"

        def calls():
            return receipt.read_bytes() if receipt.exists() else b""

        try:
            wait(lambda: b"Fixture" in output and b"F3" in output)
            # Focus verified from this owned PTY's actual legend before activation.
            output.clear()
            os.write(master, b"\x1bOR")  # Traditional F3.
            title = "Import" if locale == "en" else "Импорт"
            wait(lambda: title.encode() in output and b"Ctrl+U" in output)
            os.write(master, b"\x1b[200~synthetic one\x1b[201~")
            os.write(master, b"\x0e")  # Actual portable Ctrl+N, not a synthetic KeyEvent.
            os.write(master, b"\x1b[200~synthetic two\x1b[201~\r")
            wait(lambda: calls() == b"classify\n")
            # Original classifier callback validates EXACT multiline private input.
            assert b"synthetic one" not in output and b"synthetic two" not in output
            deadline = time.monotonic() + 0.4
            while time.monotonic() < deadline:
                drain()
            if confirm:
                os.write(master, b"\r")
                wait(lambda: calls() == b"classify\nsave\n")
            else:
                os.write(master, b"\x1b")
                deadline = time.monotonic() + 0.4
                while time.monotonic() < deadline:
                    drain()
                assert calls() == b"classify\n", "cancel must not call Save"
            os.write(master, b"\x03")
            assert child.wait(timeout=2) == 0
            assert termios.tcgetattr(slave) == original, "original terminal mode not restored"
        finally:
            if child.poll() is None:
                # This is ONLY our synthetic effect-free child, never a daemon/core.
                child.kill()
                child.wait(timeout=2)
            os.close(master)
            os.close(slave)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    args = parser.parse_args()
    args.binary = args.binary.resolve(strict=True)
    assert args.binary.is_file()
    for locale in ("en", "ru"):
        for confirm in (False, True):
            check(args.binary, locale, confirm)
    print("Synthetic import PTY: EN/RU, real F3/Ctrl+N/bracketed Paste, cancel/save and teardown PASS")
