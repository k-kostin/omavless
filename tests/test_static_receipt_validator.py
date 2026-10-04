"""Synthetic strict receipt controls; no VM, readelf or candidate execution."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).parent / "static_elf_provenance"
SPEC = importlib.util.spec_from_file_location("static_receipt_validator", ROOT / "validate_static_receipt.py")
validator = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(validator)
INVENTORY = json.loads((ROOT.parent / "real_resolved_binary/guest-inventory.json").read_bytes())


def positive():
    package = {"name": "example", "version": "1-1", "file_list_sha256": "1" * 64, "description_sha256": "2" * 64}
    aliases, records, seen = [], [], set()
    seeds = list(INVENTORY["elfs"].items()) + [(validator.CANDIDATE, {"resolved_path": validator.CANDIDATE, "sha256": "3" * 64, "mode": "0o755"})]
    for logical, seed in seeds:
        resolved = seed["resolved_path"]
        links = [] if logical == resolved else [{"path": logical, "target": resolved,
                 "identity": [31, 100, len(resolved), 0, 0, 0o120777, 1, 1, 1]}]
        aliases.append({"path": logical, "resolved_path": resolved, "links": links})
        if resolved in seen:
            continue
        seen.add(resolved)
        records.append({"path": logical, "resolved_path": resolved, "links": links,
                        "device": 31, "inode": 100 + len(records), "uid": 0, "gid": 0,
                        "mode": 0o100000 | int(seed["mode"], 8), "nlink": 1, "size": 64,
                        "sha256": seed["sha256"], "package": copy.deepcopy(package),
                        "source": "original_manifest" if logical in INVENTORY["elfs"] else "explicit_observation_candidate",
                        "depth": 0, "dynamic": {"needed": [], "interpreter": None, "declared_search_tokens": []},
                        "dependencies": [], "matches_original_manifest": logical in INVENTORY["elfs"]})
    tool = copy.deepcopy(package)
    tool.update(name="binutils", version="2.47-4")
    value = {"schema": "public-static-elf-provenance-v1", "outcome": "OBSERVED_STATIC_CANDIDATE_CLOSURE",
             "candidate_elf_executed": False, "allowlist_adoption": False, "loaded_elf_identity_proven": False,
             "compatibility_acceptance": False, "search_policy": list(validator.SEARCH),
             "records": records, "aliases": aliases, "readelf": {"path": "/usr/bin/readelf", "sha256": validator.READELF_SHA, "package": tool}}
    owned = {"schema": "elf-static-owned-child-v1", "outcome": "KNOWN_COMPLETED", "returncode": 0}
    return value, owned


class ReceiptTests(unittest.TestCase):
    def test_finite_positive_and_exact_validator_pin(self):
        value, owned = positive()
        validator.validate(value, owned, INVENTORY)
        guard = (ROOT / "vm-guard-empty-record.sh").read_text()
        self.assertIn(hashlib.sha256((ROOT / "validate_static_receipt.py").read_bytes()).hexdigest(), guard)

    def test_duplicate_keys_nonfinite_and_oversize_json_refuse(self):
        for raw in (b'{"schema":1,"schema":2}', b'{"nested":{"x":1,"x":2}}', b'NaN', b'Infinity', b' ' * (4 * 1024 * 1024 + 1)):
            with self.subTest(raw=raw[:64]), self.assertRaises(ValueError):
                validator.decode(raw)

    def test_nested_shapes_exact_keys_types_and_authority_refuse(self):
        changes = [
            lambda v, o: o.update(returncode=False),
            lambda v, o: v.update(records="x" * 16),
            lambda v, o: v.update(aliases="x" * 17),
            lambda v, o: v.update(extra=True),
            lambda v, o: o.update(extra=True),
            lambda v, o: v.update(allowlist_adoption=0),
            lambda v, o: v["records"][0].update(uid=False),
            lambda v, o: v["records"][0].update(inode=0),
            lambda v, o: v["records"][0].update(mode=0o100777),
            lambda v, o: v["records"][0].update(size=33 * 1024 * 1024),
            lambda v, o: v["records"][0].update(source="invented"),
            lambda v, o: v["records"][0].update(depth=True),
            lambda v, o: v["records"][0].update(sha256="z" * 64),
            lambda v, o: v["records"][0]["package"].update(name="/private"),
            lambda v, o: v["records"][0]["package"].update(extra=True),
            lambda v, o: v["records"][0]["dynamic"].update(needed="abc"),
            lambda v, o: v["records"][0]["dynamic"].update(declared_search_tokens=["/private"]),
            lambda v, o: v["records"][0]["dynamic"].update(interpreter=False),
            lambda v, o: v["records"][0].update(dependencies=[{}]),
            lambda v, o: v["aliases"][0]["links"][0]["identity"].__setitem__(0, False),
            lambda v, o: v["aliases"][0]["links"][0].update(target="/private"),
            lambda v, o: v["readelf"]["package"].update(version="unreviewed"),
        ]
        for index, change in enumerate(changes):
            value, owned = positive()
            change(value, owned)
            with self.subTest(change=index), self.assertRaises((ValueError, TypeError)):
                validator.validate(value, owned, INVENTORY)

    def test_dependency_alias_and_source_edges_are_cross_checked(self):
        value, owned = positive()
        parent, target = value["records"][0], value["records"][-1]
        name = Path(target["path"]).name
        parent["dynamic"]["needed"] = [name]
        parent["dependencies"] = [{"name": name, "logical_path": target["path"], "resolved_path": target["resolved_path"], "links": []}]
        validator.validate(value, owned, INVENTORY)
        parent["dependencies"][0]["resolved_path"] = "/usr/lib/unrecorded.so"
        with self.assertRaises(ValueError):
            validator.validate(value, owned, INVENTORY)

    def test_actual_shell_validator_branch_stops_all_malformed_receipts(self):
        guard = (ROOT / "vm-guard-empty-record.sh").read_text()
        line = next(row for row in guard.splitlines() if row.startswith('if ! env ') and 'validator.py' in row)
        branch = guard.split(line, 1)[1].split('\nfi\n', 1)[0]
        for fault in ("valid", "duplicate", "false", "records", "aliases", "nested"):
            value, owned = positive()
            if fault == "false": owned["returncode"] = False
            if fault == "records": value["records"] = "x" * 16
            if fault == "aliases": value["aliases"] = "x" * 17
            if fault == "nested": value["records"][0]["package"]["name"] = False
            raw = json.dumps(value)
            if fault == "duplicate": raw = raw.replace('"schema":', '"schema":"duplicate","schema":', 1)
            with self.subTest(fault=fault), tempfile.TemporaryDirectory(dir="/var/tmp") as temporary:
                stage = Path(temporary)
                (stage / "result.json").write_text(raw)
                (stage / "supervisor-receipt.json").write_text(json.dumps(owned))
                (stage / "guest-inventory.json").write_text(json.dumps(INVENTORY))
                # Only transport is replaced: execute the real strict decoder
                # and validator, then the exact production shell exit branch.
                code = 'import importlib.util,pathlib,sys;s=importlib.util.spec_from_file_location("v",sys.argv[1]);v=importlib.util.module_from_spec(s);s.loader.exec_module(v);p=pathlib.Path(sys.argv[2]);v.validate(v.decode((p/"result.json").read_bytes()),v.decode((p/"supervisor-receipt.json").read_bytes()),v.decode((p/"guest-inventory.json").read_bytes()))'
                # Function positional parameters are env's args, so retain
                # runner inputs in fixed shell variables before calling it.
                script = 'task_stage="$1"; code="$2"; validator="$3"; env() { /usr/bin/python3 -c "$code" "$validator" "$task_stage"; };\n' + line + branch + '\nfi\nprintf AFTER_VALIDATED\n'
                result = subprocess.run(["/bin/bash", "-c", script, "test", temporary, code, str(ROOT / "validate_static_receipt.py")], capture_output=True, text=True)
                self.assertEqual(result.returncode, 0 if fault == "valid" else 1)
                self.assertEqual(result.stdout, "AFTER_VALIDATED" if fault == "valid" else "")


if __name__ == "__main__":
    unittest.main()
