"""Portable text/AST integrity only; no Rust/compiler/model/tool execution."""
import ast
import hashlib
import json
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "tests/research/actor-state"
NAMES = (
    "abandoned_preparation_cannot_reset_owner",
    "every_nonpositive_fact_seals_without_promotion",
    "exact_positive_uses_original_results_only",
    "fresh_label_is_not_live_authority",
    "incomplete_drop_during_caught_unwind_only_models_label_poison",
    "uncertain_live_consumes_same_owner_without_recovery",
)


class ActorExport(unittest.TestCase):
    def test_exact_unchanged_model_and_selected_six_names(self):
        record = json.loads((FIXTURE / "provenance.json").read_bytes())
        raw = (FIXTURE / record["model"]["path"]).read_bytes()
        self.assertEqual(len(raw), record["model"]["bytes"])
        self.assertEqual(hashlib.sha256(raw).hexdigest(), record["model"]["sha256"])
        names = re.findall(rb"#\[test\]\s+fn ([a-z_]+)\(", raw)
        self.assertEqual(tuple(sorted(n.decode("ascii") for n in names)), NAMES)
        self.assertIn(b"#![forbid(unsafe_code)]", raw)
        self.assertNotIn(b"use std::process", raw)
        self.assertNotIn(b"OwnedFd", raw)
        self.assertNotIn(b"extern ", raw)

    def test_templates_remain_unbound_without_autorun_or_private_paths(self):
        record = json.loads((FIXTURE / "provenance.json").read_bytes())
        for name in ("compile_recipe.py.in", "mock_recipe.py.in"):
            raw = (FIXTURE / name).read_bytes()
            self.assertEqual(hashlib.sha256(raw).hexdigest(), record["relocated_inert_templates"][name])
            self.assertNotIn(b"/home/kk/.cache", raw)
            self.assertNotIn(b"__main__", raw)
            tree = ast.parse(raw)
            self.assertFalse(any(isinstance(n, ast.Expr) and isinstance(n.value, ast.Call)
                                 for n in tree.body))
        tree = ast.parse((FIXTURE / "compile_recipe.py.in").read_bytes())
        bindings = {n.targets[0].id: ast.literal_eval(n.value) for n in tree.body
                    if isinstance(n, ast.Assign) and isinstance(n.targets[0], ast.Name)
                    and n.targets[0].id in ("SOURCE", "RUSTC", "ROOT")}
        self.assertEqual(bindings, {"SOURCE": None, "RUSTC": None, "ROOT": None})
        main = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "main")
        expected = ast.parse("need(SOURCE is not None and RUSTC is not None and ROOT is not None)").body[0]
        self.assertEqual(ast.dump(main.body[3]), ast.dump(expected))

    def test_template_retains_lf_and_checked_output_boundaries(self):
        text = (FIXTURE / "compile_recipe.py.in").read_text()
        self.assertNotIn("splitlines", text.replace("# Whole exact bytes, literal LF and exact two terminal LFs. No splitlines.", ""))
        self.assertIn("re.fullmatch(re.escape(prefix)", text)
        self.assertIn("run.emit(receipt,sys.stdout.buffer)", text)
        tree = ast.parse(text)
        run = next(n for n in tree.body if isinstance(n, ast.ClassDef) and n.name == "Run")
        emit = next(n for n in run.body if isinstance(n, ast.FunctionDef) and n.name == "emit")
        body = ast.unparse(emit)
        self.assertLess(body.index("self.call(stream.write, raw)"), body.index("count == len(raw)"))
        self.assertLess(body.index("count == len(raw)"), body.index("self.call(stream.flush)"))
        self.assertLess(body.index("self.call(stream.flush)"), body.index("self.tick()"))
        self.assertIn("self.sealed = True", body)
        mocks = ast.parse((FIXTURE / "mock_recipe.py.in").read_bytes())
        cases = next(n for n in mocks.body if isinstance(n, ast.ClassDef) and n.name == "Controls")
        self.assertEqual(sum(isinstance(n, ast.FunctionDef) and n.name.startswith("test_")
                             for n in cases.body), 8)

    def test_receipt_never_becomes_actor_or_fatal_custody_evidence(self):
        record = json.loads((FIXTURE / "provenance.json").read_bytes())
        for field in ("actor_acquisition_executed", "socket_or_descriptor_import_executed",
                      "fatal_descriptor_custody_proven", "canonical_t4_contract_changed",
                      "production_backend_or_dependency_changed"):
            self.assertIs(record[field], False)
        self.assertIs(record["relocated_inert_templates"]["selected_or_compiled"], False)
        self.assertIs(record["relocated_inert_templates"]["inherits_external_recipe_evidence"], False)
        selected = record["selected_external_recipe"]
        self.assertEqual((selected["mock_original_exit"], selected["whole_original_exit"]), (0, 0))
        self.assertEqual((selected["mock_test_count"], record["model"]["pure_test_count"]), (8, 6))
        self.assertRegex(selected["produced_binary_sha256"], r"^[0-9a-f]{64}$")


if __name__ == "__main__":
    unittest.main()
