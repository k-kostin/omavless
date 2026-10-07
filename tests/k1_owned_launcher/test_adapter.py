"""Source-only controls. No exported Rust function, ELF, socket or child runs."""
import importlib.util
import base64
import copy
import hashlib
from io import BytesIO
import json
import re
import zlib
from pathlib import Path
import unittest
from unittest.mock import patch

HERE = Path(__file__).parent
ROOT = HERE.parents[1]
def module(name):
    spec = importlib.util.spec_from_file_location(name, HERE / (name + '.py'))
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded

a = module('adapt')
p = module('prepare')

HISTORICAL_COMMIT = '358756d30ed408259deb96be53c8ec2c53c2d96e'
HISTORICAL_SNAPSHOT_SHA = 'd09163664bbd6f968edc95066385cdc52b67d836e047f5e56a3976c2d16b5ae5'
# Public source DATA for these historical source controls, never an exporter or
# executable input. Preserve the already accepted successor pins exactly.
HISTORICAL = {
    'authority_composition.rs': (7314, 'c43a5dc3b9b49283f1a396e0ad8fe1fa89285ec99bd517b8f991451e616b2c91'),
    'launch_acquisition.rs': (8583, '033e9f826fa8522a360a972c109f986e221fe0986505b704b2eedc2cfb4f2d72'),
    'kernel_observer.rs': (35254, 'ffb0e395607c8e21c25e1e2f5a89f929689bd5ba0fe5252ca852479ee6a8b4f7'),
    'kernel_inventory.rs': (13669, '6665ffc491c0e4efd5e03bc3b07e236289c3f354d205ab93d890678da1cbbc10'),
    'kernel_chain_observer.rs': (20833, '1d1c6fe007f862cc0306e7534141d69d1d16454fd2bd8a5dc0b7cfc1bb9185d7'),
    'kernel_rule_wire.rs': (11495, '52d39aef99eb1c4bc99d26fffc3798ba286ab23a4a7af36e43a6554a46c89097'),
}
MAX_SNAPSHOT_BYTES = 65536
MAX_SOURCE_BYTES = 65536
MAX_COMPRESSED_BYTES = 16384

def unique(rows):
    result = {}
    for key, value in rows:
        if key in result: raise ValueError('historical_duplicate')
        result[key] = value
    return result

def decode_historical(data):
    if type(data) is not dict or set(data) != {'schema', 'commit', 'adapter_base', 'members'} \
            or type(data['schema']) is not int or data['schema'] != 1 \
            or data['commit'] != HISTORICAL_COMMIT or data['adapter_base'] != a.BASE \
            or type(data['members']) is not dict or data['members'].keys() != a.PINS.keys():
        raise ValueError('historical_catalogue')
    sources = {}
    for name, (size, sha) in HISTORICAL.items():
        row = data['members'][name]
        if type(row) is not dict or set(row) != {'size', 'sha256', 'zlib_base64'} \
                or type(row['size']) is not int or row['size'] != size \
                or not 0 < size <= MAX_SOURCE_BYTES or row['sha256'] != sha \
                or type(row['zlib_base64']) is not str \
                or len(row['zlib_base64']) > 4 * ((MAX_COMPRESSED_BYTES + 2) // 3):
            raise ValueError('historical_member')
        compressed = base64.b64decode(row['zlib_base64'], validate=True)
        if len(compressed) > MAX_COMPRESSED_BYTES: raise ValueError('historical_compressed_bound')
        decoder = zlib.decompressobj()
        # The immutable declared size/bounds were checked BEFORE allocation;
        # neither a forged size nor a compressed bomb can allocate unboundedly.
        raw = decoder.decompress(compressed, size + 1)
        if len(raw) != size or not decoder.eof or decoder.unused_data or decoder.unconsumed_tail \
                or hashlib.sha256(raw).hexdigest() != sha:
            raise ValueError('historical_source_pin')
        sources[name] = raw
    return sources

def historical_inputs():
    with (HERE / 'historical_inputs.json').open('rb') as source:
        raw = source.read(MAX_SNAPSHOT_BYTES + 1)
    if len(raw) > MAX_SNAPSHOT_BYTES or hashlib.sha256(raw).hexdigest() != HISTORICAL_SNAPSHOT_SHA:
        raise ValueError('historical_snapshot_pin')
    data = json.loads(raw, object_pairs_hook=unique,
                      parse_constant=lambda _: (_ for _ in ()).throw(ValueError('historical_nonfinite')))
    return decode_historical(data)

def pinned_original(name, raw):
    # The new opt-in service does NOT replace the historical exporter. Reverse
    # only exact pinned successor declarations/visibility, then reverify BASE.
    successors = {
        'authority_composition.rs': 'c43a5dc3b9b49283f1a396e0ad8fe1fa89285ec99bd517b8f991451e616b2c91',
        'launch_acquisition.rs': '794cbcaef7ade340cc2e78f1dc966e4e300aa430beb853ab5d646a08e07ce9ea',
        'kernel_observer.rs': 'ffb0e395607c8e21c25e1e2f5a89f929689bd5ba0fe5252ca852479ee6a8b4f7',
        'kernel_inventory.rs': '6665ffc491c0e4efd5e03bc3b07e236289c3f354d205ab93d890678da1cbbc10',
    }
    # 358756d3 added only these three closed diagnostic marks to this file.
    # Admit its complete bytes first, then verify the exact prior successor
    # before applying the existing historical projection below.
    if name == 'launch_acquisition.rs' and hashlib.sha256(raw).hexdigest() == \
            '033e9f826fa8522a360a972c109f986e221fe0986505b704b2eedc2cfb4f2d72':
        for mark in (b'        service_cut!(VerifierBefore);\n',
                     b'        service_cut!(Creator);\n',
                     b'        service_cut!(VerifierAfter);\n'):
            if raw.count(mark) != 1:
                raise ValueError('fixed_diagnostic_mark')
            raw = raw.replace(mark, b'')
        if hashlib.sha256(raw).hexdigest() != successors[name]:
            raise ValueError('fixed_pre_diagnostic_pin')
    if name in successors and hashlib.sha256(raw).hexdigest() == successors[name]:
        if name == 'authority_composition.rs':
            raw = raw.replace((b'//! Default-build inactive lifetime composition. The opt-in service provider\n'
                b'//! is private; historical missing-provider obligations below describe default\n'
                b'//! builds. Receipts/integers/fixtures/legacy ports cannot acquire that provider.\n'),
                (b'//! Inactive lifetime composition, not a canonical namespace authenticator.\n'
                b'//! No non-test provider exists. Receipts, matching integers, fixture witnesses\n'
                b'//! and the legacy EffectPort cannot be converted into a provider here.\n'))
            start = raw.index(b'    #[cfg(feature = "netguard-service-core")]\n    pub(crate) fn recover_one(')
            end = raw.index(b'    pub(crate) fn from_admitted(', start)
            raw = raw[:start] + raw[end:]
        elif name == 'launch_acquisition.rs':
            raw = raw.replace((b'//! Default-build inactive acquisition; historical obligations below describe\n'
                b'//! that default. The opt-in service_origin private factory is the successor.\n'
                b'//! Exact configuration bytes remain evidence, never trusted launch provenance.\n'),
                (b'//! Inactive acquisition boundary. There is NO normal constructor or verifier.\n'
                b'//! Exact configuration bytes are evidence, never trusted launch provenance.\n'))
            raw = raw.replace((b'#[cfg(feature = "netguard-service-core")]\n'
                b'#[path = "launch_service_origin.rs"]\nmod service_origin;\n'
                b'#[cfg(feature = "netguard-service-core")]\n'
                b'pub(crate) use service_origin::acquire_fixed_service;\n\n'), b'')
        elif name == 'kernel_observer.rs':
            raw = raw.replace((b'//! Default-build read-only fixed-table observation in the calling namespace.\n'
                b'//! Read projections never attest ownership/canonical origin. The opt-in private\n'
                b'//! service_creator child adds effects only through original launch acquisition.\n'),
                (b'//! Inactive, read-only fixed-table metadata observation in the calling namespace.\n'
                b'//! No ownership, policy verification, canonical-host identity or effect authority.\n'))
            raw = raw.replace((b'#[cfg(feature = "netguard-service-core")]\n'
                b'#[path = "kernel_service_creator.rs"]\n'
                b'pub(crate) mod service_creator;\n'), b'')
        else:
            raw = raw.replace(b'#[cfg(any(test, feature = "netguard-service-core"))]', b'#[cfg(test)]')
            raw = raw.replace(b'    pub(super) fn inspect_policy_inventory_before(',
                              b'    fn inspect_policy_inventory_before(')
    # The external exporter still selects immutable BASE. Only this exact
    # later cfg(test) declaration is projected out for shallow-CI source
    # controls; both the complete successor and resulting original are pinned.
    if name == 'kernel_inventory.rs' and hashlib.sha256(raw).hexdigest() == \
            '9c5cb451724693f2b48a79dadea028975eb9fd27765c5e56cfa50c8e6211aede':
        declaration = (b'#[cfg(test)]\n#[path = "kernel_create_witness.rs"]\n'
                       b'pub(super) mod create_witness;\n\n')
        if raw.count(declaration) != 1:
            raise ValueError('fixed_test_declaration')
        raw = raw.replace(declaration, b'')
    if hashlib.sha256(raw).hexdigest() != a.PINS[name]:
        raise ValueError('fixed_original_pin')
    return raw

class Controls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # Shallow CI consumes original historical DATA, not the current branch's
        # unrelated implementation. The complete historical snapshot and each
        # original/projected pin remain exact; developer export still uses BASE.
        cls.historical = historical_inputs()
        cls.sources = {name: pinned_original(name, raw) for name, raw in cls.historical.items()}
        cls.adapted = a.adapt(cls.sources)

    def test_historical_snapshot_is_exact_public_git_blob_data_not_a_current_export(self):
        self.assertEqual(set(self.historical), set(a.PINS))
        self.assertEqual(a.BASE, 'e6488ed5c39486a5f967859c20a0b84859ae4b5d')
        for name, (size, sha) in HISTORICAL.items():
            self.assertEqual(len(self.historical[name]), size)
            self.assertEqual(hashlib.sha256(self.historical[name]).hexdigest(), sha)
            self.assertEqual(hashlib.sha256(self.sources[name]).hexdigest(), a.PINS[name])
        self.assertNotIn('historical_inputs', (HERE / 'prepare.py').read_text())
        self.assertIn("adapter.BASE+':'+n", (HERE / 'prepare.py').read_text())

    def test_snapshot_artifact_size_hash_duplicate_and_catalogue_fail_closed(self):
        raw = (HERE / 'historical_inputs.json').read_bytes()
        for changed in (raw + b' ', raw[:10], b'X' * (MAX_SNAPSHOT_BYTES + 1)):
            with patch.object(Path, 'open', return_value=BytesIO(changed)):
                with self.assertRaises(ValueError): historical_inputs()
        with self.assertRaises(ValueError):
            json.loads('{"schema":1,"schema":1}', object_pairs_hook=unique)
        data = json.loads(raw)
        for changed in ({**data, 'schema': True}, {**data, 'commit': '0' * 40},
                        {**data, 'adapter_base': HISTORICAL_COMMIT}, {**data, 'extra': 0},
                        {**data, 'members': {}}):
            with self.assertRaises(ValueError): decode_historical(changed)

    def test_declared_sizes_and_encoded_bounds_refuse_before_decompress_allocation(self):
        original = json.loads((HERE / 'historical_inputs.json').read_bytes())
        name = next(iter(original['members']))
        for size in (True, -1, 0, MAX_SOURCE_BYTES + 1, original['members'][name]['size'] + 1):
            data = copy.deepcopy(original);data['members'][name]['size'] = size
            with patch.object(zlib, 'decompressobj') as decoder:
                with self.assertRaises(ValueError): decode_historical(data)
                decoder.assert_not_called()
        data = copy.deepcopy(original)
        data['members'][name]['zlib_base64'] = 'X' * (4 * ((MAX_COMPRESSED_BYTES + 2) // 3) + 1)
        with patch.object(zlib, 'decompressobj') as decoder:
            with self.assertRaises(ValueError): decode_historical(data)
            decoder.assert_not_called()

    def test_compressed_truncation_trailing_checksum_and_bomb_cannot_become_original(self):
        original = json.loads((HERE / 'historical_inputs.json').read_bytes())
        name = next(iter(original['members']))
        compressed = base64.b64decode(original['members'][name]['zlib_base64'])
        for changed in (compressed[:-1], compressed + b'ignored',
                        zlib.compress(b'X' * HISTORICAL[name][0]),
                        zlib.compress(b'X' * (MAX_SOURCE_BYTES * 8))):
            data = copy.deepcopy(original)
            data['members'][name]['zlib_base64'] = base64.b64encode(changed).decode()
            with self.assertRaises(ValueError): decode_historical(data)
        data = copy.deepcopy(original);data['members'][name]['sha256'] = '0' * 64
        with patch.object(zlib, 'decompressobj') as decoder:
            with self.assertRaises(ValueError): decode_historical(data)
            decoder.assert_not_called()

    def test_modern_cold_graph_is_not_reinterpreted_as_historical_authority(self):
        current = {name: (ROOT / 'crates/omavless-netguard/src' / name).read_bytes() for name in a.PINS}
        with self.assertRaises(ValueError): a.adapt(current)
        for name in ('authority_composition.rs', 'launch_acquisition.rs'):
            self.assertNotEqual(current[name], self.historical[name])
            with self.assertRaises(ValueError): pinned_original(name, current[name])

    def test_successor_projection_refuses_any_other_source_change(self):
        name = 'kernel_inventory.rs'
        raw = self.historical[name]
        self.assertEqual(pinned_original(name, raw), self.sources[name])
        self.assertEqual(pinned_original(name, self.sources[name]), self.sources[name])
        for changed in (raw + b' ', raw.replace(b'#[cfg(test)]', b'#[cfg(any())]', 1),
                        raw.replace(b'create_witness;', b'other_witness;', 1)):
            with self.assertRaises(ValueError):
                pinned_original(name, changed)
        for name in ('launch_acquisition.rs', 'kernel_observer.rs', 'authority_composition.rs'):
            raw = self.historical[name]
            self.assertEqual(pinned_original(name, raw), self.sources[name])
            for changed in (raw + b' ', raw.replace(b'netguard-service-core', b'unreviewed-feature', 1)):
                with self.assertRaises(ValueError):
                    pinned_original(name, changed)

    def test_exact_catalog_and_each_pin_refuse(self):
        for name in a.PINS:
            changed = dict(self.sources)
            changed[name] += b' '
            with self.assertRaises(ValueError): a.adapt(changed)
            del changed[name]
            with self.assertRaises(ValueError): a.adapt(changed)
        with self.assertRaises(ValueError): a.adapt({**self.sources, 'extra.rs': b''})

    def test_diagnostic_successor_requires_complete_exact_source(self):
        name = 'launch_acquisition.rs'
        raw = self.historical[name]
        self.assertEqual(hashlib.sha256(raw).hexdigest(),
                         '033e9f826fa8522a360a972c109f986e221fe0986505b704b2eedc2cfb4f2d72')
        prior = raw
        for label in (b'VerifierBefore', b'Creator', b'VerifierAfter'):
            mark = b'        service_cut!(' + label + b');\n'
            self.assertEqual(raw.count(mark), 1)
            prior = prior.replace(mark, b'')
            for changed in (raw.replace(mark, b''),
                            raw.replace(mark, mark + mark),
                            raw.replace(mark, mark.replace(label, b'Unreviewed')),
                            raw.replace(mark, b' ' + mark)):
                with self.assertRaises(ValueError):
                    pinned_original(name, changed)
        self.assertEqual(hashlib.sha256(prior).hexdigest(),
                         '794cbcaef7ade340cc2e78f1dc966e4e300aa430beb853ab5d646a08e07ce9ea')
        self.assertEqual(pinned_original(name, prior), self.sources[name])
        self.assertEqual(hashlib.sha256(pinned_original(name, raw)).hexdigest(),
                         a.PINS[name])

    def test_actual_creator_borrow_and_no_second_socket_owner(self):
        raw = self.adapted['launch_acquisition.rs'].decode()
        self.assertNotIn('creator_socket: OwnedFd', raw)
        self.assertEqual(raw.count('creator_socket: creator.creator_socket()'), 2)
        self.assertIn('fn with_lease<T>', raw)
        self.assertNotIn('pub(crate) fn with_lease', raw)
        bridge = (HERE / 'owned_creator.rs').read_text()
        self.assertIn('self.session.socket.as_fd()', bridge)
        self.assertNotIn('impl CanonicalCreator', bridge)
        self.assertNotIn('impl EffectPort', bridge)

    def test_actual_inventory_reuses_existing_lease_through_owner_verification(self):
        adapted = self.adapted['kernel_inventory.rs'].decode()
        self.assertIn('pub(super) fn borrow_policy_inventory_before', adapted)
        original = self.sources['kernel_inventory.rs'].decode()
        start = '        let result = self.inspect_policy_inventory_before(deadline);'
        end = '    #[cfg(test)]\n    pub(super) fn inspect_policy_inventory_once'
        self.assertIn(original[original.index(start):original.index(end)], adapted)
        creator = (HERE / 'owned_creator.rs').read_text()
        self.assertIn('self.session.borrow_policy_inventory_before(self.deadline)?', creator)
        self.assertIn("lease: LocalInventoryLease<'a>", creator)
        self.assertIn('if !self.completed { self.lease.session.poisoned = true; }', creator)
        self.assertNotIn('let (inventory, _, _) = result?', creator)
        launcher = (HERE / 'owned_launcher.rs').read_text()
        body = launcher[launcher.index('    pub(crate) fn inventory(&mut self)'):]
        self.assertLess(body.index('self.acquired.sealed = true'), body.index('creator.borrow_inventory()'))
        self.assertLess(body.index('inventory_sequence::Attempt'), body.index('inventory.complete()'))
        self.assertLess(body.index('inventory.complete()'), body.index('self.acquired.sealed = false'))
        self.assertIn('creator_socket: self.inventory.socket()', launcher)

    def test_all_four_actual_readback_leaf_pairs_and_no_sender_added(self):
        for name in ('kernel_observer.rs', 'kernel_inventory.rs',
                     'kernel_chain_observer.rs', 'kernel_rule_wire.rs'):
            raw = self.adapted[name].decode()
            self.assertEqual(raw.count('self.launch_leaf(|| sendto('), 1)
            self.assertEqual(raw.count('self.launch_leaf(|| recvmsg::<NetlinkAddr>('), 1)
            self.assertEqual(raw.count('sendto('), self.sources[name].decode().count('sendto('))
            self.assertEqual(raw.count('recvmsg::<NetlinkAddr>('),
                             self.sources[name].decode().count('recvmsg::<NetlinkAddr>('))
        self.assertIn('launch.session_check(', self.adapted['kernel_observer.rs'].decode())

    def test_fixed_inventory_entry_shares_deadline_and_retains_before_late_check(self):
        raw = (HERE / 'inventory_gate.rs').read_text()
        self.assertEqual(raw.count('Duration::from_secs(5)'), 1)
        self.assertIn('Prototype::open_fixed_before(self.deadline)', raw)
        self.assertLess(raw.index('*self.owner = Some(owner)'), raw.index('self.budget()', raw.index('*self.owner = Some(owner)')))
        self.assertIn('owner: ManuallyDrop<Option<Prototype>>', raw)
        self.assertIn('--fixed-owned-readonly-inventory', raw)
        self.assertIn('rustix::io::write(rustix::stdio::stdout(), bytes)', raw)
        self.assertNotIn('write_all', raw)
        self.assertNotIn('unsafe', raw)
        launcher = (HERE / 'owned_launcher.rs').read_text()
        bounded = launcher[launcher.index('pub(crate) fn open_fixed_before'):launcher.index('pub(crate) fn inventory')]
        self.assertIn('deadline.duration_since(now)<=Duration::from_secs(5)', bounded)
        self.assertNotIn('checked_add', bounded)
        self.assertIn('open_actual(deadline)', bounded)
        sequence = (HERE / 'inventory_gate_sequence.rs').read_text()
        run = sequence[sequence.index('pub(super) fn run'):sequence.index('#[cfg(test)]')]
        self.assertLess(run.index('self.sealed = true'), run.index('backend.open()'))
        self.assertLess(run.index('backend.inventory()?'), run.index('backend.finish()?'))
        self.assertEqual(run.count('backend.inventory()?'), 1)
        self.assertEqual(run.count('backend.finish()?'), 1)
        for name in ('TableAbsent', 'ExactUntrusted', 'OtherUntrusted'):
            self.assertIn('LocalPolicyInventory::' + name, raw)

    def test_inventory_entry_is_opt_in_external_export_only(self):
        raw = (HERE / 'prepare.py').read_text()
        self.assertIn("'netguard-inventory-entry-v1'", raw)
        self.assertIn('name = "k1-owned-readonly-inventory"', raw)
        self.assertIn('required-features = ["owned-launch-readonly-inventory"]', raw)
        self.assertIn('include!("inventory_gate.rs")', raw)
        self.assertNotIn('inventory()', (HERE / 'no_policy_gate.rs').read_text())

    def test_fixed_semantic_acquisition_tail_is_byte_identical(self):
        old = self.sources['launch_acquisition.rs'].decode()
        new = self.adapted['launch_acquisition.rs'].decode()
        start = '    pub(crate) fn retained_epoch('
        end = '    // The ONLY constructor is synthetic.'
        tail = old[old.index(start):old.index(end)]
        self.assertIn(tail, new)
        self.assertNotIn('pub(crate) fn open_fixed', new)
        self.assertIn('pub(crate) trait CreatorOwner', new)

    def test_export_name_refuses_before_git_or_filesystem(self):
        with patch.object(p, 'git') as git:
            for name in ('../netguard', 'owned_launcher.rs', '', None):
                with self.assertRaises(ValueError): p.export('/absent', name)
            git.assert_not_called()

    def test_full_original_image_timestamp_fields(self):
        raw = (HERE / 'owned_launcher.rs').read_text()
        for field in ('s.dev()', 's.ino()', 's.mode()', 's.uid()', 's.gid()',
                      's.nlink()', 's.size()', 's.mtime()', 's.mtime_nsec()',
                      's.ctime()', 's.ctime_nsec()'):
            self.assertIn(field, raw)
        self.assertIn('WaitPidFlag::WNOWAIT', raw)
        self.assertEqual(raw.count('nix::sys::wait::waitpid('), 1)
        self.assertNotIn('.kill(', raw)
        self.assertNotIn('setns(', raw)

    def test_prelaunch_does_not_enter_ungated_nested_helpers(self):
        raw = (HERE / 'owned_creator.rs').read_text()
        self.assertNotIn('namespace_identity(', raw)
        self.assertNotIn('owner.session.check(deadline)', raw)
        self.assertIn('fn local_identity(file: &File, deadline: Instant)', raw)
        self.assertIn('let proc_ns = File::open("/proc/thread-self/ns").map(ManuallyDrop::new)', raw)
        self.assertIn('let current = File::open("/proc/thread-self/ns/net").map(ManuallyDrop::new)', raw)
        self.assertIn('filesystem.map_err(|_| REFUSE)?.filesystem_type() == PROC_SUPER_MAGIC', raw)
        self.assertIn('actual.map_err(|_| REFUSE)? == owner.session.local', raw)

    def test_frozen_pin_zero_refusal_and_original_exec_then_ready_before_live_verifier(self):
        image = (HERE / 'child_executable.rs').read_text()
        # Exact reviewed frozen bytes, not merely any nonzero replacement.
        sha = re.search(r'const SHA: \[u8; 32\] = \[([^\]]+)\];', image)
        self.assertIsNotNone(sha)
        actual = bytes(int(part.strip(), 16) for part in sha.group(1).split(','))
        self.assertEqual(actual.hex(), '7838d1c3b1b26fa247d0bb16608153f576477c442817f8e1528f6f9c85fe6311')
        self.assertIn('const SIZE: u64 = 1_475_200;', image)
        self.assertIn('const CHILD: &str = "/run/omavless-k1-owned-inventory-v1/child";', image)
        self.assertNotIn('omavless-k1-owned-launch-v1', image)
        self.assertEqual(image.count('"/run/omavless-k1-owned-inventory-v1"'), 2)
        self.assertIn('index < 3 && (mode & 0o7777 == 0o755 || (index == 0 && mode & 0o7777 == 0o555))', image)
        self.assertIn('info.is_dir() && info.uid()==0 && info.gid()==0 && directory_mode(index, info.mode())', image)
        # Preserve the zero-pin and bounded-size refusal BEFORE original I/O.
        self.assertIn('require(SHA != [0; 32] && (64..=16*1024*1024).contains(&SIZE))?;', image)
        self.assertLess(image.index('require(SHA !='), image.index('let root = retain_after(open('))
        self.assertIn('info.nlink()==1 && info.size()==SIZE', image)
        self.assertIn('static_elf(&bytes) && <[u8;32]>::from(Sha256::digest(&bytes))==SHA', image)
        self.assertIn('file.as_raw_fd() >= 3', image)
        self.assertIn('format!("/proc/self/fd/{}",self.file.as_raw_fd())', image)
        source = (HERE / 'owned_launcher.rs').read_text()
        self.assertLess(source.index('Executable::admit(deadline)'), source.index('File::open("/proc/thread-self/ns/net")', source.index('pub(crate) fn open_fixed')))
        self.assertLess(source.index('child.ready(deadline)'), source.index('creator.attach_launch'))
        self.assertNotIn('Command::', source)
        self.assertLess(source.index('self.acquired.sealed=true'), source.index('self.life.finish()'))
        finish=source[source.index('struct FinishBackend'):source.index('\nstruct Verify')]
        self.assertIn('WaitStatus::Exited(actual,0) if actual==pid', finish)
        sequence=(HERE/'completion.rs').read_text()
        self.assertLess(sequence.index('backend.done_frame()'),sequence.index('backend.observe()'))
        self.assertLess(sequence.index('backend.observe()'),sequence.index('backend.eof()'))
        self.assertLess(sequence.index('backend.eof()'),sequence.index('backend.reap_exact_zero()'))

    def test_external_posix_patch_exact_scope_and_error_ownership(self):
        metadata=json.loads((HERE/'spawn-upstream.json').read_text())
        raw=(HERE/metadata['patch']).read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(),metadata['patch_sha256'])
        patch=raw.decode()
        self.assertEqual(patch.count('diff --git '),1)
        self.assertIn('diff --git a/src/spawn.rs b/src/spawn.rs',patch)
        for fragment in ('fn posix_result(', 'unsafe fn initialized<T>', 'unsafe fn reinitialize<T>',
                         'destroy_or_reinit_error_does_not_destroy_again',
                         'failed_initializer_never_reads_uninitialized_output'):
            self.assertIn(fragment,patch)
        self.assertEqual(metadata['commit'],'e35c00891f52468979f92b795de2dc1f3dd58a87')
        self.assertFalse(metadata['normal_product_constructor'])

    def test_owned_spawn_has_two_original_pairs_and_only_three_fixed_duplications(self):
        source=(HERE/'owned_child.rs').read_text()
        self.assertEqual(source.count('pipe2('),1)
        self.assertEqual(source.count('posix_spawn('),1)
        sequence=(HERE/'spawn_sequence.rs').read_text()
        self.assertEqual(sequence.count('acquired(backend.pipe(), backend)'),2)
        self.assertIn('(B::descriptor(&child_write), 1), (B::descriptor(&child_write), 2)',sequence)
        self.assertIn('let environment:[&CStr;0]=[]',source)
        self.assertIn('_same_thread: PhantomData<Rc<()>>',source)
        self.assertNotIn('Command::',source)
        self.assertNotIn('waitpid(',source)
        self.assertNotIn('kill(',source)
        self.assertNotIn('unsafe',source)
        launcher=(HERE/'owned_launcher.rs').read_text()
        self.assertLess(launcher.index('acquired.with_lease(|_|Ok(()))?'),launcher.index('life.child.complete_handoff'))

if __name__ == '__main__':
    unittest.main()
