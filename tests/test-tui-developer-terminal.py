#!/usr/bin/env python3
"""Production CLI in owned public PTYs; no runtime, IPC, store or network effects."""
import argparse
import fcntl
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import tempfile
import termios
import time
import tty
import unittest

ENTER = b"\x1b[?1049h"
LEAVE = b"\x1b[?1049l"
LIMIT = 262144


class DeveloperTerminalTests(unittest.TestCase):
    def environment(self, root):
        # Do not inherit the operator's runtime/configuration or locale.
        return {"PATH": "/usr/bin:/bin", "TERM": "xterm-256color",
                "LANG": "C.UTF-8", "OMAVLESS_LOCALE": "en", "HOME": str(root),
                "OMAVLESS_HOME": str(root), "XDG_RUNTIME_DIR": str(root),
                "XDG_STATE_HOME": str(root / "state"),
                "XDG_CONFIG_HOME": str(root / "config")}

    def arguments(self, selected):
        return [str(BINARY), "tui", "--developer-conditional-close"] + (
            ["--selected-set"] if selected else [])

    def enabled(self, selected):
        return MODE == "product" if selected else MODE in ("single", "product")

    def terminal(self, selected, action, baseline="canonical"):
        with tempfile.TemporaryDirectory(prefix="ovt-terminal-return-") as temporary:
            root = Path(temporary)
            master, slave = pty.openpty()
            child = None
            output = bytearray()
            try:
                fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 70, 0, 0))
                if baseline == "custom":
                    attributes = termios.tcgetattr(slave)
                    attributes[3] &= ~termios.ECHO
                    attributes[0] &= ~termios.IXON
                    termios.tcsetattr(slave, termios.TCSANOW, attributes)
                elif baseline == "raw":
                    tty.setraw(slave)
                original = termios.tcgetattr(slave)
                child = subprocess.Popen(
                    self.arguments(selected), stdin=slave,
                    stdout=slave, stderr=slave,
                    start_new_session=True, env=self.environment(root))
                deadline = time.monotonic() + 5

                def drain():
                    if select.select([master], [], [], 0.02)[0]:
                        output.extend(os.read(master, 4096))
                        self.assertLessEqual(len(output), LIMIT, "public PTY output bound")

                enabled = self.enabled(selected)
                if enabled:
                    while not (ENTER in output and b"OmaVLESS" in output):
                        self.assertIsNone(child.poll(), "CLI exited before its actual screen")
                        self.assertLess(time.monotonic(), deadline, "screen readiness deadline")
                        drain()
                    active = termios.tcgetattr(slave)
                    self.assertEqual(active[3] & (termios.ICANON | termios.ECHO | termios.ISIG), 0)
                    if isinstance(action, bytes):
                        os.write(master, action)
                    else:
                        child.send_signal(action)  # Only this owned effect-free CLI.
                while child.poll() is None:
                    self.assertLess(time.monotonic(), deadline, "original CLI exit deadline")
                    drain()
                self.assertLess(time.monotonic(), deadline, "original CLI exit deadline")
                while select.select([master], [], [], 0)[0]:
                    drain()
                expected = 0 if enabled else 2
                self.assertEqual(child.wait(timeout=1), expected, "original CLI status")
                self.assertEqual(termios.tcgetattr(slave), original,
                                 "exact initial termios must be restored, not reset to cooked defaults")
                if enabled:
                    self.assertIn(LEAVE, output, "alternate screen must be left")
                    self.assertGreater(output.rfind(LEAVE), output.find(ENTER))
                else:
                    self.assertNotIn(ENTER, output, "refusal must not enter alternate screen")
                self.assertEqual(list(root.iterdir()), [], "no runtime/store material may be created")
            finally:
                if child is not None and child.poll() is None:
                    # Test-owned public CLI only; never an installed daemon/core.
                    child.kill()
                    child.wait(timeout=2)
                os.close(master)
                os.close(slave)

    def test_normal_q_restores_each_initial_mode(self):
        for selected in (False, True):
            for baseline in ("canonical", "custom", "raw"):
                with self.subTest(selected=selected, baseline=baseline):
                    self.terminal(selected, b"q", baseline)

    def test_handled_signals_and_ctrl_c_restore_live_terminal(self):
        for selected in (False, True):
            for action in (b"\x03", signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
                with self.subTest(selected=selected, action=action):
                    self.terminal(selected, action)

    # A read-only stdout does not inject a Rust Stdout write failure: EBADF is
    # suppressed by the actual CLI. Partial-init fault restoration is not
    # claimed by these live-terminal return controls.
    def test_non_terminal_refusal_does_not_initialize_or_touch_runtime(self):
        for selected in (False, True):
            with self.subTest(selected=selected), tempfile.TemporaryDirectory(
                    prefix="ovt-terminal-pipe-") as temporary:
                root = Path(temporary)
                result = subprocess.run(self.arguments(selected), input=b"", capture_output=True,
                                        timeout=3, env=self.environment(root))
                self.assertEqual(result.returncode, 2)
                self.assertNotIn(ENTER, result.stdout + result.stderr)
                if self.enabled(selected):
                    self.assertIn(b"requires an interactive terminal", result.stderr)
                self.assertEqual(list(root.iterdir()), [])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("mode", choices=("none", "single", "product"))
    args = parser.parse_args()
    BINARY = args.binary.resolve(strict=True)
    MODE = args.mode
    unittest.main(argv=[__file__], verbosity=2)
