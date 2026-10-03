#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Explicit disposable-VM Mihomo AWG cookie/MAC2 smoke; never run on host.

The tagged peer is an attested developer instrument, not a production engine.
Credentials/config/logs exist only in owned private namespace scratch.
"""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import socket
import stat
import subprocess
import sys
import tempfile
import time

import p4_awg_loopback_smoke as awg
from p4_awg_loopback_smoke import wg

EXPORT_SHA = "716c0eec8a7557485555397f1217e0cac56317af8b41fefc287027d0c74ce00d"
OBS_FIELDS = {"junk", "special", "special_mask", "init", "response", "transport", "protected", "trailers", "padding_size_matches",
              "cookie", "corrupted_cookie", "mac1_no_mac2", "valid_mac2", "invalid_mac1", "cookie_frame_error", "forced_load", "cookie_corrupt_mode", "mac2_after_cookie", "mac2_without_cookie"}


def observation(root, pid):
    facts = json.loads(awg.socket_payload(root / "stats.sock", pid))
    wg.require(set(facts) == OBS_FIELDS and all(type(value) is int and 0 <= value <= 10000 for value in facts.values()), "cookie_observation")
    return facts


def server_policy(payload, expected, generation, bypass=False):
    # Separate endpoint roles: 3.1 client retains its native DisableCookies=true;
    # receiving/consuming a cookie is independent of that server-challenge flag.
    values = dict(expected)
    if generation == "3.1" and not bypass:
        wg.require(payload.count(b"disable_cookies=true\n") == 1, "cookie_peer_policy")
        payload = payload.replace(b"disable_cookies=true\n", b"disable_cookies=false\n", 1)
    values[b"disable_cookies"] = b"1" if bypass else b"0"
    wg.require(not bypass or generation == "3.1", "cookie_peer_policy")
    return payload, values


def check_facts(facts, negative=False, bypass=False):
    wg.require(facts["forced_load"] == 1 and facts["cookie_frame_error"] == 0 and facts["mac1_no_mac2"] >= 1,
               "cookie_underload_active")
    wg.require(facts["cookie_corrupt_mode"] == int(negative), "cookie_peer_policy")
    if bypass:
        wg.require(facts["cookie"] == facts["corrupted_cookie"] == facts["valid_mac2"] == 0 and facts["response"] >= 1, "cookie_bypass")
    elif negative:
        wg.require(facts["cookie"] >= 1 and facts["corrupted_cookie"] == facts["cookie"] and facts["valid_mac2"] == 0
                   and facts["response"] == facts["transport"] == 0, "corrupt_cookie_refusal")
    else:
        wg.require(facts["cookie"] >= 1 and facts["corrupted_cookie"] == 0 and facts["valid_mac2"] >= 1
                   and facts["mac2_after_cookie"] == facts["valid_mac2"] and facts["mac2_without_cookie"] == 0
                   and facts["response"] >= 1 and facts["transport"] >= 1, "cookie_mac2_http_chain")


def run_phase(root, args, generation, public, payload, expected, negative=False, bypass=False):
    peer = http = None; tun_fd = http_fd = None
    try:
        wg.private_write(root / "cookie.mode", b"corrupt\n" if negative else b"pass\n")
        peer, tun_fd = awg.start_peer(root, args)
        payload, expected = server_policy(payload, expected, generation, bypass)
        wg.require(awg.socket_payload(root / "peer.sock", peer.pid, payload) == b"errno=0\n\n", "awg_peer_configuration")
        actual = awg.peer_get(root, peer.pid)
        wg.require(all(actual.get(key) == value for key, value in expected.items()), "awg_fields_active")
        def stats(public_key):
            values = awg.peer_get(root, peer.pid)
            wg.require(values.get(b"public_key") == base64.b64decode(public_key).hex().encode(), "peer_identity")
            return tuple(int(values[key]) for key in (b"last_handshake_time_sec", b"rx_bytes", b"tx_bytes"))
        wg.require(stats(public) == (0, 0, 0), "fresh_peer")
        before = observation(root, peer.pid)
        wg.require(before["forced_load"] == 1 and before["cookie_corrupt_mode"] == int(negative)
                   and before["cookie"] == before["valid_mac2"] == before["mac1_no_mac2"] == 0, "fresh_cookie_peer")
        with wg.log_handle(root / "http.log") as log:
            http_fd = os.open("/proc/self/ns/net", os.O_RDONLY | os.O_CLOEXEC)
            http = wg.launch([sys.executable, Path(wg.__file__).resolve(), "--http-child", "--parent-pid", str(os.getpid()), "--parent-net-fd", str(http_fd)], log, pass_fds=(http_fd,))
            deadline = time.monotonic() + 3
            while True:
                wg.require(http.poll() is None and time.monotonic() < deadline, "http_readiness")
                try:
                    with socket.create_connection(("10.203.0.1", 8089), timeout=.1): break
                except OSError: time.sleep(.05)
            wg.check_process(http, wg.namespace("net"))
            config_name = "negative" if negative else "positive"
            rendered = wg.command(wg.dropped([args.renderer, str(root), config_name, generation]), "private_render")
            wg.require(json.loads(rendered.stdout) == {"private_roundtrip": True, "flavor": generation}, "private_roundtrip")
            wg.phase(root, Path(args.core), config_name, "cookie", public, wg.namespace("net"), observe=stats)
            after = observation(root, peer.pid)
            check_facts(after, negative, bypass)
            if not negative:
                wg.require(after["junk"] >= 4 and after["special_mask"] == 31 and after["protected"] >= 3 and after["padding_size_matches"] >= 1, "awg_wire_fields")
                wg.require(after["trailers"] > 0 if generation == "3.1" else after["trailers"] == 0, "awg_generation_wire")
            if negative: wg.require(stats(public) == (0, 0, 0), "corrupt_cookie_peer_unchanged")
            wg.check_process(peer, wg.namespace("net"))
            return after
    except wg.Refused as error:
        if peer is not None and peer.returncode is None:
            try: error.facts = observation(root, peer.pid)
            except (OSError, ValueError, wg.Refused): pass
        raise
    finally:
        wg.stop(http); wg.stop(peer)
        for descriptor in (http_fd, tun_fd):
            if descriptor is not None: os.close(descriptor)
        links = json.loads(wg.command(["/usr/bin/ip", "-j", "link", "show"], "namespace_inventory").stdout)
        wg.require([row["ifname"] for row in links] == ["lo"], "awg_interface_cleanup")


def namespace_run(args):
    root = Path(args.scratch)
    wg.require(wg.private_directory(root) and os.getuid() == 0 and os.getppid() == int(args.parent_pid)
               and os.fstat(int(args.parent_net_fd)).st_ino == int(args.parent_net)
               and os.fstat(int(args.parent_user_fd)).st_ino == int(args.parent_user)
               and wg.namespace("net") != int(args.parent_net) and wg.namespace("user") != int(args.parent_user), "namespace_identity")
    wg.private_write(root / "namespace-identity.json", json.dumps({"net": wg.namespace("net"), "user": wg.namespace("user")}).encode())
    links = json.loads(wg.command(["/usr/bin/ip", "-j", "link", "show"], "namespace_inventory").stdout)
    routes = json.loads(wg.command(["/usr/bin/ip", "-j", "route", "show", "table", "all"], "namespace_inventory").stdout)
    wg.require([row["ifname"] for row in links] == ["lo"] and not routes, "namespace_interfaces")
    wg.command(["/usr/bin/ip", "link", "set", "lo", "up"], "namespace_loopback")
    result = {"positive": 0, "corrupt_negative": 0, "recovery": 0, "bypass": 0, "private_roundtrip": 0, "generations": {"3": [], "3.1": []}}
    for generation in ("3", "3.1"):
        for number in range(args.rounds):
            seed = root / f"g{generation}-r{number+1}"; seed.mkdir(mode=0o700)
            public, payload, expected = awg.private_keys(seed, generation)
            facts = {}
            for name in ("positive", "corrupt_negative", "recovery", *( ["bypass"] if generation == "3.1" else [] )):
                phase = seed / name; phase.mkdir(mode=0o700)
                wg.private_write(phase / "header.key", (seed / "header.key").read_bytes())
                # Negative is the exact same VALID credential/native profile.
                # Only all received cookies are corrupted, never the import/key.
                config = (seed / "positive.conf").read_bytes()
                for config_name in ("positive", "negative"): wg.private_write(phase / f"{config_name}.conf", config)
                facts[name] = run_phase(phase, args, generation, public, payload, expected, name == "corrupt_negative", name == "bypass")
                result[name] += 1; result["private_roundtrip"] += 1
            result["generations"][generation].append(facts)
    result["interface_cleanup"] = True
    print(json.dumps(result, sort_keys=True))


def verify_receipt(path, expected_receipt, expected_peer):
    wg.require(path.is_absolute() and wg.private_directory(path.parent), "cookie_peer_receipt")
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    with os.fdopen(descriptor, "rb") as file:
        before = os.fstat(file.fileno())
        wg.require(stat.S_ISREG(before.st_mode) and before.st_uid == os.getuid() and before.st_mode & 0o077 == 0
                   and before.st_nlink == 1 and 0 < before.st_size <= wg.BODY_LIMIT, "cookie_peer_receipt")
        data = file.read(wg.BODY_LIMIT + 1); after = os.fstat(file.fileno())
    wg.require((before.st_dev, before.st_ino, before.st_mode, before.st_uid, before.st_nlink, before.st_size, before.st_mtime_ns, before.st_ctime_ns)
               == (after.st_dev, after.st_ino, after.st_mode, after.st_uid, after.st_nlink, after.st_size, after.st_mtime_ns, after.st_ctime_ns)
               and len(data) == before.st_size and re.fullmatch(r"[a-f0-9]{64}", expected_receipt or "") and hashlib.sha256(data).hexdigest() == expected_receipt, "cookie_peer_receipt")
    def unique(pairs):
        result = {}
        for key, value in pairs:
            wg.require(key not in result, "cookie_peer_receipt"); result[key] = value
        return result
    receipt = json.loads(data, object_pairs_hook=unique)
    fields = {"upstream_sha", "export_sha256", "fixture_sha256", "builder_sha256", "exporter_sha256", "source_tests_only", "namespace_run", "count", "race_tests", "toolchain", "binary_sha256", "binary_attestation", "normal_peer_has_no_tagged_hooks", "source_unchanged", "export_overlay_removed", "engine_subcases", "wrapper_tests"}
    wg.require(isinstance(receipt, dict) and set(receipt) == fields, "cookie_peer_receipt")
    wg.require(receipt.get("upstream_sha") == awg.UPSTREAM and receipt.get("export_sha256") == EXPORT_SHA
               and receipt.get("binary_sha256") == expected_peer and receipt.get("normal_peer_has_no_tagged_hooks") is True
               and receipt.get("source_unchanged") is True and receipt.get("export_overlay_removed") is True,
               "cookie_peer_receipt")
    count = receipt.get("count")
    wg.require(type(count) is int and 1 <= count <= 50 and receipt.get("engine_subcases") == count*6 and receipt.get("wrapper_tests") == count*4
               and receipt.get("source_tests_only") is True and receipt.get("namespace_run") is False
               and type(receipt.get("race_tests")) is bool and "\tbuild\t-tags=p4_cookie_transport" in receipt.get("binary_attestation", []), "cookie_peer_receipt")


def verify_outcome(outcome, rounds):
    wg.require(isinstance(outcome, dict) and set(outcome) == {"positive", "corrupt_negative", "recovery", "bypass", "private_roundtrip", "generations", "interface_cleanup"}, "namespace_result")
    wg.require(all(type(outcome.get(name)) is int for name in ("positive", "corrupt_negative", "recovery", "bypass", "private_roundtrip"))
               and all(outcome.get(name) == 2*rounds for name in ("positive", "corrupt_negative", "recovery"))
               and outcome.get("bypass") == rounds and outcome.get("private_roundtrip") == 7*rounds and outcome.get("interface_cleanup") is True, "namespace_result")
    generations = outcome.get("generations")
    wg.require(isinstance(generations, dict) and set(generations) == {"3", "3.1"}, "namespace_result")
    for generation, rows in generations.items():
        wg.require(isinstance(rows, list) and len(rows) == rounds, "namespace_result")
        for row in rows:
            wg.require(isinstance(row, dict) and set(row) == {"positive", "corrupt_negative", "recovery", *( ["bypass"] if generation == "3.1" else [] )}, "namespace_result")
            for name, facts in row.items():
                wg.require(isinstance(facts, dict) and set(facts) == OBS_FIELDS and all(type(value) is int and 0 <= value <= 10000 for value in facts.values()), "namespace_result")
                check_facts(facts, name == "corrupt_negative", name == "bypass")


def outer(args):
    wg.require(args.run and os.getuid() != 0 and 1 <= args.rounds <= 3, "explicit_vm_opt_in")
    wg.require(re.fullmatch(r"[a-f0-9]{40}", args.source_sha or "") and args.upstream_sha == awg.UPSTREAM, "source_identity")
    verify_receipt(Path(args.peer_receipt), args.expected_receipt_sha256, args.expected_peer_sha256)
    for tool in ("unshare", "ip", "wg", "curl", "setpriv", "getcap"):
        wg.require(Path(f"/usr/bin/{tool}").is_file(), "missing_tool")
    wg.require(Path("/dev/net/tun").is_char_device(), "tun_unavailable")
    before = wg.outside_snapshot()
    sources = {str(Path(path).resolve()): wg.digest(Path(path).resolve()) for path in (__file__, awg.__file__, wg.__file__)}
    cache = Path.home() / ".cache/omavless-p4-awg-cookie"; cache.mkdir(mode=0o700, exist_ok=True)
    wg.require(wg.private_directory(cache), "scratch_parent")
    root = Path(tempfile.mkdtemp(prefix="run-", dir=cache)); child = None; descriptors = []; outcome = None
    try:
        for name, source, identity in (("core", args.core, args.expected_core_sha256), ("renderer", args.renderer, args.expected_renderer_sha256), ("peer", args.peer, args.expected_peer_sha256)):
            wg.copy_binary(Path(source), root / name, identity)
        descriptors = [os.open(f"/proc/self/ns/{name}", os.O_RDONLY | os.O_CLOEXEC) for name in ("net", "user")]
        argv = ["/usr/bin/unshare", "--user", "--map-root-user", "--net", sys.executable, str(Path(__file__).resolve()), "--namespace-child", "--scratch", str(root), "--parent-pid", str(os.getpid()),
                "--parent-net", str(wg.namespace("net")), "--parent-user", str(wg.namespace("user")), "--parent-net-fd", str(descriptors[0]), "--parent-user-fd", str(descriptors[1]),
                "--core", str(root / "core"), "--renderer", str(root / "renderer"), "--peer", str(root / "peer"), "--rounds", str(args.rounds)]
        with wg.log_handle(root / "namespace-result.json") as log:
            child = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=log, stderr=log, env=wg.ENV, start_new_session=True, preexec_fn=wg.child_limit, pass_fds=descriptors)
            code = child.wait(timeout=150)
        data = (root / "namespace-result.json").read_bytes(); wg.require(len(data) <= wg.BODY_LIMIT, "namespace_result_bound")
        outcome = json.loads(data)
        if code != 0:
            stage = outcome.get("stage", "") if isinstance(outcome, dict) else ""
            wg.require(stage in SAFE_STAGES, "namespace_smoke")
            error = wg.Refused(stage)
            if isinstance(outcome.get("facts"), dict): error.facts = outcome["facts"]
            raise error
        verify_outcome(outcome, args.rounds)
    finally:
        if child is not None:
            # A reaped namespace leader never authorizes another group signal.
            # Preserve its unreaped identity until timeout group cleanup finishes.
            if child.returncode is None:
                for action in (signal.SIGTERM, signal.SIGKILL):
                    try: os.killpg(child.pid, action)
                    except ProcessLookupError: pass
                    try: child.wait(timeout=3); break
                    except subprocess.TimeoutExpired: pass
            wg.require(child.returncode is not None, "owned_namespace_cleanup")
        for descriptor in descriptors: os.close(descriptor)
        identity = root / "namespace-identity.json"
        if identity.exists():
            net = json.loads(identity.read_bytes())["net"]
            for entry in Path("/proc").iterdir():
                if entry.name.isdecimal():
                    try: inode = wg.namespace("net", entry.name)
                    except OSError: continue
                    wg.require(inode != net, "namespace_process_cleanup")
        wg.require(root.parent == cache and wg.private_directory(root), "scratch_cleanup_target")
        shutil.rmtree(root)
        wg.require(not root.exists() and wg.outside_snapshot() == before, "outside_state_changed")
        wg.require(all(wg.digest(Path(path)) == identity for path, identity in sources.items()), "harness_identity")
    outcome.update(status="PASS", source_sha=args.source_sha, upstream_sha=args.upstream_sha, harness_sha256=sources[str(Path(__file__).resolve())],
                   core_sha256=args.expected_core_sha256, renderer_sha256=args.expected_renderer_sha256, peer_sha256=args.expected_peer_sha256,
                   receipt_sha256=args.expected_receipt_sha256, scratch_cleanup=True, outside_state_unchanged=True, installed_runtime_unchanged=True)
    print(json.dumps(outcome, sort_keys=True))


SAFE_STAGES = awg.SAFE_STAGES | {"cookie_peer_policy", "cookie_observation", "cookie_underload_active", "cookie_bypass", "corrupt_cookie_refusal", "cookie_mac2_http_chain", "fresh_cookie_peer", "cookie_peer_receipt", "corrupt_cookie_peer_unchanged"}


def main():
    parser = argparse.ArgumentParser(description=__doc__); parser.add_argument("--run", action="store_true")
    parser.add_argument("--namespace-child", action="store_true", help=argparse.SUPPRESS)
    for name in ("scratch", "parent-net", "parent-user", "parent-pid", "parent-net-fd", "parent-user-fd"):
        parser.add_argument(f"--{name}", help=argparse.SUPPRESS)
    parser.add_argument("--core", default="/usr/bin/mihomo"); parser.add_argument("--rounds", type=int, default=3)
    for name in ("renderer", "peer", "peer-receipt", "expected-receipt-sha256", "expected-core-sha256", "expected-renderer-sha256", "expected-peer-sha256", "source-sha", "upstream-sha"):
        parser.add_argument(f"--{name}")
    args = parser.parse_args()
    try: namespace_run(args) if args.namespace_child else outer(args)
    except wg.Refused as error:
        outcome = {"status": "REFUSED", "stage": error.stage}; facts = getattr(error, "facts", None)
        if isinstance(facts, dict) and set(facts) <= OBS_FIELDS and all(type(value) is int and 0 <= value <= 10000 for value in facts.values()): outcome["facts"] = facts
        print(json.dumps(outcome, sort_keys=True)); return 2
    except (Exception, KeyboardInterrupt) as error:
        trace = error.__traceback__
        while trace.tb_next is not None: trace = trace.tb_next
        awg.diagnostic(type(error).__name__, trace.tb_lineno, trace.tb_frame.f_code.co_name); return 2
    return 0


if __name__ == "__main__": sys.exit(main())
