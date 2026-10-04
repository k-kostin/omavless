#!/usr/bin/env python3
"""Strict finite public receipt validation; inert import, no candidate admission."""
import hashlib
import json
import os
from pathlib import Path
import posixpath
import re
import stat
import sys

STAGE = Path("/home/kdk_vm/.cache/t3-brotli-encoder-provenance-review-1")
MANIFEST_SHA = "d6ca9cd599adb573f4419f49147677f681db08fcc54a06cb0f34ed950ce19c36"
READELF_SHA = "a72f12f3dd8c560554a3ba818bcecbda9178db5befbcc64d91bbb6a241fc21bc"
SEARCH = ["/usr/lib", "/usr/lib/systemd"]
CANDIDATE = "/usr/lib/libbrotlienc.so.1.2.0"


def need(value):
    if not value:
        raise ValueError("static_receipt_refused")


def keys(value, expected):
    need(type(value) is dict and set(value) == set(expected.split()))


def number(value, minimum=0, maximum=2**64 - 1):
    need(type(value) is int and minimum <= value <= maximum)


def text(value, pattern):
    need(type(value) is str and re.fullmatch(pattern, value) is not None)


def digest(value):
    text(value, r"[0-9a-f]{64}")


def basename(value):
    text(value, r"[A-Za-z0-9_+.-]{1,160}")
    need(value not in (".", ".."))


def public(value, canonical=False):
    need(type(value) is str)
    roots = ("/usr/bin", "/usr/lib", "/usr/lib/systemd") if canonical else ("/usr/bin", "/usr/lib", "/usr/lib/systemd", "/usr/lib64", "/lib64")
    if not canonical and value in ("/lib64", "/usr/lib64"):
        return
    need(any(value.startswith(root + "/") and re.fullmatch(r"[A-Za-z0-9_+.-]{1,160}", value[len(root) + 1:]) for root in roots))
    need(posixpath.normpath(value) == value)


def package(value):
    keys(value, "name version file_list_sha256 description_sha256")
    for field in ("name", "version"):
        text(value[field], r"[A-Za-z0-9_+.@:-]{1,160}")
    digest(value["file_list_sha256"])
    digest(value["description_sha256"])


def links(value, logical, resolved):
    need(type(value) is list and len(value) <= 8)
    current = logical
    for link in value:
        keys(link, "path target identity")
        public(link["path"])
        text(link["target"], r"[A-Za-z0-9_./+-]{1,256}")
        identity = link["identity"]
        need(type(identity) is list and len(identity) == 9)
        for item in identity:
            number(item)
        dev, ino, size, uid, gid, mode, nlink, mtime, ctime = identity
        need(ino > 0 and nlink > 0 and uid == gid == 0 and stat.S_ISLNK(mode)
             and mode <= 0o177777 and size == len(link["target"].encode()))
        need(current == link["path"] or current.startswith(link["path"] + "/"))
        tail = current[len(link["path"]):].lstrip("/")
        current = posixpath.normpath(posixpath.join(posixpath.dirname(link["path"]), link["target"]))
        if tail:
            current = posixpath.join(current, tail)
        public(current)
    need(current == resolved)


PACKAGE_PINS = {'desc':'974d3bdc717e12ac7f08e3f15afe3a67168d07aa854237269cf6528a05e1e0cc',
                'files':'ff22c1aea2a86cca33a7b4028870948e6e76b292722bdcd49c9e0dac2465c3d4'}


def metadata(value, maximum=32*1024*1024):
    need(type(value) is list and len(value) == 9)
    for item in value:
        number(item)
    dev, ino, mode, uid, gid, nlink, size, _, _ = value
    need(dev > 0 and ino > 0 and stat.S_ISREG(mode) and mode <= 0o177777
         and not mode & 0o022 and uid == gid == 0 and nlink == 1 and 0 < size <= maximum)


def validate(value, manifest):
    keys(value, "schema outcome candidate manifest_sha256 readelf_sha256 readelf_identity package records aliases candidate_elf_executed allowlist_adoption loaded_elf_identity_proven compatibility_acceptance")
    need(value['schema'] == 'fixed-brotli-encoder-provenance-v1'
         and value['outcome'] == 'OBSERVED_STATIC_ONLY' and value['candidate'] == CANDIDATE
         and value['manifest_sha256'] == MANIFEST_SHA and value['readelf_sha256'] == READELF_SHA)
    for key in ('candidate_elf_executed','allowlist_adoption','loaded_elf_identity_proven','compatibility_acceptance'):
        need(value[key] is False)
    metadata(value['readelf_identity'])
    need(value['readelf_identity'][:7] == [31,29149,0o100755,0,0,1,810072])
    pkg = value['package']
    keys(pkg, 'name version hashes identities candidate_listed global_package_owner_uniqueness_proven')
    need(pkg['name'] == 'brotli' and pkg['version'] == '1.2.0-1'
         and pkg['hashes'] == PACKAGE_PINS and pkg['candidate_listed'] is True
         and pkg['global_package_owner_uniqueness_proven'] is False)
    keys(pkg['identities'], 'desc files')
    for identity in pkg['identities'].values():
        metadata(identity, 4*1024*1024)
    known = manifest['source_provenance']
    need(len(known) == 17 and CANDIDATE not in known)
    rows, aliases = value['records'], value['aliases']
    need(type(rows) is list and 1 <= len(rows) <= 18
         and type(aliases) is list and 1 <= len(aliases) <= 128)
    indexed, alias_index = {}, {}
    for row in rows:
        keys(row, 'path identity sha256 header dynamic dependencies matches_reviewed_source')
        path = row['path']
        public(path, True)
        need(path not in indexed and (path == CANDIDATE or path in known))
        metadata(row['identity'])
        digest(row['sha256'])
        need(type(row['matches_reviewed_source']) is bool and row['matches_reviewed_source'] == (path in known))
        if path in known:
            old = known[path]
            need(row['sha256'] == old['sha256'] and row['identity'][:7] ==
                 [old[k] for k in ('device','inode','mode','uid','gid','nlink','size')])
        header = row['header']
        keys(header, 'elf_class data_encoding version object_type machine header_size program_header_count')
        for key, expected in (('elf_class',2),('data_encoding',1),('version',1),('object_type',3),('machine',62),('header_size',64)):
            number(header[key], expected, expected)
        number(header['program_header_count'], 0, 65535)
        dynamic = row['dynamic']
        keys(dynamic, 'needed interpreter declared_search_tokens')
        need(type(dynamic['needed']) is list and len(dynamic['needed']) <= 64)
        for name in dynamic['needed']:
            basename(name)
        need(len(dynamic['needed']) == len(set(dynamic['needed'])))
        need(dynamic['interpreter'] in (None,'/lib64/ld-linux-x86-64.so.2','/usr/lib/ld-linux-x86-64.so.2'))
        need(type(dynamic['declared_search_tokens']) is list and len(dynamic['declared_search_tokens']) <= 128)
        for token in dynamic['declared_search_tokens']:
            need(type(token) is str and token in ('/usr/lib','/usr/lib/systemd','$ORIGIN','${ORIGIN}'))
        need(type(row['dependencies']) is list and len(row['dependencies']) == len(dynamic['needed']))
        for edge, name in zip(row['dependencies'], dynamic['needed']):
            keys(edge, 'name logical_path resolved_path links')
            need(edge['name'] == name and edge['resolved_path'] in known
                 and edge['logical_path'] in (root+'/'+name for root in SEARCH))
            links(edge['links'], edge['logical_path'], edge['resolved_path'])
        indexed[path] = row
    need(CANDIDATE in indexed and [r['path'] for r in rows] == sorted(indexed))
    need(sum(row['identity'][6] for row in rows) + value['readelf_identity'][6]
         + sum(row[6] for row in pkg['identities'].values()) <= 128*1024*1024)
    for alias in aliases:
        keys(alias, 'path resolved_path links')
        public(alias['path'])
        public(alias['resolved_path'], True)
        need(alias['path'] not in alias_index and alias['resolved_path'] in indexed)
        links(alias['links'], alias['path'], alias['resolved_path'])
        alias_index[alias['path']] = alias
    need([a['path'] for a in aliases] == sorted(alias_index) and CANDIDATE in alias_index)
    need(alias_index[CANDIDATE] == {'path':CANDIDATE,'resolved_path':CANDIDATE,'links':[]})
    expected_aliases, reached = {CANDIDATE}, {CANDIDATE}
    for _ in range(18):
        for path in tuple(reached):
            row = indexed[path]
            for edge in row['dependencies']:
                need(edge['logical_path'] in alias_index)
                alias = alias_index[edge['logical_path']]
                need(alias['resolved_path'] == edge['resolved_path'] and alias['links'] == edge['links'])
                expected_aliases.add(edge['logical_path'])
                reached.add(edge['resolved_path'])
            interpreter = row['dynamic']['interpreter']
            if interpreter:
                need(interpreter in alias_index)
                expected_aliases.add(interpreter)
                reached.add(alias_index[interpreter]['resolved_path'])
    need(reached == set(indexed) and expected_aliases == set(alias_index))
    return value


def decode(raw):
    need(type(raw) is bytes and 0 < len(raw) <= 262144)
    def unique(items):
        result = {}
        for key,value in items:
            need(key not in result)
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=unique,
                      parse_constant=lambda _: (_ for _ in ()).throw(ValueError()))


def read_fixed(name):
    need(name in ('result.json','copy-manifest.json'))
    fd = os.open(STAGE/name, os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
    try:
        s = os.fstat(fd)
        need(stat.S_ISREG(s.st_mode) and s.st_uid == s.st_gid == 1000
             and stat.S_IMODE(s.st_mode) == 0o600 and s.st_nlink == 1
             and 0 < s.st_size <= 262144 and not os.listxattr(fd))
        data = os.pread(fd,262145,0)
        fields = lambda v:(v.st_dev,v.st_ino,v.st_mode,v.st_uid,v.st_gid,v.st_nlink,v.st_size,v.st_mtime_ns,v.st_ctime_ns)
        need(len(data) == s.st_size and fields(s) == fields(os.fstat(fd)) == fields((STAGE/name).lstat()))
        return data
    finally:
        os.close(fd)


if __name__ == '__main__':
    need(Path(__file__) == STAGE/'validator.py' and sys.argv[1:] == ['--validate-fixed-encoder'])
    manifest = read_fixed('copy-manifest.json')
    need(hashlib.sha256(manifest).hexdigest() == MANIFEST_SHA)
    validate(decode(read_fixed('result.json')), decode(manifest))
    print('FIXED_ENCODER_TYPED_STATIC_ONLY')
