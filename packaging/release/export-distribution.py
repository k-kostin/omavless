#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Offline stable-tree export and fail-closed exact-source validation.

Developer tooling only: never installs, changes a ref, publishes or runs Cargo.
Offline validation is not release authorization or installed acceptance.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import posixpath
import re
import stat
import subprocess
import textwrap
import tomllib
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
REPO = 'k-kostin/omavless'
WEB = 'https://github.com/' + REPO + '/blob/'
SHA = r'[0-9a-f]{40}'
DIGEST = r'[0-9a-f]{64}'
# Reviewed complete full-source workflow, not merely a job display name.
# A workflow change needs an explicit corresponding trust-policy review.
SOURCE_WORKFLOW_BLOB = '79e65ac74a89604158818d55fc2a0d7de9888d94'
RECORD = 'distribution.json'
TOOLS = ('packaging/release/export-distribution.py',
         'packaging/release/pair-frontend.py', 'packaging/release/build-candidate.py',
         'tests/installed_native_package.py', 'tests/human_authorization.py')
FILES = {'README.md', 'backend.sh', 'manifest.json', 'preview.png', 'LICENSE',
         'THIRD_PARTY_NOTICES.md', 'CHANGELOG.md', 'install.sh', 'tools/uninstall.sh',
         'docs/user/NATIVE_INSTALL.md', 'docs/user/NATIVE_INSTALL_RU.md', 'docs/user/USER_AGENT_GUIDE.md',
         'docs/user/CLEAN_REINSTALL_098.md'}
ASSETS = {'docs/marketing/images/' + name + '.png' for name in
          ('app-en', 'main-en', 'settings-en', 'setup-en', 'subscription-en')}
ALIASES = {'INSTALL.md': 'install', 'INSTALL_082_REFERENCE.md': 'updates',
           'NATIVE_INSTALL_082_REFERENCE.md': 'updates', 'NATIVE_USAGE.md': 'connect-and-disconnect',
           'USAGE.md': 'connect-and-disconnect', 'PROTOCOLS.md': 'help',
           'SECURITY.md': 'help', 'TROUBLESHOOTING.md': 'help'}
TEMPLATES = 'packaging/release/distribution/'
LINK = re.compile(r'\]\(([^\s)]+)\)')
# The public source guide stays on main's reviewed removal recipe. A newer
# nonpublic helper may rebind ONLY the generated draft's canonical pin pair.
PUBLIC_HELPER_COMMIT = '0250efeb87ff5f6c331a2d067c181a29084b37a2'
PUBLIC_HELPER_SHA256 = '2ecda6be26438e3f6ffa46cc66c7f1c3b55ae1db6d176cf8471b4963724532f1'
PUBLIC_REMOVAL_GUIDE_SHA256 = '982eab7b13486b1634e38d860bef55e99d947faa25cb6aba705823a9de9e6b88'
RAW_SOURCE = 'https://raw.githubusercontent.com/' + REPO + '/'


def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args], timeout=30,
                                   stderr=subprocess.DEVNULL)


def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False) + '\n').encode()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate JSON field')
        result[key] = value
    return result


def decode(data):
    return json.loads(data, object_pairs_hook=unique)


def source(root, sha):
    if not isinstance(sha, str) or not re.fullmatch(SHA, sha):
        raise ValueError('full source commit required')
    if git(root, 'rev-parse', sha + '^{commit}').decode().strip() != sha:
        raise ValueError('not an exact commit')
    result = {}
    for item in git(root, 'ls-tree', '-rz', '--full-tree', sha).split(b'\0'):
        if item:
            meta, name = item.split(b'\t', 1)
            mode, kind, oid = meta.decode().split()
            result[name.decode('utf-8', 'strict')] = (mode, kind, oid)
    return result


def blob(root, tree, name):
    mode, kind, oid = tree[name]
    if kind != 'blob' or mode not in ('100644', '100755'):
        raise ValueError('non-regular product input')
    value = git(root, 'cat-file', 'blob', oid)
    if len(value) > 8 * 1024 * 1024:
        raise ValueError('product member too large')
    return value, int(mode, 8) & 0o777


def template(root, tree, name):
    data, mode = blob(root, tree, TEMPLATES + name)
    if mode != 0o644:
        raise ValueError('template mode')
    return data.decode('utf-8')


def inventory(entries):
    return {name: {'mode': mode, 'length': len(data), 'sha256': digest(data)}
            for name, (data, mode) in sorted(entries.items())}


def native(root, sha, tree, entries):
    app = decode(entries['plugin/runtime-release.json'][0])
    dns = decode(entries['plugin/dns-release.json'][0])
    updates = decode(entries['plugin/component-updates.json'][0])
    version = decode(entries['manifest.json'][0])['version']
    cargo = tomllib.loads(blob(root, tree, 'Cargo.toml')[0].decode('utf-8'))
    if (not isinstance(version, str) or not re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+', version)
            or cargo['workspace']['package']['version'] != version):
        raise ValueError('stable product version')
    producers = set()
    for record in (app, dns):
        if (set(record) != {'schemaVersion', 'version', 'packages'}
                or type(record['schemaVersion']) is not int or record['schemaVersion'] != 1
                or record['version'] != version or set(record['packages']) != {'x86_64', 'aarch64'}):
            raise ValueError('paired architectures/version')
        for pin in record['packages'].values():
            if (set(pin) != {'sha256', 'sourceCommit'} or not re.fullmatch(DIGEST, pin['sha256'])
                    or not re.fullmatch(SHA, pin['sourceCommit'])):
                raise ValueError('package identity')
            producers.add(pin['sourceCommit'])
    if (set(updates) != {'schemaVersion', 'sourceCommit', 'updatePair', 'identities'}
            or len(producers) != 1 or type(updates.get('schemaVersion')) is not int
            or updates.get('schemaVersion') != 2
            or updates.get('sourceCommit') not in producers or updates.get('updatePair') is not True
            or set(updates.get('identities', {})) != {'x86_64', 'aarch64'}):
        raise ValueError('inconsistent app/DNS tuple')
    for identity in updates['identities'].values():
        if (set(identity) != {'appSha', 'coreSha', 'brokerSha', 'dnsReceiptSha'}
                or any(not isinstance(v, str) or not re.fullmatch(DIGEST, v) for v in identity.values())):
            raise ValueError('native payload identities')
    producer = producers.pop()
    spec = importlib.util.spec_from_file_location('distribution_pair', ROOT / TOOLS[1])
    pair = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(pair)
    # Keep the existing complete-blob projection. cfg(test) is NOT an exception.
    native_digest = pair.equivalent_inputs(root, producer, sha)
    producer_tree = source(root, producer)
    return {'version': version, 'producer': producer, 'sourceUrl': WEB + producer,
            'inputTreeSha256': native_digest,
            'featureSelectionBlob': producer_tree['packaging/release/client-features.sh'][2],
            'appPackages': app['packages'], 'dnsPackages': dns['packages'],
            'payloadIdentities': updates['identities']}


def anchors(data):
    text = data.decode('utf-8')
    found = set(re.findall(r'<a\s+id="([^"]+)"', text))
    counts = {}
    for title in re.findall(r'^#{1,6}\s+(.+?)\s*#*$', text, re.M):
        slug = re.sub(r'[^\w\s-]', '', title.lower()).replace(' ', '-')
        number = counts.get(slug, 0)
        counts[slug] = number + 1
        found.add(slug + ('-' + str(number) if number else ''))
    return found


def guide_manifest(entries, version):
    """Validate original source identities before any allowed link rewrite."""
    name = 'plugin/GuideLinks.js'
    if name not in entries:
        if version == '0.9.9':
            raise ValueError('missing guide manifest')
        return None  # Historical English-only source fixtures remain distinct.
    data = entries[name][0].decode('utf-8')
    start = '// BEGIN GUIDE MANIFEST\nvar guideManifest = '
    end = ';\n// END GUIDE MANIFEST'
    if data.count(start) != 1 or data.count(end) != 1:
        raise ValueError('guide manifest framing')
    value = decode(data.split(start, 1)[1].split(end, 1)[0])
    if (set(value) != {'schemaVersion', 'version', 'english', 'russian'}
            or type(value['schemaVersion']) is not int or value['schemaVersion'] != 1
            or value['version'] != version):
        raise ValueError('guide manifest version')
    for language, path in (('english', 'docs/user/NATIVE_INSTALL.md'),
                           ('russian', 'docs/user/NATIVE_INSTALL_RU.md')):
        entry = value[language]
        keys = {'path', 'sha256'} | ({'humanReview'} if language == 'russian' else set())
        if (not isinstance(entry, dict) or set(entry) != keys or entry['path'] != path
                or not isinstance(entry['sha256'], str) or not re.fullmatch(DIGEST, entry['sha256'])
                or path not in entries or digest(entries[path][0]) != entry['sha256']):
            raise ValueError('stale or missing guide source')
    review = value['russian']['humanReview']
    if review is not None:
        if (not isinstance(review, dict)
                or set(review) != {'kind', 'version', 'englishSha256', 'russianSha256', 'reviewedOn'}
                or review['kind'] != 'owner-human-meaning' or review['version'] != version
                or review['englishSha256'] != value['english']['sha256']
                or review['russianSha256'] != value['russian']['sha256']
                or not isinstance(review['reviewedOn'], str)
                or not re.fullmatch(r'\d{4}-\d{2}-\d{2}', review['reviewedOn'])):
            raise ValueError('guide human review identity')
    guide_structure(entries, value)
    return value


def guide_structure(entries, manifest):
    """Also applies after export: anchors and copyable commands must survive."""
    english = entries[manifest['english']['path']][0]
    russian = entries[manifest['russian']['path']][0]
    if not anchors(english) <= anchors(russian):
        raise ValueError('missing translated guide anchor')
    for data in (english, russian):
        if not {'install', 'updates', 'help'} <= anchors(data):
            raise ValueError('missing locale help destination')
    def commands(data):
        pattern = r'(?m)^([ \t]*)`{3}(bash|sh|text)\n(.*?)^\1`{3}'
        return [(match[2], textwrap.dedent(match[3]))
                for match in re.finditer(pattern, data.decode('utf-8'), re.S)]
    if not commands(english) or commands(english) != commands(russian):
        raise ValueError('translated guide commands differ')


def links(root, sha, tree, entries):
    for name, (data, mode) in list(entries.items()):
        if not name.endswith('.md'):
            continue
        def resolve(match):
            target = match[1]
            absolute = False
            if re.match(r'[a-zA-Z][\w+.-]*:', target):
                for prefix in (WEB + 'main/', WEB + sha + '/'):
                    if target.startswith(prefix):
                        target = target[len(prefix):]
                        absolute = True
                        break
                else:
                    return match[0]
            parsed = urllib.parse.urlsplit(target)
            parent = '' if absolute else posixpath.dirname(name)
            resolved = posixpath.normpath(posixpath.join(parent, urllib.parse.unquote(parsed.path))) if parsed.path else name
            fragment = urllib.parse.unquote(parsed.fragment)
            if resolved.startswith(('../', '/')):
                raise ValueError('guide link outside tree')
            if resolved.startswith('docs/user/') and resolved.removeprefix('docs/user/') in ALIASES and fragment:
                if fragment not in anchors(blob(root, tree, resolved)[0]):
                    raise ValueError('missing historical guide anchor')
                return '](' + WEB + sha + '/' + resolved + '#' + parsed.fragment + ')'
            if resolved not in entries:
                if resolved not in tree:
                    raise ValueError('missing guide destination')
                if fragment and resolved.endswith('.md') and fragment not in anchors(blob(root, tree, resolved)[0]):
                    raise ValueError('missing immutable guide anchor')
                return '](' + WEB + sha + '/' + resolved + ('#' + parsed.fragment if parsed.fragment else '') + ')'
            if fragment and resolved.endswith('.md') and fragment not in anchors(entries[resolved][0]):
                raise ValueError('missing guide anchor: ' + resolved + '#' + parsed.fragment)
            return match[0]
        entries[name] = (LINK.sub(resolve, data.decode('utf-8')).encode(), mode)
    for name in ('plugin/SetupPage.qml', 'plugin/Panel.qml'):
        text = entries[name][0].decode('utf-8')
        for destination in re.findall(re.escape(WEB + 'main/') + r'([^"\s)]+)', text):
            file, _, anchor = destination.partition('#')
            if file not in entries or (anchor and anchor not in anchors(entries[file][0])):
                raise ValueError('missing shipped help destination')


def removal_guide(root, sha, tree, entries):
    name = 'docs/user/CLEAN_REINSTALL_098.md'
    data, mode = entries[name]
    text = data.decode('utf-8')
    urls = re.findall(r'https?://raw\.githubusercontent\.com/[^\s]+', text)
    checksums = re.findall(r"printf '%s  %s\\n' ([0-9a-f]{64}) \\\n", text)
    all_digests = re.findall(r'(?<![0-9a-f])[0-9a-f]{64}(?![0-9a-f])', text)
    if (len(urls) != 1 or not re.fullmatch(re.escape(RAW_SOURCE) + SHA + '/tools/uninstall\\.sh', urls[0])
            or len(checksums) != 1 or all_digests != checksums):
        raise ValueError('removal guide/helper pin shape differs')
    helper_sha = digest(entries['tools/uninstall.sh'][0])
    if checksums[0] != helper_sha:
        # Refuse unknown public recipes, edited witnesses and ambiguous pins;
        # never globally replace a caller-selected URL/hash or weaken admission.
        public_url = RAW_SOURCE + PUBLIC_HELPER_COMMIT + '/tools/uninstall.sh'
        snapshot, snapshot_mode = blob(root, tree, 'tests/fixtures/public-uninstaller/'
                                       + PUBLIC_HELPER_COMMIT + '.sh.txt')
        if (digest(data) != PUBLIC_REMOVAL_GUIDE_SHA256 or urls[0] != public_url
                or checksums[0] != PUBLIC_HELPER_SHA256 or snapshot_mode != 0o644
                or digest(snapshot) != PUBLIC_HELPER_SHA256):
            raise ValueError('unreviewed public removal guide/helper')
        selected_url = RAW_SOURCE + sha + '/tools/uninstall.sh'
        text = text.replace(public_url, selected_url, 1).replace(PUBLIC_HELPER_SHA256, helper_sha, 1)
        if (re.findall(r'https?://raw\.githubusercontent\.com/[^\s]+', text) != [selected_url]
                or re.findall(r"printf '%s  %s\\n' ([0-9a-f]{64}) \\\n", text) != [helper_sha]):
            raise ValueError('generated removal guide/helper identity differs')
    # Preserve the same-helper invariant for every generated distribution.
    if helper_sha not in text:
        raise ValueError('removal guide/helper identity differs')
    entries[name] = (text.encode('utf-8'), mode)
    return helper_sha


def compose(root, sha):
    tree = source(root, sha)
    # A different working-tree implementation must not export under this SHA.
    for name in TOOLS:
        if blob(root, tree, name)[0] != (ROOT / name).read_bytes():
            raise ValueError('exporter implementation differs from selected source')
    selected = FILES | ASSETS | {name for name in tree if name.startswith(('plugin/', 'templates/'))}
    entries = {name: blob(root, tree, name) for name in sorted(selected)}
    guides = guide_manifest(entries, decode(entries['manifest.json'][0])['version'])
    for name in selected:
        if (not re.fullmatch(r'[A-Za-z0-9_./-]+', name) or name.startswith(('/', '.'))
                or '..' in name.split('/') or name.endswith('.py')):
            raise ValueError('unsafe product member')
    for name in ('AGENTS.md', 'CONTRIBUTING.md'):
        entries[name] = (template(root, tree, name).replace('@SOURCE_URL@', WEB + sha).encode(), 0o644)
    entries['.github/workflows/test.yml'] = (template(root, tree, 'test.yml').encode(), 0o644)
    entries['.github/source-bootstrap.py'] = (template(root, tree, 'source-bootstrap.py').encode(), 0o644)
    pins = decode(blob(root, tree, TEMPLATES + 'source-validator-pins.json')[0])
    if pins != {name: digest(blob(root, tree, name)[0]) for name in TOOLS}:
        raise ValueError('reviewed validator pins differ from source code')
    entries['.github/source-validator-pins.json'] = (canonical(pins), 0o644)
    helper_sha = removal_guide(root, sha, tree, entries)
    for old, anchor in ALIASES.items():
        name = 'docs/user/' + old
        if name in tree:
            page = template(root, tree, 'compatibility.md')
            page = page.replace('@TITLE@', 'OmaVLESS help').replace('@GUIDE@', 'NATIVE_INSTALL.md#' + anchor)
            old_anchors = anchors(blob(root, tree, name)[0])
            if any(not re.fullmatch(r'[\w-]+', value) for value in old_anchors):
                raise ValueError('unsupported historical anchor')
            page = page.replace('@ANCHORS@', '\n'.join('<a id="' + value + '"></a>' for value in sorted(old_anchors)))
            entries[name] = (page.replace('@REFERENCE@', WEB + sha + '/' + name).encode(), 0o644)
    links(root, sha, tree, entries)
    if guides is not None:
        guide_structure(entries, guides)
    members = inventory(entries)
    record = {'schemaVersion': 1, 'kind': 'omavless-stable-distribution', 'repository': REPO,
              'sourceCommit': sha, 'sourceTree': git(root, 'rev-parse', sha + '^{tree}').decode().strip(),
              'sourceWorkflowBlob': tree['.github/workflows/test.yml'][2],
              'state': 'draft-unpublished', 'native': native(root, sha, tree, entries),
              'helperSha256': helper_sha,
              'files': members, 'treeSha256': digest(canonical(members))}
    return entries, record


def safe_directory(path, root, empty=False):
    if not path.is_absolute() or path.resolve(strict=True) != path:
        raise ValueError('absolute non-redirected directory required')
    for parent in (path, *path.parents):
        info = parent.lstat()
        if (not stat.S_ISDIR(info.st_mode) or info.st_uid not in (0, os.getuid())
                or info.st_mode & 0o022):
            raise ValueError('redirected directory')
    if path.stat().st_uid != os.getuid() or path == root or path in root.parents or root in path.parents:
        raise ValueError('output ownership/location')
    live = Path.home() / '.config/omarchy'
    if path == live or live in path.parents:
        raise ValueError('live desktop is not an export target')
    if empty and any(path.iterdir()):
        raise ValueError('output is not empty')


def export(root, sha, output):
    safe_directory(output, root, empty=True)
    entries, record = compose(root, sha)
    output.chmod(0o700)
    for name, (data, mode) in entries.items():
        target = output / name
        target.parent.mkdir(parents=True, exist_ok=True)
        with target.open('xb') as stream:
            stream.write(data)
        target.chmod(mode)
    # Publish the record last. A partial export is not a validated distribution.
    record_path = output / RECORD
    record_path.write_bytes(canonical(record))
    record_path.chmod(0o644)
    return record


def api(path):
    token = os.environ.get('GH_TOKEN', '')
    # RFC6750 bearer bytes only: do not trim/extract polluted launcher output
    # or silently retry anonymously. Refuse BEFORE constructing a request.
    if token and (len(token) > 4096 or not re.fullmatch(r'[A-Za-z0-9._~+/-]+=*', token, re.ASCII)):
        raise ValueError('CI API authentication invalid')
    headers = {'Accept': 'application/vnd.github+json', 'User-Agent': 'OmaVLESS-distribution-check'}
    if token:
        headers['Authorization'] = 'Bearer ' + token
    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, req, fp, code, msg, headers, newurl):
            raise ValueError('CI API redirect refused')
    try:
        request = urllib.request.Request('https://api.github.com/repos/' + REPO + '/' + path, headers=headers)
        with urllib.request.build_opener(NoRedirect).open(request, timeout=30) as response:
            data = response.read(1024 * 1024 + 1)
        if len(data) > 1024 * 1024:
            raise ValueError('CI response bound')
        return decode(data)
    except Exception:
        # HTTP protocol exceptions, OS errors and parser failures can all carry
        # headers or remote text. No payload, repr or chained traceback escapes.
        raise ValueError('CI API request failed') from None


def check_ci(root, sha, record):
    workflow = git(root, 'show', sha + ':.github/workflows/test.yml').decode('utf-8')
    # Exact head checkout is mandatory; a PR merge checkout is not silently the head.
    if (record['sourceWorkflowBlob'] != SOURCE_WORKFLOW_BLOB
            or 'ref: ${{ github.event.pull_request.head.sha || github.sha }}' not in workflow
            or 'run: ./tests/run.sh' not in workflow or './tests/run-rust.sh' not in workflow):
        raise ValueError('source workflow lacks explicit full-head gate')
    workflow_id = api('actions/workflows/test.yml')['id']
    runs = api('actions/workflows/test.yml/runs?head_sha=' + sha + '&per_page=100')['workflow_runs']
    for run in runs:
        if (run.get('head_sha') != sha or run.get('workflow_id') != workflow_id
                or (run.get('head_repository') or {}).get('full_name') != REPO
                or run.get('path') != '.github/workflows/test.yml' or run.get('status') != 'completed'
                or run.get('conclusion') != 'success' or run.get('event') != 'push'):
            continue
        jobs = api('actions/runs/' + str(int(run['id'])) + '/attempts/' + str(int(run['run_attempt'])) + '/jobs?per_page=100')
        if jobs.get('total_count') != 1 or len(jobs.get('jobs', [])) != 1:
            continue
        job = jobs['jobs'][0]
        steps = {step.get('name'): step.get('conclusion') for step in job.get('steps', [])}
        if (job.get('name') == 'test' and job.get('conclusion') == 'success'
                and steps.get('Run isolated checks') == 'success'
                and steps.get('Run Rust workspace and parity checks') == 'success'):
            # Bind the checked source workflow blob independently of its display name.
            remote = api('contents/.github/workflows/test.yml?ref=' + sha)
            if remote.get('sha') == record['sourceWorkflowBlob']:
                return int(run['id'])
    raise ValueError('no complete successful exact-source CI gate')


def verify(root, output, source_ci=False):
    safe_directory(output, root)
    record_path = output / RECORD
    if (not stat.S_ISREG(record_path.lstat().st_mode) or record_path.stat().st_size > 256 * 1024
            or stat.S_IMODE(record_path.stat().st_mode) != 0o644):
        raise ValueError('invalid distribution record')
    record = decode(record_path.read_bytes())
    entries, expected = compose(root, record['sourceCommit'])
    if record_path.read_bytes() != canonical(expected):
        raise ValueError('record differs from immutable source export')
    found = set()
    parents = {parent.as_posix() for name in entries for parent in Path(name).parents if str(parent) != '.'}
    for directory, dirs, files in os.walk(output, followlinks=False):
        if Path(directory) == output and '.git' in dirs:
            if not stat.S_ISDIR((output / '.git').lstat().st_mode):
                raise ValueError('redirected Git directory')
            dirs.remove('.git')
        for name in dirs:
            path = Path(directory) / name
            if path.relative_to(output).as_posix() not in parents or not stat.S_ISDIR(path.lstat().st_mode):
                raise ValueError('redirected distribution directory')
        for name in files:
            path = Path(directory) / name
            relative = path.relative_to(output).as_posix()
            if relative == '.git' and stat.S_ISREG(path.lstat().st_mode):
                # Only normal checkout administrative data is outside the payload.
                continue
            if relative == RECORD:
                continue
            if relative not in entries or not stat.S_ISREG(path.lstat().st_mode):
                raise ValueError('unexpected/nonregular distribution member')
            data, mode = entries[relative]
            if path.stat().st_size != len(data) or stat.S_IMODE(path.stat().st_mode) != mode or path.read_bytes() != data:
                raise ValueError('distribution member differs from source')
            found.add(relative)
    if found != set(entries):
        raise ValueError('incomplete distribution')
    return check_ci(root, record['sourceCommit'], record) if source_ci else None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    create = commands.add_parser('export')
    create.add_argument('output', type=Path)
    create.add_argument('source_commit')
    check = commands.add_parser('verify')
    check.add_argument('output', type=Path)
    check.add_argument('--source-ci', action='store_true')
    args = parser.parse_args()
    try:
        if args.command == 'export':
            export(ROOT, args.source_commit, args.output)
            print('Draft distribution exported; no installation, publication or ARM acceptance.')
        else:
            run = verify(ROOT, args.output, args.source_ci)
            print('Exact-source CI verified: ' + str(run) if run else
                  'Offline distribution verified; source CI/publication/installed gates remain separate.')
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError):
        parser.exit(2, 'Distribution refused: check source, output and CI prerequisites.\n')


if __name__ == '__main__':
    main()
