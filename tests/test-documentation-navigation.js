// SPDX-License-Identifier: MIT
// Read-only guard for the small agent entry point and preserved detailed docs.
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const root = path.resolve(__dirname, '..');
const files = ['AGENTS.md', 'CONTRIBUTING.md', 'docs/README.md',
  'docs/development/README.md', 'docs/development/AGENT_GUIDE.md',
  'docs/roadmap/DEVELOPMENT_WORKFLOW.md'];
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
assert(workflow.includes('archive/python-legacy'));
console.log('documentation navigation: ' + links + ' local links, agent discovery and retained policy PASS');
