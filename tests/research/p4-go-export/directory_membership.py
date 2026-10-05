"""Developer-only directory-membership regression; no CLI or Go execution.

root_members is the exact function from the separately selected fixed exporter.
The caller must supply its retained original directory and checked Store.call
boundary. This module neither acquires a directory nor admits a tool tree.
Ordinary tests use synthetic operation adapters, never real directory FDs.
"""
import os

HELD=[]

class Refused(Exception):pass

def need(value):
    if not value:raise Refused()

def root_members(store,root,wanted):
    # Owned-directory reset also refreshes per-open enumeration state on Btrfs.
    # It is an explicit checked operation, not a passive cursor observation.
    offset=store.call(os.lseek,root,0,os.SEEK_SET)
    need(type(offset)is int and offset==0)
    iterator=store.call(os.scandir,root);HELD.append(iterator)
    seen=set();sentinel=object()
    while True:
        entry=store.call(next,iterator,sentinel)
        if entry is sentinel:break
        need(entry.name in wanted and entry.name not in seen and len(seen)<len(wanted))
        seen.add(entry.name)
    need(seen==wanted);store.call(iterator.close)
