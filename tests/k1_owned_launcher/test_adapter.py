"""Source-only controls. No exported Rust function, ELF, socket or child runs."""
import importlib.util
import hashlib
import json
import re
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

def pinned_original(name, raw):
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
        # CI's shallow checkout need not contain BASE. All six public originals
        # are admitted by exact pins; the fixed cfg(test)-only addition is
        # explicitly removed after complete-source admission, never skipped.
        # Developer export still requires its immutable BASE.
        cls.sources = {name: pinned_original(name,
                       (ROOT / 'crates/omavless-netguard/src' / name).read_bytes())
                       for name in a.PINS}
        cls.adapted = a.adapt(cls.sources)

    def test_successor_projection_refuses_any_other_source_change(self):
        name = 'kernel_inventory.rs'
        raw = (ROOT / 'crates/omavless-netguard/src' / name).read_bytes()
        self.assertEqual(pinned_original(name, raw), self.sources[name])
        self.assertEqual(pinned_original(name, self.sources[name]), self.sources[name])
        for changed in (raw + b' ', raw.replace(b'#[cfg(test)]', b'#[cfg(any())]', 1),
                        raw.replace(b'create_witness;', b'other_witness;', 1)):
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
