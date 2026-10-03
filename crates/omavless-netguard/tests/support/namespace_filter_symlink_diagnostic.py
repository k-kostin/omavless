#!/usr/bin/python3
"""Metadata-only activation escape provenance; never grants target authority."""
import hashlib
import json
import os
from pathlib import Path
import stat
import sys

ORIGINAL = Path('/run/omavless-k1-namespace-filter-fixture/guard.py')
ORIGINAL_SHA = 'c4a688875037f1d990ee93f0018108ab82bceb211090f61f77c64b83a4c7cc40'
DESTINATION = Path('/run/omavless-k1-namespace-symlink-diagnostic')
ALLOWED = ('/usr/lib/systemd', '/etc/systemd', '/run/systemd',
           '/home/kdk_vm/.config/systemd')


class Refused(Exception):
    def __init__(self, code):
        self.code = code


def require(value, code):
    if not value:
        raise Refused(code)


def metadata(info):
    # Access time is intentionally excluded; no regular file is opened here.
    return [info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid,
            info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns, info.st_rdev]


def target_class(path):
    for prefix, category in (('/usr', 'system_usr'), ('/etc', 'system_etc'),
                             ('/run', 'system_run'), ('/home/kdk_vm', 'guest_home')):
        if path.is_relative_to(Path(prefix)):
            return category
    return 'other'


class Inventory:
    def __init__(self, roots, limit=30000, depth_limit=128):
        self.roots = tuple(map(Path, roots))
        self.limit = limit
        self.depth_limit = depth_limit
        self.observed = {}
        self.visited = set()
        self.used = False

    def observe(self, path):
        require(path.is_absolute() and len(os.fsencode(path)) <= 4096, 'path_bound')
        require(len(self.observed) < self.limit or str(path) in self.observed, 'entry_bound')
        try:
            info = path.lstat()
            row = {'metadata': metadata(info)}
            if stat.S_ISLNK(info.st_mode):
                row['link'] = os.readlink(path)
                require(len(os.fsencode(row['link'])) <= 4096, 'path_bound')
                require(metadata(path.lstat()) == row['metadata'], 'changed')
        except FileNotFoundError:
            row = {'absent': True}
        old = self.observed.setdefault(str(path), row)
        require(old == row, 'changed')
        return row

    def resolve(self, path):
        # Bounded component-by-component resolution records every ancestor and
        # link. Never open the target or follow it with a content-reading API.
        pending = list(path.parts[1:])
        current = Path('/')
        links = 0
        self.observe(current)
        while pending:
            require(len(pending) <= self.depth_limit, 'depth_bound')
            component = pending.pop(0)
            if component == '..':
                current = current.parent
                continue
            if component == '.':
                continue
            candidate = current / component
            row = self.observe(candidate)
            require('absent' not in row, 'dangling')
            if 'link' in row:
                links += 1
                require(links <= 40, 'link_bound')
                link = Path(row['link'])
                if link.is_absolute():
                    current = Path('/')
                    pending = list(link.parts[1:]) + pending
                else:
                    pending = list(link.parts) + pending
            else:
                require(not pending or stat.S_ISDIR(row['metadata'][2]), 'not_directory')
                current = candidate
        return current

    def stable(self):
        for name in tuple(self.observed):
            self.observe(Path(name))

    def visit(self, path, ordinal, depth=0):
        require(depth <= self.depth_limit, 'depth_bound')
        if str(path) in self.visited:
            return None
        self.visited.add(str(path))
        row = self.observe(path)
        if 'absent' in row:
            return None
        if 'link' in row:
            resolved = self.resolve(path)
            if resolved != Path('/dev/null') and not any(
                    resolved.is_relative_to(Path(prefix)) for prefix in ALLOWED):
                return {'root_ordinal': ordinal, 'root': str(self.roots[ordinal]),
                        'source': str(path), 'direct_target': row['link'],
                        'resolved_target': str(resolved),
                        'target_class': target_class(resolved),
                        'target_metadata': self.observed[str(resolved)]['metadata']}
            return self.visit(resolved, ordinal, depth + 1)
        if stat.S_ISDIR(row['metadata'][2]):
            # Bound enumeration before sorting, including directories with an
            # unexpectedly large child count. Entries are never opened.
            children = []
            with os.scandir(path) as stream:
                for child in stream:
                    require(len(children) + len(self.observed) < self.limit, 'entry_bound')
                    children.append(Path(child.path))
            for child in sorted(children):
                result = self.visit(child, ordinal, depth + 1)
                if result is not None:
                    return result
        elif not stat.S_ISREG(row['metadata'][2]):
            require(path == Path('/dev/null') and stat.S_ISCHR(row['metadata'][2])
                    and os.major(row['metadata'][9]) == 1
                    and os.minor(row['metadata'][9]) == 3, 'unsupported_type')
        return None

    def run(self):
        require(not self.used, 'already_used')
        self.used = True
        result = None
        for ordinal, root in enumerate(self.roots):
            # Root ancestors may themselves be symlinks; retain their metadata
            # without treating a resolved prefix as trusted authority.
            try:
                self.resolve(root)
            except Refused as error:
                if error.code != 'dangling':
                    raise
            result = self.visit(root, ordinal)
            if result is not None:
                break
        self.stable()
        return result


def diagnose(inventory):
    public = {'schema': 'k1-symlink-provenance-v1', 'original_guard_sha256': ORIGINAL_SHA,
              'runner_invoked': False, 'normal_authority': False,
              'activation_contents_read': False, 'processes_spawned': False}
    private = {}
    try:
        result = inventory.run()
        public['category'] = 'escape_observed' if result else 'no_escape_observed'
        if result:
            public.update(root_ordinal=result['root_ordinal'], target_class=result['target_class'],
                          target_mode=result['target_metadata'][2],
                          target_uid=result['target_metadata'][3],
                          target_gid=result['target_metadata'][4])
            private['escape'] = result
    except Refused as error:
        public.update(category='refused', reason=error.code)
    except OSError:
        public.update(category='refused', reason='os_error')
    private['observations'] = inventory.observed
    private['public'] = public
    return public, private


def main():
    require(os.geteuid() == 0 and len(sys.argv) == 1, 'entry')
    info = DESTINATION.lstat()
    require(stat.S_ISDIR(info.st_mode) and info.st_uid == info.st_gid == 0
            and stat.S_IMODE(info.st_mode) == 0o700, 'entry')
    fd = os.open(ORIGINAL, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    with os.fdopen(fd, 'rb') as stream:
        before = os.fstat(stream.fileno())
        require(stat.S_ISREG(before.st_mode) and before.st_uid == before.st_gid == 0
                and stat.S_IMODE(before.st_mode) == 0o600 and before.st_nlink == 1, 'original')
        data = stream.read(256 * 1024 + 1)
        require(metadata(before) == metadata(os.fstat(stream.fileno())), 'original')
    require(hashlib.sha256(data).hexdigest() == ORIGINAL_SHA, 'original')
    original = {'__name__': 'retained_original_guard'}
    exec(compile(data, '<pinned-original-guard>', 'exec'), original)
    public, private = diagnose(Inventory(original['ACTIVATION_ROOTS']))
    for name, result in (('private.json', private), ('result.json', public)):
        fd = os.open(DESTINATION / name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, 'w') as stream:
            json.dump(result, stream, sort_keys=True)
    print(json.dumps(public, sort_keys=True))
    return 0 if public['category'] == 'escape_observed' else 2


if __name__ == '__main__':
    os.umask(0o077)
    try:
        sys.exit(main())
    except Exception:
        print('K1_SYMLINK_DIAGNOSTIC_ENTRY_REFUSED', file=sys.stderr)
        sys.exit(2)
