// SPDX-License-Identifier: MIT
// Read-only guard for the small agent entry point and preserved detailed docs.
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const root = path.resolve(__dirname, '..');
const files = ['AGENTS.md', 'CONTRIBUTING.md', 'docs/README.md',
  'docs/development/README.md', 'docs/development/AGENT_GUIDE.md',
  'docs/development/EXECUTION_POLICY.md',
  'docs/development/STABLE_DISTRIBUTION_TRANSITION.md',
  'docs/roadmap/DEVELOPMENT_WORKFLOW.md', 'docs/development/BETA_095.md',
  'docs/development/BETA_098.md', 'docs/development/RC_097.md', 'docs/development/RC_096.md', 'docs/development/RC_095.md',
  'docs/development/RC_090.md', 'docs/user/NATIVE_INSTALL.md',
  'docs/user/INSTALL.md', 'docs/user/SECURITY.md', 'docs/user/TROUBLESHOOTING.md',
  'docs/user/NATIVE_INSTALL_082_REFERENCE.md',
  'docs/user/INSTALL_082_REFERENCE.md', 'README.md',
  'docs/user/USER_AGENT_GUIDE.md', 'docs/user/CLEAN_REINSTALL_098.md'];
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
// All current setup entry points must address the same current user task.
// This protects navigation; it is not installed rendering/OS acceptance.
const setupGuide = 'https://github.com/k-kostin/omavless/blob/main/docs/user/NATIVE_INSTALL.md';
const setupPage = fs.readFileSync(path.join(root, 'plugin/SetupPage.qml'), 'utf8');
const panel = fs.readFileSync(path.join(root, 'plugin/Panel.qml'), 'utf8');
assert(setupPage.includes('function guide() { helpRequested(SetupState.hasUpdate(facts) ? "updates" : "install") }'));
assert(panel.includes('onSetupGuideRequested: root.openHelp("install", onboardingWizard)'));
assert(panel.includes('onGuideRequested: function(section) { Qt.openUrlExternally("' + setupGuide + '#" + section) }'));
const installGuide = fs.readFileSync(path.join(root, 'docs/user/NATIVE_INSTALL.md'), 'utf8');
const troubleshooting = fs.readFileSync(path.join(root, 'docs/user/TROUBLESHOOTING.md'), 'utf8');
const security = fs.readFileSync(path.join(root, 'docs/user/SECURITY.md'), 'utf8');
for (const text of [troubleshooting, security]) {
  assert(!text.includes('uninstall.sh --purge'), 'current help must not advertise the legacy remover');
  assert(text.includes('CLEAN_REINSTALL_098.md'), 'current removal help must reach the reviewed helper');
}
assert(!troubleshooting.includes('omavless.service'), 'current help must not use the retired Python service');
assert(!troubleshooting.includes('systemctl --user stop'), 'do not replace recovery with an unverified service stop');
assert(troubleshooting.includes('Support report'), 'start support with the bounded current report');
for (const anchor of ['guided-first-run', 'find-your-next-step', 'continue-dns-setup',
  'existing-dns-installations', 'when-setup-stops', 'updates', 'verify-the-installation',
  'updates-close-quit-and-removal', 'optional-helpers-firewall-and-older-installations',
  'set-up-update-or-reopen-omavless', 'install', 'connect-and-disconnect', 'remove', 'help',
  'updating-the-public-082-application', 'already-using-098']) {
  const headings = [...installGuide.matchAll(/^#+ (.+)$/gm)].map(match =>
    match[1].toLowerCase().replace(/[^\w\s-]/g, '').replace(/\s/g, '-'));
  const explicit = [...installGuide.matchAll(/<a id="([a-z0-9-]+)"><\/a>/g)].map(match => match[1]);
  assert(headings.includes(anchor) || explicit.includes(anchor), 'missing current setup destination: ' + anchor);
}
assert(/^## Set up DNS helper$/m.test(installGuide));
assert(installGuide.includes('<a id="guided-first-run--release-preparation"></a>'),
  'historical first-run links must retain their destination');
for (const file of ['NATIVE_INSTALL_082_REFERENCE.md', 'INSTALL_082_REFERENCE.md']) {
  assert(fs.readFileSync(path.join(root, 'docs/user', file), 'utf8').includes('NATIVE_INSTALL.md'),
    'historical instructions must point back to current setup help');
}
assert(!installGuide.includes('omarchy pkg aur add mihomo-bin'),
  'do not offer stock core installation as the current managed-pair route');
const assistantGuide = fs.readFileSync(path.join(root, 'docs/user/USER_AGENT_GUIDE.md'), 'utf8');
// Discoverability/content guards, not proof of a future assistant's behavior.
assert(assistantGuide.includes('## Report a bug or contribute a fix'));
const upstreamHelp = assistantGuide.split('## Report a bug or contribute a fix')[1].replace(/\s+/g, ' ');
for (const rule of ['Search the repository', 'https://github.com/k-kostin/omavless/issues',
  'https://github.com/k-kostin/omavless/pulls', 'titles and descriptions in English', 'separate full-source checkout',
  'Agree with the user before preparing a code contribution',
  'AGENTS.md and CONTRIBUTING.md', 'Obtain explicit user approval',
  'permission to publish', 'copyable draft', 'unresolved, worked around or verified fixed',
  'An open PR is not a shipped fix']) {
  assert(upstreamHelp.includes(rule), 'retain safe upstream support handoff: ' + rule);
}
const sourceEntryTemplate = fs.readFileSync(path.join(root, 'packaging/release/distribution/CONTRIBUTING.md'), 'utf8');
assert(sourceEntryTemplate.includes('USER_AGENT_GUIDE.md#report-a-bug-or-contribute-a-fix'));
const guideLanguages = fs.readFileSync(path.join(root, 'docs/roadmap/I18N.md'), 'utf8');
for (const rule of ['### Published guide languages', 'Agent instructions remain English',
  "plugin's effective UI locale", 'not part of the current fix.8 implementation',
  'Fall back to the matching English guide']) {
  assert(guideLanguages.replace(/\s+/g, ' ').includes(rule), 'retain guide-language boundary: ' + rule);
}
const quickRemoval = fs.readFileSync(path.join(root, 'docs/user/CLEAN_REINSTALL_098.md'), 'utf8');
const readme = fs.readFileSync(path.join(root, 'README.md'), 'utf8');
for (const text of [installGuide, readme]) {
  assert(text.includes('USER_AGENT_GUIDE.md'), 'the end-user assistant guide must be discoverable');
}
for (const text of [installGuide, assistantGuide, readme]) {
  assert(!text.includes('Save report'), 'support uses the actual Save file label');
  assert(!text.includes('After integration'), 'user instructions are not an agent handoff');
}
const i18n = require('../plugin/I18n.js');
for (const key of ['setup.install_all', 'setup.update_components', 'setup.prepare_update',
  'setup.resume_dns', 'setup.start_broker', 'setup.restore_enrollment', 'setup.start_app',
  'setup.terminal_closed', 'setup.check', 'native.support.copy', 'native.support.save',
  'native.quit.title', 'native.app.open']) {
  const label = i18n.translate(key, 'en', {});
  assert(!label.includes('Missing translation') && installGuide.includes(label),
    'guide must name the actual supported control: ' + key);
}
const quickBlocks = [...quickRemoval.matchAll(/```bash\n([\s\S]*?)\n```/g)].map(m => m[1].trim());
const installBlocks = [...installGuide.matchAll(/```bash\n([\s\S]*?)\n\s*```/g)].map(m => m[1].trim());
for (const block of quickBlocks.slice(0, 2)) {
  assert(installBlocks.includes(block),
    'unified and short guides must use the same checked remover and continuation');
}
assert(installGuide.includes(quickBlocks[2]), 'reinstall and initial install must agree');
for (const text of [installGuide, assistantGuide]) {
  assert(/Restore is not\s+available in 0\.9\.8/.test(text), 'do not advertise development-only Restore');
  assert(text.includes('without a backup'), 'full removal must disclose private-data deletion');
}
// Check links/anchors on the affected user entry points, including in-page links.
// This is navigation/content coverage, not installed or visual acceptance.
function anchors(text) {
  return new Set([
    ...[...text.matchAll(/^#+ (.+)$/gm)].map(m =>
      m[1].toLowerCase().replace(/[^\w\s-]/g, '').replace(/\s/g, '-')),
    ...[...text.matchAll(/<a id="([a-z0-9-]+)"><\/a>/g)].map(m => m[1]),
  ]);
}
for (const file of ['README.md', 'docs/user/NATIVE_INSTALL.md',
  'docs/user/USER_AGENT_GUIDE.md', 'docs/user/CLEAN_REINSTALL_098.md']) {
  const text = fs.readFileSync(path.join(root, file), 'utf8');
  for (const match of text.matchAll(/\]\(([^\s)]+)\)/g)) {
    if (/^[a-z][a-z0-9+.-]*:/i.test(match[1])) continue;
    const [target, anchor] = match[1].split('#');
    const resolved = target ? path.resolve(root, path.dirname(file), target) : path.join(root, file);
    if (anchor && resolved.endsWith('.md')) {
      assert(anchors(fs.readFileSync(resolved, 'utf8')).has(anchor),
        'missing help anchor from ' + file + ': ' + match[1]);
    }
  }
  for (const block of text.matchAll(/```(?:bash|sh)\n([\s\S]*?)\n\s*```/g)) {
    const syntax = spawnSync('bash', ['-n'], { input: block[1], encoding: 'utf8', timeout: 2000 });
    assert.equal(syntax.status, 0, file + ': ' + syntax.stderr);
  }
}
const entry = fs.readFileSync(path.join(root, 'AGENTS.md'), 'utf8');
assert(entry.split('\n').length <= 80, 'root AGENTS must remain a concise entry point');
const publicGuidePolicy = path.join(root, 'docs/development/README.md');
assert(fs.readFileSync(publicGuidePolicy, 'utf8').includes('## Public guides follow main'));
assert(fs.readFileSync(publicGuidePolicy, 'utf8').includes('### Acceptance reports stay current'),
  'retained acceptance reports need a current summary distinct from historical attempts');
for (const file of ['AGENTS.md', 'docs/development/AGENT_GUIDE.md',
  'docs/roadmap/DEVELOPMENT_WORKFLOW.md']) {
  const text = fs.readFileSync(path.join(root, file), 'utf8');
  assert([...text.matchAll(/\]\(([^\s)]+)\)/g)].some(match => {
    const [target, anchor] = match[1].split('#');
    return anchor === 'public-guides-follow-main' &&
      path.resolve(root, path.dirname(file), target) === publicGuidePolicy;
  }), 'current public-guide contract must be discoverable: ' + file);
}
for (const target of ['docs/development/AGENT_GUIDE.md', 'DEVELOPMENT_ROADMAP.md',
  'docs/roadmap/CURRENT_STATUS.md', 'docs/roadmap/DEVELOPMENT_WORKFLOW.md',
  'docs/roadmap/ACCEPTANCE_ENVIRONMENTS.md', 'docs/roadmap/RUST_MIGRATION.md',
  'skills/omavless-ui-review/SKILL.md', 'skills/omavless-localization/SKILL.md']) {
  assert(entry.includes('](' + target + ')'), 'agent discovery must retain ' + target);
}
const guide = fs.readFileSync(path.join(root, 'docs/development/AGENT_GUIDE.md'), 'utf8');
// Discovery/retention guard only: this cannot prove that a particular retro
// was implemented or that an agent followed its required checks.
const execution = fs.readFileSync(path.join(root, 'docs/development/EXECUTION_POLICY.md'), 'utf8');
assert(entry.includes('](docs/development/EXECUTION_POLICY.md)'),
  'execution rules must remain mandatory from the root entry');
assert(guide.includes('](EXECUTION_POLICY.md#retrospective-closure-and-distribution)'),
  'retro closure/distribution must be discoverable before older-branch work');
assert(execution.includes('### Retrospective closure and distribution'));
const compactExecution = execution.replace(/\s+/g, ' ');
for (const rule of ['update the applicable project rule, contract or',
  'add a regression or tooling correction where practical',
  'Distinguish analysis complete, implemented/checked, and integrated',
  'mark distribution pending', 'check current remote main']) {
  assert(compactExecution.includes(rule), 'retain the retro closure rule: ' + rule);
}
// Discovery only: assertions cannot prove that an agent followed the procedure.
const closurePolicy = fs.readFileSync(path.join(root, 'docs/development/EXECUTION_POLICY.md'), 'utf8');
assert(guide.includes('](EXECUTION_POLICY.md#delivery-closure-check)'),
  'the whole-task closure check must be discoverable from the mandatory guide');
assert(closurePolicy.includes('### Delivery closure check'));
assert(closurePolicy.includes('](#delivery-closure-check)'),
  'retro closure must check the next real outcome against the closure procedure');
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
