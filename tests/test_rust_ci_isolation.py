"""Timing-fixture isolation retains execution in both runtime configurations."""
import hashlib
import json
import re
import shlex
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class RustCiIsolationTests(unittest.TestCase):
    def runner_trace(self, fail_test=False):
        """Execute only the shell recipe; every external leaf is an inert stub."""
        with tempfile.TemporaryDirectory(prefix="rust-recipe-") as directory:
            scratch = Path(directory)
            tools = scratch / "bin"
            tools.mkdir()
            trace = scratch / "trace.jsonl"
            stub = tools / "stub"
            stub.write_text(
                "#!/usr/bin/python3\n"
                "import json, os, sys\n"
                "from pathlib import Path\n"
                "row = [Path(sys.argv[0]).name, *sys.argv[1:]]\n"
                "with open(os.environ['RECIPE_TRACE'], 'a') as stream:\n"
                "    stream.write(json.dumps(row) + '\\n')\n"
                "if row[:2] == ['cargo', 'test'] and os.environ.get('RECIPE_FAIL') == '1':\n"
                "    sys.exit(17)\n"
            )
            stub.chmod(0o755)
            for name in ("cargo", "rustfmt", "python3", "cc"):
                (tools / name).symlink_to(stub)
            result = subprocess.run(
                ["/bin/bash", str(ROOT / "tests/run-rust.sh")], cwd=ROOT,
                env={"PATH": str(tools) + ":/usr/bin:/bin", "HOME": directory,
                     "CARGO_TARGET_DIR": "target", "RECIPE_TRACE": str(trace),
                     "RECIPE_FAIL": "1" if fail_test else "0"},
                capture_output=True, text=True, timeout=10,
            )
            return result, [json.loads(line) for line in trace.read_text().splitlines()]

    def test_sha2_test_profile_preserves_the_complete_recipe(self):
        result, commands = self.runner_trace()
        self.assertEqual(result.returncode, 0, result.stderr)
        profile = [
            "--config", "profile.test.package.sha2.opt-level=3",
            "--config", "profile.test.package.sha2.debug-assertions=true",
            "--config", "profile.test.package.sha2.overflow-checks=true",
        ]
        normalized = []
        for command in commands:
            if command[:2] == ["cargo", "test"]:
                self.assertEqual(command[2:8], profile)
                command = command[:2] + command[8:]
            self.assertFalse(any("profile." in arg for arg in command))
            normalized.append(command)
        # Captured from the pre-optimization recipe: all expanded commands,
        # features, filters, skips, isolated deadlines, lint and parity entries.
        payload = json.dumps(normalized, separators=(",", ":")).encode()
        self.assertEqual(hashlib.sha256(payload).hexdigest(),
                         "320a9b1a338fb1f4f0aa66f35741bbb98ef1ed47e533a6403ccb43294b03af83")

    def test_sha2_runner_stops_on_the_original_test_failure(self):
        result, commands = self.runner_trace(fail_test=True)
        self.assertEqual(result.returncode, 17, result.stderr)
        self.assertEqual([row[:2] for row in commands],
                         [["cargo", "fmt"], ["cargo", "test"]])

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
