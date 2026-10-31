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
  'docs/development/AGENT_OPERATING_WORKFLOW.md',
  'docs/roadmap/UI_UX_CONTRACT.md',
  'skills/omavless-ui-review/SKILL.md', 'skills/omavless-dev-vm/SKILL.md',
  'docs/development/POST_FIX8_RETROSPECTIVE.md',
  'docs/development/DEVELOPMENT_INVENTORY.md',
  'docs/roadmap/README.md',
  'docs/development/STABLE_DISTRIBUTION_TRANSITION.md',
  'docs/roadmap/DEVELOPMENT_WORKFLOW.md', 'docs/development/BETA_095.md',
  'docs/development/BETA_098.md', 'docs/development/RC_097.md', 'docs/development/RC_096.md', 'docs/development/RC_095.md',
  'docs/development/RC_090.md', 'docs/user/NATIVE_INSTALL.md',
  'docs/user/INSTALL.md', 'docs/user/SECURITY.md', 'docs/user/TROUBLESHOOTING.md',
  'docs/user/NATIVE_INSTALL_082_REFERENCE.md',
  'docs/user/INSTALL_082_REFERENCE.md', 'README.md',
  'docs/user/USER_AGENT_GUIDE.md', 'docs/user/NATIVE_INSTALL_RU.md', 'docs/user/CLEAN_REINSTALL_098.md'];
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
assert(panel.includes('onGuideRequested: function(section) { Qt.openUrlExternally(GuideLinks.guideUrl(root.uiLocale, root.releaseManifest ? root.releaseManifest.version : "", section)) }'));
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
// The next 0.9.9 guide uses the accepted all-state Remove UI, whose delivered
// helper is checked separately. The frozen 0.9.8 download/checksum recipe is
// still tested in test-clean-reinstall-guide.js and by the exporter; it must
// not be presented as the helper for another release.
assert(installGuide.includes('proposed **0.9.9** snapshot'));
assert(installGuide.includes(i18n.translate('remove.entry', 'en', {})));
assert(installGuide.includes('same cached helper command printed'));
assert(installGuide.includes('[0.9.8 terminal-removal reference](CLEAN_REINSTALL_098.md)'));
assert(!installGuide.includes('raw.githubusercontent.com/'),
  'the 0.9.9 UI guide must not advertise a different release checksum/helper');
assert(!quickBlocks.slice(0, 2).some(block => installGuide.includes(block)),
  'historical 0.9.8 removal remains a separate reference');
assert(installGuide.includes(quickBlocks[2]), 'reinstall and initial install must agree');
for (const text of [installGuide, assistantGuide]) {
  assert(/Restore is not\s+available in 0\.9\.8/.test(text), 'do not advertise development-only Restore');
  assert(text.includes('without a backup'), 'full removal must disclose private-data deletion');
}
// Exercise the exact copyable Bash/jq request framing. The fixed CLI leaf is
// inert; this is not another store export or installed preservation acceptance.
const preserveBlocks = [...installGuide.matchAll(/^```sh\n([\s\S]*?)\n```$/gm)]
  .map(match => match[1]).filter(block => block.includes('store-preserve-original'));
assert.equal(preserveBlocks.length, 1);
const preserveBlock = preserveBlocks[0];
assert(preserveBlock.startsWith("bash -c '\n") && preserveBlock.endsWith("\n'"));
const preserveSyntax = spawnSync('bash', ['-n'], { input: preserveBlock, encoding: 'utf8', timeout: 2000 });
assert.equal(preserveSyntax.status, 0, 'qualify the complete guide block before executing it');
const inertPreserve = 'omavless() { [[ "$#" == 1 && "$1" == store-preserve-original ]] || return 9; '
  + 'jq -e \'keys == ["confirmUnencrypted", "destination"] and '
  + '.destination == "/home/example/private/original-profiles.json" and '
  + '.confirmUnencrypted == true\' >/dev/null || return 10; '
  + 'printf REQUEST_ADMITTED; return "$OV_GUIDE_TEST_EXIT"; }; export -f omavless;\n'
  + 'omarchy() { return 99; }; sudo() { return 99; }; pacman() { return 99; }; curl() { return 99; }; '
  + 'export -f omarchy sudo pacman curl;\n';
for (const [input, exit, expected] of [
  ['/home/example/private/original-profiles.json\n', '0', 0],
  ['/home/example/private/original-profiles.json\n', '73', 73],
  ['\n', '0', 2],
]) {
  const result = spawnSync('bash', ['--noprofile', '--norc', '-c', inertPreserve + preserveBlock], {
    input, env: { PATH: '/usr/bin:/bin', OV_GUIDE_TEST_EXIT: exit },
    encoding: 'utf8', timeout: 2000,
  });
  assert.equal(result.status, expected, 'preservation guide must retain the original CLI result');
  assert.equal(result.stdout.includes('REQUEST_ADMITTED'), expected !== 2);
  assert(!result.stdout.includes('/home/example/') && !result.stderr.includes('/home/example/'));
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
// October21 process regression: routing/content controls, NOT a simulation of
// agent decisions, OS authorization, installed acceptance or improved CI time.
const operating = fs.readFileSync(path.join(root, 'docs/development/AGENT_OPERATING_WORKFLOW.md'), 'utf8');
const operatingRules = [
  'do not ask for it again',
  'request only the missing input once',
  'compaction or unchanged heartbeat is not revocation',
  'Agent-entered authorization is not physical-human acceptance',
  'service active AND enabled state',
  'Separate files-installed proof from distinct loaded-feature proof',
  'Exclude ONLY the demonstrated volatile field',
  'reject every other changed field',
  'old comparison RED and the corrected one GREEN',
  'Freeze one user-visible journey and its complete kit',
  'Do not automatically commission another preparatory component',
  'A collection of component passes is not whole-kit qualification',
  'Batch compatible scenarios on one unchanged package pair',
  'implementation, harness, build/CI, VM, rework and waiting',
  'Check critical input availability before harness implementation or delegation',
  'Unimplemented tooling is agent-owned work, not an external blocker',
  'A held actor blocks only actions with a demonstrated dependency',
  'Keep one cumulative preparation clock for the same user outcome',
  'Assign the complete missing link, not another prerequisite proposal',
  'Trace each proposed harness guarantee to an existing contract or measured failure',
  'scheduler ACTIVE is not feature development ACTIVE',
  'A no-defect assessment closes only that assessment, not the feature backlog',
  'Do not require a pre-existing bug to implement an approved missing capability',
  'Before entering quiet wait, classify the remaining user outcomes in the authorized source scope',
  'bool-versus-int receipts',
  'Aggregate success follows original child/server EXIT0',
  'both the product child outcome and the harness qualification separately',
  'Preserve UNKNOWN',
  'ALL selected-tuple consumers',
  'hosted runner discovers new tests',
  'Stop polling a completed gate',
  'independent exact merged-push checks',
  '### Early feedback and timings',
  'not a hosted speedup',
  '### Hosted CI completion',
  'ends only this read-only observer',
  'CLI exit success alone does not admit a gate',
  'unexpected head/event/attempt',
  'Record actual completion-to-observation lag',
  'normally within 25 lines',
  'do no VM probes, closed-CI replay or memory imports',
  'Pause only the completed continuation',
  'queued is not started',
  'last material progress time',
  'two consecutive observations show no progress, or after 30 minutes',
  'review checkpoints, NOT operation timeouts',
  'resource/dependency map',
  'block only actions whose safety depends on that outcome',
  'A shared Git directory can be a real dependency',
  'remaining authority must come from the owner, not this triage',
  'missing status/completion API is a concrete tool blocker',
  'Report once when productive work stops',
  'scheduled observation is not development progress',
  'plain-language outcome, last actual change and time',
  'GitHub branches, local branches and working folders separately',
];
function requireOperatingWorkflow(text, rootEntry, agentGuide, policy) {
  const normalized = text.replace(/\s+/g, ' ');
  for (const rule of operatingRules)
    assert(normalized.includes(rule), 'retain operating rule: ' + rule);
  assert(rootEntry.includes('](docs/development/AGENT_OPERATING_WORKFLOW.md)'), 'operating root route');
  assert(agentGuide.includes('](AGENT_OPERATING_WORKFLOW.md)'), 'operating guide route');
  assert(policy.includes('](AGENT_OPERATING_WORKFLOW.md)'), 'operating policy route');
  for (let step = 1; step <= 7; step++)
    assert(new RegExp('^## ' + step + '\\. ', 'm').test(text), 'operating step ' + step);
}
const operatingPolicy = fs.readFileSync(path.join(root, 'docs/development/EXECUTION_POLICY.md'), 'utf8');
requireOperatingWorkflow(operating, entry, guide, operatingPolicy);
assert(anchors(operating).has('stalled-operation-triage'), 'stalled-operation triage anchor');
assert(guide.includes('](AGENT_OPERATING_WORKFLOW.md#stalled-operation-triage)'),
  'stalled-operation triage must be discoverable');
assert.throws(() => assert(guide.replace('#stalled-operation-triage', '#missing-triage')
  .includes('](AGENT_OPERATING_WORKFLOW.md#stalled-operation-triage)'),
  'stalled-operation triage must be discoverable'), /stalled-operation triage/);
for (const rule of operatingRules)
  assert.throws(() => requireOperatingWorkflow(
    operating.replace(/\s+/g, ' ').replace(rule, '[removed]'), entry, guide, operatingPolicy),
  /retain operating rule/);
assert.throws(() => requireOperatingWorkflow(operating,
  entry.replace('AGENT_OPERATING_WORKFLOW.md', 'missing.md'), guide, operatingPolicy), /root route/);
assert.throws(() => requireOperatingWorkflow(operating, entry,
  guide.replace('AGENT_OPERATING_WORKFLOW.md', 'missing.md'), operatingPolicy), /guide route/);
assert.throws(() => requireOperatingWorkflow(operating, entry, guide,
  operatingPolicy.replace('AGENT_OPERATING_WORKFLOW.md', 'missing.md')), /policy route/);
// Mandatory discovery/content, not proof of an agent's actual visual judgment.
const uiContract = fs.readFileSync(path.join(root, 'docs/roadmap/UI_UX_CONTRACT.md'), 'utf8');
const uiReviewSkill = fs.readFileSync(path.join(root, 'skills/omavless-ui-review/SKILL.md'), 'utf8');
function requireAffectedUiGate(contract, agentGuide, skill) {
  const normalized = contract.replace(/\s+/g, ' ');
  for (const rule of ['### Premerge affected-surface UX gate',
    'all controls on the affected surface', 'not only changed labels',
    'primary, secondary, navigation and recovery roles',
    'visibility, enabled state, target and actual handler',
    'Known relevant UX findings block merge',
    'unless the owner explicitly defers them',
    'Text legibility alone is not final UX acceptance']) {
    assert(normalized.includes(rule), 'retain mandatory affected-surface UX gate: ' + rule);
  }
  assert(agentGuide.includes('](../roadmap/UI_UX_CONTRACT.md#premerge-affected-surface-ux-gate)'),
    'mandatory agent guide must discover the premerge UX gate');
  assert(skill.includes('](../../docs/roadmap/UI_UX_CONTRACT.md)'),
    'existing UI-review skill must route to the canonical contract');
}
requireAffectedUiGate(uiContract, guide, uiReviewSkill);
assert.throws(() => requireAffectedUiGate(
  uiContract.replace('### Premerge affected-surface UX gate', '### Removed gate'), guide, uiReviewSkill),
  /mandatory affected-surface UX gate/);
assert.throws(() => requireAffectedUiGate(uiContract,
  guide.replace('#premerge-affected-surface-ux-gate', '#removed-gate'), uiReviewSkill),
  /discover the premerge UX gate/);
// Role/discovery regressions only: no screenshot or installed acceptance is
// inferred from the existence of these rules.
function requireControlGrammar(contract, agentGuide, skill) {
  assert(anchors(contract).has('control-roles-and-visual-grammar'), 'missing control grammar anchor');
  for (const source of [agentGuide, skill]) {
    assert(source.includes('UI_UX_CONTRACT.md#control-roles-and-visual-grammar)'),
      'control grammar must be discoverable before composition');
  }
  const grammar = contract.split('## Control roles and visual grammar')[1]
    .split('### Explicit exit during connection recovery')[0].replace(/\s+/g, ' ');
  for (const rule of ['Its effect chooses the affordance',
    'Priority chooses emphasis within that affordance',
    '| Command/action | Bordered button; accent when primary, neutral when secondary.',
    '| In-product navigation | Bordered button, neutral by default, no external-link marker.',
    '| External navigation | A visibly identifiable link ending in `↗`',
    '| Disclosure |', '| Value/selection control |', '| Destructive or recovery action |',
    '| Information |', '### Concrete Help decision',
    'Setup help and Settings Help are in-product navigation',
    'both the handler and its visual role',
    'visible at rest', 'No hit area or keyboard stop',
    'does not mean its referenced UI has already been fixed or inspected']) {
    assert(grammar.includes(rule), 'retain control role rule: ' + rule);
  }
}
requireControlGrammar(uiContract, guide, uiReviewSkill);
assert.throws(() => requireControlGrammar(uiContract.replace(
  '| In-product navigation | Bordered button', '| In-product navigation | Plain text'),
guide, uiReviewSkill), /retain control role rule/);
assert.throws(() => requireControlGrammar(uiContract, guide,
  uiReviewSkill.replace('#control-roles-and-visual-grammar', '#missing-grammar')),
  /discoverable before composition/);
// Repository-role/discovery guards, not proof of an agent following the rules.
const sourceMap = fs.readFileSync(path.join(root, 'docs/development/STABLE_DISTRIBUTION_TRANSITION.md'), 'utf8');
const currentMap = sourceMap.split('## Current source composition')[0];
assert(currentMap.includes('## Current map — after fix.8 delivery'));
assert(currentMap.includes('bcd744ecb3c1c61bbfe4a8ab090358b225ce1e76'));
assert(currentMap.includes('5edf4eb203926d8bd39a3ff80f611a19435c3787'));
assert(!currentMap.includes('product tree has not been migrated'));
assert(!guide.includes('is preparation, not a deployed layout'));
assert(guide.includes('](POST_FIX8_RETROSPECTIVE.md)'));
assert(entry.includes('Developer rules and retrospectives stay in full source'));
assert(entry.includes('](docs/development/DEVELOPMENT_INVENTORY.md)'));
const developmentInventory = fs.readFileSync(path.join(root, 'docs/development/DEVELOPMENT_INVENTORY.md'), 'utf8');
// Clone-only discovery, not proof that an agent read the ledger or that its
// dated statuses remain current. Do not duplicate the individual disposition table.
function requireCurrentBacklogRoute(agentEntry, agentGuide, inventory) {
  assert(agentEntry.includes('](docs/development/DEVELOPMENT_INVENTORY.md)'),
    'root entry must discover the backlog inventory');
  assert(agentGuide.includes('](DEVELOPMENT_INVENTORY.md)'),
    'mandatory guide must discover the backlog inventory');
  const current = inventory.split('## Current backlog (October 21, 2026)')[1]?.split('## New-base queue')[0];
  assert(current, 'inventory must identify the dated current backlog');
  assert(current.includes('https://github.com/k-kostin/omavless/issues/553#issuecomment-6312681149'),
    'inventory must reach the exact individual disposition ledger');
  for (const boundary of ['7932893eff9a3184ce4cc65c8e61045be9947174',
    'Refresh actual GitHub heads, bases and states', 'not an active task',
    'not implementation or merge authority']) {
    assert(current.replace(/\s+/g, ' ').includes(boundary),
      'retain dated-backlog boundary: ' + boundary);
  }
  assert(inventory.split('## New-base queue')[1].split('## Retained track entries')[0]
    .includes('](#current-backlog-october-21-2026)'),
  'older queue entry points must reach the current backlog before the historical map');
}
requireCurrentBacklogRoute(entry, guide, developmentInventory);
for (const [agentEntry, agentGuide, inventory] of [
  [entry.replace('](docs/development/DEVELOPMENT_INVENTORY.md)', '](removed.md)'), guide, developmentInventory],
  [entry, guide.replace('](DEVELOPMENT_INVENTORY.md)', '](removed.md)'), developmentInventory],
  [entry, guide, developmentInventory.replace('## Current backlog (October 21, 2026)', '## Removed backlog')],
  [entry, guide, developmentInventory.replace('issuecomment-6312681149', 'issuecomment-removed')],
  [entry, guide, developmentInventory.replace('](#current-backlog-october-21-2026)', '](#removed-backlog)')],
  [entry, guide, developmentInventory.replace('not implementation or merge authority', 'automatic merge authority')],
]) {
  assert.throws(() => requireCurrentBacklogRoute(agentEntry, agentGuide, inventory),
    /backlog|ledger/);
}
assert(developmentInventory.includes('## Admission of old-base work'));
assert(developmentInventory.includes('## Source CI before the next export'));
assert(developmentInventory.includes('## Verified missing-port boundaries'));
const missingPorts = developmentInventory.split('## Verified missing-port boundaries')[1]
  .split('## Admission of old-base work')[0];
// Discovery guard only: these assertions do not establish code incorporation.
for (const carrier of ['#517', '#520', '#550', '#572', '#659', '#751', '#546', '#684', '#676']) {
  assert(missingPorts.includes(carrier), 'retain independently missing port: ' + carrier);
}
assert(missingPorts.includes('absent'), 'similar concepts must not imply incorporation');
assert(missingPorts.includes('not implementation authority'));
assert(developmentInventory.includes('not a demonstrated merge conflict'));
// Current developer navigation must not turn a historical migration recipe
// or old beta into today's product/base. These are routing/content controls,
// not acceptance of any future feature or proof of agent behavior.
const roadmapIndex = fs.readFileSync(path.join(root, 'docs/roadmap/README.md'), 'utf8');
assert(!roadmapIndex.includes('Current published 0.7.0 remains'),
  'roadmap entry must not advertise the historical Python product as current');
assert(roadmapIndex.includes('Current published 0.9.8'));
assert(roadmapIndex.includes('DEVELOPMENT_INVENTORY.md#new-base-queue'));
assert(developmentInventory.includes('## New-base queue'));
assert(developmentInventory.includes('### Open CI performance and assurance decision'));
assert(developmentInventory.includes('EXECUTION_POLICY.md#technical-judgment-before-execution'));
const ciDecision = developmentInventory.split('### Open CI performance and assurance decision')[1]
  .split('## Retained track entries')[0].replace(/\s+/g, ' ');
for (const boundary of ['not a selected replacement policy', 'different feature configurations',
  'actually executed workflow', 'does not waive current gates',
  '#### Current recommendation', 'retain full PR and exact development push checks',
  'not evidence of a full hosted speedup', 'not adopted']) {
  assert(ciDecision.includes(boundary), 'retain unresolved CI decision boundary: ' + boundary);
}
const nextQueue = developmentInventory.split('## New-base queue')[1].split('## Retained track entries')[0];
for (const boundary of ['consumer Restore', 'not a whole-beta merge',
  'owner-paused', 'private fixtures', 'Backup-only', 'not feature implementation']) {
  assert(nextQueue.includes(boundary), 'retain new-base scope boundary: ' + boundary);
}
for (const file of ['DEVELOPMENT_ROADMAP.md', 'docs/roadmap/CURRENT_STATUS.md']) {
  assert(fs.readFileSync(path.join(root, file), 'utf8').includes('DEVELOPMENT_INVENTORY.md#new-base-queue'),
    'current developer front doors must reach the one queue: ' + file);
}
const retro = fs.readFileSync(path.join(root, 'docs/development/POST_FIX8_RETROSPECTIVE.md'), 'utf8');
for (const evidence of ['test-uninstaller.js', 'test-native-onboarding.js',
  'test_rust_ci_isolation.py', 'waived, not earned', 'not yet integrated']) {
  assert(retro.replace(/\s+/g, ' ').includes(evidence), 'retain bounded retro evidence/disposition: ' + evidence);
}
// Discovery/retention guard only: this cannot prove that a particular retro
// was implemented or that an agent followed its required checks.
const execution = fs.readFileSync(path.join(root, 'docs/development/EXECUTION_POLICY.md'), 'utf8');
const vmSkill = fs.readFileSync(path.join(root, 'skills/omavless-dev-vm/SKILL.md'), 'utf8');
function requireVmLifetime(policy, skill) {
  assert(anchors(policy).has('vm-session-lifetime'), 'missing VM lifetime anchor');
  assert(skill.includes('EXECUTION_POLICY.md#vm-session-lifetime)'), 'VM skill must route lifetime policy');
  const lifetime = policy.split('### VM session lifetime')[1]
    .split('Cleanup of a known owned completed fixture')[0].replace(/\s+/g, ' ');
  for (const rule of ['no automatic wall-clock VM termination by default',
    'bounded SSH/API/capture/test commands', 'authentication and product confirmation deadlines',
    'Fault-injection actors', 'obtain a separate owner decision before launch',
    'future launch design only', 'terminated/sealed fixture',
    'single-operator and recovery boundaries remain unchanged']) {
    assert(lifetime.includes(rule), 'retain VM lifetime boundary: ' + rule);
  }
}
requireVmLifetime(execution, vmSkill);
assert.throws(() => requireVmLifetime(execution.replace(
  'no automatic wall-clock VM termination by default', 'kill the VM after four hours'), vmSkill),
  /retain VM lifetime boundary/);
assert.throws(() => requireVmLifetime(execution,
  vmSkill.replace('#vm-session-lifetime', '#missing-lifetime')), /route lifetime policy/);
// Protect mandatory discovery/content, not proof of an agent's judgment.
assert(guide.includes('](EXECUTION_POLICY.md#technical-judgment-before-execution)'));
assert(execution.includes('### Technical judgment before execution'));
const judgment = execution.split('### Technical judgment before execution')[1].split('## 1.')[0]
  .replace(/\s+/g, ' ');
for (const rule of ['proposed means, not automatically the right technical decision',
  'Distinguish a question or hypothesis from an explicit instruction',
  'explain the conflict, material cost, risks and practical alternatives',
  'Do not silently replace a clear owner instruction',
  'Mandatory exact-source CI/export/installed gates remain separate']) {
  assert(judgment.includes(rule), 'retain engineering judgment boundary: ' + rule);
}
assert(judgment.includes('#### Local and hosted check allocation'));
for (const rule of ['Focused local iteration', 'Full hosted integration gate',
  'Purposeful full local run', 'not an automatic duplicate',
  'actual selected compiler version', 'original run/attempt']) {
  assert(judgment.includes(rule), 'retain deliberate local/hosted allocation: ' + rule);
}
// Discovery/retention only, not evidence that preflights happened or CI sped up.
const retention = fs.readFileSync(path.join(root, 'docs/development/README.md'), 'utf8');
assert(guide.includes('](README.md#compact-operational-checkpoints)'),
  'compact current-state and fixture workflow must be discoverable');
for (const rule of ['### Compact operational checkpoints', 'complete original bytes',
  'historical ACTIVE/PENDING', 'packaging/release/source-fixture.py', 'does not import candidate code']) {
  assert(retention.includes(rule), 'retain operational checkpoint boundary: ' + rule);
}
assert(execution.includes('synthetic credentials and inert transports'));
assert(execution.includes('without\ntrimming, extracting a token or falling back'));
assert(guide.includes('](EXECUTION_POLICY.md#producer-and-final-integration-checkpoints)'),
  'native producer/final checkpoint procedure must be discoverable');
assert(judgment.includes('#### Producer and final integration checkpoints'));
for (const rule of ['first meaningful Draft checkpoint',
  'whole affected surface', 'actual produced packages',
  'matching metadata successor', 'actual candidate export',
  'does not suppress automatic Draft checks',
  'reason for each successor', 'compilation from test execution']) {
  assert(judgment.includes(rule), 'retain measured checkpoint planning: ' + rule);
}
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
assert(workflow.includes('Never retarget an old full-source PR to stripped user main'));
assert(workflow.includes('Reachable branches with\nno open head/base PR are review candidates'));
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
assert(entry.includes('Historical accepted artifact baseline is `rc/0.9.5`'));
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
console.log('documentation navigation: ' + links + ' local links, ' + operatingRules.length +
  ' operating-rule removal controls, agent discovery and retained policy PASS');
