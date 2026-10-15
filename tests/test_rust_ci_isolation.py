"""Timing-fixture isolation retains execution in both runtime configurations."""
import re
import shlex
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class RustCiIsolationTests(unittest.TestCase):
    def test_superseded_pr_runs_cancel_without_skipping_latest_checks(self):
        source = (ROOT / ".github/workflows/test.yml").read_text()
        self.assertIn("group: ${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}", source)
        self.assertIn("cancel-in-progress: ${{ github.event_name == 'pull_request' }}", source)
        self.assertIn("run: ./tests/run.sh", source)
        self.assertIn("./tests/run-rust.sh", source)
        self.assertNotIn("paths-ignore:", source)
        self.assertIn("ref: ${{ github.event.pull_request.head.sha || github.sha }}", source)
        self.assertIn("persist-credentials: false", source)

    def test_exact_proof_drain_is_isolated_not_disabled(self):
        source = (ROOT / "tests/run-rust.sh").read_text()
        name = ("native_coordinator::connection_close::tests::"
                "actual_owner_disconnect_drains_exact_proof_lease_or_returns_bounded_busy")
        self.assertIn("close_drain_test='" + name + "'", source)
        commands = [shlex.split(line) for line in re.sub(r"\\\n", " ", source).splitlines()
                    if line.startswith("cargo test ")]
        isolated = [c for c in commands if "$close_drain_test" in c and "--skip" not in c]
        skipped = [c for c in commands if "$close_drain_test" in c and "--skip" in c]
        self.assertEqual(len(isolated), 2)
        self.assertEqual(len(skipped), 2)
        for c in isolated:
            self.assertIn("--lib", c)
            self.assertIn("--exact", c)
            self.assertIn("--test-threads=1", c)
            self.assertNotIn("--ignored", c)
            self.assertEqual(c[c.index("-p") + 1], "omavless-runtime")
        for group in (isolated, skipped):
            self.assertEqual(sum("--features" not in c for c in group), 1)
            union = next(c for c in group if "--features" in c)
            self.assertEqual(union[union.index("--features") + 1],
                             "product-image-witness,t4-manager-actor-service")


if __name__ == "__main__":
    unittest.main()
