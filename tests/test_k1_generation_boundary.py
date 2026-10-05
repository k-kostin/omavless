"""Source-only shared observation routing; not namespace or kernel proof."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "crates/omavless-netguard/src"


class GenerationBoundaryTests(unittest.TestCase):
    def test_one_normal_private_fence_and_retained_state(self):
        source = (SRC / "kernel_observer.rs").read_text()
        normal = source.split("#[cfg(test)]\nmod tests {", 1)[0]
        self.assertEqual(normal.count("fn finish_read_exchange("), 1)
        self.assertNotIn("pub fn finish_read_exchange", normal)
        self.assertNotIn("#[cfg(test)]\nfn finish_read_exchange", normal)
        self.assertEqual(normal.count("last_generation: Option<u32>"), 1)
        wrapper = normal.split("    fn exchange(&mut self,", 1)[1].split("    fn exchange_once(", 1)[0]
        self.assertIn("self.exchange_once(kind, seq, deadline)", wrapper)
        self.assertIn("finish_read_exchange(&mut self.last_generation, &mut self.poisoned, result)", wrapper)
        once = normal.split("    fn exchange_once(", 1)[1].split("    /// Inspect exactly", 1)[0]
        self.assertLess(once.index("self.check(deadline)?"), once.index("sendto("))
        self.assertEqual(normal.count("exchange_once("), 2)
        fence = normal.split("fn finish_read_exchange(", 1)[1].split("/// A retained", 1)[0]
        for expected in ["require(!*poisoned)?", "exchange.complete()", "exchange.generation()?",
                         "observed >= last", "*poisoned = true"]:
            self.assertIn(expected, fence)
        for forbidden in ["sendto(", "socket(", "recvmsg", "EffectIdentity", "Canonical",
                          "EffectPort", "false", "unsafe"]:
            self.assertNotIn(forbidden, fence)

    def test_all_five_readers_preserve_central_routing_and_equal_brackets(self):
        files = ["observer", "chain_observer", "rule_observer", "rule_wire", "inventory"]
        for file in files:
            source = (SRC / f"kernel_{file}.rs").read_text().split("#[cfg(test)]\nmod tests {", 1)[0]
            self.assertEqual(source.count("self.exchange(GET_GEN,"), 2, file)
            self.assertNotIn("self.exchange_once(GET_GEN", source)
            self.assertIn("self.poisoned = true", source)
            if file in ["observer", "chain_observer", "rule_observer"]:
                self.assertIn("require(before == after)?", source)
            else:
                self.assertRegex(source, r"require\(self\.exchange\(GET_GEN,.*\?\.generation\(\)\? == generation\)\?")

    def test_creator_and_delete_cannot_keep_a_separate_generation_history(self):
        creator = (SRC / "kernel_creator_lifecycle.rs").read_text()
        self.assertNotIn("last_generation", creator)
        self.assertIn("let result = self.session.inspect_policy_inventory_once();", creator)
        self.assertIn("self.created = None;", creator)
        self.assertIn("self.session.prepare_inventory_delete()?", creator)
        self.assertIn("self.session.borrow_policy_inventory()?", creator)
        self.assertIn("session: lease.session", creator)
        self.assertIn("deadline.min(lease.deadline)", creator)
        delete = (SRC / "kernel_conditional_delete.rs").read_text()
        self.assertIn("lease: LocalInventoryLease<'a>", delete)
        self.assertIn("self.borrow_policy_inventory()?", delete)
        self.assertIn("lease.recheck()", delete)
        self.assertIn("let session = lease.session;", delete)
        self.assertIn("session.inspect_policy_inventory()", delete)
        self.assertNotIn("inspect_policy_inventory_once()", delete)
        self.assertNotIn("last_generation", delete)
        inventory = (SRC / "kernel_inventory.rs").read_text()
        self.assertIn("self.session.last_generation == Some(self.generation)", inventory)
        self.assertIn("self.inspect_policy_inventory_before(deadline)", inventory)
        self.assertIn("self.borrow_policy_inventory().map(|lease| lease.observed())", inventory)
        observer = (SRC / "kernel_observer.rs").read_text()
        for module in ["creator_lifecycle", "conditional_delete"]:
            self.assertIn(f'#[cfg(test)]\n#[path = "kernel_{module}.rs"]\nmod {module};', observer)


if __name__ == "__main__":
    unittest.main()
