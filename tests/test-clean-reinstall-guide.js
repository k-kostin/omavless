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
assert.equal(blocks.length, 3);
for (const block of blocks) {
  const syntax = spawnSync('bash', ['-n'], { input: block, encoding: 'utf8' });
  assert.equal(syntax.status, 0, syntax.stderr);
}
assert.match(guide, /never connected a VPN/);
assert.match(guide, /without a backup/);
assert.match(guide, /public 0\.8\.2 and 0\.9\.8 installation/);
assert(!guide.includes('After integration'));
assert(!guide.includes('must be reconciled'));
assert(!guide.includes('PRIVATE_BACKUP'));
assert(guide.split(/\s+/).length < 500, 'Quick path must remain short; detailed internals belong elsewhere.');
assert.match(blocks[0], /raw\.githubusercontent\.com\/k-kostin\/omavless\/[0-9a-f]{40}\/tools\/clean-reinstall-098\.sh/);
assert(blocks[0].includes(crypto.createHash('sha256').update(helper).digest('hex')));
assert(blocks[0].indexOf('sha256sum --check') < blocks[0].indexOf('bash "$HOME/'));
assert.equal(blocks[1].trim(), 'bash "$HOME/.cache/omavless-maintenance/reinstall-098/clean-reinstall-098.sh"');
assert.equal(blocks[2].trim(), 'omarchy plugin add https://github.com/k-kostin/omavless.git --enable');
assert(!blocks.some(block => /\.sh" (prepare|remove)/.test(block)));
assert(!guide.includes('not published yet'));
assert.match(guide, /Enter `REBOOT`/);
assert.match(guide, /Do not reopen or reinstall/);
assert(!guide.includes('Shut down OmaVLESS'), 'Shutdown must be offered inside the helper, not an unearned UI prerequisite.');
assert(!guide.includes('.cache/ovt/'));
for (const forbidden of ['sudo bash', 'curl |', 'rm -', '--nodeps', '--overwrite', '--noconfirm']) {
  assert(!blocks.some(block => block.includes(forbidden)), forbidden);
}
if (process.argv.length === 4 && process.argv[2] === '--extract' && /^[0-2]$/.test(process.argv[3])) {
  process.stdout.write(blocks[Number(process.argv[3])]);
} else if (process.argv.length === 2) {
  console.log('Guided removal guide: one entry, same-command continuation, immutable checksum and reinstall order PASS; no host effects.');
} else {
  throw new Error('Usage: node tests/test-clean-reinstall-guide.js [--extract 0..2]');
}
