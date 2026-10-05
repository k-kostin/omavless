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

class Controls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # CI's shallow checkout need not contain BASE. The six unchanged public
        # originals are admitted by EXACT existing SHA-256 pins, never fallback
        # acceptance or skip. Developer export still requires its immutable BASE.
        cls.sources = {name: (ROOT / 'crates/omavless-netguard/src' / name).read_bytes()
                       for name in a.PINS}
        cls.adapted = a.adapt(cls.sources)

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
        self.assertIn('const CHILD: &str = "/run/omavless-k1-owned-launch-v3/child";', image)
        self.assertNotIn('omavless-k1-owned-launch-v1', image)
        self.assertEqual(image.count('"/run/omavless-k1-owned-launch-v3"'), 2)
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
