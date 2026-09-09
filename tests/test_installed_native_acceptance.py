"""Deterministic installed-gate policy tests; no host/service/network access."""
import contextlib
import importlib.util
import io
from pathlib import Path
import unittest
from unittest.mock import patch

_spec = importlib.util.spec_from_file_location("installed_gate", Path(__file__).with_name("installed_native_acceptance.py"))
subject = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(subject)

TEMPLATE = b"mixed-port: 7890\nallow-lan: false\nbind-address: '127.0.0.1'\ntun:\n  stack: system\n"
PROXY = ("tcp", b"0100007F", 7890, "11")
FORWARDER = ("tcp", subject.proc_address("198.18.0.1"), 40000, "12")
ADDRESSES = {FORWARDER[1]}
PROOF = b'LISTEN users:(("mihomo",pid=42,fd=1)) ino:11\nLISTEN users:(("mihomo",pid=42,fd=2)) ino:12\n'


class InstalledNativeAcceptanceTests(unittest.TestCase):
    def test_exact_effective_environment_and_absent_home_override(self):
        home, runtime = Path("/home/synthetic"), Path("/run/user/1234")
        self.assertTrue(subject.valid_environment({}, home, runtime))
        explicit = {"XDG_RUNTIME_DIR": str(runtime), "XDG_STATE_HOME": str(home / ".local/state"),
                    "XDG_CONFIG_HOME": str(home / ".config")}
        self.assertTrue(subject.valid_environment(explicit, home, runtime))
        for key in ("OMAVLESS_HOME", "XDG_RUNTIME_DIR", "XDG_STATE_HOME", "XDG_CONFIG_HOME"):
            for value in ("", "/other"):
                self.assertFalse(subject.valid_environment({**explicit, key: value}, home, runtime))

    def test_default_and_partial_optins_never_access_host(self):
        with patch.object(subject, "run_gate", side_effect=AssertionError("host access")):
            for args in ([], ["--run"], ["--authorize-socket-inspection"]):
                output = io.StringIO()
                with contextlib.redirect_stdout(output):
                    self.assertEqual(subject.main(args), 0)
                self.assertIn("NOT RUN", output.getvalue())

    def test_supported_policy_and_fail_closed_unsupported_templates(self):
        self.assertEqual(subject.template_policy(TEMPLATE), 7890)
        for config in (TEMPLATE + b"mixed-port: 1234\n", TEMPLATE.replace(b"false", b"true"),
                       TEMPLATE.replace(b"127.0.0.1", b"0.0.0.0"), TEMPLATE.replace(b"system", b"gvisor"),
                       TEMPLATE + b"socks-port: 7891\n", TEMPLATE + b"external-controller: 127.0.0.1:9090\n",
                       TEMPLATE.replace(b"7890", b"0"), TEMPLATE.replace(b"7890", b"65536")):
            with self.assertRaises(subject.gate.Failure) as raised:
                subject.template_policy(config)
            self.assertEqual(str(raised.exception), "fixture_unavailable")

    def test_expected_proxy_forwarder_and_unrelated_baseline(self):
        unrelated = ("tcp", b"00000000", 22, "9")
        self.assertTrue(subject.classify_listeners({"9"}, [unrelated, PROXY, FORWARDER], 7890, ADDRESSES, PROOF, 42))
        self.assertEqual(subject.proc_address("127.0.0.1"), b"0100007F")
        self.assertEqual(subject.proc_address("::1"), b"00000000000000000000000001000000")

    def test_unexpected_wildcard_nonloopback_and_unconfigured_proxy_refused(self):
        for row in (("tcp", b"00000000", 7890, "13"),
                    ("tcp", subject.proc_address("192.0.2.1"), 7890, "13"),
                    ("tcp", b"0100007F", 9090, "13"),
                    ("tcp6", b"0" * 32, 7890, "13")):
            with self.assertRaises(subject.gate.Failure):
                subject.classify_listeners(set(), [PROXY, FORWARDER, row], 7890, ADDRESSES, PROOF, 42)

    def test_pid_and_inode_proof_both_required(self):
        for proof in (b"", PROOF.replace(b"pid=42", b"pid=43"), PROOF.replace(b"ino:12", b"ino:99"),
                      PROOF.replace(b"pid=42,", b"pid=420,")):
            with self.assertRaises(subject.gate.Failure):
                subject.classify_listeners(set(), [PROXY, FORWARDER], 7890, ADDRESSES, proof, 42)
        for rows in ([PROXY], [FORWARDER], [PROXY, PROXY, FORWARDER]):
            with self.assertRaises(subject.gate.Failure):
                subject.classify_listeners(set(), rows, 7890, ADDRESSES, PROOF, 42)

    def test_strict_proc_rows_and_bounds(self):
        header = b"sl local_address rem_address st tx rx tr tm retr uid inode\n"
        row = b"0: 0100007F:1ED2 00000000:0000 0A 0 0 0 0 0 11\n"
        self.assertEqual(subject.listener_rows(header + row, header), [PROXY])
        for raw in (b"", header + b"malformed\n", header + row.replace(b"0A", b"XX"),
                    header + row.replace(b" 11", b" secret"), header + row.replace(b"0100007F", b"BAD"),
                    b"x" * (subject.LIMIT + 1)):
            with self.assertRaises(subject.gate.Failure):
                subject.listener_rows(raw, header)

    def test_probe_fixed_https_tun_and_private_errors_not_printed(self):
        argv = subject.probe_args("Meta")
        self.assertIn("if!Meta", argv)
        self.assertEqual(argv[-1], "https://example.com/")
        self.assertEqual(argv[argv.index("--max-time") + 1], "15")
        self.assertEqual(argv[argv.index("--proxy") + 1], "")
        for name in ("--bad", "bad;command", "x" * 16, "../tun", ""):
            with self.assertRaises(subject.gate.Failure):
                subject.probe_args(name)
        output = io.StringIO()
        with patch.object(subject, "run_gate", side_effect=ValueError("private://credential")), contextlib.redirect_stdout(output):
            self.assertEqual(subject.main(["--run", "--authorize-socket-inspection"]), 1)
        self.assertNotIn("private://credential", output.getvalue())
        self.assertIn("installed_gate_failed_check_safe_state", output.getvalue())


if __name__ == "__main__":
    unittest.main()
