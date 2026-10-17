#!/usr/bin/env node
// Documentation/source checks only: no network, host or VM operations.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const os = require('node:os');
const guide = fs.readFileSync(path.join(__dirname, '../docs/user/CLEAN_REINSTALL_098.md'), 'utf8');
const candidateHelper = fs.readFileSync(path.join(__dirname, '../tools/uninstall.sh'));
const blocks = Array.from(guide.matchAll(/```bash\n([\s\S]*?)\n```/g), match => match[1] + '\n');
assert.equal(blocks.length, 3);
for (const block of blocks) {
  const syntax = spawnSync('bash', ['-n'], { input: block, encoding: 'utf8' });
  assert.equal(syntax.status, 0, syntax.stderr);
}
assert(!/never connected|used it before|ask for help/.test(guide));
assert.match(guide, /running\nor stopped application/);
assert.match(guide, /If interrupted, run the same command/);
assert.match(guide, /without a backup/);
assert.match(guide, /public 0\.8\.2 and 0\.9\.8 installation/);
assert(!guide.includes('After integration'));
assert(!guide.includes('must be reconciled'));
assert(!guide.includes('PRIVATE_BACKUP'));
assert(guide.split(/\s+/).length < 500, 'Quick path must remain short; detailed internals belong elsewhere.');
// Public help follows main, not the current unpublished developer helper.
// Use the exact immutable download snapshot, without a Git/network dependency
// (CI checkouts can be shallow). Never execute the frozen helper body.
function publishedHelper(bootstrap) {
  const pin = bootstrap.match(/raw\.githubusercontent\.com\/k-kostin\/omavless\/([0-9a-f]{40})\/tools\/uninstall\.sh/);
  assert(pin, 'Guide must name an immutable source');
  const helper = fs.readFileSync(path.join(__dirname, 'fixtures/public-uninstaller', pin[1] + '.sh.txt'));
  assert(bootstrap.includes(crypto.createHash('sha256').update(helper).digest('hex')),
    'Published download bytes must match the actual guide checksum');
  return helper;
}
const helper = publishedHelper(blocks[0]);
const publishedHash = crypto.createHash('sha256').update(helper).digest('hex');
assert.throws(() => publishedHelper(blocks[0].replace(publishedHash, '0'.repeat(64))));
assert.throws(() => publishedHelper(blocks[0].replace(/omavless\/[0-9a-f]{40}\/tools/, 'omavless/' + '0'.repeat(40) + '/tools')));
assert(blocks[0].indexOf('sha256sum --check') < blocks[0].indexOf('bash "$HOME/'));
assert(blocks[0].indexOf('sha256sum --check') < blocks[0].indexOf('mv -T --'));
assert.match(blocks[0], /--connect-timeout 10 --max-time 60/);
assert.match(blocks[0], /-o "\$HOME\/\.cache\/omavless-maintenance\/reinstall-098\/\.download"/);
assert(blocks[0].includes('if [[ -f "$HOME/.config/omarchy/plugins/kdk.omavless/tools/uninstall.sh" ]]'));
assert(blocks[0].includes('cp -- "$HOME/.config/omarchy/plugins/kdk.omavless/tools/uninstall.sh"'));
assert.equal(blocks[1].trim(), 'bash "$HOME/.cache/omavless-maintenance/reinstall-098/uninstall.sh"');
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
// Execute the real bootstrap with inert download/entry leaves. The helper's
// actual body is never executed here; all files live in a private HOME fixture.
for (const kind of ['offline-installed', 'download-missing-widget', 'modified-local', 'candidate-local']) {
  const parent = path.join(os.homedir(), '.cache/ovt');
  fs.mkdirSync(parent, { recursive: true, mode: 0o700 });
  const home = fs.mkdtempSync(path.join(parent, 'uninstall-bootstrap.'));
  try {
    const local = path.join(home, '.config/omarchy/plugins/kdk.omavless/tools/uninstall.sh');
    fs.writeFileSync(path.join(home, 'approved-download'), helper, { mode: 0o600 });
    if (kind !== 'download-missing-widget') {
      fs.mkdirSync(path.dirname(local), { recursive: true });
      fs.writeFileSync(local, kind === 'modified-local' ? 'modified local helper'
        : kind === 'candidate-local' ? candidateHelper : helper);
    }
    const download = kind === 'download-missing-widget'
      ? 'printf DOWNLOAD_ADMITTED; cp -- "$HOME/approved-download" "${@: -1}";'
      : 'printf FORBIDDEN_NETWORK; return 9;';
    const script = 'set -euo pipefail\nbash() { printf HELPER_ADMITTED; }\n'
      + 'curl() { ' + download + ' }\n' + blocks[0];
    const result = spawnSync('bash', ['--noprofile', '--norc'], {
      input: script, env: { ...process.env, HOME: home }, encoding: 'utf8', timeout: 5000,
    });
    assert(!result.error, kind + ': ' + result.error);
    const admitted = kind !== 'modified-local' && (kind !== 'candidate-local' || candidateHelper.equals(helper));
    assert.equal(result.status === 0, admitted, kind + ': ' + result.stderr);
    assert(!result.stdout.includes('FORBIDDEN_NETWORK'));
    assert.equal(result.stdout.includes('HELPER_ADMITTED'), admitted);
    assert.equal(result.stdout.includes('DOWNLOAD_ADMITTED'), kind === 'download-missing-widget');
  } finally {
    fs.rmSync(home, { recursive: true });
  }
}
if (process.argv.length === 4 && process.argv[2] === '--extract' && /^[0-2]$/.test(process.argv[3])) {
  process.stdout.write(blocks[Number(process.argv[3])]);
} else if (process.argv.length === 2) {
  console.log('Guided removal guide: one entry, same-command continuation, immutable checksum and reinstall order PASS; no host effects.');
} else {
  throw new Error('Usage: node tests/test-clean-reinstall-guide.js [--extract 0..2]');
}
