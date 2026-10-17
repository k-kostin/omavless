// SPDX-License-Identifier: MIT
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const root = path.join(__dirname, '..');

function executable(source) {
  return source.replace(/\/\*[\s\S]*?\*\//g, '').replace(/^\s*\/\/.*$/gm, '');
}
function check(source, file) {
  const code = executable(source);
  assert(!/(?<![\w.])Color\s*\./.test(code), `${file}: ambiguous QtQuick/Commons Color`);
  if (/\bCommons\.Color\s*\./.test(code)) {
    assert.match(code, /^import qs\.Commons as Commons\s*$/m, `${file}: explicit Commons namespace`);
  }
}

// Reject a reintroduced collision, even beside a correct qualified use/import.
assert.throws(() => check('import QtQuick\nimport qs.Commons as Commons\ncolor: Color.foreground', 'negative'));
assert.throws(() => check('color: Commons.Color.foreground', 'missing import'));
check('import QtQuick\nimport qs.Commons as Commons\ncolor: Commons.Color.foreground', 'qualified');
check('// Color.foreground is deliberately only a comment', 'comment');

const directories = ['plugin', 'tests/setup-visual', 'tests/help-visual', 'tests/onboarding-visual'];
let checked = 0, themed = 0;
for (const directory of directories) {
  for (const name of fs.readdirSync(path.join(root, directory)).filter(name => name.endsWith('.qml'))) {
    const file = `${directory}/${name}`;
    const source = fs.readFileSync(path.join(root, file), 'utf8');
    check(source, file);
    checked++;
    if (/\bCommons\.Color\./.test(executable(source))) themed++;
  }
}
assert(checked > 0 && themed > 0, 'nonempty actual frontend/theme inventory');
console.log(`QML Color namespace: ${checked} files / ${themed} themed files and negative controls PASS`);
