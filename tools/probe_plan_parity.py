#!/usr/bin/env python3
"""Synthetic-only pure probe oracle; never starts a core or resolves an endpoint."""
import hashlib
import json
import sys
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
import backend  # noqa: E402
from tools.vless_canonical_parity import canonical_fingerprint, mihomo_semantics  # noqa: E402
from tools.non_vless_canonical_parity import mihomo  # noqa: E402


def main():
    try:
        raw = sys.stdin.buffer.read(8 * 1024 * 1024 + 1)
        if len(raw) > 8 * 1024 * 1024 or sys.argv[1:]:
            return 1
        data = json.loads(raw)
        if set(data) != {"cases"} or len(data["cases"]) > 256:
            return 1
        output = []
        for case in data["cases"]:
            profile = {"id": "fixture", "uri": case["uri"], "subscriptionId": "fixture-sub"}
            targets = [{"profile": profile, "profileId": "fixture", "alias": f"p0000a{i}", "address": address}
                       for i, address in enumerate(case["addresses"])]
            samples = {target["alias"]: [] for target in targets}
            for status, payload in case["responses"]:
                backend.collect_probe_response(samples, status, payload)
            with patch.object(backend, "load_store", return_value={"profiles": [profile]}), \
                 patch.object(backend, "subscription_by_id", return_value={"id": "fixture-sub"}), \
                 patch.object(backend, "configured_working_probe_resolvers", return_value=[]), \
                 patch.object(backend, "resolve_probe_addresses", return_value=case["addresses"]), \
                 patch.object(backend, "run_mihomo_probe", return_value=samples):
                result = backend.probe_subscription(None, "fixture-sub")["results"][0]
            result.pop("id")
            config = backend.probe_core_config(targets) if targets else ""
            proxy_hashes = []
            for target in targets:
                protocol = backend.profile_protocol(case["uri"])
                node = backend.PROFILE_ADAPTERS[protocol].parse(case["uri"], True)
                mapping = (mihomo_semantics(node, target["alias"], target["address"])
                           if protocol == "vless" else mihomo(node, protocol, target["alias"], target["address"]))
                proxy_hashes.append(canonical_fingerprint(mapping))
            if targets:
                proxies = "\n".join(backend.probe_proxy_yaml(target["profile"], target["alias"], target["address"]) for target in targets)
                config = config.replace(proxies, "<canonical-proxies>", 1)
            output.append({"configHash": hashlib.sha256(config.encode()).hexdigest(), "proxyHashes": proxy_hashes, "result": result})
        # Verify actual executor schedule structurally, without invoking it.
        import ast
        import inspect
        tree = ast.parse(inspect.getsource(backend.run_mihomo_probe))
        loops = [node for node in ast.walk(tree) if isinstance(node, ast.For)
                 and isinstance(node.iter, ast.Name) and node.iter.id == "PROBE_URLS"]
        if len(loops) != 1:
            return 1
        parents = {child: node for node in ast.walk(tree) for child in ast.iter_child_nodes(node)}
        parent = parents.get(loops[0])
        while parent is not None:
            if isinstance(parent, (ast.For, ast.While)):
                return 1
            parent = parents.get(parent)
        reads = [node for node in ast.walk(loops[0]) if isinstance(node, ast.Call)
                 and isinstance(node.func, ast.Name) and node.func.id == "controller_json"]
        if len(reads) != 1 or any(isinstance(node, (ast.For, ast.While))
                                  for child in loops[0].body for node in ast.walk(child)):
            return 1
        print(json.dumps({"cases": output, "urls": backend.PROBE_URLS}))
        return 0
    except Exception:
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
