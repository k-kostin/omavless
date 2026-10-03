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
with patch.dict("sys.modules", {"loopback": LIVE}):
    UDP = load("udp_loopback")
with patch.dict("sys.modules", {"loopback": LIVE, "review": REVIEW, "udp_loopback": UDP}):
    COMPOSITION = load("managed_composition")


class ConditionalCoreAdapterTests(unittest.TestCase):
    def test_both_entrypoints_share_verified_export_and_receipt_checker(self):
        self.assertIs(COMPOSITION.export, REVIEW.export)
        self.assertIs(COMPOSITION.git_environment, REVIEW.git_environment)
        self.assertIs(COMPOSITION.matrix_receipt, REVIEW.matrix_receipt)
        self.assertIs(COMPOSITION.CONDITIONAL_TESTS, REVIEW.CONDITIONAL_TESTS)
        source = (ROOT / "tests/core_connections_adapter/review.py").read_text()
        self.assertIn("export(args.source, PIN, source)", source)
        self.assertIn("matrix_receipt(receipt, CONDITIONAL_TESTS)", source)
        self.assertIn('GO, "mod", "verify"', source)

    def test_compiler_environment_excludes_ambient_execution_and_download_overrides(self):
        with tempfile.TemporaryDirectory(prefix="compiler-unit-") as name:
            poison = {"HOME": name, "PATH": "/synthetic/untrusted", "GOENV": "/private/settings",
                      "GOFLAGS": "-toolexec=private-command", "GOTOOLCHAIN": "auto",
                      "GOWORK": "/private/workspace", "GOEXPERIMENT": "private-input",
                      "CGO_CFLAGS": "private-input", "LD_PRELOAD": "private-input",
                      "GOPROXY": "private-provider", "CC": "private-command"}
            with patch.dict("os.environ", poison, clear=True):
                env = REVIEW.compiler_environment(Path(name))
            self.assertEqual(REVIEW.GO, "/usr/bin/go")
            self.assertEqual(env["PATH"], "/usr/bin:/bin")
            self.assertEqual(env["GOENV"], "off")
            self.assertEqual(env["GOPROXY"], "off")
            self.assertEqual(env["GOTOOLCHAIN"], "local")
            self.assertEqual(env["GOWORK"], "off")
            self.assertEqual(env["GOFLAGS"], "")
            self.assertEqual(env["CC"], "/usr/bin/gcc")
            self.assertEqual(env["GOMODCACHE"], str(Path(name) / "go/pkg/mod"))
            for key in ("GOEXPERIMENT", "CGO_CFLAGS", "LD_PRELOAD"):
                self.assertNotIn(key, env)
        with patch.dict("os.environ", {}, clear=True):
            with self.assertRaisesRegex(RuntimeError, "^Local compiler home refused$"):
                REVIEW.compiler_environment(Path("/synthetic"))

    def test_udp_association_requires_exact_local_reply_and_closes_refusal(self):
        from unittest.mock import Mock
        client = Mock()
        client.getsockname.return_value = ("127.0.0.1", 12346)
        expected = b"\x05\x00\x00" + UDP.header(12345)[3:]
        with patch.object(UDP.socket, "create_connection") as create:
            connection = create.return_value
            connection.recv.side_effect = [b"\x05", b"\x00", expected]
            self.assertIs(UDP.associate(12345, client), connection)
            create.assert_called_once_with(("127.0.0.1", 12345), timeout=2)
            self.assertEqual(connection.sendall.call_args_list[-1].args[0], b"\x05\x03\x00" + UDP.header(12346)[3:])
            connection.close.assert_not_called()
            for reply in (expected[:-1] + b"\x00", b"\x05\x00\x00\x03" + expected[4:], b"", b"\x05\x01" + expected[2:]):
                connection.reset_mock()
                connection.recv.side_effect = [b"\x05\x00", reply, b""]
                with self.assertRaises(ValueError):
                    UDP.associate(12345, client)
                connection.close.assert_called_once()

    def test_udp_association_eof_or_unexpected_readability_refuses(self):
        with patch.object(UDP.select, "select", return_value=(["synthetic"], [], [])):
            with self.assertRaisesRegex(ValueError, "^Synthetic UDP association unexpectedly ended$"):
                UDP.association_live("synthetic")
        with patch.object(UDP.select, "select", return_value=([], [], [])):
            UDP.association_live("synthetic")

    def test_udp_close_requires_empty_exact_receipt_and_never_retries_unknown(self):
        with patch.object(LIVE, "control") as control:
            control.return_value = (204, b"")
            UDP.close_once(12345, "synthetic", "private-fixture", "42", 204)
            control.assert_called_once_with(12345, "synthetic", "POST", "/connections/private-fixture/close-conditional", "42")
            for receipt in ((404, b""), (204, b"private-input"), (500, b""), (200, b"")):
                control.reset_mock()
                control.return_value = receipt
                with self.assertRaisesRegex(ValueError, "^Conditional UDP receipt ambiguous; not retried$"):
                    UDP.close_once(12345, "synthetic", "fixture", "42", 204)
                self.assertEqual(control.call_count, 1)
            control.reset_mock()
            control.side_effect = TimeoutError("synthetic private reply loss")
            with self.assertRaises(TimeoutError):
                UDP.close_once(12345, "synthetic", "fixture", "42", 204)
            self.assertEqual(control.call_count, 1)

    def test_udp_echo_accepts_only_exact_loopback_relay_target_and_payload(self):
        challenge = b"synthetic"
        raw = UDP.header(12346) + challenge
        UDP.validate_echo(raw, ("127.0.0.1", 12345), 12345, 12346, challenge)
        for data, peer in ((raw, ("127.0.0.2", 12345)), (raw, ("127.0.0.1", 12344)),
                           (raw[:-1], ("127.0.0.1", 12345)), (b"\x00\x00\x01" + raw[3:], ("127.0.0.1", 12345)),
                           (UDP.header(12347) + challenge, ("127.0.0.1", 12345)), (raw + b"x" * 4097, ("127.0.0.1", 12345))):
            with self.assertRaisesRegex(ValueError, "^Unexpected synthetic UDP response$"):
                UDP.validate_echo(data, peer, 12345, 12346, challenge)

    def test_udp_application_datagram_retry_is_bounded_and_not_an_effect_retry(self):
        import socket
        from unittest.mock import Mock
        client = Mock()
        client.recvfrom.side_effect = socket.timeout
        with patch.object(LIVE, "control") as control:
            with self.assertRaisesRegex(ValueError, "^UDP application reconnect unavailable$"):
                UDP.prove_echo(client, 12345, 12346, reconnect=True)
            self.assertEqual(client.sendto.call_count, 5)
            control.assert_not_called()

    def test_udp_snapshot_rejects_foreign_malformed_duplicate_identity_and_token(self):
        row = {"id": "00000000-0000-0000-0000-000000000001", "omavlessCloseToken": "42",
               "metadata": {"sourcePort": "12345", "destinationPort": "12346", "network": "udp", "type": "Socks5",
                            "sourceIP": "127.0.0.1", "destinationIP": "127.0.0.1"}}
        with patch.object(LIVE, "control") as control:
            control.return_value = (200, json.dumps({"connections": [row]}).encode())
            self.assertEqual(UDP.snapshot(12344, "synthetic", {"12345": 12346}), {"12345": (row["id"], "42")})
            invalid = [dict(row, id="../foreign"), dict(row, omavlessCloseToken="01"),
                       dict(row, omavlessCloseToken="18446744073709551616"),
                       dict(row, metadata=dict(row["metadata"], destinationIP="192.0.2.1")),
                       dict(row, metadata=dict(row["metadata"], network="tcp")),
                       dict(row, metadata=dict(row["metadata"], sourcePort="12347"))]
            for rows in [[value] for value in invalid] + [[row, row], [row, row, row]]:
                control.return_value = (200, json.dumps({"connections": rows}).encode())
                with self.assertRaises(ValueError):
                    UDP.snapshot(12344, "synthetic", {"12345": 12346})

    def test_udp_unsafe_inputs_refuse_before_process_creation(self):
        with tempfile.TemporaryDirectory(prefix="udp-unit-") as name:
            root = Path(name)
            core = root / "candidate"
            core.write_bytes(b"synthetic")
            with patch.object(UDP.subprocess, "Popen") as start:
                with self.assertRaises(ValueError):
                    UDP.exercise(Path("relative"), root)
                root.chmod(0o755)
                with self.assertRaises(ValueError):
                    UDP.exercise(core, root)
                start.assert_not_called()

    def test_composition_requires_actual_go_cases_not_zero_tests_or_unexpected_skips(self):
        events = [{"Action": "pass", "Test": "TestFixture"}] * 20
        events += [{"Action": "skip", "Test": "TestExplicitOptin"}] * 20
        def receipt(values):
            return b"\n".join(json.dumps(value).encode("ascii") for value in values)
        COMPOSITION.matrix_receipt(receipt(events), ("TestFixture",), ("TestExplicitOptin",))
        for invalid in ([], events[:-1], events + events[:1], events + [{"Action": "fail"}],
                        events + [{"Action": "skip", "Test": "TestUnknown"}]):
            with self.assertRaisesRegex(RuntimeError, "^Composition test execution receipt refused$"):
                COMPOSITION.matrix_receipt(receipt(invalid), ("TestFixture",), ("TestExplicitOptin",))

    def test_composition_export_ignores_local_archive_attributes_and_dirty_source(self):
        with tempfile.TemporaryDirectory(prefix="composition-unit-") as name:
            root = Path(name)
            repository = root / "repo"
            repository.mkdir(mode=0o700)
            REVIEW.run(["/usr/bin/git", "init", "--quiet", "--template=", str(repository)],
                       env=COMPOSITION.git_environment())
            source = repository / "fixture.txt"
            source.write_bytes(b"exact committed fixture\n")
            REVIEW.run(["/usr/bin/git", "-C", str(repository), "add", "fixture.txt"], env=COMPOSITION.git_environment())
            REVIEW.run(["/usr/bin/git", "-C", str(repository), "-c", "user.name=Fixture", "-c",
                        "user.email=fixture@example.invalid", "commit", "--quiet", "-m", "fixture"],
                       env=COMPOSITION.git_environment())
            identity = REVIEW.run(["/usr/bin/git", "-C", str(repository), "rev-parse", "HEAD"],
                                  env=COMPOSITION.git_environment()).decode("ascii").strip()
            (repository / ".git" / "info").mkdir(mode=0o700)
            (repository / ".git" / "info" / "attributes").write_text("* export-ignore\n", encoding="ascii")
            source.write_bytes(b"uncommitted changed fixture\n")
            COMPOSITION.export(repository, identity, root / "exported")
            self.assertEqual((root / "exported" / "fixture.txt").read_bytes(), b"exact committed fixture\n")

    def test_composition_overlay_is_only_test_code_and_reversal_is_explicit(self):
        import hashlib
        data = COMPOSITION.SOCKET_TEST_PATCH.read_bytes()
        self.assertEqual(hashlib.sha256(data).hexdigest(), COMPOSITION.SOCKET_TEST_SHA256)
        paths = [line for line in data.decode("utf-8").splitlines() if line.startswith("diff --git ")]
        self.assertEqual(paths, ["diff --git a/listener/sing_tun/system_dns_linux_test.go b/listener/sing_tun/system_dns_linux_test.go"])
        with patch.object(REVIEW, "run") as run:
            COMPOSITION.apply(Path("/source"), COMPOSITION.SOCKET_TEST_PATCH, reverse=True)
            self.assertEqual(run.call_count, 2)
            for call in run.call_args_list:
                self.assertIn("--reverse", call.args[0])

    def test_managed_composition_pins_exact_git_objects_and_patch_bytes(self):
        self.assertEqual(COMPOSITION.DNS_REVISION, "c4e800425243c1b02165f82153e4bf418fe465e6")
        self.assertEqual(COMPOSITION.SING_TUN, "b50ae28a1409c7bce8e96e6c6966cf57d8ace754")
        with patch.object(REVIEW, "run", return_value=b"synthetic-unknown-patch") as run:
            with self.assertRaisesRegex(RuntimeError, "^Exact managed-DNS patch identity refused$"):
                COMPOSITION.dns_patches(Path("/synthetic/repo"))
            self.assertIn(COMPOSITION.DNS_REVISION + ":tests/core_dns_adapter/mihomo-dns-broker.patch", run.call_args.args[0])

    def test_composition_git_environment_cannot_inherit_replace_or_user_hooks(self):
        env = COMPOSITION.git_environment()
        self.assertEqual(env["GIT_NO_REPLACE_OBJECTS"], "1")
        self.assertEqual(env["GIT_CONFIG_GLOBAL"], "/dev/null")
        self.assertEqual(env["GIT_NO_LAZY_FETCH"], "1")
        self.assertEqual(env["GIT_ALLOW_PROTOCOL"], "")
        self.assertEqual(env["GIT_TERMINAL_PROMPT"], "0")
        self.assertNotIn("GIT_CONFIG_COUNT", env)
        self.assertNotIn("GIT_SSH_COMMAND", env)

    def test_composition_unsafe_paths_refuse_before_git_or_core(self):
        with tempfile.TemporaryDirectory(prefix="composition-unit-") as name:
            root = Path(name)
            with patch.object(REVIEW, "run") as run, patch.object(LIVE, "exercise") as start:
                with self.assertRaises(RuntimeError):
                    COMPOSITION.exercise(Path("relative"), root, root, root)
                root.chmod(0o755)
                with self.assertRaises(RuntimeError):
                    COMPOSITION.exercise(root, root, root, root)
                root.chmod(0o700)
                link = root / "link"
                link.symlink_to(root, target_is_directory=True)
                with self.assertRaises(RuntimeError):
                    COMPOSITION.exercise(root, root, root, link)
                run.assert_not_called()
                start.assert_not_called()

    def test_composition_wrong_upstream_never_exports_or_applies(self):
        with tempfile.TemporaryDirectory(prefix="composition-unit-") as name:
            with patch.object(REVIEW, "run", return_value=b"wrong"), patch.object(COMPOSITION.subprocess, "run") as archive:
                with self.assertRaisesRegex(RuntimeError, "^Composition upstream identity refused$"):
                    COMPOSITION.export(Path(name), COMPOSITION.SING_TUN, Path(name) / "source")
                archive.assert_not_called()
                self.assertFalse((Path(name) / "source.tar").exists())

    def test_explicit_readiness_rejects_older_ambiguous_or_changed_abi(self):
        with patch.object(LIVE, "control") as control:
            for raw in [b'{}', b'{"abi":2,"ready":true}', b'{"abi":true,"ready":true}',
                        b'{"abi":1,"ready":1}', b'{"abi":1,"ready":true,"extra":0}',
                        b'{"abi":1,"abi":1,"ready":true}']:
                control.return_value = (200, raw)
                with self.assertRaises(ValueError):
                    LIVE.conditional_ready(12345, "synthetic")
            for ready in [False, True]:
                control.return_value = (200, json.dumps({"abi": 1, "ready": ready}).encode())
                self.assertIs(LIVE.conditional_ready(12345, "synthetic"), ready)

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
