#!/usr/bin/env node
// Real guided shell functions, disposable user-owned state and inert commands.
// No sudo, real service/package operations, reboot or production paths.
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');
const {spawnSync} = require('node:child_process');
const source = fs.readFileSync(path.join(__dirname, '../tools/clean-reinstall-098.sh'), 'utf8');
const functions = source.slice(source.indexOf('fail()'), source.indexOf('\n# CLI entry point\n'));
const parent = path.join(os.homedir(), '.cache/ovt');
for (const directory of [path.dirname(parent), parent]) {
  if (!fs.existsSync(directory)) fs.mkdirSync(directory, {mode:0o700});
  const metadata = fs.lstatSync(directory);
  assert(metadata.isDirectory() && !metadata.isSymbolicLink());
  assert.equal(metadata.uid, process.getuid());
  assert.equal(metadata.mode & 0o022, 0);
}
const helperHash = 'a'.repeat(64);
const boot = '11111111-1111-1111-1111-111111111111';
const otherBoot = '22222222-2222-2222-2222-222222222222';
const record = (changes={}) => ({schema:1, uid:process.getuid(), helper:helperHash, boot, lane:'never_connected', phase:'prepared', ...changes});
let cases = 0;
function test(label, options, body, expected) {
  const directory = fs.mkdtempSync(path.join(parent, 'guided-removal.'));
  const filename = path.join(directory, 'session.json');
  if (options.record !== undefined) fs.writeFileSync(filename, options.record, {mode:options.mode || 0o600});
  if (options.symlink) {
    fs.writeFileSync(filename + '.target', JSON.stringify(record()), {mode:0o600});
    fs.symlinkSync(filename + '.target', filename);
  }
  if (options.hardlink) fs.linkSync(filename, filename + '.link');
  const script = `set -euo pipefail
umask 077
${functions}
guided=true
spinner='|/-\\'
helper_path=/synthetic/reviewed-helper
helper_hash=${helperHash}
boot_id=${boot}
stage_directory=${JSON.stringify(directory)}
stage_file=${JSON.stringify(filename)}
systemctl() { printf 'FORBIDDEN SYSTEMCTL:%s\\n' "$*" >&2; return 9; }
sync() { ${options.syncFailure ? 'return 1' : 'return 0'}; }
${body}
`;
  try {
    const result = spawnSync('bash', ['--noprofile','--norc'], {input:script, encoding:'utf8', timeout:3000});
    assert(!result.error, label + ': ' + result.error);
    assert.equal(result.status === 0, expected, label + ': ' + result.stdout + result.stderr);
    assert(!result.stderr.includes('FORBIDDEN SYSTEMCTL'), label + ': unexpected service effect');
    cases++;
    return result.stdout;
  } finally {
    fs.rmSync(directory, {recursive:true});
  }
}
test('no record selects fresh prepare', {}, 'read_stage; [[ $(guided_operation) == prepare ]]', true);
test('same boot only offers restart', {record:JSON.stringify(record())}, 'read_stage; [[ $(guided_operation) == restart ]]', true);
test('new boot selects fresh guarded remove', {record:JSON.stringify(record())}, `boot_id=${otherBoot}; read_stage; [[ $(guided_operation) == remove ]]`, true);
for (const phase of ['preparing','removing']) {
  test('uncertain ' + phase + ' cannot replay', {record:JSON.stringify(record({phase}))}, 'read_stage; guided_operation', false);
}
for (const [label, text] of [
  ['malformed JSON','{'], ['empty',''], ['array','[]'],
  ['wrong helper',JSON.stringify(record({helper:'b'.repeat(64)}))],
  ['foreign account',JSON.stringify(record({uid:process.getuid()+1}))],
  ['unknown lane',JSON.stringify(record({lane:'forced'}))],
  ['unknown phase',JSON.stringify(record({phase:'done'}))],
  ['invalid boot',JSON.stringify(record({boot:'unknown'}))],
  ['extra field',JSON.stringify(record({command:'synthetic'}))],
  ['oversized',JSON.stringify(record({padding:'x'.repeat(600)}))],
  ['duplicate key',JSON.stringify(record()).replace('"schema":1','"schema":0,"schema":1')],
]) test(label + ' record refuses', {record:text}, 'read_stage', false);
test('symlink record refuses', {symlink:true}, 'read_stage', false);
test('hardlink record refuses', {record:JSON.stringify(record()), hardlink:true}, 'read_stage', false);
test('writable record refuses', {record:JSON.stringify(record()), mode:0o644}, 'read_stage', false);
test('real exclusive record creation and transition', {}, 'read_stage; lane=never_connected; write_stage preparing; [[ $stage == preparing ]]; write_stage prepared; [[ $(guided_operation) == restart ]]; [[ $(stat -c %a "$stage_file") == 600 ]]', true);
test('never skip preparation to prepared', {}, 'read_stage; lane=never_connected; write_stage prepared', false);
test('never skip original preparation to removing', {}, 'read_stage; lane=never_connected; write_stage preparing; write_stage removing', false);
test('flush failure cannot proceed to effects', {syncFailure:true}, 'read_stage; lane=never_connected; write_stage preparing; systemctl forbidden', false);
test('changed record cannot be replaced', {record:JSON.stringify(record())}, 'read_stage; printf changed > "$stage_file"; write_stage removing', false);
test('new boot preserves original admission lane', {record:JSON.stringify(record({lane:'live_managed098'}))}, `boot_id=${otherBoot}; read_stage; [[ $(guided_operation) == remove ]]; write_stage removing; [[ $lane == live_managed098 && $stage == removing ]]`, true);
test('cancel restart keeps prepared record', {record:JSON.stringify(record())}, 'read_stage; offer_restart <<< ""; [[ -f "$stage_file" ]]; [[ $(guided_operation) == restart ]]', true);
test('single live confirmation covers close and data deletion', {}, 'lane=live_managed098; confirm_reset prepare <<< REMOVE', true);
test('cancel confirmation never runs service operation', {}, 'lane=live_managed098; confirm_reset prepare <<< ""', false);
const continued = test('continuation does not promise another disconnect', {}, 'lane=live_managed098; confirm_reset remove <<< REMOVE', true);
assert(!continued.includes('connection will be disconnected'));
const legacy = test('082/partial lane requires never-connected attestation', {}, 'lane=never_connected; confirm_reset prepare <<< REMOVE', true);
assert(legacy.includes('never connected'));
assert(source.indexOf('write_stage preparing;') > source.indexOf('confirm_reset prepare'));
assert(source.indexOf('write_stage prepared;') > source.indexOf('sudo /usr/bin/bash -c "$root_prepare"'));
assert(source.indexOf('write_stage removing;') < source.indexOf('sudo /usr/bin/bash -c "$root_remove"'));
assert(source.indexOf('sync -f -- "$stage_directory"') < source.indexOf('\n  read_stage\n}'));
assert(source.includes('jobs -pr | grep -qx -- "$child"'));
assert(!source.includes('kill -'));
// A real inert PTY proves the spinner does not replace/launder the original
// child's outcome. Only the mock native function runs, never the installed CLI.
for (const status of [0, 7]) {
  const directory = fs.mkdtempSync(path.join(parent, 'guided-spinner.'));
  const filename = path.join(directory, 'fixture.sh');
  fs.writeFileSync(filename, `set -euo pipefail\numask 077\n${functions}
guided=true
spinner='|/-\\'
stage_directory=${JSON.stringify(directory)}
function /usr/bin/omavless { sleep 0.35; printf '{"synthetic":true}'; return ${status}; }
native_shutdown plugin watch-removal
`, {mode:0o600});
  try {
    const result = spawnSync('script', ['-q','-e','-c','bash --noprofile --norc ' + filename, '/dev/null'], {encoding:'utf8', timeout:4000});
    assert(!result.error, String(result.error));
    assert.equal(result.status, status, result.stdout + result.stderr);
    assert(result.stdout.includes('Closing OmaVLESS...'));
    assert(result.stdout.includes('{"synthetic":true}'));
    assert.deepEqual(fs.readdirSync(directory), ['fixture.sh']);
    cases++;
  } finally {
    fs.rmSync(directory, {recursive:true});
  }
}
console.log(`Guided removal: ${cases} isolated phase/consent tests PASS; no VM, sudo or reboot.`);
