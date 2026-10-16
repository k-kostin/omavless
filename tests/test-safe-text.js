// SPDX-License-Identifier: MIT
const assert = require('node:assert/strict'), fs = require('node:fs'), path = require('node:path'), vm = require('node:vm');
const root = path.join(__dirname, '..'), context = vm.createContext({});
vm.runInContext(fs.readFileSync(path.join(root, 'plugin/SafeText.js'), 'utf8'), context);
let count = 0;
function test(name, run) { try { run(); count++; } catch (e) { e.message = name + ': ' + e.message; throw e; } }
test('single-line UTF8, limits and plain markup', () => {
  for (const value of ['', 'Иван', '<b>not markup</b>', 'x'.repeat(80)]) assert(context.valid(value, 80));
  for (const value of ['x'.repeat(81), 'a\nb', 'a\rb', 'a\tb', 'a\0b', '\ud800', 'a\u2028b', 'a\u2029b']) assert(!context.valid(value, 80));
  assert(!context.valid('😀'.repeat(9000), 32768));
});
test('bounded replacement, cursor and refusal without truncation', () => {
  const value = context.insertion('abc', 'И', 1, 2, 80);
  assert.equal(value.text, 'aИc'); assert.equal(value.cursor, 2);
  for (const args of [['abc','x\ny',1,2,80], ['abc','long',1,2,3], ['abc','x',-1,2,80], ['abc','x',2,1,80], ['abc','x',0,4,80]]) assert.equal(context.insertion(...args), null);
});
test('all editable fields use the shared guarded boundary', () => {
  const files = fs.readdirSync(path.join(root, 'plugin')).filter(name => name.endsWith('.qml'));
  for (const file of files) {
    const source = fs.readFileSync(path.join(root, 'plugin', file), 'utf8');
    if (file === 'SafeTextField.qml') continue;
    const instances = [...source.matchAll(/\b(TextField|TextInput|TextEdit|TextArea)\s*\{/g)];
    if (file === 'HelpPrompt.qml') {
      // This is a selectable public display, not an editable input. Unlike
      // TextInput, QQuickTextEdit's canPaste path inspects formats only; the
      // constant readOnly guard also prevents every paste/edit activation.
      assert.equal(instances.length, 1);
      assert.equal(instances[0][1], 'TextEdit');
      assert.equal((source.match(/\breadOnly\s*:/g) || []).length, 1);
      assert.match(source, /readOnly: true/);
      assert.match(source, /textFormat: TextEdit.PlainText/);
      assert.match(source, /text: help.agentPrompt/);
      assert(!/\.paste\(|clipboard-read|wl-paste|Quickshell\.clipboard|vless\./.test(source));
    } else if (file === 'SubscriptionPrompt.qml') {
      assert.equal(instances.length, 1);
      const masked = source.slice(instances[0].index, source.indexOf('SafeTextField {', instances[0].index));
      assert.match(masked, /readOnly: true/); assert.match(masked, /echoMode: TextInput.Password/);
      assert(!/readOnly:.*(?:checked|loading|focus)/.test(masked));
    } else assert.equal(instances.length, 0, file);
  }
});
test('no passive clipboard read, native menu or middle paste in shared editor', () => {
  const source = fs.readFileSync(path.join(root, 'plugin/SafeTextField.qml'), 'utf8');
  assert.match(source, /Controls.TextArea/); assert.match(source, /textFormat: TextEdit.PlainText/);
  assert.match(source, /Controls.ContextMenu.menu: editMenu/);
  assert.match(source, /enabled: field.canPaste; onTriggered: field.requestPaste\(\)/);
  assert.match(source, /event.matches\(StandardKey.Paste\)/);
  assert.match(source, /acceptedButtons: Qt.MiddleButton/);
  const executable = source.split('\n').filter(line => !line.trim().startsWith('//')).join('\n');
  assert(!/editor\.paste\(|QClipboard|Clipboard\.text|Quickshell\.clipboard/.test(executable));
  const reader = fs.readFileSync(path.join(root, 'plugin/BoundedClipboard.qml'), 'utf8');
  assert.match(reader, /command: \["\/usr\/bin\/timeout", "6s", "\/usr\/bin\/omavless", "desktop", "clipboard-read"\]/);
  assert.match(reader, /process.destroy\(\)/);
  assert(!/onCompleted|clipboardChanged|clipboard\.text|console\./.test(reader));
  assert.match(reader, /function cancel\(\) \{ generation\+\+ \}/);
});
test('explicit paste preserves late-result fences and safe error codes', () => {
  const source = fs.readFileSync(path.join(root, 'plugin/SafeTextField.qml'), 'utf8');
  for (const name of ['finishPaste']) {
    const start = source.indexOf('  function ' + name + '('), end = source.indexOf('\n  }', start) + 4;
    vm.runInContext(source.slice(start, end), context);
  }
  for (const change of [c => c.visible = false, c => c.enabled = false, c => c.readOnly = true, c => c.activeFocus = false, c => c.text = 'changed', c => c.cursorPosition = 1, c => c.selectionEnd = 1]) {
    Object.assign(context, {text:'abc', visible:true, enabled:true, readOnly:false, activeFocus:true, maximumLength:80,
      selectionStart:0, selectionEnd:3, cursorPosition:3, pasteContext:{text:'abc', start:0, end:3, cursor:3}, edits:0,
      pasteError:'', SafeText:context, textEdited:() => context.edits++});
    change(context); const before = context.text; context.finishPaste('secret-not-logged');
    assert.equal(context.text, before); assert.equal(context.edits, 0);
  }
});
console.log(`safe text: ${count} groups PASS`);
