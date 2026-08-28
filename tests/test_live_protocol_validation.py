import importlib.util
import json
import os
import stat
import tempfile
import unittest
from pathlib import Path
from unittest import mock


MODULE_PATH = Path(__file__).with_name("live_protocol_validation.py")
SPEC = importlib.util.spec_from_file_location("live_protocol_validation", MODULE_PATH)
live = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(live)


PROFILE_ID = "22222222-2222-4222-8222-222222222222"


def case_payload(**updates):
    case = {
        "id": "trojan-provider-1",
        "profileId": PROFILE_ID,
        "protocol": "trojan",
        "features": ["tls", "grpc"],
        "network": "normal",
        "sourceClass": "provider",
        "expectSuccess": True,
    }
    case.update(updates)
    return {"schemaVersion": 1, "probeUrl": "https://example.com/", "cases": [case]}


class FakeBackend:
    def __init__(self, fail=None):
        self.fail = fail
        self.calls = []

    def status(self):
        return {
            "activeId": PROFILE_ID if ("connect", PROFILE_ID) in self.calls else "",
            "profiles": [{"id": PROFILE_ID, "protocol": "trojan", "name": "private name"}],
            "routing": {"mode": "rule"},
        }

    def call(self, *args, stage="preflight"):
        self.calls.append(tuple(args))
        if self.fail == stage:
            raise live.ValidationError("private-detail-must-not-escape", stage)
        return ""


class LiveProtocolValidationTests(unittest.TestCase):
    def write_cases(self, root: Path, payload):
        path = root / "cases.json"
        path.write_text(json.dumps(payload), encoding="utf-8")
        path.chmod(0o600)
        return path

    def test_cases_file_is_private_bounded_and_contains_no_profile_uri(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            path = self.write_cases(root, case_payload())
            loaded = live.load_cases(path)
            self.assertEqual(loaded["cases"][0]["protocol"], "trojan")
            self.assertNotIn("profileId", live._public_case(loaded["cases"][0]))
            path.chmod(0o644)
            with self.assertRaisesRegex(live.ValidationError, "cases-file-not-private"):
                live.load_cases(path)

    def test_unknown_fields_and_credential_bearing_probe_urls_are_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            payload = case_payload()
            payload["cases"][0]["uri"] = "trojan://secret@example.com"
            with self.assertRaisesRegex(live.ValidationError, "unknown-case-field"):
                live.load_cases(self.write_cases(root, payload))
            payload = case_payload()
            payload["probeUrl"] = "https://user:secret@example.com/?token=secret"
            with self.assertRaisesRegex(live.ValidationError, "invalid-probe-url"):
                live.load_cases(self.write_cases(root, payload))

    def test_success_result_omits_profile_id_names_endpoints_and_raw_output(self):
        backend = FakeBackend()
        case = live.load_cases(self._temporary_cases(case_payload()))["cases"][0]
        result = live.evaluate_case(backend, case, "https://example.com/", 5, lambda *_: None)
        encoded = json.dumps(result)
        self.assertTrue(result["accepted"])
        self.assertNotIn(PROFILE_ID, encoded)
        self.assertNotIn("private name", encoded)
        self.assertEqual(
            backend.calls,
            [("down-all",), ("set-mode", "global"), ("connect", PROFILE_ID), ("down-all",)],
        )

    def test_expected_restricted_network_failure_is_recorded_without_raw_error(self):
        backend = FakeBackend(fail="connect")
        payload = case_payload(
            network="udp-restricted",
            expectSuccess=False,
            expectedFailureStages=["connect", "probe"],
            pairId="hy2-network-pair",
        )
        case = live.load_cases(self._temporary_cases(payload))["cases"][0]
        result = live.evaluate_case(backend, case, "https://example.com/", 5, lambda *_: None)
        self.assertTrue(result["accepted"])
        self.assertEqual(result["failureStage"], "connect")
        self.assertEqual(result["errorCode"], "private-detail-must-not-escape")
        self.assertNotIn("profileId", result)

    def test_atomic_output_is_mode_0600_and_refuses_symlink(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "result.json"
            live.atomic_private_json(output, {"ok": True})
            self.assertEqual(stat.S_IMODE(output.stat().st_mode), 0o600)
            output.unlink()
            target = root / "target"
            target.write_text("keep", encoding="utf-8")
            output.symlink_to(target)
            with self.assertRaisesRegex(live.ValidationError, "unsafe-output-path"):
                live.atomic_private_json(output, {"ok": False})
            self.assertEqual(target.read_text(encoding="utf-8"), "keep")

    def test_restore_reinstates_mode_and_previous_profile(self):
        backend = FakeBackend()
        state = {"activeId": PROFILE_ID, "routing": {"mode": "rule"}}
        self.assertEqual(live.restore_state(backend, state), "restored")
        self.assertEqual(
            backend.calls,
            [("down-all",), ("set-mode", "rule"), ("connect", PROFILE_ID)],
        )

    def _temporary_cases(self, payload):
        temp = tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", delete=False)
        json.dump(payload, temp)
        temp.close()
        os.chmod(temp.name, 0o600)
        self.addCleanup(lambda: Path(temp.name).unlink(missing_ok=True))
        return Path(temp.name)


if __name__ == "__main__":
    unittest.main()
