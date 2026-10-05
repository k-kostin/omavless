"""Explicit external developer export; never installs or executes the prototype.

Creates a fresh directory under the supplied private build root. No product
Cargo source/lock is changed. Existing exports are never overwritten.
"""
import importlib.util
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
    if export_name not in ('netguard', 'netguard-leaf', 'netguard-prelaunch', 'netguard-prelaunch-v2', 'netguard-protocol', 'netguard-protocol-v2', 'netguard-protocol-final', 'netguard-protocol-reviewed'):raise ValueError('fixed_export_name')
    parent=Path(parent)
    if not parent.is_absolute() or parent.is_symlink() or not parent.is_dir():raise ValueError('private_build_root')
    if parent.stat().st_mode&0o777!=0o700:raise ValueError('private_build_mode')
    prefix='crates/omavless-netguard/src/'
    names=git('ls-tree','-r','--name-only',adapter.BASE,'--',prefix).decode().splitlines()
    if not 1<len(names)<=512 or any(not n.startswith(prefix) or '..' in Path(n).parts for n in names):raise ValueError('source_catalog')
    sources={n[len(prefix):]:git('show',adapter.BASE+':'+n) for n in names}
    sources.update(adapter.adapt({name:sources[name] for name in adapter.PINS}))
    for name in ('owned_creator.rs','owned_launcher.rs','child_protocol.rs','child_executable.rs','fixed_child.rs','retained_return.rs','static_elf.rs'):sources[name]=(HERE/name).read_bytes()
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
[dependencies]
nix = { path = "../nix", default-features = false, features = ["fs", "sched", "socket", "uio", "user", "ioctl", "process"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sha2 = "=0.10.9"
rustix = { version = "=1.1.5", default-features = false, features = ["std", "fs"] }
[patch.crates-io]
libc = { path = "../libc" }
[lints.rust]
unsafe_code = "forbid"
'''
    with (target/'Cargo.toml').open('x') as output:output.write(manifest)
    with (target/'Cargo.lock').open('xb') as output:output.write((HERE/'Cargo.lock').read_bytes())
    return target

if __name__=='__main__':
    if len(sys.argv)!=2:raise SystemExit(2)
    export(sys.argv[1])
