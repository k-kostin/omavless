#!/usr/bin/env node
// Read-only source and pure validator tests. Never calls sudo, rm or systemctl.
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const source = fs.readFileSync(path.join(__dirname, '../tools/clean-reinstall-098.sh'), 'utf8');
assert.equal(spawnSync('bash', ['-n'], { input: source }).status, 0);
const functions = source.slice(source.indexOf('fail()'), source.indexOf('[[ $# == 1'));
assert(!/\bsudo\b|\brm\b/.test(functions));
let cases = 0;
function validate(command, overrides, pass) {
  const fixture = `set -euo pipefail
HOME=/synthetic/desktop
id() { printf '1234\\n'; }
getent() { printf 'desktop:x:1234:1234:Desktop:/synthetic/desktop:/bin/bash\\n'; }
findmnt() { printf '%s\\n' '{"filesystems":[{"target":"/"}]}'; }
stat() { printf '1234\\n'; }
find() { :; }
${functions}
${overrides || ''}
${command}
`;
  const result = spawnSync('bash', ['--noprofile', '--norc'], { input: fixture, encoding: 'utf8' });
  assert.equal(result.status === 0, pass, result.stdout + result.stderr);
  cases++;
}
validate('check_account', '', true);
validate('check_account', 'HOME=/somewhere/else', false);
validate('check_account', 'getent() { return 1; }', false);
validate('check_account', `getent() { printf 'other:x:9999:9999:Other:/synthetic/desktop:/bin/bash\\n'; }`, false);
validate('check_private_targets', '', true);
for (const target of ['.config/omavless', '.local/state/omavless', '.cache/omavless']) {
  for (const suffix of ['', '/bind', '/deeper/same-filesystem-bind']) {
    const json = JSON.stringify({ filesystems: [{ target: '/' }, { target: '/synthetic/desktop/' + target + suffix }] });
    validate('check_private_targets', `findmnt() { printf '%s\\n' '${json}'; }`, false);
  }
  const sibling = JSON.stringify({ filesystems: [{ target: '/synthetic/desktop/' + target + '-other' }] });
  validate('check_private_targets', `findmnt() { printf '%s\\n' '${sibling}'; }`, true);
}
for (const json of ['{}', '{"filesystems":"invalid"}', '{"filesystems":[{"target":null}]}', 'broken']) {
  validate('check_private_targets', `findmnt() { printf '%s\\n' '${json}'; }`, false);
}
// Disposable, credential-free paths only. The helper performs no deletion.
// Standalone/CI runs may have no cache yet. Create only missing parents and
// refuse unsafe existing ones; do not repair another owner's permissions.
for (const directory of [path.join(os.homedir(), '.cache'), path.join(os.homedir(), '.cache/ovt')]) {
  if (!fs.existsSync(directory)) fs.mkdirSync(directory, { mode: 0o700 });
  const metadata = fs.lstatSync(directory);
  assert(metadata.isDirectory() && !metadata.isSymbolicLink());
  assert.equal(metadata.uid, process.getuid());
  assert.equal(metadata.mode & 0o022, 0);
}
const scratch = fs.mkdtempSync(path.join(os.homedir(), '.cache/ovt/clean-helper-test.'));
try {
  const target = path.join(scratch, '.config/omavless');
  fs.mkdirSync(target, { recursive: true });
  fs.writeFileSync(path.join(target, 'marker'), 'synthetic');
  validate('check_private_targets', `HOME=${JSON.stringify(scratch)}`, true);
  validate('check_private_targets', `HOME=${JSON.stringify(scratch)}; stat() { printf '9999\\n'; }`, false);
  validate('check_private_targets', `HOME=${JSON.stringify(scratch)}; find() { printf 'foreign-owner-file\\n'; }`, false);
  assert.equal(fs.readFileSync(path.join(target, 'marker'), 'utf8'), 'synthetic');
  fs.mkdirSync(path.join(scratch, '.cache'));
  fs.symlinkSync(target, path.join(scratch, '.cache/omavless'));
  validate('check_private_targets', `HOME=${JSON.stringify(scratch)}`, false);
} finally {
  // Remove only the exact directory created by this test, not any helper target.
  fs.rmSync(scratch, { recursive: true });
}
// Strong source boundaries supplement pure checks, not a live removal PASS.
assert(source.indexOf('check_private_targets\n') < source.indexOf('sudo /usr/bin/bash -c "$root_remove"'));
assert(source.lastIndexOf('check_private_targets\n') < source.indexOf('rm -r --one-file-system'));
assert(source.includes('getent passwd "$(id -u)"'));
assert(source.includes('No backup is created.'));
for (const forbidden of ['sudo bash "$', 'sudo /usr/bin/bash "$', '--nodeps', '--overwrite', '--noconfirm', 'rm -rf', 'kill -', 'reset-failed', '--what=fdstore']) {
  assert(!source.includes(forbidden), forbidden);
}
for (const field of ['NFileDescriptorStore', 'NeedDaemonReload', 'FragmentPath', 'SUDO_UID', 'package-guard', '--revoke', 'original_pid', 'ExecMainCode']) {
  assert(source.includes(field), field);
}
assert(source.indexOf('/usr/lib/omavless-dns/package-guard') < source.indexOf('/usr/lib/omavless-dns/omavless-dns-broker --revoke'));
assert(source.indexOf('--revoke') < source.indexOf('sudo pacman -R -- "${packages[@]}"'));
assert(source.includes('packages+=(omavless)') && source.includes('packages+=(omavless-dns)'));
// Exercise the actual shutdown coordinator with inert commands. Never execute
// the top-level helper, a native program, systemctl or a root shell on this host.
const coordinator = source.slice(source.indexOf('runtime_pid()'), source.indexOf('check_supported_installation()'));
function shutdown(overrides, input, pass, expected) {
  const stateDirectory = fs.mkdtempSync(path.join(os.homedir(), '.cache/ovt/shutdown-fixture.'));
  const stateFile = path.join(stateDirectory, 'pid');
  fs.writeFileSync(stateFile, '0\n');
  const fixture = `set -euo pipefail
STATE=${JSON.stringify(stateFile)}
fail() { printf 'REFUSED:%s\\n' "$*"; exit 1; }
${coordinator}
runtime_pid() { /usr/bin/cat "$STATE"; }
plugin_state() { printf 'enabled\\n'; }
hello_fence() { printf 'synthetic-instance\\t7\\n'; }
systemctl() { printf 'inactive\\n'; }
function /usr/bin/omavless {
  printf 'CALL:%s\\n' "$*" >&2
  printf '0\\n' > "$STATE"
  case "$2" in
    quit) printf '%s\\n' '{"schemaVersion":1,"shutdown":true,"pluginDisabled":true}' ;;
    watch-removal) printf '%s\\n' '{"schemaVersion":1,"runtimeStopped":true}' ;;
    *) exit 9 ;;
  esac
}
${overrides}
shutdown_running_app
`;
  let result;
  try {
    result = spawnSync('bash', ['--noprofile', '--norc', '-c', fixture], { input, encoding:'utf8', timeout:5000 });
  } finally {
    fs.rmSync(stateDirectory, {recursive:true});
  }
  assert.equal(result.status === 0, pass, result.stdout + result.stderr);
  for (const fragment of expected) assert((result.stdout + result.stderr).includes(fragment), fragment);
  cases++;
  return result;
}
shutdown('', '', true, ['already stopped']);
shutdown("printf '123\\n' > \"$STATE\"", 'STOP\n', true, ['CALL:plugin quit synthetic-instance 7', 'OmaVLESS closed']);
shutdown("printf '123\\n' > \"$STATE\"; plugin_state() { printf 'absent\\n'; }", 'STOP\n', true, ['CALL:plugin watch-removal', 'OmaVLESS closed']);
shutdown("printf '123\\n' > \"$STATE\"; plugin_state() { printf 'disabled\\n'; }", 'STOP\n', true, ['CALL:plugin watch-removal', 'OmaVLESS closed']);
shutdown("runtime_pid() { printf '123\\n'; }", '\n', false, ['Cancelled']);
shutdown("runtime_pid() { printf '123\\n'; }; plugin_state() { return 1; }", 'STOP\n', false, ['Cannot check the plugin']);
shutdown("runtime_pid() { printf '123\\n'; }; hello_fence() { return 1; }", 'STOP\n', false, ['Cannot safely close']);
// A failed submitted Quit never falls back to watcher/removal or retries.
const uncertain = shutdown("runtime_pid() { printf '123\\n'; }; function /usr/bin/omavless { printf 'CALL:%s\\n' \"$*\" >&2; return 73; }", 'STOP\n', false, ['did not confirm shutdown']);
assert.equal((uncertain.stderr.match(/CALL:plugin quit/g) || []).length, 1);
assert(!uncertain.stderr.includes('watch-removal'));
const coalesced = shutdown("printf '123\\n' > \"$STATE\"; plugin_state() { printf 'absent\\n'; }; function /usr/bin/omavless { printf '%s\\n' '{\"schemaVersion\":1,\"runtimeStopped\":false}'; }; sleep() { SECONDS=$((SECONDS + 100)); }", 'STOP\n', false, ['Shutdown is still unconfirmed']);
assert(!coalesced.stdout.includes('OmaVLESS closed'));
for (const [load, enabled, fail, expected] of [
  ['not-found','',false,true], ['loaded','disabled',false,true],
  ['loaded','enabled',false,false], ['loaded','enabled-runtime',false,false],
  ['loaded','',true,false], ['unknown','disabled',false,false]
]) {
  const fixture = `set -euo pipefail\n${coordinator}\nsystemctl() { case "$*" in *LoadState*) printf '%s\\n' '${load}' ;; *UnitFileState*) ${fail ? 'return 1' : `printf '%s\\n' '${enabled}'`} ;; esac; }\nruntime_disabled\n`;
  const result = spawnSync('bash', ['--noprofile', '--norc'], {input:fixture, encoding:'utf8'});
  assert.equal(result.status === 0, expected, result.stdout + result.stderr);
  cases++;
}
// Use the real parser functions independently, with timeout replaced by a
// bounded fixture producer; malformed input must not acquire a shutdown fence.
function parseFunction(name, json, pass) {
  const fixture = `set -euo pipefail\n${coordinator}\ntimeout() { printf '%s' '${json}'; }\n${name}\n`;
  const result = spawnSync('bash', ['--noprofile', '--norc'], { input:fixture, encoding:'utf8' });
  assert.equal(result.status === 0, pass, result.stdout + result.stderr);
  cases++;
}
parseFunction('hello_fence', '{"ok":true,"revision":7,"result":{"instanceId":"synthetic","version":1,"runtimeOwnership":true}}', true);
for (const json of ['{}', '{"ok":false}', '{"ok":true,"revision":7,"result":{"instanceId":"synthetic","version":1,"runtimeOwnership":false}}', '{"ok":true,"revision":7.5,"result":{"instanceId":"synthetic","version":1,"runtimeOwnership":true}}', '{"ok":true,"revision":9007199254740992,"result":{"instanceId":"synthetic","version":1,"runtimeOwnership":true}}']) {
  parseFunction('hello_fence', json, false);
}
parseFunction('plugin_state', '[]', true);
parseFunction('plugin_state', '[{"id":"kdk.omavless","enabled":false}]', true);
parseFunction('plugin_state', '[{"id":"kdk.omavless","enabled":true}]', true);
for (const json of ['{}', '[{"id":"kdk.omavless"}]', '[{"id":"kdk.omavless","enabled":true},{"id":"kdk.omavless","enabled":false}]']) parseFunction('plugin_state', json, false);
const classification = source.slice(source.indexOf('check_supported_installation()'), source.indexOf('[[ $# == 1'));
for (const [app, dns, digest, expected] of [
  ['omavless 0.8.2-1', 'absent', '3ff795316aa331463aff4e26571546433bdfd9317f348712d135be2f0a8d28a1', true],
  ['omavless 0.9.8-1', 'omavless-dns 0.9.8-1', '7c7b0f0c350b8903353d2fed3c8effb693aaaca0c271279b85efa51dfebb8881', true],
  ['omavless 0.9.8-1', 'omavless-dns 0.9.8-1', '0'.repeat(64), false],
  ['omavless 0.0.0.r123.gtest-1', 'omavless-dns 0.9.8-1', '7c7b0f0c350b8903353d2fed3c8effb693aaaca0c271279b85efa51dfebb8881', false],
  ['omavless 0.8.2-1', 'omavless-dns 0.9.8-1', '3ff795316aa331463aff4e26571546433bdfd9317f348712d135be2f0a8d28a1', false]
]) {
  const directory = fs.mkdtempSync(path.join(os.homedir(), '.cache/ovt/removal-app-fixture.'));
  const binary = path.join(directory, 'omavless');
  fs.writeFileSync(binary, 'synthetic');
  const fixture = `set -euo pipefail\nfail() { exit 1; }\n${classification.replaceAll('/usr/bin/omavless', binary)}
pacman() { case "$2" in omavless) printf '%s\\n' '${app}' ;; omavless-dns) ${dns === 'absent' ? 'return 1' : `printf '%s\\n' '${dns}'`} ;; *) exit 9 ;; esac; }
stat() { printf '0:755\\n'; }
sha256sum() { printf '%s  %s\\n' '${digest}' "$1"; }
check_supported_installation\n`;
  try {
    const result = spawnSync('bash', ['--noprofile', '--norc'], {input:fixture, encoding:'utf8'});
    assert.equal(result.status === 0, expected, result.stdout + result.stderr);
    cases++;
  } finally { fs.rmSync(directory, {recursive:true}); }
}
console.log(`Clean-reinstall helper: ${cases} pure validator cases and source boundaries PASS; no host effects.`);
