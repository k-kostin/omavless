#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Candidate-core real socket gate: synthetic loopback only, never TUN/install."""
import argparse
import contextlib
import hashlib
import http.client
import json
import os
from pathlib import Path
import secrets
import socket
import subprocess
import tempfile
import threading
import time


def port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


def exact_pairs(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("Duplicate controller key")
        result[key] = value
    return result


def control(number, secret, method, path, token=None):
    with contextlib.closing(http.client.HTTPConnection("127.0.0.1", number, timeout=2)) as connection:
        headers = {"Authorization": "Bearer " + secret}
        if token is not None:
            headers["If-Match"] = '"' + token + '"'
        connection.request(method, path, headers=headers)
        response = connection.getresponse()
        raw = response.read(1024 * 1024 + 1)
        if len(raw) > 1024 * 1024:
            raise ValueError("Oversized controller response")
        return response.status, raw


def echo_loop(listener, stopped):
    peers = []
    threads = []

    def echo(peer):
        peer.settimeout(0.1)
        try:
            while not stopped.is_set():
                try:
                    data = peer.recv(4096)
                except socket.timeout:
                    continue
                if not data:
                    return
                peer.sendall(data)
        except OSError:
            pass
        finally:
            peer.close()

    listener.settimeout(0.1)
    try:
        while not stopped.is_set():
            try:
                peer, _ = listener.accept()
            except socket.timeout:
                continue
            peers.append(peer)
            thread = threading.Thread(target=echo, args=(peer,), daemon=True)
            threads.append(thread)
            thread.start()
    finally:
        for peer in peers:
            peer.close()
        for thread in threads:
            thread.join(timeout=1)


def tunnel(mixed, target):
    connection = socket.create_connection(("127.0.0.1", mixed), timeout=2)
    try:
        connection.sendall(f"CONNECT 127.0.0.1:{target} HTTP/1.1\r\nHost: 127.0.0.1:{target}\r\n\r\n".encode())
        header = b""
        while not header.endswith(b"\r\n\r\n") and len(header) < 4096:
            data = connection.recv(1)
            if not data:
                raise ValueError("CONNECT ended early")
            header += data
        if not header.startswith(b"HTTP/1.1 200 "):
            raise ValueError("CONNECT refused")
        return connection
    except Exception:
        connection.close()
        raise


def prove_echo(connection):
    data = b"synthetic-conditional-close-check"
    connection.sendall(data)
    output = b""
    while len(output) < len(data):
        chunk = connection.recv(len(data) - len(output))
        if not chunk:
            raise ValueError("Tunnel closed unexpectedly")
        output += chunk
    if output != data:
        raise ValueError("Unexpected tunnel data")


def exercise(core, scratch_parent):
    if not core.is_absolute() or core.is_symlink() or not core.is_file():
        raise ValueError("Existing absolute regular core required")
    if not scratch_parent.is_absolute() or scratch_parent.is_symlink() or not scratch_parent.is_dir():
        raise ValueError("Private scratch required")
    metadata = scratch_parent.stat()
    if metadata.st_uid != os.getuid() or metadata.st_mode & 0o077:
        raise ValueError("Scratch ownership refused")
    secret = secrets.token_hex(32)
    mixed, controller = port(), port()
    with tempfile.TemporaryDirectory(prefix="conditional-loopback-", dir=scratch_parent) as scratch:
        root = Path(scratch)
        root.chmod(0o700)
        config = root / "config.yaml"
        config.write_text(f"mixed-port: {mixed}\nexternal-controller: 127.0.0.1:{controller}\n"
                          f"secret: {secret}\nallow-lan: false\nbind-address: 127.0.0.1\n"
                          "mode: direct\nlog-level: silent\nipv6: false\n"
                          "tun:\n  enable: false\ndns:\n  enable: false\nrules:\n  - MATCH,DIRECT\n")
        config.chmod(0o600)
        process = subprocess.Popen([str(core), "-d", str(root), "-f", str(config)],
                                   stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                                   stderr=subprocess.DEVNULL)
        stopped = threading.Event()
        thread = None
        sockets = []
        try:
            for _ in range(100):
                if process.poll() is not None:
                    raise ValueError("Isolated core exited")
                try:
                    if control(controller, secret, "GET", "/connections")[0] == 200:
                        break
                except OSError:
                    pass
                time.sleep(0.05)
            else:
                raise ValueError("Isolated controller unavailable")
            with socket.socket() as listener:
                listener.bind(("127.0.0.1", 0))
                listener.listen(4)
                target = listener.getsockname()[1]
                thread = threading.Thread(target=echo_loop, args=(listener, stopped), daemon=True)
                thread.start()
                sockets = [tunnel(mixed, target), tunnel(mixed, target)]
                for connection in sockets:
                    prove_echo(connection)
                status, raw = control(controller, secret, "GET", "/connections")
                snapshot = json.loads(raw, object_pairs_hook=exact_pairs)
                rows = snapshot.get("connections")
                if status != 200 or not isinstance(rows, list) or len(rows) != 2:
                    raise ValueError("Exact synthetic two-row snapshot unavailable")
                by_source = {int(row["metadata"]["sourcePort"]): row for row in rows}
                first = by_source[sockets[0].getsockname()[1]]
                second = by_source[sockets[1].getsockname()[1]]
                identity, token = first["id"], first["omavlessCloseToken"]
                if not isinstance(token, str) or not token.isdecimal() or token == "0" or token == second["omavlessCloseToken"]:
                    raise ValueError("Nonunique incarnation")
                path = "/connections/" + identity + "/close-conditional"
                if control(controller, secret, "POST", path, second["omavlessCloseToken"])[0] != 409:
                    raise ValueError("Wrong incarnation accepted")
                for connection in sockets:
                    prove_echo(connection)
                if control(controller, secret, "POST", path, token)[0] != 204:
                    raise ValueError("Conditional close failed")
                try:
                    if sockets[0].recv(1) != b"":
                        raise ValueError("Closed target still open")
                except ConnectionResetError:
                    pass
                prove_echo(sockets[1])
                if control(controller, secret, "POST", path, token)[0] != 404:
                    raise ValueError("Replay outcome not explicit")
                status, raw = control(controller, secret, "GET", "/connections")
                remaining = json.loads(raw, object_pairs_hook=exact_pairs)["connections"]
                if status != 200 or len(remaining) != 1 or remaining[0]["id"] != second["id"]:
                    raise ValueError("Unselected tracker affected")
                stopped.set()
                thread.join(timeout=2)
        finally:
            stopped.set()
            for connection in sockets:
                connection.close()
            if thread is not None:
                thread.join(timeout=2)
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
    return hashlib.sha256(core.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--core", type=Path, required=True)
    parser.add_argument("--scratch-parent", type=Path, required=True)
    args = parser.parse_args()
    digest = exercise(args.core, args.scratch_parent)
    print("conditional_core_loopback: passed; two real tunnels; exact target; no TUN/install")
    print("core_sha256=" + digest)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError):
        raise SystemExit("conditional_core_loopback: refused; no TUN/install") from None
