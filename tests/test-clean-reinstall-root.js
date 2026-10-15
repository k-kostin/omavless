#!/usr/bin/env node
// Execute production fixed root branches only inside a path-rewritten inert
// filesystem. No sudo, pacman, service manager, native binary or real root path.
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');
const {spawnSync} = require('node:child_process');
const source = fs.readFileSync(path.join(__dirname, '../tools/clean-reinstall-098.sh'), 'utf8');
function section(name) {
  const match = source.match(new RegExp("<<'" + name + "' \\|\\| true\\n([\\s\\S]*?)\\n" + name));
  assert(match, name);
  return match[1];
}
const checks = section('ROOT_CHECKS');
const prepare = section('ROOT_PREPARE');
const remove = section('ROOT_REMOVE');
let count = 0;
function test(label, options, body, expected, forbidden = []) {
  const directory = fs.mkdtempSync(path.join(os.homedir(), '.cache/ovt/removal-root-fixture.'));
  const write = (target, content = '') => {
    const filename = directory + target;
    fs.mkdirSync(path.dirname(filename), {recursive:true});
    fs.writeFileSync(filename, content);
  };
  fs.mkdirSync(directory + '/proc');
  if (!options.noDns) {
    write('/usr/lib/omavless-dns/package-guard');
    write('/usr/lib/omavless-dns/omavless-dns-broker');
    write('/usr/lib/systemd/system/omavless-dns-broker.service');
    fs.mkdirSync(directory + '/run/omavless-dns/private', {recursive:true});
    if (options.enrollment !== 'absent') write('/etc/omavless-dns/release-enrollment.json', JSON.stringify({schema:1, uid:options.foreign ? 5678 : 1234, policy:'meta-ipv4-release-v1'}));
    if (options.staged) write('/etc/omavless-dns/.enrollment.pending', 'synthetic');
    if (options.unknownPrivate) write('/run/omavless-dns/private/unknown', 'synthetic');
  } else if (options.residual) {
    write(options.residual, 'synthetic');
  }
  // Rewrite data paths only; production shell decisions remain unchanged.
  const rewrite = text => text.replace(/\/(usr|etc|run|proc|sys)(?=\/|\b)/g, (_, component) => directory + '/' + component);
  const prelude = `
set -euo pipefail
SUDO_UID=1234
ROOT=${JSON.stringify(directory)}
pacman() { ${options.noDns ? 'return 1' : 'return 0'}; }
systemctl() {
  case "$*" in
    *--property=LoadState*) printf '%s\\n' '${options.noDns ? (options.loadedResidual ? 'loaded' : 'not-found') : 'loaded'}' ;;
    *--property=MainPID*|*--property=NFileDescriptorStore*) printf '0\\n' ;;
    *--property=ActiveState*) printf 'inactive\\n' ;;
    *--property=SubState*) printf 'dead\\n' ;;
    *--property=DropInPaths*|*--property=Job*) printf '\\n' ;;
    *--property=FragmentPath*) printf '%s\\n' "$ROOT/usr/lib/systemd/system/omavless-dns-broker.service" ;;
    *--property=NeedDaemonReload*) printf 'no\\n' ;;
    disable*) printf 'DISABLE\\n' ;;
    *) printf 'UNEXPECTED SYSTEMCTL\\n' >&2; return 2 ;;
  esac
}
stat() {
  case "$2" in
    '%u %a') printf '0 755\\n' ;;
    '%u:%a') printf '0:700\\n' ;;
    '%u:%a:%h') if [[ "$3" == *release-enrollment.json ]]; then printf '0:600:1\\n'; else printf '0:755:1\\n'; fi ;;
    '%s') /usr/bin/stat "$@" ;;
    *) printf 'UNEXPECTED STAT\\n' >&2; return 2 ;;
  esac
}
sha256sum() {
  case "$1" in
    *package-guard) printf '%s  %s\\n' '${options.badGuard ? 'invalid' : '624c08b458afd1dd48f6a16ef36e80d97027ee1062e0a7c8fa4ef0239fa89b03'}' "$1" ;;
    *omavless-dns-broker.service) printf '%s  %s\\n' '126b208158e9288c475084c8bd74029e2808edf9846da514a369266b6306462d' "$1" ;;
    *omavless-dns-broker) printf '%s  %s\\n' '${options.unknownBroker ? 'invalid' : '1855ed920a4b064c811ed14134b5ae14e3c5ece0a26695a84afc1ccfd711dc93'}' "$1" ;;
    *) return 2 ;;
  esac
}
function ${directory}/usr/lib/omavless-dns/package-guard { printf 'GUARD\\n'; ${options.guardFailure ? 'return 1' : 'return 0'}; }
function ${directory}/usr/lib/omavless-dns/omavless-dns-broker { [[ "$*" == --revoke ]] || exit 9; printf 'REVOKE\\n'; }
`;
  let result;
  try {
    result = spawnSync('bash', ['--noprofile', '--norc'], {
      input:prelude + '\n' + rewrite(checks) + '\n' + rewrite(body),
      encoding:'utf8', timeout:3000
    });
    assert(!result.error, label + ': ' + result.error);
    assert.equal(result.status === 0, expected, label + ': ' + result.stdout + result.stderr);
    for (const token of forbidden) assert(!result.stdout.includes(token), label + ': ' + token);
    count++;
    return result.stdout;
  } finally {
    fs.rmSync(directory, {recursive:true});
  }
}
test('absent DNS requires absent paths and unit', {noDns:true}, 'dns_absent', true);
for (const residual of ['/etc/omavless-dns/unknown', '/run/omavless-dns/control.sock', '/usr/lib/omavless-dns/unknown', '/usr/lib/systemd/system/omavless-dns-broker.service']) {
  test('absent package with residual ' + residual, {noDns:true, residual}, 'dns_absent', false);
}
test('absent package with loaded residual unit', {noDns:true, loadedResidual:true}, 'dns_absent', false);
test('pre-enrollment is safe absence', {enrollment:'absent'}, 'check_enrollment; [[ "$enrolled" == false ]]', true);
test('current account enrollment', {}, 'check_enrollment; [[ "$enrolled" == true ]]', true);
test('other account enrollment', {foreign:true}, 'check_enrollment', false);
test('pending enrollment', {staged:true}, 'check_enrollment', false);
test('known guard and broker', {}, 'check_dns_programs', true);
test('unknown broker', {unknownBroker:true}, 'check_dns_programs', false, ['GUARD', 'REVOKE']);
test('unknown guard', {badGuard:true}, 'check_dns_programs', false, ['GUARD', 'REVOKE']);
test('inactive pre-enrollment prepare', {enrollment:'absent'}, prepare, true, ['REVOKE']);
test('inactive guard failure refuses preparation', {guardFailure:true}, prepare, false, ['DISABLE', 'REVOKE']);
test('unknown private journal refuses preparation', {unknownPrivate:true}, prepare, false, ['GUARD', 'DISABLE', 'REVOKE']);
test('guard failure prevents revocation', {guardFailure:true}, remove, false, ['REVOKE']);
test('pre-enrollment skips revocation', {enrollment:'absent'}, remove, true, ['REVOKE']);
assert(test('own enrollment revocation', {}, remove, true).includes('REVOKE'));
test('foreign enrollment refuses removal', {foreign:true}, remove, false, ['REVOKE']);
test('absent DNS preparation', {noDns:true}, prepare, true, ['REVOKE', 'GUARD']);
test('absent DNS removal', {noDns:true}, remove, true, ['REVOKE', 'GUARD']);
console.log(`Clean-reinstall root branches: ${count} isolated cases PASS; no privileged/host effects.`);
