"""Test-only crypto profiles and fixture isolation preserve the complete recipe."""
import hashlib
import json
import re
import shlex
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CANONICAL_STAGE_TEST = "manager_actor_service::transfer::tests::failed_canonical_stage_keeps_auth_and_normalized_private_returns"
AUXILIARY_TEARDOWN_TEST = "auxiliary_core::tests::finish_inventory_refusal_retains_owned_child_and_first_failure_evidence"
STARTUP_CAPABILITY_TEST = "startup_validation::tests::startup_capability_probe_is_fixed_bounded_and_checks_effective_flags"
ARGON2_TEST_PROFILE = [
    "--config", "profile.test.package.argon2.opt-level=3",
    "--config", "profile.test.package.argon2.debug-assertions=true",
    "--config", "profile.test.package.argon2.overflow-checks=true",
]


class RustCiIsolationTests(unittest.TestCase):
    def without_startup_scheduling(self, commands):
        """Undo only the two independently asserted skip/exact pairs."""
        selected = [c for c in commands if STARTUP_CAPABILITY_TEST in c]
        self.assertEqual(len(selected), 4)
        for features in (None, "product-image-witness,t4-manager-actor-service"):
            same = [c for c in selected if
                    (c[c.index("--features") + 1] if "--features" in c else None) == features]
            self.assertEqual(len(same), 2)
            isolated = next(c for c in same if "--exact" in c)
            skipped = next(c for c in same if "--skip" in c)
            self.assertEqual(isolated[-4:],
                             [STARTUP_CAPABILITY_TEST, "--", "--exact", "--test-threads=1"])
            self.assertIn("--lib", isolated)
            self.assertIn("--locked", isolated)
            self.assertEqual(isolated[isolated.index("-p") + 1], "omavless-runtime")
            self.assertNotIn("--ignored", isolated)
            self.assertLess(commands.index(isolated), commands.index(skipped))
            expected = ["cargo", "test"]
            if isolated[2:3] == ["--config"]:
                expected += [
                    "--config", "profile.test.package.sha2.opt-level=3",
                    "--config", "profile.test.package.sha2.debug-assertions=true",
                    "--config", "profile.test.package.sha2.overflow-checks=true",
                ] + ARGON2_TEST_PROFILE
            expected += ["--locked", "-p", "omavless-runtime", "--lib"]
            if features is not None:
                expected += ["--features", features]
            expected += [STARTUP_CAPABILITY_TEST, "--", "--exact", "--test-threads=1"]
            self.assertEqual(isolated, expected)
        restored = []
        for row in commands:
            command = row[:]
            if STARTUP_CAPABILITY_TEST in command:
                if "--exact" in command:
                    continue
                index = command.index(STARTUP_CAPABILITY_TEST)
                self.assertEqual(command[index - 1], "--skip")
                del command[index - 1:index + 1]
            restored.append(command)
        return restored

    def without_auxiliary_scheduling(self, commands):
        """Undo only the two independently asserted skip/exact pairs."""
        selected = [c for c in commands if AUXILIARY_TEARDOWN_TEST in c]
        self.assertEqual(len(selected), 4)
        for features in (None, "product-image-witness,t4-manager-actor-service"):
            same = [c for c in selected if
                    (c[c.index("--features") + 1] if "--features" in c else None) == features]
            self.assertEqual(len(same), 2)
            isolated = next(c for c in same if "--exact" in c)
            skipped = next(c for c in same if "--skip" in c)
            self.assertEqual(isolated[-4:],
                             [AUXILIARY_TEARDOWN_TEST, "--", "--exact", "--test-threads=1"])
            self.assertIn("--lib", isolated)
            self.assertIn("--locked", isolated)
            self.assertEqual(isolated[isolated.index("-p") + 1], "omavless-runtime")
            self.assertNotIn("--ignored", isolated)
            self.assertLess(commands.index(isolated), commands.index(skipped))
            expected = ["cargo", "test"]
            if isolated[2:3] == ["--config"]:
                expected += [
                    "--config", "profile.test.package.sha2.opt-level=3",
                    "--config", "profile.test.package.sha2.debug-assertions=true",
                    "--config", "profile.test.package.sha2.overflow-checks=true",
                ] + ARGON2_TEST_PROFILE
            expected += ["--locked", "-p", "omavless-runtime", "--lib"]
            if features is not None:
                expected += ["--features", features]
            expected += [AUXILIARY_TEARDOWN_TEST, "--", "--exact", "--test-threads=1"]
            self.assertEqual(isolated, expected)
        restored = []
        for row in commands:
            command = row[:]
            if AUXILIARY_TEARDOWN_TEST in command:
                if "--exact" in command:
                    continue
                index = command.index(AUXILIARY_TEARDOWN_TEST)
                self.assertEqual(command[index - 1], "--skip")
                del command[index - 1:index + 1]
            restored.append(command)
        return restored

    def test_auxiliary_inventory_teardown_is_exact_and_isolated_in_both_configurations(self):
        result, commands = self.runner_trace()
        self.assertEqual(result.returncode, 0, result.stderr)
        restored = self.without_auxiliary_scheduling(commands)
        self.assertEqual(len(commands) - len(restored), 2)
        self.assertFalse(any(AUXILIARY_TEARDOWN_TEST in c for c in restored))

    def runner_trace(self, fail_test=False, fail_canonical_stage=False, fail_clippy=False,
                     fail_auxiliary_inventory=False, fail_image_upgrade=False,
                     fail_reply_loss=False, fail_terminal_return=False, fail_startup_capability=False):
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
                "if row[:2] == ['cargo', 'clippy'] and os.environ.get('RECIPE_FAIL_CLIPPY') == '1':\n"
                "    sys.exit(23)\n"
                "if os.environ.get('RECIPE_FAIL_CANONICAL_STAGE') == '1' and '--exact' in row and "
                + repr(CANONICAL_STAGE_TEST) + " in row:\n"
                "    sys.exit(19)\n"
                "if os.environ.get('RECIPE_FAIL_AUXILIARY_INVENTORY') == '1' and '--exact' in row and "
                + repr(AUXILIARY_TEARDOWN_TEST) + " in row:\n"
                "    sys.exit(21)\n"
                "if os.environ.get('RECIPE_FAIL_STARTUP_CAPABILITY') == '1' and '--exact' in row and "
                + repr(STARTUP_CAPABILITY_TEST) + " in row:\n"
                "    sys.exit(39)\n"
                "if os.environ.get('RECIPE_FAIL_IMAGE_UPGRADE') == '1' and "
                "row[:2] == ['cargo', 'test'] and 'omavless-image-witness' in row:\n"
                "    sys.exit(29)\n"
                "if os.environ.get('RECIPE_FAIL_TERMINAL_RETURN') == '1' and "
                "row[:2] == ['cargo', 'test'] and 'developer_terminal_pty_' in row:\n"
                "    sys.exit(37)\n"
                "if os.environ.get('RECIPE_FAIL_REPLY_LOSS') == '1' and "
                "row[:2] == ['cargo', 'test'] and 'reply_loss_' in row:\n"
                "    sys.exit(31)\n"
            )
            stub.chmod(0o755)
            for name in ("cargo", "rustfmt", "python3", "cc", "bash"):
                (tools / name).symlink_to(stub)
            result = subprocess.run(
                ["/bin/bash", str(ROOT / "tests/run-rust.sh")], cwd=ROOT,
                env={"PATH": str(tools) + ":/usr/bin:/bin", "HOME": directory,
                     "CARGO_TARGET_DIR": "target", "RECIPE_TRACE": str(trace),
                     "RECIPE_FAIL": "1" if fail_test else "0",
                     "RECIPE_FAIL_CLIPPY": "1" if fail_clippy else "0",
                     "RECIPE_FAIL_AUXILIARY_INVENTORY": "1" if fail_auxiliary_inventory else "0",
                     "RECIPE_FAIL_CANONICAL_STAGE": "1" if fail_canonical_stage else "0",
                     "RECIPE_FAIL_IMAGE_UPGRADE": "1" if fail_image_upgrade else "0",
                     "RECIPE_FAIL_REPLY_LOSS": "1" if fail_reply_loss else "0",
                     "RECIPE_FAIL_STARTUP_CAPABILITY": "1" if fail_startup_capability else "0",
                     "RECIPE_FAIL_TERMINAL_RETURN": "1" if fail_terminal_return else "0"},
                capture_output=True, text=True, timeout=10,
            )
            return result, [json.loads(line) for line in trace.read_text().splitlines()]

    def without_terminal_return_cli(self, commands):
        selected = [row for row in commands if "developer_terminal_pty_" in row]
        self.assertEqual(len(selected), 4)
        expected = []
        for features in (None, "developer-conditional-close",
                         "product-image-witness,t4-manager-actor-service,setup-upgrade082",
                         "product-private-backup,product-image-witness,setup-upgrade082"):
            profile = (["--config", "profile.test.package.sha2.opt-level=3",
                        "--config", "profile.test.package.sha2.debug-assertions=true",
                        "--config", "profile.test.package.sha2.overflow-checks=true"]
                       + ARGON2_TEST_PROFILE) if selected[0][2:3] == ["--config"] else []
            command = ["cargo", "test", *profile, "--locked", "-p", "omavless-runtime"]
            if features is not None:
                command += ["--features", features]
            expected.append(command + ["--test", "cli", "developer_terminal_pty_",
                                       "--", "--test-threads=1"])
        self.assertEqual(selected, expected)
        return [row for row in commands if row not in selected]

    def assert_terminal_return_crypto_profile(self, commands):
        selected = [row for row in commands if "developer_terminal_pty_" in row]
        self.assertEqual(len(selected), 4)
        expected = ["--config", "profile.test.package.sha2.opt-level=3",
                    "--config", "profile.test.package.sha2.debug-assertions=true",
                    "--config", "profile.test.package.sha2.overflow-checks=true"] + ARGON2_TEST_PROFILE
        for row in selected:
            self.assertEqual(row[2:14], expected)

    def test_terminal_return_cli_is_discovered_in_all_four_configurations(self):
        result, commands = self.runner_trace()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.without_terminal_return_cli(commands)
        self.assert_terminal_return_crypto_profile(commands)
        source = (ROOT / "crates/omavless-runtime/tests/cli.rs").read_text()
        self.assertRegex(source, r"#\[test\]\s+fn developer_terminal_pty_[a-z0-9_]+\(")
        self.assertIn("test-tui-developer-terminal.py", source)
        for index, row in enumerate(commands):
            if "developer_terminal_pty_" not in row:
                continue
            with self.assertRaises(AssertionError):
                self.without_terminal_return_cli(commands[:index] + commands[index + 1:])
            changed = [command[:] for command in commands]
            changed[index][-1] = "--test-threads=2"
            with self.assertRaises(AssertionError):
                self.without_terminal_return_cli(changed)
            missing_crypto = [command[:] for command in commands]
            missing_crypto[index] = row[:2] + row[14:]
            with self.assertRaises(AssertionError):
                self.assert_terminal_return_crypto_profile(missing_crypto)

    def test_terminal_return_failure_stops_the_recipe(self):
        result, commands = self.runner_trace(fail_terminal_return=True)
        self.assertEqual(result.returncode, 37, result.stderr)
        self.assertEqual(commands[-1][-5:],
                         ["--test", "cli", "developer_terminal_pty_", "--", "--test-threads=1"])

    def without_reply_loss_cli(self, commands):
        selected = [row for row in commands if "reply_loss_selector" in row or "reply_loss_" in row]
        self.assertEqual(len(selected), 6)
        expected = []
        for features in (
            "developer-conditional-close",
            "product-image-witness,t4-manager-actor-service,setup-upgrade082",
            "product-private-backup,product-image-witness,setup-upgrade082",
        ):
            for target in (["--bin", "omavless", "reply_loss_selector"],
                           ["--test", "cli", "reply_loss_"]):
                profile = (["--config", "profile.test.package.sha2.opt-level=3",
                            "--config", "profile.test.package.sha2.debug-assertions=true",
                            "--config", "profile.test.package.sha2.overflow-checks=true"]
                           + ARGON2_TEST_PROFILE) if selected[0][2:3] == ["--config"] else []
                expected.append(["cargo", "test", *profile, "--locked", "-p",
                                 "omavless-runtime", "--features", features, *target])
        self.assertEqual(selected, expected)
        return [row for row in commands if row not in selected]

    def test_reply_loss_cli_discovery_covers_product_backup_and_developer(self):
        result, commands = self.runner_trace()
        self.assertEqual(result.returncode, 0, result.stderr)
        original = self.without_reply_loss_cli(commands)
        self.assertEqual(len(commands) - len(original), 6)
        # Bind both filters to actual maintained Rust test declarations.
        for file, selector in (("src/main.rs", "reply_loss_selector"),
                               ("tests/cli.rs", "reply_loss_")):
            source = (ROOT / "crates/omavless-runtime" / file).read_text()
            names = re.findall(r"#\[test\]\s+fn ([a-z0-9_]+)\(", source)
            self.assertTrue(any(selector in name for name in names))
        for index, row in enumerate(commands):
            if "reply_loss_selector" not in row and "reply_loss_" not in row:
                continue
            with self.assertRaises(AssertionError):
                self.without_reply_loss_cli(commands[:index] + commands[index + 1:])
            changed = [command[:] for command in commands]
            changed[index][changed[index].index("--features") + 1] = "tui"
            with self.assertRaises(AssertionError):
                self.without_reply_loss_cli(changed)

    def test_reply_loss_cli_original_failure_stops_the_recipe(self):
        result, commands = self.runner_trace(fail_reply_loss=True)
        self.assertEqual(result.returncode, 31, result.stderr)
        self.assertEqual(commands[-1][-3:], ["--test", "cli", "reply_loss_"])
        self.assertFalse(any(row[:2] == ["cargo", "run"] for row in commands))

    def without_selected_set_formatting(self, commands):
        # Assert ONLY the newly maintained include! source/test operands, then
        # retain every byte of the historical command recipe/hash below.
        added = [
            "crates/omavless-runtime/src/native_coordinator/connection_close_set.rs",
            "crates/omavless-runtime/src/native_coordinator/connection_close_preview_tests.rs",
            "crates/omavless-runtime/src/native_coordinator/connection_close_preview.rs",
        ]
        formatting = [row for row in commands if row[0] == "rustfmt"]
        self.assertEqual(len(formatting), 1)
        self.assertEqual(formatting[0][-len(added):], added)
        return [row[:-len(added)] if row[0] == "rustfmt" else row[:]
                for row in commands]

    def without_close_acceptance(self, commands):
        feature = "acceptance-connection-close,t4-manager-actor-service,setup-upgrade082"
        expected = [
            ["cargo", "check", "--locked", "-p", "omavless-runtime", "--all-targets", "--features", feature],
            ["cargo", "clippy", "--locked", "-p", "omavless-runtime", "--all-targets", "--features", feature, "--", "-D", "warnings"],
            ["cargo", "test", "--locked", "-p", "omavless-image-witness", "--features", "product-epochs,acceptance-channel-cut", "--lib", "acceptance_channel_tests"],
            ["cargo", "test", "--locked", "-p", "omavless-runtime", "--features", feature, "--lib", "acceptance_"],
            ["cargo", "test", "--locked", "-p", "omavless-runtime", "--features", feature, "--lib", "product_selected_set"],
            ["cargo", "test", "--locked", "-p", "omavless-runtime", "--features", feature, "--bin", "omavless", "close_acceptance_selector"],
            ["cargo", "test", "--locked", "-p", "omavless-runtime", "--features", "product-private-backup,acceptance-connection-close,setup-upgrade082", "--bin", "omavless", "close_acceptance_selector"],
        ]
        retained = []
        seen = []
        for command in commands:
            if not any("acceptance-connection-close" in arg or "acceptance-channel-cut" in arg for arg in command):
                retained.append(command)
                continue
            normalized = command[:]
            if normalized[:3] == ["cargo", "test", "--config"]:
                self.assertEqual(normalized[8:14], ARGON2_TEST_PROFILE)
                normalized = normalized[:2] + normalized[14:]
            self.assertIn(normalized, expected)
            self.assertNotIn(normalized, seen)
            seen.append(normalized)
        return retained

    def test_close_acceptance_additions_are_exact_and_preserve_prior_recipe(self):
        result, commands = self.runner_trace()
        self.assertEqual(result.returncode, 0, result.stderr)
        original = self.without_close_acceptance(commands)
        self.assertEqual(len(commands) - len(original), 7)

    def test_late_product_source_cases_are_discovered_without_running_ignored_native_probe(self):
        result, commands = self.runner_trace()
        self.assertEqual(result.returncode, 0, result.stderr)
        channel = [c for c in commands if c[-1:] == ["acceptance_channel_tests"]]
        owner = [c for c in commands if c[-1:] == ["acceptance_"]]
        self.assertEqual(len(channel), 1)
        self.assertEqual(len(owner), 1)
        for command in channel + owner:
            self.assertIn("--lib", command)
            self.assertIn("--locked", command)
            self.assertNotIn("--ignored", command)
            self.assertEqual(command[command.index("--config")+1], "profile.test.package.sha2.opt-level=3")
        service = (ROOT / "crates/omavless-image-witness/src/service.rs").read_text()
        for name in ("acceptance_late_finish_decodes_original_seqpacket_ack_then_refuses_without_renewal",
                     "acceptance_late_finish_rejects_missing_ack_and_attached_fd",
                     "acceptance_late_finish_refuses_nonproduct_and_already_terminal_before_send"):
            self.assertIn("fn " + name + "(", service)
        owner_source = (ROOT / "crates/omavless-runtime/src/native_coordinator/connection_close_preview_tests.rs").read_text()
        self.assertIn("fn acceptance_late_publication_uses_real_retirement_object_and_actual_owner_refusal(", owner_source)
        product = (ROOT / "crates/omavless-image-witness/src/kernel_product_tests.rs").read_text()
        self.assertIn('fn installed_product_same_binding_refuses_original_core_death(', product)
        self.assertFalse(any("installed_product_same_binding_refuses_original_core_death" in c for c in commands))

    def without_product_image_upgrade(self, commands):
        added = [row for row in commands if "omavless-image-witness" in row and "product-epochs" in row]
        self.assertEqual(len(added), 1)
        command = added[0]
        self.assertEqual(command[:2], ["cargo", "test"])
        self.assertEqual(command[-6:], ["--locked", "-p", "omavless-image-witness",
                                       "--features", "product-epochs", "--lib"])
        self.assertNotIn("--ignored", command)
        return [row for row in commands if row is not command]

    def test_product_image_upgrade_controls_execute_in_the_hosted_recipe(self):
        result, commands = self.runner_trace()
        self.assertEqual(result.returncode, 0, result.stderr)
        original = self.without_product_image_upgrade(commands)
        self.assertEqual(len(commands) - len(original), 1)

    def test_product_image_upgrade_failure_stops_the_original_recipe(self):
        result, commands = self.runner_trace(fail_image_upgrade=True)
        self.assertEqual(result.returncode, 29, result.stderr)
        self.assertEqual(commands[-1][-6:], ["--locked", "-p", "omavless-image-witness",
                                            "--features", "product-epochs", "--lib"])
        self.assertFalse(any(row[:2] == ["cargo", "test"] and "-p" in row
                             and row[row.index("-p") + 1] == "omavless-dns-broker"
                             for row in commands))
        self.assertFalse(any(row[:2] == ["cargo", "run"] for row in commands))

    def test_lint_failure_is_reported_before_any_test_or_fixture(self):
        result, commands = self.runner_trace(fail_clippy=True)
        self.assertEqual(result.returncode, 23, result.stderr)
        self.assertEqual([row[:2] for row in commands],
                         [["cargo", "fmt"], ["cargo", "clippy"]])
        self.assertRegex(result.stderr,
                         r"OVT_CHECK_END lint-workspace-2 elapsed_s=\d+ exit=23")

    def test_static_preflight_precedes_all_execution_without_changing_commands(self):
        result, commands = self.runner_trace()
        self.assertEqual(result.returncode, 0, result.stderr)
        static = [c for c in commands if c[:2] in
                  [["cargo", "fmt"], ["cargo", "clippy"], ["cargo", "check"]]
                  or c[0] == "rustfmt"]
        execution = [c for c in commands if c not in static]
        self.assertEqual(commands, static + execution)
        # Original expanded commands captured at 6d382b9a, before scheduling:
        # all 12 static checks and all 45 executable gates, same order within
        # each group, exact arguments/configurations/skips and multiplicities.
        # Remove ONLY the exact new Argon2 argv before comparing the original
        # recipe. Do not regenerate the baseline digest to conceal other changes.
        legacy_execution = []
        for command in self.without_startup_scheduling(self.without_terminal_return_cli(self.without_reply_loss_cli(self.without_auxiliary_scheduling(self.without_product_image_upgrade(self.without_close_acceptance(execution)))))):
            if command[:2] == ["cargo", "test"]:
                self.assertEqual(command[8:14], ARGON2_TEST_PROFILE)
                command = command[:8] + command[14:]
            legacy_execution.append(command)
        static = self.without_selected_set_formatting(self.without_close_acceptance(static))
        for rows, digest in [(static, "15533c34bf33cb3189a3b4bca162f6b165786bff2e4f9f84f449e99b94dd59a7"),
                             (legacy_execution, "19d3a07ff0ad54c42344d1859330e606a2d2ffcdfc197a5244864286bb32e4e3")]:
            payload = json.dumps(rows, separators=(",", ":")).encode()
            self.assertEqual(hashlib.sha256(payload).hexdigest(), digest)

    def test_every_command_has_a_paired_closed_label_timing(self):
        result, commands = self.runner_trace()
        self.assertEqual(result.returncode, 0, result.stderr)
        starts = re.findall(r"^OVT_CHECK_START ([a-z0-9-]+)$", result.stderr, re.M)
        ends = re.findall(r"^OVT_CHECK_END ([a-z0-9-]+) elapsed_s=(\d+) exit=(\d+)$",
                          result.stderr, re.M)
        self.assertEqual(len(starts), len(commands))
        self.assertEqual(starts, [row[0] for row in ends])
        self.assertEqual(len(set(starts)), len(starts))
        self.assertTrue(all(row[2] == "0" for row in ends))

    def test_crypto_test_profile_preserves_the_complete_recipe(self):
        result, commands = self.runner_trace()
        self.assertEqual(result.returncode, 0, result.stderr)
        profile = [
            "--config", "profile.test.package.sha2.opt-level=3",
            "--config", "profile.test.package.sha2.debug-assertions=true",
            "--config", "profile.test.package.sha2.overflow-checks=true",
        ] + ARGON2_TEST_PROFILE
        normalized = []
        for command in commands:
            if command[:2] == ["cargo", "test"]:
                self.assertEqual(command[2:2 + len(profile)], profile)
                command = command[:2] + command[2 + len(profile):]
            self.assertFalse(any("profile." in arg for arg in command))
            normalized.append(command)
        normalized = self.without_startup_scheduling(normalized)
        normalized = self.without_close_acceptance(normalized)
        normalized = self.without_auxiliary_scheduling(self.without_product_image_upgrade(normalized))
        normalized = self.without_reply_loss_cli(normalized)
        normalized = self.without_terminal_return_cli(normalized)
        normalized = self.without_selected_set_formatting(normalized)
        # Assert each bounded import addition explicitly, then retain the exact
        # pre-import command multiset: no prior feature/filter/deadline/parity gate
        # may disappear behind an opaque replacement digest.
        additions = [
            ["cargo", "build", "--locked", "-p", "omavless-tui", "--example", "private_import_preview"],
            ["python3", "tests/test-tui-import-terminal.py", "target/debug/examples/private_import_preview"],
            ["cargo", "build", "--locked", "-p", "omavless-runtime", "--bin", "omavless", "--no-default-features", "--features", "tui"],
            ["bash", "tests/run-tui-native-import-wire.sh", "target/debug/omavless"],
            ["cargo", "test", "--locked", "-p", "omavless-runtime", "--features", "tui", "tui_private_import_uses_existing_canonical"],
            ["cargo", "test", "--locked", "-p", "omavless-runtime", "--bin", "omavless", "--features", "tui", "tui_import_entry::tests"],
        ]
        for command in additions:
            self.assertEqual(normalized.count(command), 1, command)
            normalized.remove(command)
        # Explicitly reconcile the independently asserted serial fixture below;
        # retain its skip/exact relationship rather than hiding it in a digest.
        for command in normalized[:]:
            if CANONICAL_STAGE_TEST not in command:
                continue
            if "--exact" in command:
                normalized.remove(command)
            else:
                index = command.index(CANONICAL_STAGE_TEST)
                self.assertEqual(command[index - 1], "--skip")
                del command[index - 1:index + 1]
        # Captured from the pre-optimization recipe: all expanded commands,
        # features, filters, skips, isolated deadlines, lint and parity entries.
        # Scheduling now puts static checks first. Preserve the historical
        # command multiset; the exact new two-group order is checked separately.
        payload = json.dumps(sorted(normalized), separators=(",", ":")).encode()
        self.assertEqual(hashlib.sha256(payload).hexdigest(),
                         "f4ec4499885b7d90fe30a81d5e8462a7ed13b4f04e7f76256f1097a5eccffd4d")

    def test_crypto_runner_stops_on_the_original_test_failure(self):
        result, commands = self.runner_trace(fail_test=True)
        self.assertEqual(result.returncode, 17, result.stderr)
        self.assertEqual(commands[-1][:2], ["cargo", "test"])
        self.assertFalse(any(c[:2] in [["cargo", "test"], ["cargo", "build"],
                                     ["cargo", "run"]] for c in commands[:-1]))
        self.assertRegex(result.stderr, r"OVT_CHECK_END test-auxiliary-inventory-15 elapsed_s=\d+ exit=17")

    def test_auxiliary_inventory_failure_stops_before_parallel_tests(self):
        result, commands = self.runner_trace(fail_auxiliary_inventory=True)
        self.assertEqual(result.returncode, 21, result.stderr)
        self.assertEqual(commands[-1][-4:],
                         [AUXILIARY_TEARDOWN_TEST, "--", "--exact", "--test-threads=1"])
        self.assertEqual(sum(c[:2] == ["cargo", "test"] for c in commands), 1)
        self.assertRegex(result.stderr,
                         r"OVT_CHECK_END test-auxiliary-inventory-15 elapsed_s=\d+ exit=21")

    def test_auxiliary_normalization_rejects_missing_or_parallel_execution(self):
        result, commands = self.runner_trace()
        self.assertEqual(result.returncode, 0, result.stderr)
        isolated = [i for i, c in enumerate(commands)
                    if AUXILIARY_TEARDOWN_TEST in c and "--exact" in c]
        self.assertEqual(len(isolated), 2)
        for index in isolated:
            with self.subTest(index=index, mutation="missing"):
                with self.assertRaises(AssertionError):
                    self.without_auxiliary_scheduling(commands[:index] + commands[index + 1:])
            changed = [c[:] for c in commands]
            changed[index][-1] = "--test-threads=2"
            with self.subTest(index=index, mutation="parallel"):
                with self.assertRaises(AssertionError):
                    self.without_auxiliary_scheduling(changed)
            changed = [c[:] for c in commands]
            changed[index].insert(2, "--release")
            with self.subTest(index=index, mutation="different-profile"):
                with self.assertRaises(AssertionError):
                    self.without_auxiliary_scheduling(changed)

    def test_canonical_stage_retention_executes_once_in_the_same_union(self):
        result, commands = self.runner_trace()
        self.assertEqual(result.returncode, 0, result.stderr)
        selected = [c for c in commands if CANONICAL_STAGE_TEST in c]
        self.assertEqual(len(selected), 2)
        skipped, isolated = selected
        index = skipped.index(CANONICAL_STAGE_TEST)
        self.assertEqual(skipped[index - 1], "--skip")
        for command in selected:
            self.assertEqual(command[:2], ["cargo", "test"])
            self.assertEqual(command[command.index("-p") + 1], "omavless-runtime")
            self.assertIn("--locked", command)
            self.assertIn("--lib", command)
            self.assertEqual(command[command.index("--features") + 1],
                             "product-image-witness,t4-manager-actor-service")
            self.assertNotIn("--ignored", command)
        self.assertEqual(isolated[-4:], [CANONICAL_STAGE_TEST, "--", "--exact", "--test-threads=1"])
        self.assertEqual(commands[-1][:3], ["cargo", "run", "--quiet"])
        self.assertEqual(commands[-2], isolated)

    def test_canonical_stage_failure_stops_before_parity(self):
        result, commands = self.runner_trace(fail_canonical_stage=True)
        self.assertEqual(result.returncode, 19, result.stderr)
        self.assertEqual(commands[-1][-4:],
                         [CANONICAL_STAGE_TEST, "--", "--exact", "--test-threads=1"])
        self.assertFalse(any(c[:2] == ["cargo", "run"] for c in commands))

    def assert_startup_crypto(self, commands):
        selected = [row for row in commands if STARTUP_CAPABILITY_TEST in row and "--exact" in row]
        self.assertEqual(len(selected), 2)
        profile = ["--config", "profile.test.package.sha2.opt-level=3",
                   "--config", "profile.test.package.sha2.debug-assertions=true",
                   "--config", "profile.test.package.sha2.overflow-checks=true"] + ARGON2_TEST_PROFILE
        for row in selected:
            self.assertEqual(row[2:14], profile)

    def test_startup_capability_probe_is_exact_isolated_and_keeps_crypto(self):
        result, commands = self.runner_trace()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_startup_crypto(commands)
        restored = self.without_startup_scheduling(commands)
        self.assertEqual(len(commands) - len(restored), 2)
        self.assertFalse(any(STARTUP_CAPABILITY_TEST in row for row in restored))
        profile = ["--config", "profile.test.package.sha2.opt-level=3",
                   "--config", "profile.test.package.sha2.debug-assertions=true",
                   "--config", "profile.test.package.sha2.overflow-checks=true"] + ARGON2_TEST_PROFILE
        for row in commands:
            if STARTUP_CAPABILITY_TEST in row and "--exact" in row:
                self.assertEqual(row[2:14], profile)
        source = (ROOT / "crates/omavless-runtime/src/startup_validation.rs").read_text()
        self.assertIn("fn " + STARTUP_CAPABILITY_TEST.rsplit("::", 1)[1] + "()", source)
        self.assertIn("Instant::now() + Duration::from_secs(1)", source)

    def test_startup_capability_scheduling_rejects_missing_parallel_or_changed_config(self):
        result, commands = self.runner_trace()
        self.assertEqual(result.returncode, 0, result.stderr)
        isolated = [i for i, row in enumerate(commands)
                    if STARTUP_CAPABILITY_TEST in row and "--exact" in row]
        self.assertEqual(len(isolated), 2)
        for index in isolated:
            with self.assertRaises(AssertionError):
                self.without_startup_scheduling(commands[:index] + commands[index + 1:])
            for mutation in ("threads", "profile", "crypto", "filter"):
                changed = [row[:] for row in commands]
                if mutation == "threads":
                    changed[index][-1] = "--test-threads=2"
                elif mutation == "profile":
                    changed[index].insert(2, "--release")
                elif mutation == "crypto":
                    del changed[index][2:14]
                    with self.assertRaises(AssertionError):
                        self.assert_startup_crypto(changed)
                    continue
                else:
                    changed[index][changed[index].index(STARTUP_CAPABILITY_TEST)] = "startup_validation::tests"
                with self.assertRaises(AssertionError):
                    self.without_startup_scheduling(changed)

    def test_startup_capability_original_failure_stops_before_parallel_suite(self):
        result, commands = self.runner_trace(fail_startup_capability=True)
        self.assertEqual(result.returncode, 39, result.stderr)
        self.assertEqual(commands[-1][-4:],
                         [STARTUP_CAPABILITY_TEST, "--", "--exact", "--test-threads=1"])
        self.assertFalse(any("--workspace" in row and row[:2] == ["cargo", "test"]
                             for row in commands))
        self.assertFalse(any(row[:2] == ["cargo", "run"] for row in commands))

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
        commands = [shlex.split(line)[2:] for line in re.sub(r"\\\n", " ", source).splitlines()
                    if re.match(r"\s*run_check [a-z0-9-]+ cargo test ", line)]
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
