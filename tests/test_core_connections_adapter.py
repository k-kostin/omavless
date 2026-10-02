# SPDX-License-Identifier: MIT
"""Offline tests for the review-only core gate; no installed controller access."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "tests/core_connections_adapter" / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


LIVE = load("loopback")
REVIEW = load("review")


class ConditionalCoreAdapterTests(unittest.TestCase):
    def test_duplicate_keys_refuse_without_exposing_input(self):
        with self.assertRaisesRegex(ValueError, "^Duplicate controller key$"):
            json.loads('{"secret":"synthetic-private","secret":"other"}', object_pairs_hook=LIVE.exact_pairs)
        self.assertEqual(LIVE.exact_pairs([("id", "one"), ("token", "two")]), {"id": "one", "token": "two"})

    def test_scratch_and_core_refusal_cannot_start_process(self):
        with tempfile.TemporaryDirectory(prefix="conditional-unit-") as name:
            root = Path(name)
            core = root / "candidate"
            core.write_bytes(b"synthetic-not-executable")
            with patch.object(LIVE.subprocess, "Popen") as start:
                for bad in [Path("relative"), root / "missing"]:
                    with self.assertRaises(ValueError):
                        LIVE.exercise(bad, root)
                root.chmod(0o755)
                with self.assertRaises(ValueError):
                    LIVE.exercise(core, root)
                root.chmod(0o700)
                link = root / "link"
                link.symlink_to(core)
                with self.assertRaises(ValueError):
                    LIVE.exercise(link, root)
                start.assert_not_called()

    def test_controller_request_is_bodyless_and_bound_to_loopback(self):
        with patch.object(LIVE.http.client, "HTTPConnection") as factory:
            connection = factory.return_value
            response = connection.getresponse.return_value
            response.status = 204
            response.read.return_value = b""
            self.assertEqual(LIVE.control(12345, "synthetic", "POST", "/connections/id/close-conditional", "42"), (204, b""))
            factory.assert_called_once_with("127.0.0.1", 12345, timeout=2)
            connection.request.assert_called_once_with("POST", "/connections/id/close-conditional",
                headers={"Authorization": "Bearer synthetic", "If-Match": '"42"'})
            connection.close.assert_called_once()

    def test_controller_response_has_strict_bound(self):
        with patch.object(LIVE.http.client, "HTTPConnection") as factory:
            response = factory.return_value.getresponse.return_value
            response.read.return_value = b"x" * (1024 * 1024 + 1)
            with self.assertRaisesRegex(ValueError, "^Oversized controller response$"):
                LIVE.control(12345, "synthetic", "GET", "/connections")

    def test_pinned_source_and_offline_patch_are_present(self):
        self.assertEqual(REVIEW.PIN, "ab405bad5beeeac8b003bb01f60f134f6df54471")
        content = REVIEW.PATCH.read_text()
        for required in ["CloseIfToken", "close-conditional", "If-Match", "TestConditionalCloseDelayedLeave", "math.MaxUint64"]:
            self.assertIn(required, content)
        self.assertNotIn("sudo", content)

    def test_failed_review_step_redacts_subprocess_output(self):
        result = REVIEW.subprocess.CompletedProcess(["synthetic"], 1, b"private-input", b"private-error")
        with patch.object(REVIEW.subprocess, "run", return_value=result):
            with self.assertRaisesRegex(RuntimeError, "^Pinned core review step failed$"):
                REVIEW.run(["synthetic"])


if __name__ == "__main__":
    unittest.main()
