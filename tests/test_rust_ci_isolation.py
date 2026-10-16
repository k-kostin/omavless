"""Timing-fixture isolation retains execution in both runtime configurations."""
import re
import shlex
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class RustCiIsolationTests(unittest.TestCase):
    def test_full_source_push_lane_is_development_not_the_user_distribution(self):
        source = (ROOT / ".github/workflows/test.yml").read_text()
        self.assertIn("branches: [development]", source)
        self.assertIn("tags: ['**']", source)
        self.assertIn("  pull_request:", source)
        self.assertNotIn("dev/098-finalization-integration", source)
        self.assertNotIn("branches: [main", source)
        distribution = (ROOT / "packaging/release/distribution/test.yml").read_text()
        self.assertIn("branches: [main]", distribution)
        self.assertIn("source-bootstrap.py", distribution)

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
        names = [
            "conditional_close_candidate::tests::cancel_and_drain_held_exact_lease_times_out_and_permanently_cancels",
            "conditional_close_candidate::tests::cancel_and_drain_released_exact_lease_acknowledges_only_after_unlock",
            "native_coordinator::connection_close::tests::actual_owner_disconnect_drains_exact_proof_lease_or_returns_bounded_busy",
            "native_coordinator::connection_close::tests::actual_owner_disconnect_slow_core_preserves_graceful_stop_and_cleanup",
            "native_coordinator::connection_close::tests::actual_owner_disconnect_paused_proof_unwind_releases_before_fixture_cleanup",
        ]
        declaration = re.search(r"^close_drain_tests=\(\n(.*?)^\)", source, re.M | re.S)
        self.assertIsNotNone(declaration)
        self.assertEqual(shlex.split(declaration.group(1)), names)
        self.assertIn('close_drain_skips+=(--skip "$close_drain_test")', source)
        self.assertEqual(source.count('for close_drain_test in "${close_drain_tests[@]}"; do'), 3)
        commands = [shlex.split(line) for line in re.sub(r"\\\n", " ", source).splitlines()
                    if line.lstrip().startswith("cargo test ")]
        isolated = [c for c in commands if "$close_drain_test" in c]
        skipped = [c for c in commands if "${close_drain_skips[@]}" in c]
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
