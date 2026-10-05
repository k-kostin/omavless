#!/usr/bin/env python3
"""Test-only PTYs and synthetic client; no real runtime/store/network access."""
import argparse
import fcntl
import os
import pathlib
import pty
import select
import signal
import struct
import subprocess
import termios
import tempfile
import time
import unittest


class TerminalTests(unittest.TestCase):
    def action_wait_exit(self, close):
        """A synthetic 20-second mutation cannot own terminal shutdown."""
        master, slave = pty.openpty()
        original = termios.tcgetattr(slave)
        temporary = tempfile.TemporaryDirectory(prefix="omavless-tui-test-")
        receipt = pathlib.Path(temporary.name) / "action-started"
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 100, 0, 0))
        child = subprocess.Popen([str(BINARY.with_name("action_preview")), "slow", str(receipt)],
                                 stdin=slave, stdout=slave, stderr=slave,
                                 start_new_session=True,
                                 env={**os.environ, "TERM": "xterm-256color", "OMAVLESS_LOCALE": "en"})
        def wait_text(text):
            output = bytearray()
            deadline = time.monotonic() + 5
            while text not in output and time.monotonic() < deadline:
                if select.select([master], [], [], 0.1)[0]:
                    output.extend(os.read(master, 65536))
            self.assertIn(text, output, "synthetic action state not rendered")
        try:
            # Ratatui may cursor-address spaces, so do not match a full phrase.
            wait_text(b"Helsinki")
            os.write(master, b"d\r")
            deadline = time.monotonic() + 5
            while not receipt.exists() and time.monotonic() < deadline:
                if select.select([master], [], [], 0.05)[0]:
                    os.read(master, 65536)
            self.assertTrue(receipt.exists(), "synthetic action callback not reached")
            self.assertEqual(receipt.read_bytes(), b"synthetic-action-started\n")
            start = time.monotonic()
            if isinstance(close, bytes):
                os.write(master, close)
            else:
                child.send_signal(close)
            self.assertEqual(child.wait(timeout=2), 0)
            self.assertLess(time.monotonic() - start, 2)
            self.assertEqual(termios.tcgetattr(slave), original)
        finally:
            if child.poll() is None:
                child.kill()
                child.wait(timeout=2)
            os.close(master)
            os.close(slave)
            temporary.cleanup()

    def test_q_does_not_wait_for_or_cancel_action(self):
        self.action_wait_exit(b"q")

    def test_sigterm_during_action_restores_terminal(self):
        self.action_wait_exit(signal.SIGTERM)

    def run_terminal(self, action, scenario="", locale="en"):
        master, slave = pty.openpty()
        original = termios.tcgetattr(slave)
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 90, 0, 0))
        env = {**os.environ, "TERM": "xterm-256color", "OMAVLESS_LOCALE": locale}
        child = subprocess.Popen([str(BINARY), scenario], stdin=slave, stdout=slave,
                                 stderr=slave, env=env, start_new_session=True)
        output = bytearray()
        try:
            deadline = time.monotonic() + 5
            while b"OmaVLESS" not in output and time.monotonic() < deadline:
                if select.select([master], [], [], 0.1)[0]:
                    output.extend(os.read(master, 65536))
            self.assertIn(b"OmaVLESS", output, "synthetic screen did not initialize")
            start = time.monotonic()
            if isinstance(action, bytes):
                os.write(master, action)
            else:
                child.send_signal(action)
            self.assertEqual(child.wait(timeout=2), 0)
            self.assertLess(time.monotonic() - start, 2)
            self.assertEqual(termios.tcgetattr(slave), original, "terminal mode not restored")
        finally:
            if child.poll() is None:
                child.kill()
                child.wait(timeout=2)
            os.close(master)
            os.close(slave)

    def test_q_closes_while_read_is_slow(self):
        self.run_terminal(b"q", "slow")

    def test_ctrl_c_closes_in_search(self):
        self.run_terminal(b"/query\x03")

    def test_sigterm_restores_terminal(self):
        self.run_terminal(signal.SIGTERM)

    def test_hangup_does_not_wait_for_read(self):
        self.run_terminal(signal.SIGHUP, "slow")

    def test_help_and_russian_close(self):
        self.run_terminal(b"?q", locale="ru_RU.UTF-8")

    def test_runtime_unavailable_can_close(self):
        self.run_terminal(b"q", "unavailable")

    def test_real_terminal_teardown_cannot_leave_poll_spinning(self):
        # Unlike merely signalling a live PTY, this revokes the actual terminal
        # before sending HUP. Regression for orphaned Crossterm poll after close.
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 90, 0, 0))
        child = subprocess.Popen([str(BINARY)], stdin=slave, stdout=slave,
                                 stderr=slave, start_new_session=True,
                                 env={**os.environ, "TERM": "xterm-256color"})
        os.close(slave)
        try:
            output = bytearray()
            deadline = time.monotonic() + 5
            while b"OmaVLESS" not in output and time.monotonic() < deadline:
                if select.select([master], [], [], 0.1)[0]:
                    output.extend(os.read(master, 65536))
            self.assertIn(b"OmaVLESS", output, "synthetic screen did not initialize")
            os.close(master)
            master = None
            child.send_signal(signal.SIGHUP)
            self.assertIn(child.wait(timeout=2), (0, 1))
        finally:
            if master is not None:
                os.close(master)
            if child.poll() is None:
                child.kill()
                child.wait(timeout=2)

    def test_non_terminal_rejected(self):
        result = subprocess.run([str(BINARY)], input=b"", capture_output=True, timeout=3)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stderr.strip(), b"OmaVLESS TUI requires an interactive terminal")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=pathlib.Path)
    BINARY = parser.parse_args().binary.resolve(strict=True)
    unittest.main(argv=[__file__], verbosity=2)
