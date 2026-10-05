"""Source-only fixed post-send/pre-receive crash seam; never executes a VM."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "crates/omavless-netguard/src"


class SendReturnBoundaryTests(unittest.TestCase):
    def test_hook_and_both_senders_remain_test_only(self):
        source = (SRC / "kernel_observer.rs").read_text()
        for name in ["send_return_cut", "creator_lifecycle", "conditional_delete"]:
            self.assertIn(f'#[cfg(test)]\n#[path = "kernel_{name}.rs"]\nmod {name};', source)
        hook = (SRC / "kernel_send_return_cut.rs").read_text()
        for forbidden in ["sendto(", "recvmsg", "Command::", "EffectIdentity", "NamespaceObservation", "unsafe"]:
            self.assertNotIn(forbidden, hook)
        self.assertIn("replies.acks().iter().all(|ack| !ack)", hook)
        self.assertIn("!replies.complete()", hook)
        self.assertIn("self.consumed = true", hook)

    def test_exact_hook_after_single_send_before_any_receive(self):
        for file in ["kernel_creator_lifecycle.rs", "kernel_conditional_delete.rs"]:
            source = (SRC / file).read_text()
            self.assertEqual(source.count("sendto("), 1)
            self.assertEqual(source.count(".after_send("), 1)
            self.assertLess(source.index("sendto("), source.index("== batch.len()"))
            self.assertLess(source.index("== batch.len()"), source.index(".after_send("))
            self.assertLess(source.index(".after_send("), source.index("while !replies.complete()"))
            self.assertLess(source.index(".after_send("), source.index("recvmsg::<"))

    def test_holder_keeps_unreaped_writer_and_never_promotes_orphan(self):
        source = (SRC / "kernel_creator_lifecycle_tests.rs").read_text()
        for case in ["create", "replace", "delete"]:
            self.assertIn(f'isolated_cases(&["send-{case}"])', source)
        held = source.split("struct UnreapedWriter", 1)[1].split("fn send_return_receipt", 1)[0]
        self.assertIn("WaitPidFlag::WNOWAIT", held)
        self.assertNotIn("try_wait", held)
        self.assertIn("!self.anchor_known", held)
        self.assertIn("self.anchor_known = false", held)
        runner = source.split("fn isolated_send_case(", 1)[1].split("fn isolated_cases(", 1)[0]
        self.assertNotIn("ChildGuard", runner)
        self.assertNotIn("try_wait", runner)
        self.assertNotIn(".kill(", runner)
        for expected in ["WaitPidFlag::WNOWAIT", "OFlag::O_NONBLOCK", "stdout_eof && stderr_eof",
                         "assert_fixed_group(&[leader, child.id() as i32])"]:
            self.assertIn(expected, runner)
        self.assertLess(runner.index("assert_fixed_group(&[leader, child.id() as i32])"), runner.index("child.wait()"))
        self.assertIn("self.1 && std::thread::panicking()", source)
        case = source.split("fn send_return_case(", 1)[1].split("#[test]", 1)[0]
        for expected in ["StateError::Busy", "writer.kill_and_reap()", "kind.phase()", "OtherUntrusted",
                         "assert_eq!(table.owner, None)", "assert_eq!(fresh.effects, 0)",
                         "assert_eq!(fixture.records(), records)"]:
            self.assertIn(expected, case)
        self.assertNotIn("OwnedVerified", case)
        self.assertNotIn("unwrap_or", case)


if __name__ == "__main__":
    unittest.main()
