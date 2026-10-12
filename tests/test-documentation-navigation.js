// SPDX-License-Identifier: MIT
// Read-only guard for the small agent entry point and preserved detailed docs.
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const root = path.resolve(__dirname, '..');
const files = ['AGENTS.md', 'CONTRIBUTING.md', 'docs/README.md',
  'docs/development/README.md', 'docs/development/AGENT_GUIDE.md',
  'docs/roadmap/DEVELOPMENT_WORKFLOW.md', 'docs/development/BETA_095.md',
  'docs/development/BETA_098.md', 'docs/development/RC_097.md', 'docs/development/RC_096.md', 'docs/development/RC_095.md',
  'docs/development/RC_090.md'];
let links = 0;
for (const file of files) {
  const text = fs.readFileSync(path.join(root, file), 'utf8');
  for (const match of text.matchAll(/\]\(([^\s)]+)\)/g)) {
    const target = match[1].split('#')[0];
    if (!target || /^[a-z][a-z0-9+.-]*:/i.test(target)) continue;
    const resolved = path.resolve(root, path.dirname(file), target);
    assert(resolved.startsWith(root + path.sep), 'documentation must stay inside repository');
    assert(fs.existsSync(resolved), 'missing documentation target: ' + target);
    links++;
  }
}
const entry = fs.readFileSync(path.join(root, 'AGENTS.md'), 'utf8');
assert(entry.split('\n').length <= 80, 'root AGENTS must remain a concise entry point');
for (const target of ['docs/development/AGENT_GUIDE.md', 'DEVELOPMENT_ROADMAP.md',
  'docs/roadmap/CURRENT_STATUS.md', 'docs/roadmap/DEVELOPMENT_WORKFLOW.md',
  'docs/roadmap/ACCEPTANCE_ENVIRONMENTS.md', 'docs/roadmap/RUST_MIGRATION.md',
  'skills/omavless-ui-review/SKILL.md', 'skills/omavless-localization/SKILL.md']) {
  assert(entry.includes('](' + target + ')'), 'agent discovery must retain ' + target);
}
const guide = fs.readFileSync(path.join(root, 'docs/development/AGENT_GUIDE.md'), 'utf8');
for (const section of ['Mandatory freshness', 'Acceptance environments',
  'Exact-head discipline', 'Git discipline', 'Cross-agent handoff',
  'UI interaction', 'Localization work', 'Historical continuity checkpoint'])
  assert(guide.includes(section), 'preserved guide section: ' + section);
const workflow = fs.readFileSync(path.join(root, 'docs/roadmap/DEVELOPMENT_WORKFLOW.md'), 'utf8');
assert(workflow.includes('dev/<topic>') && workflow.includes('rc/<version>'));
assert(workflow.includes('beta/<version>'), 'versioned development assembly must be documented');
assert(workflow.includes('### Beta-integrated / not released'));
assert(workflow.includes('### Internal candidate checkpoints (owner decision, 2026-10-02)'));
assert(workflow.includes('Installed checks remain necessary'));
assert(workflow.includes('Pure models\nand green CI alone cannot establish a working backup/restore feature'));
assert(!workflow.includes('not a new branch convention'), 'do not forbid the accepted versioned beta role');
for (const file of ['AGENTS.md', 'CONTRIBUTING.md', 'docs/development/AGENT_GUIDE.md',
  'DEVELOPMENT_ROADMAP.md', 'docs/roadmap/CURRENT_STATUS.md']) {
  const text = fs.readFileSync(path.join(root, file), 'utf8');
  assert(text.includes('rc/0.9.5') && text.includes('rc/0.9.0'),
    'selected RC and retained predecessor must remain discoverable: ' + file);
  assert(/supersede|histor/i.test(text),
    'previous RC must be identified as retained history: ' + file);
}
assert(entry.includes('Publicly accepted artifact baseline is `rc/0.9.5`'));
assert(entry.includes('`dev/098-stable-preparation`') && entry.includes('`rc/0.9.8`'),
  'agent entry must point to the selected release preparation, not its historical beta');
assert(entry.includes('`v0.9.8-fix.1`') && entry.includes('immutable'),
  'corrected validation delivery must not redirect the immutable original assets');
assert(fs.readFileSync(path.join(root, 'docs/development/RC_097.md'), 'utf8')
  .includes('0.9.6 was RC-integrated,\nnot accepted or publicly released'));
assert(fs.readFileSync(path.join(root, 'docs/development/RC_096.md'), 'utf8')
  .includes('RC-integrated, not yet an accepted or published RC'));
assert(workflow.includes('accepted `rc/0.9.5` supersedes 0.9.0'));
const contributing = fs.readFileSync(path.join(root, 'CONTRIBUTING.md'), 'utf8');
assert(contributing.includes('accepted artifact baseline is `rc/0.9.5`'));
assert(contributing.includes('`beta/0.9.5` retains development history'));
assert(!contributing.includes('0.9.5 work targets `beta/0.9.5`'),
  'do not direct frozen RC fixes to the former development beta');
assert(workflow.includes('archive/python-legacy'));
assert(workflow.includes('### Release reconciliation checklist'));
assert(workflow.includes('**revoked**'), 'old automatic docs merge permission must be explicitly superseded');
assert(entry.includes('former standing authorization\n  is revoked'));
assert(guide.includes('readiness is not authorization to update `main`'));
for (const stale of ['owner grants standing merge', 'standing documentation merge authorization',
  'includes authorization to merge that documentation', 'merge it, verify the result on remote `main`']) {
  assert(![entry, guide, workflow].some(text => text.includes(stale)),
    'do not restore automatic documentation merge permission: ' + stale);
}
for (const file of ['CONTRIBUTING.md', 'docs/development/README.md', 'DEVELOPMENT_ROADMAP.md',
  'docs/roadmap/CURRENT_STATUS.md']) {
  assert(fs.readFileSync(path.join(root, file), 'utf8').includes('reconciliation'),
    'release documentation reconciliation must remain discoverable: ' + file);
}
console.log('documentation navigation: ' + links + ' local links, agent discovery and retained policy PASS');
