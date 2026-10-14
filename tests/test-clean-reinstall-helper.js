#!/usr/bin/env node
// Read-only source and pure validator tests. Never calls sudo, rm or systemctl.
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const source = fs.readFileSync(path.join(__dirname, '../tools/clean-reinstall-098.sh'), 'utf8');
assert.equal(spawnSync('bash', ['-n'], { input: source }).status, 0);
const functions = source.slice(source.indexOf('fail()'), source.indexOf('[[ $# == 1'));
assert(!/\bsudo\b|\brm\b/.test(functions));
let cases = 0;
function validate(command, overrides, pass) {
  const fixture = `set -euo pipefail
HOME=/synthetic/desktop
id() { printf '1234\\n'; }
getent() { printf 'desktop:x:1234:1234:Desktop:/synthetic/desktop:/bin/bash\\n'; }
findmnt() { printf '%s\\n' '{"filesystems":[{"target":"/"}]}'; }
stat() { printf '1234\\n'; }
find() { :; }
${functions}
${overrides || ''}
${command}
`;
  const result = spawnSync('bash', ['--noprofile', '--norc'], { input: fixture, encoding: 'utf8' });
  assert.equal(result.status === 0, pass, result.stdout + result.stderr);
  cases++;
}
validate('check_account', '', true);
validate('check_account', 'HOME=/somewhere/else', false);
validate('check_account', 'getent() { return 1; }', false);
validate('check_account', `getent() { printf 'other:x:9999:9999:Other:/synthetic/desktop:/bin/bash\\n'; }`, false);
validate('check_private_targets', '', true);
for (const target of ['.config/omavless', '.local/state/omavless', '.cache/omavless']) {
  for (const suffix of ['', '/bind', '/deeper/same-filesystem-bind']) {
    const json = JSON.stringify({ filesystems: [{ target: '/' }, { target: '/synthetic/desktop/' + target + suffix }] });
    validate('check_private_targets', `findmnt() { printf '%s\\n' '${json}'; }`, false);
  }
  const sibling = JSON.stringify({ filesystems: [{ target: '/synthetic/desktop/' + target + '-other' }] });
  validate('check_private_targets', `findmnt() { printf '%s\\n' '${sibling}'; }`, true);
}
for (const json of ['{}', '{"filesystems":"invalid"}', '{"filesystems":[{"target":null}]}', 'broken']) {
  validate('check_private_targets', `findmnt() { printf '%s\\n' '${json}'; }`, false);
}
// Disposable, credential-free paths only. The helper performs no deletion.
const scratch = fs.mkdtempSync(path.join(os.homedir(), '.cache/ovt/clean-helper-test.'));
try {
  const target = path.join(scratch, '.config/omavless');
  fs.mkdirSync(target, { recursive: true });
  fs.writeFileSync(path.join(target, 'marker'), 'synthetic');
  validate('check_private_targets', `HOME=${JSON.stringify(scratch)}`, true);
  validate('check_private_targets', `HOME=${JSON.stringify(scratch)}; stat() { printf '9999\\n'; }`, false);
  validate('check_private_targets', `HOME=${JSON.stringify(scratch)}; find() { printf 'foreign-owner-file\\n'; }`, false);
  assert.equal(fs.readFileSync(path.join(target, 'marker'), 'utf8'), 'synthetic');
  fs.mkdirSync(path.join(scratch, '.cache'));
  fs.symlinkSync(target, path.join(scratch, '.cache/omavless'));
  validate('check_private_targets', `HOME=${JSON.stringify(scratch)}`, false);
} finally {
  // Remove only the exact directory created by this test, not any helper target.
  fs.rmSync(scratch, { recursive: true });
}
// Strong source boundaries supplement pure checks, not a live removal PASS.
assert(source.indexOf('check_private_targets\n') < source.indexOf('sudo /usr/bin/bash -c "$root_remove"'));
assert(source.lastIndexOf('check_private_targets\n') < source.indexOf('rm -r --one-file-system'));
assert(source.includes('getent passwd "$(id -u)"'));
assert(source.includes('No backup is created.'));
for (const forbidden of ['sudo bash "$', 'sudo /usr/bin/bash "$', '--nodeps', '--overwrite', '--noconfirm', 'rm -rf', 'kill -', 'reset-failed', '--what=fdstore']) {
  assert(!source.includes(forbidden), forbidden);
}
for (const field of ['NFileDescriptorStore', 'NeedDaemonReload', 'FragmentPath', 'SUDO_UID', 'package-guard', '--revoke', 'original_pid', 'ExecMainCode']) {
  assert(source.includes(field), field);
}
assert(source.indexOf('/usr/lib/omavless-dns/package-guard') < source.indexOf('/usr/lib/omavless-dns/omavless-dns-broker --revoke'));
assert(source.indexOf('--revoke') < source.indexOf('sudo pacman -R -- omavless omavless-dns'));
console.log(`Clean-reinstall helper: ${cases} pure validator cases and source boundaries PASS; no host effects.`);
