#!/usr/bin/env node
// Documentation/source checks only: no network, host or VM operations.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const guide = fs.readFileSync(path.join(__dirname, '../docs/user/CLEAN_REINSTALL_098.md'), 'utf8');
const helper = fs.readFileSync(path.join(__dirname, '../tools/clean-reinstall-098.sh'));
const blocks = Array.from(guide.matchAll(/```bash\n([\s\S]*?)\n```/g), match => match[1] + '\n');
assert.equal(blocks.length, 4);
for (const block of blocks) {
  const syntax = spawnSync('bash', ['-n'], { input: block, encoding: 'utf8' });
  assert.equal(syntax.status, 0, syntax.stderr);
}
assert.match(guide, /never connected a VPN/);
assert.match(guide, /without a backup/);
assert.match(guide, /test build with the setup fix/);
assert(!guide.includes('After integration'));
assert(!guide.includes('must be reconciled'));
assert(!guide.includes('PRIVATE_BACKUP'));
assert(guide.split(/\s+/).length < 470, 'Quick path must remain short; detailed internals belong elsewhere.');
assert.match(blocks[0], /raw\.githubusercontent\.com\/k-kostin\/omavless\/[0-9a-f]{40}\/tools\/clean-reinstall-098\.sh/);
assert(blocks[0].includes(crypto.createHash('sha256').update(helper).digest('hex')));
assert(blocks[0].indexOf('sha256sum --check') < blocks[0].indexOf('.sh" prepare'));
assert.equal(blocks[1].trim(), 'systemctl reboot');
assert.equal(blocks[2].trim(), 'bash "$HOME/.cache/ovt/reinstall-098/clean-reinstall-098.sh" remove');
const selected = blocks[3].match(/checkout --detach ([0-9a-f]{40})/);
assert(selected, 'Unpublished candidate must select an exact immutable source.');
assert(blocks[3].indexOf('checkout --detach ' + selected[1]) < blocks[3].indexOf('plugin enable'));
assert(blocks[3].includes('rev-parse HEAD)" = ' + selected[1]));
assert.match(guide, /packages are not published yet/);
assert.match(guide, /answer \*\*No\*\* to/);
for (const forbidden of ['sudo bash', 'curl |', 'rm -', '--nodeps', '--overwrite', '--noconfirm']) {
  assert(!blocks.some(block => block.includes(forbidden)), forbidden);
}
if (process.argv.length === 4 && process.argv[2] === '--extract' && /^[0-3]$/.test(process.argv[3])) {
  process.stdout.write(blocks[Number(process.argv[3])]);
} else if (process.argv.length === 2) {
  console.log('Clean-reinstall quick guide: 4 blocks, immutable helper/checksum, reset scope and install order PASS; no host effects.');
} else {
  throw new Error('Usage: node tests/test-clean-reinstall-guide.js [--extract 0..3]');
}
