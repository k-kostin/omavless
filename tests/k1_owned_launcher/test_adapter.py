"""Source-only controls. No exported Rust function, ELF, socket or child runs."""
import importlib.util
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

    def test_zero_pin_and_original_exec_then_ready_before_live_verifier(self):
        image = (HERE / 'child_executable.rs').read_text()
        self.assertIn('const SHA: [u8; 32] = [0; 32]', image)
        self.assertLess(image.index('require(SHA !='), image.index('let root = retain_after(open('))
        self.assertIn('file.as_raw_fd() >= 3', image)
        self.assertIn('format!("/proc/self/fd/{}",self.file.as_raw_fd())', image)
        source = (HERE / 'owned_launcher.rs').read_text()
        self.assertLess(source.index('Executable::admit(deadline)'), source.index('File::open("/proc/thread-self/ns/net")', source.index('pub(crate) fn open_fixed')))
        self.assertLess(source.index('protocol::READY'), source.index('creator.attach_launch'))
        self.assertIn('Command::new(executable.exec_path()).arg0(CHILD)', source)
        self.assertLess(source.index('self.acquired.sealed=true'), source.index('self.life.finish()'))
        finish=source[source.index('    fn finish(&self)'):source.index('\nstruct Verify')]
        self.assertLess(finish.index('protocol::DONE'),finish.index('WaitPidFlag::WNOWAIT'))
        self.assertLess(finish.index('WaitPidFlag::WNOWAIT'),finish.index('protocol::eof'))
        self.assertLess(finish.index('protocol::eof'),finish.index('nix::sys::wait::waitpid'))
        self.assertIn('WaitStatus::Exited(actual,0) if actual==pid', finish)

if __name__ == '__main__':
    unittest.main()
