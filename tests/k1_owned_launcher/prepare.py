"""Explicit external developer export; never installs or executes the prototype.

Creates a fresh directory under the supplied private build root. No product
Cargo source/lock is changed. Existing exports are never overwritten.
"""
import importlib.util
import hashlib
import json
from pathlib import Path
import subprocess
import sys

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
spec=importlib.util.spec_from_file_location('adapt',HERE/'adapt.py')
adapter=importlib.util.module_from_spec(spec);spec.loader.exec_module(adapter)

def git(*args):
    return subprocess.run(['git','-C',str(ROOT),*args],check=True,capture_output=True,timeout=30).stdout

def export(parent, export_name='netguard'):
    if export_name not in ('netguard', 'netguard-leaf', 'netguard-prelaunch', 'netguard-prelaunch-v2', 'netguard-protocol', 'netguard-protocol-v2', 'netguard-protocol-final', 'netguard-protocol-reviewed', 'netguard-owned-spawn', 'netguard-checked-handoff', 'netguard-owner-boundaries', 'netguard-owner-boundaries-final', 'netguard-static-child-v1', 'netguard-types-v1', 'netguard-types-v2', 'netguard-parent-v1', 'netguard-parent-v3', 'netguard-types-v3', 'netguard-inventory-v1', 'netguard-inventory-types-v1'):raise ValueError('fixed_export_name')
    parent=Path(parent)
    if not parent.is_absolute() or parent.is_symlink() or not parent.is_dir():raise ValueError('private_build_root')
    if parent.stat().st_mode&0o777!=0o700:raise ValueError('private_build_mode')
    provenance=json.loads((HERE/'spawn-upstream.json').read_text())
    if hashlib.sha256((HERE/provenance['patch']).read_bytes()).hexdigest()!=provenance['patch_sha256']:raise ValueError('spawn_patch_pin')
    if hashlib.sha256((parent/'nix-spawn'/provenance['source']).read_bytes()).hexdigest()!=provenance['patched_sha256']:raise ValueError('spawn_source_pin')
    actual=subprocess.run(['git','-C',str(parent/'nix-spawn'),'rev-parse','HEAD'],check=True,capture_output=True,timeout=30).stdout.decode().strip()
    if actual!=provenance['commit']:raise ValueError('spawn_upstream_pin')
    prefix='crates/omavless-netguard/src/'
    names=git('ls-tree','-r','--name-only',adapter.BASE,'--',prefix).decode().splitlines()
    if not 1<len(names)<=512 or any(not n.startswith(prefix) or '..' in Path(n).parts for n in names):raise ValueError('source_catalog')
    sources={n[len(prefix):]:git('show',adapter.BASE+':'+n) for n in names}
    sources.update(adapter.adapt({name:sources[name] for name in adapter.PINS}))
    for name in ('owned_creator.rs','owned_launcher.rs','child_protocol.rs','child_executable.rs','fixed_child.rs','retained_return.rs','static_elf.rs','owned_child.rs','handoff.rs','spawn_sequence.rs','completion.rs','inventory_sequence.rs'):sources[name]=(HERE/name).read_bytes()
    sources['no_policy_gate.rs']=(HERE/'no_policy_gate.rs').read_bytes()
    # Same actual module tree, not a public acquisition constructor or stubs.
    sources['parent_main.rs']=sources['lib.rs']+b'\ninclude!("no_policy_gate.rs");\n'
    controls={}
    if export_name in ('netguard-types-v1', 'netguard-types-v2', 'netguard-types-v3', 'netguard-inventory-types-v1'):
        types_spec=importlib.util.spec_from_file_location('owned_types',HERE/'type_controls.py')
        types=importlib.util.module_from_spec(types_spec);types_spec.loader.exec_module(types)
        controls=types.cases()
        sources.update(types.render(sources['lib.rs']))
    target=parent/export_name;target.mkdir(mode=0o700)
    (target/'src').mkdir(mode=0o700)
    for name,raw in sources.items():
        destination=target/'src'/name;destination.parent.mkdir(parents=True,exist_ok=True,mode=0o700)
        with destination.open('xb') as output:output.write(raw)
    manifest='''[package]
name = "omavless-netguard"
version = "0.0.0"
edition = "2024"
rust-version = "1.98"
publish = false
[workspace]
[[bin]]
name = "k1-fixed-child"
path = "src/fixed_child.rs"
[[bin]]
name = "k1-owned-no-policy"
path = "src/parent_main.rs"
required-features = ["owned-launch-no-policy"]
[features]
owned-launch-no-policy = []
[dependencies]
nix = { path = "../nix-spawn", default-features = false, features = ["fs", "sched", "socket", "uio", "user", "ioctl", "process"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sha2 = "=0.10.9"
rustix = { version = "=1.1.5", default-features = false, features = ["std", "fs", "stdio"] }
[patch.crates-io]
libc = { path = "../libc" }
[lints.rust]
unsafe_code = "forbid"
'''
    for name in controls:
        manifest+=f'\n[[bin]]\nname = "type-{name}"\npath = "src/type_{name}.rs"\n'
    with (target/'Cargo.toml').open('x') as output:output.write(manifest)
    with (target/'Cargo.lock').open('xb') as output:output.write((HERE/'Cargo.lock').read_bytes())
    return target

if __name__=='__main__':
    if len(sys.argv)!=2:raise SystemExit(2)
    export(sys.argv[1])
