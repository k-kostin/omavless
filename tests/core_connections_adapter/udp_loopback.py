#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Disposable conditional-close UDP gate; fixed IPv4 loopback only, DNS/TUN off."""
import argparse
import contextlib
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import select
import socket
import subprocess
import tempfile
import threading
import time

import loopback


def receive_exact(connection, size):
    result = b""
    while len(result) < size:
        data = connection.recv(size - len(result))
        if not data:
            raise ValueError("UDP association ended early")
        result += data
    return result


def header(target):
    if type(target) is not int or not 0 < target < 65536:
        raise ValueError("Invalid synthetic UDP target")
    return b"\x00\x00\x00\x01\x7f\x00\x00\x01" + target.to_bytes(2, "big")


def associate(mixed, client):
    connection = socket.create_connection(("127.0.0.1", mixed), timeout=2)
    try:
        connection.sendall(b"\x05\x01\x00")
        if receive_exact(connection, 2) != b"\x05\x00":
            raise ValueError("Synthetic SOCKS negotiation refused")
        connection.sendall(b"\x05\x03\x00" + header(client.getsockname()[1])[3:])
        expected = b"\x05\x00\x00" + header(mixed)[3:]
        if receive_exact(connection, 10) != expected:
            raise ValueError("Nonlocal or unexpected UDP relay refused")
        return connection
    except BaseException:
        connection.close()
        raise


def validate_echo(raw, peer, mixed, target, challenge):
    if peer != ("127.0.0.1", mixed) or raw != header(target) + challenge:
        raise ValueError("Unexpected synthetic UDP response")


def prove_echo(client, mixed, target, *, reconnect=False):
    # Only application datagrams may be repeated after asynchronous NAT cleanup.
    # There is never a retry of a conditional effect, including on lost replies.
    for _ in range(5 if reconnect else 1):
        challenge = b"synthetic-udp-" + secrets.token_bytes(16)
        client.sendto(header(target) + challenge, ("127.0.0.1", mixed))
        try:
            raw, peer = client.recvfrom(4097)
        except socket.timeout:
            if reconnect:
                continue
            raise ValueError("Synthetic UDP echo timed out") from None
        validate_echo(raw, peer, mixed, target, challenge)
        return
    raise ValueError("UDP application reconnect unavailable")


def echo_loop(listener, stopped, failed):
    listener.settimeout(0.1)
    while not stopped.is_set():
        try:
            raw, peer = listener.recvfrom(4097)
        except socket.timeout:
            continue
        except OSError:
            if not stopped.is_set():
                failed.set()
            return
        if peer[0] != "127.0.0.1" or len(raw) > 4096:
            continue
        try:
            listener.sendto(raw, peer)
        except OSError:
            if not stopped.is_set():
                failed.set()
            return


def association_live(connection):
    # The server should send no further bytes on a negotiated association.
    if select.select([connection], [], [], 0)[0]:
        raise ValueError("Synthetic UDP association unexpectedly ended")


def snapshot(controller, secret, endpoints):
    status, raw = loopback.control(controller, secret, "GET", "/connections")
    report = json.loads(raw, object_pairs_hook=loopback.exact_pairs)
    if status != 200 or not isinstance(report, dict) or not isinstance(report.get("connections"), list):
        raise ValueError("Synthetic UDP snapshot unavailable")
    rows = report["connections"]
    if len(rows) > 2:
        raise ValueError("Unexpected synthetic UDP trackers")
    by_source, ids, tokens = {}, set(), set()
    for row in rows:
        if not isinstance(row, dict) or not isinstance(row.get("metadata"), dict):
            raise ValueError("Malformed synthetic UDP tracker")
        metadata = row["metadata"]
        source = metadata.get("sourcePort")
        identity, token = row.get("id"), row.get("omavlessCloseToken")
        # IDs/tokens stay private; the grammar cannot select a controller path.
        if (not isinstance(identity, str) or not re.fullmatch(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}", identity)
                or not isinstance(token, str) or not re.fullmatch(r"[1-9][0-9]{0,19}", token)
                or int(token) > 2**64 - 1 or identity in ids or token in tokens
                or source not in endpoints or source in by_source
                or metadata.get("network") != "udp" or metadata.get("type") != "Socks5"
                or metadata.get("sourceIP") != "127.0.0.1" or metadata.get("destinationIP") != "127.0.0.1"
                or str(metadata.get("destinationPort")) != str(endpoints[source])):
            raise ValueError("Unexpected synthetic UDP identity")
        ids.add(identity)
        tokens.add(token)
        by_source[source] = (identity, token)
    return by_source


def close_once(controller, secret, identity, token, expected):
    # Only the exact typed empty receipt is evidence; exceptions/other replies
    # abort, without resending or consulting later absence to infer success.
    receipt = loopback.control(controller, secret, "POST", "/connections/" + identity + "/close-conditional", token)
    if receipt != (expected, b""):
        raise ValueError("Conditional UDP receipt ambiguous; not retried")


def exercise(core, scratch_parent):
    if not core.is_absolute() or core.is_symlink() or not core.is_file():
        raise ValueError("Existing absolute regular core required")
    if not scratch_parent.is_absolute() or scratch_parent.is_symlink() or not scratch_parent.is_dir():
        raise ValueError("Private scratch required")
    st = scratch_parent.stat()
    if st.st_uid != os.getuid() or st.st_mode & 0o077:
        raise ValueError("Scratch ownership refused")
    secret = secrets.token_hex(32)
    mixed, controller = loopback.port(), loopback.port()
    with tempfile.TemporaryDirectory(prefix="udp-close-", dir=scratch_parent) as name:
        root = Path(name)
        config = root / "config.yaml"
        config.write_text(f"mixed-port: {mixed}\nexternal-controller: 127.0.0.1:{controller}\nsecret: {secret}\n"
                          "allow-lan: false\nbind-address: 127.0.0.1\nmode: direct\nlog-level: silent\nipv6: false\n"
                          "find-process-mode: off\ntun:\n  enable: false\ndns:\n  enable: false\nrules:\n  - MATCH,DIRECT\n")
        config.chmod(0o600)
        process = subprocess.Popen([str(core), "-d", str(root), "-f", str(config)],
                                   stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        stopped, failed, threads = threading.Event(), threading.Event(), []
        try:
            for _ in range(100):
                if process.poll() is not None:
                    raise ValueError("Isolated core exited")
                try:
                    if loopback.conditional_ready(controller, secret):
                        break
                except OSError:
                    pass
                time.sleep(0.05)
            else:
                raise ValueError("Isolated controller unavailable")
            with contextlib.ExitStack() as owned:
                targets, clients, associations = [], [], []
                for _ in range(2):
                    target = owned.enter_context(socket.socket(socket.AF_INET, socket.SOCK_DGRAM))
                    target.bind(("127.0.0.1", 0))
                    targets.append(target.getsockname()[1])
                    thread = threading.Thread(target=echo_loop, args=(target, stopped, failed), daemon=True)
                    threads.append(thread)
                    thread.start()
                    client = owned.enter_context(socket.socket(socket.AF_INET, socket.SOCK_DGRAM))
                    client.bind(("127.0.0.1", 0))
                    client.settimeout(0.5)
                    clients.append(client)
                    associations.append(owned.enter_context(associate(mixed, client)))
                    prove_echo(client, mixed, targets[-1])
                sources = [str(client.getsockname()[1]) for client in clients]
                endpoints = dict(zip(sources, targets))
                before = snapshot(controller, secret, endpoints)
                if set(before) != set(sources):
                    raise ValueError("Two synthetic UDP trackers required")
                first, other = (before[source] for source in sources)
                close_once(controller, secret, first[0], other[1], 409)
                for client, target in zip(clients, targets):
                    prove_echo(client, mixed, target)
                if snapshot(controller, secret, endpoints) != before:
                    raise ValueError("Wrong-token refusal changed UDP trackers")
                close_once(controller, secret, *first, 204)
                for connection in associations:
                    association_live(connection)
                prove_echo(clients[1], mixed, targets[1])
                for connection in associations:
                    association_live(connection)
                # This corroborates the prior receipt; absence is never a receipt.
                if snapshot(controller, secret, endpoints) != {sources[1]: other}:
                    raise ValueError("Unselected UDP tracker affected")
                # Keep the SAME TCP association and UDP source socket alive.
                prove_echo(clients[0], mixed, targets[0], reconnect=True)
                after = snapshot(controller, secret, endpoints)
                if (set(after) != set(sources) or after[sources[1]] != other
                        or after[sources[0]][1] in {first[1], other[1]}):
                    raise ValueError("UDP replacement incarnation unavailable")
                prove_echo(clients[1], mixed, targets[1])
                stopped.set()
                for thread in threads:
                    thread.join(timeout=2)
        finally:
            stopped.set()
            for thread in threads:
                thread.join(timeout=2)
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
            if any(thread.is_alive() for thread in threads):
                raise ValueError("Synthetic UDP echo cleanup incomplete")
            if failed.is_set():
                raise ValueError("Synthetic UDP echo worker failed")
    return hashlib.sha256(core.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--core", type=Path, required=True)
    parser.add_argument("--scratch-parent", type=Path, required=True)
    args = parser.parse_args()
    digest = exercise(args.core, args.scratch_parent)
    print("conditional_udp_loopback: passed; exact retained close receipt; other flow live; application reconnects")
    print("core_sha256=" + digest)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError):
        raise SystemExit("conditional_udp_loopback: refused; no effect retry; no TUN/install") from None
