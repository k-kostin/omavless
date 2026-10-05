"""Source-only closed one-create wiring; never imports a native fixture."""
import hashlib
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "crates/omavless-netguard/src"


class ExclusiveCreateSource(unittest.TestCase):
    def test_unit_is_disjoint_fixed_private_and_nonrestarting(self):
        unit = (ROOT / "crates/omavless-netguard/tests/fixtures/omavless-k1-exclusive-create.service").read_bytes()
        identity = (SRC / "manager_fixture_identity.rs").read_text()
        self.assertIn(hashlib.sha256(unit).hexdigest(), identity)
        text = unit.decode("ascii")
        for line in ["PrivateNetwork=yes", "RestrictNamespaces=yes", "Restart=no",
                     "KillMode=none", "SendSIGKILL=no", "RuntimeMaxSec=infinity",
                     "CapabilityBoundingSet=CAP_NET_ADMIN", "NoNewPrivileges=yes"]:
            self.assertIn(line + "\n", text)
        self.assertEqual(text.count("ExecStart="), 1)
        self.assertNotIn("ExecStop=", text)
        self.assertNotIn("JoinsNamespaceOf=", text)
        self.assertNotIn("retained-lease-regression", text)

    def test_worker_uses_one_state_request_not_direct_effect_or_cleanup(self):
        source = (SRC / "kernel_exclusive_create_fixture.rs").read_text()
        self.assertEqual(source.count("?.request("), 1)
        self.assertIn("Request::Arm { generation: 7, mode: Mode::Full }", source)
        self.assertIn("creator.effects == 1 && creator.created.is_some()", source)
        for call in [".full(", ".replace_owned(", ".delete_owned(", ".send_batch(",
                     "remove_file(", "remove_dir", "Command::", "setns(", "unshare("]:
            self.assertNotIn(call, source)
        self.assertLess(source.index("self.state = Some"), source.index("self.creator = Some"))
        self.assertLess(source.index("self.creator = Some"), source.index("?.request("))
        self.assertLess(source.index("?.request("), source.index("self.phase(3)?"))

    def test_original_pending_and_witness_stay_in_existing_creator(self):
        source = (SRC / "kernel_creator_lifecycle.rs").read_text()
        full = source.split("    fn full(", 1)[1].split("impl sealed::Sealed", 1)[0]
        self.assertLess(full.index("Self::pending_at("), full.index("PreparedCreate::new("))
        self.assertIn("self.session.borrow_policy_inventory()?", full)
        self.assertIn(".send_batch_replies(requests, allowance)", full)
        self.assertIn("let handle = witness.finish()?", full)
        witness = (SRC / "kernel_create_witness.rs").read_text()
        self.assertIn("session.inspect_policy_inventory_before(deadline)?", witness)
        self.assertIn("UntrustedStatus::AllAcknowledged", witness)

    def test_manager_never_routes_to_old_cleanup_coordinator(self):
        source = (SRC / "manager_exclusive_create_fixture.rs").read_text()
        for call in ["Coordinator::", ".stop_once(", ".unref_once(", ".empty_cgroup(",
                     "StopUnit\"", "UnrefUnit\"", ".native()", "Command::"]:
            self.assertNotIn(call, source)
        self.assertIn("self.real.admit_retaining_reference()?", source)
        self.assertIn("let original_job = self.real.start_once()?", source)
        self.assertIn("self.real.observe_start(&original_job)?", source)
        self.assertLess(source.index("ensure(completed"), source.index('join("create.frames")'))
        old = (SRC / "manager_retained_lifecycle_adapter.rs").read_text()
        self.assertIn("assert_ne!(fixture, Fixture::ExclusiveCreate);", old)

    def test_custody_and_handled_failure_have_no_secondary_output(self):
        for name in ["kernel_exclusive_create_fixture.rs", "manager_exclusive_create_fixture.rs"]:
            source = (SRC / name).read_text()
            self.assertLess(source.index("Box::leak("), source.index("catch_unwind("))
            self.assertIn("loop { std::thread::park(); }", source)
            self.assertNotIn("println!", source)
            self.assertNotIn("write_all(", source)
            self.assertIn("rustix::io::write(", source)

    def test_shared_frame_catalogue_and_budgets_are_finite(self):
        identity = (SRC / "manager_fixture_identity.rs").read_text()
        declaration = identity.split("EXCLUSIVE_CREATE_FRAMES", 1)[1]
        frames = declaration.split("= [\n", 1)[1].split("\n];", 1)[0]
        self.assertEqual(frames.count('b"K1_CREATE_'), 6)
        worker = (SRC / "kernel_exclusive_create_fixture.rs").read_text()
        manager = (SRC / "manager_exclusive_create_fixture.rs").read_text()
        self.assertIn("EXCLUSIVE_CREATE_FRAMES as FRAMES", worker)
        self.assertIn("EXCLUSIVE_CREATE_FRAMES.concat()", manager)
        self.assertIn("checked_add(Duration::from_secs(5))", worker)
        self.assertIn("checked_add(PHASE_BUDGET)", manager)
        self.assertIn("for _ in 0..MAX_POLLS", manager)


if __name__ == "__main__":
    unittest.main()
