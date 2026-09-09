#!/usr/bin/python3
# SPDX-License-Identifier: MIT
"""Opt-in staged user-unit DIRECTORY gate, not installation or VPN acceptance.

Stages an explicit prebuilt binary, links one uniquely named runtime-only copy
of the packaged unit, and starts/restarts its absent-ownership read-only daemon.
Never enables a unit, writes /usr, seeds native ownership, connects a profile,
starts Mihomo, or touches installed OmaVLESS units/private stores. Systemd owns
directory creation. All test paths are unique children of the actual user
manager's XDG bases. Command output is retained privately on failure.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import pwd
import re
import shlex
import shutil
import stat
import subprocess
import tempfile
import time
import uuid

from native_service_acceptance import Failure, bounded, emit, private_write, processes, require


def canonical(path):
    value = Path(path)
    require(value.is_absolute() and value.resolve() == value
            and re.fullmatch(r"[A-Za-z0-9_./-]+", str(value)), "unsafe_or_unsupported_path")
    return value


def manager_bases(raw, uid, account_home):
    selected = {}
    for line in raw.decode("utf-8").splitlines():
        # Only parse the needed keys; do not copy or report the manager's other
        # potentially sensitive environment variables.
        key = line.split("=", 1)[0]
        if key in {"HOME", "XDG_CONFIG_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME", "XDG_RUNTIME_DIR"}:
            words = shlex.split(line)
            require(len(words) == 1 and "=" in words[0], "manager_environment_shape")
            selected[key] = words[0].split("=", 1)[1]
    home = canonical(account_home)
    require(not selected.get("HOME") or canonical(selected["HOME"]) == home, "manager_home_ambiguous")
    return {
        "config": canonical(selected.get("XDG_CONFIG_HOME") or home / ".config"),
        "state": canonical(selected.get("XDG_STATE_HOME") or home / ".local/state"),
        "cache": canonical(selected.get("XDG_CACHE_HOME") or home / ".cache"),
        "runtime": canonical(selected.get("XDG_RUNTIME_DIR") or f"/run/user/{uid}"),
    }


def render_unit(packaged, binary, namespace, bases):
    require(re.fullmatch(r"omavless-stage-[a-f0-9]{12}", namespace), "namespace_invalid")
    binary = canonical(binary)
    overrides = {
        "ExecStart": str(binary) + " daemon", "ConditionFileIsExecutable": str(binary),
        "ConfigurationDirectory": namespace + "/.config/omavless",
        "StateDirectory": namespace + "/omavless", "CacheDirectory": namespace + "/omavless",
        "RuntimeDirectory": namespace + "/omavless",
    }
    seen, output = set(), []
    for line in packaged.splitlines():
        key = line.split("=", 1)[0]
        if key in overrides:
            require(key not in seen, "packaged_directive_duplicate")
            seen.add(key)
            line = key + "=" + overrides[key]
        output.append(line)
    require(seen == set(overrides), "packaged_directive_missing")
    # Environment overrides never determine where systemd provisions paths.
    # They only point this isolated daemon at the same namespaced locations.
    environment = {
        "OMAVLESS_HOME": str(bases["config"] / namespace),
        "XDG_STATE_HOME": str(bases["state"] / namespace),
        "XDG_CACHE_HOME": str(bases["cache"] / namespace),
        "XDG_RUNTIME_DIR": str(bases["runtime"] / namespace),
        "OMAVLESS_MIHOMO": "/usr/bin/mihomo",
    }
    for value in environment.values():
        canonical(value)
    output += ["", "[Service]"] + [f"Environment={key}={value}" for key, value in environment.items()]
    return "\n".join(output) + "\n", environment


def private_directory(path, uid):
    require(path.resolve() == path, "directory_symlink")
    metadata = path.lstat()
    require(stat.S_ISDIR(metadata.st_mode) and metadata.st_uid == uid
            and stat.S_IMODE(metadata.st_mode) == 0o700, "directory_not_private")


def run(options):
    uid = os.getuid()
    binary = canonical(options.binary)
    require(stat.S_ISREG(binary.lstat().st_mode) and os.access(binary, os.X_OK), "binary_not_executable")
    repository = Path(__file__).resolve().parents[1]
    artifact, count = None, 0

    def command(args, env=None, checked=True):
        nonlocal count
        result = subprocess.run(args, env=env, stdin=subprocess.DEVNULL, capture_output=True, timeout=120)
        require(len(result.stdout) + len(result.stderr) <= 1048576, "command_output_oversized")
        if artifact is not None:
            count += 1
            private_write(artifact / f"capture-{count}.stdout", result.stdout)
            private_write(artifact / f"capture-{count}.stderr", result.stderr)
        require(not checked or result.returncode == 0, "command_rejected")
        return result

    def no_tun():
        links = json.loads(command(["ip", "-d", "-j", "link", "show"]).stdout)
        return not any(link.get("linkinfo", {}).get("info_kind") == "tun" for link in links)

    require(not any(name in (b"mihomo", b"omavless") for name in processes().values())
            and no_tun(), "baseline_runtime_present")
    bases = manager_bases(command(["systemctl", "--user", "show-environment"]).stdout,
                          uid, pwd.getpwuid(uid).pw_dir)
    private_directory(bases["runtime"], uid)
    namespace = "omavless-stage-" + uuid.uuid4().hex[:12]
    unit = namespace + ".service"
    roots = {base / namespace for base in bases.values()}
    require(all(not path.exists() and not path.is_symlink() for path in roots), "namespace_exists")
    leaves = {key: base / namespace / (".config/omavless" if key == "config" else "omavless")
              for key, base in bases.items()}
    artifact = Path(tempfile.mkdtemp(prefix="omavless-staged-payload-", dir=bases["runtime"]))
    payload = artifact / "payload"
    payload.mkdir(mode=0o700)
    runtime_link = bases["runtime"] / "systemd/user" / unit
    linked, started, passed, cgroup = False, False, False, None
    try:
        command(["bash", str(repository / "packaging/arch/stage-payload.sh"), str(payload), str(binary)])
        staged_binary = payload / "usr/bin/omavless"
        digest = hashlib.sha256(binary.read_bytes()).hexdigest()
        require(hashlib.sha256(staged_binary.read_bytes()).hexdigest() == digest, "payload_binary_identity")
        packaged = bounded(payload / "usr/lib/systemd/user/omavless-runtime.service").decode("utf-8")
        text, overrides = render_unit(packaged, staged_binary, namespace, bases)
        unit_file = artifact / unit
        private_write(unit_file, text)
        env = dict(os.environ, **overrides)
        require(not runtime_link.exists() and not runtime_link.is_symlink(), "runtime_unit_exists")
        linked = True
        command(["systemctl", "--user", "link", "--runtime", str(unit_file)])
        require(runtime_link.is_symlink() and runtime_link.resolve() == unit_file, "runtime_unit_link_identity")
        command(["systemctl", "--user", "daemon-reload"])

        def verify():
            deadline = time.monotonic() + 30
            control = leaves["runtime"] / "control.sock"
            while not control.exists() and time.monotonic() < deadline:
                time.sleep(0.05)
            pid = int(command(["systemctl", "--user", "show", unit, "-p", "MainPID", "--value"]).stdout)
            require(pid > 1, "service_pid_invalid")
            exported = dict(item.split(b"=", 1) for item in bounded(
                Path("/proc") / str(pid) / "environ", 1048576).split(b"\0") if b"=" in item)
            for key, variable in (("config", b"CONFIGURATION_DIRECTORY"), ("state", b"STATE_DIRECTORY"),
                                  ("cache", b"CACHE_DIRECTORY"), ("runtime", b"RUNTIME_DIRECTORY")):
                require(exported.get(variable) == os.fsencode(leaves[key]), "provisioned_directory_mismatch")
            for path in leaves.values():
                private_directory(path, uid)
            metadata = control.lstat()
            require(stat.S_ISSOCK(metadata.st_mode) and metadata.st_uid == uid
                    and stat.S_IMODE(metadata.st_mode) == 0o600, "control_socket_private")
            hello = json.loads(command([str(staged_binary), "hello"], env).stdout)
            status = json.loads(command([str(staged_binary), "status"], env).stdout)
            capabilities = json.loads(command([str(staged_binary), "capabilities"], env).stdout)
            require(hello.get("ok") is True and status.get("ok") is True
                    and status["result"]["actual"] == "disconnected"
                    and hello["result"].get("runtimeOwnership") is False
                    and capabilities["result"].get("mutations") is False, "read_only_startup")
            require(not (leaves["state"] / "ownership.json").exists()
                    and not (leaves["config"] / "profiles.json").exists(), "private_state_created")
            require(no_tun() and not any(name == b"mihomo" for name in processes().values()), "unexpected_core_or_tun")
            require(set(map(int, bounded(cgroup / "cgroup.procs").split())) == {pid}, "service_members")

        started = True
        command(["systemctl", "--user", "start", unit])
        group = command(["systemctl", "--user", "show", unit, "-p", "ControlGroup", "--value"]).stdout.decode().strip()
        cgroup = (Path("/sys/fs/cgroup") / group.lstrip("/")).resolve()
        require(cgroup.is_relative_to("/sys/fs/cgroup") and cgroup.name == unit, "service_cgroup_identity")
        verify()
        for key in ("config", "state", "cache", "runtime"):
            private_write(leaves[key] / "acceptance-sentinel", key)
        command(["systemctl", "--user", "stop", unit])
        require(not leaves["runtime"].exists(), "runtime_directory_not_removed")
        for key in ("config", "state", "cache"):
            require(bounded(leaves[key] / "acceptance-sentinel") == key.encode(), "persistent_directory_lost")
        command(["systemctl", "--user", "start", unit])
        verify()
        require(not (leaves["runtime"] / "acceptance-sentinel").exists(), "runtime_not_recreated")
        command(["systemctl", "--user", "restart", unit])
        verify()
        require(hashlib.sha256(binary.read_bytes()).hexdigest() == digest
                and hashlib.sha256(staged_binary.read_bytes()).hexdigest() == digest, "binary_changed")
        passed = True
        emit(staged_binary_sha256=digest, directory_modes=True, absent_ownership_startup=True,
             stop_start=True, restart=True, persistent_directories=True, runtime_recreated=True,
             core_count=0, tun_count=0, installed_units_changed=False)
    finally:
        clean = not started
        try:
            if started:
                command(["systemctl", "--user", "stop", unit], checked=False)
                active = command(["systemctl", "--user", "is-active", "--quiet", unit], checked=False)
                clean = active.returncode in (3, 4) and no_tun() and not any(
                    name in (b"mihomo", b"omavless") for name in processes().values())
                if cgroup is not None and cgroup.exists():
                    clean = clean and not bounded(cgroup / "cgroup.procs").split()
            if clean and linked:
                require(runtime_link.is_symlink() and runtime_link.resolve() == artifact / unit, "cleanup_link_changed")
                runtime_link.unlink()
                command(["systemctl", "--user", "daemon-reload"])
            if clean and passed:
                for path in roots:
                    if path.exists():
                        require(path.resolve() == path and path.stat().st_uid == uid, "cleanup_directory_changed")
                        shutil.rmtree(path)
                shutil.rmtree(artifact)
        except Exception:
            clean = False
        emit(cleanup=clean, diagnostic_files_retained=not (clean and passed))
        require(clean, "manual_cleanup_required")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true")
    parser.add_argument("--binary", required=True)
    options = parser.parse_args()
    require(options.run, "explicit_run_required")
    run(options)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        emit(result="FAIL", classification=str(error) if isinstance(error, Failure) else "staged_unit_probe_failed")
        raise SystemExit(1)
