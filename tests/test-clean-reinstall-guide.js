#!/usr/bin/env node
// Documentation-block checks/extraction only: never operates a host or VM.
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const guide = fs.readFileSync(path.join(__dirname, '../docs/user/CLEAN_REINSTALL_098.md'), 'utf8');
const blocks = Array.from(guide.matchAll(/```bash\n([\s\S]*?)\n```/g), match => match[1] + '\n');
assert.equal(blocks.length, 7);
for (const block of blocks) {
  const syntax = spawnSync('bash', ['-n'], { input: block, encoding: 'utf8' });
  assert.equal(syntax.status, 0, syntax.stderr);
}
assert.match(guide, /not yet an instruction shipped by main/);
assert.match(guide, /never\s+connected OmaVLESS/);
assert.match(guide, /journal is volatile/);
assert.match(blocks[2], /clean\nsystemctl|clean\n\[\[/);
assert(blocks[2].indexOf('clean\n') < blocks[2].indexOf('systemctl disable --now'));
assert(blocks[2].lastIndexOf('clean\n') > blocks[2].indexOf('systemctl disable --now'));
for (const fact of ['FragmentPath', 'NeedDaemonReload', 'DropInPaths', 'Job', 'NFileDescriptorStore', 'original_pid', 'sha256sum']) {
  assert(blocks[2].includes(fact), fact);
}
assert.equal(blocks[3].trim(), 'systemctl reboot');
assert(blocks[4].indexOf('package-guard') < blocks[4].indexOf('--revoke'));
assert(blocks[4].indexOf('--revoke') < blocks[4].indexOf('pacman -R'));
for (const forbidden of ['rm -', 'unlink ', '--nodeps', '--overwrite', '--noconfirm', '--what=fdstore', 'kill -', 'reset-failed']) {
  assert(!blocks.some(block => block.includes(forbidden)), forbidden);
}
assert(blocks[5].includes('omavless-login.receipt'));
assert(blocks[5].includes('PRIVATE_BACKUP_SAVED'));
assert(blocks[6].indexOf('checkout --detach v0.9.8-fix.5') < blocks[6].indexOf('plugin enable'));
if (process.argv.length === 4 && process.argv[2] === '--extract' && /^[0-6]$/.test(process.argv[3])) {
  process.stdout.write(blocks[Number(process.argv[3])]);
} else if (process.argv.length === 2) {
  console.log('Clean-reinstall guide: 7 Bash blocks syntax/order/boundary checks PASS; no host effects.');
} else {
  throw new Error('Usage: node tests/test-clean-reinstall-guide.js [--extract 0..6]');
}
