"""Inert mapping originals/identity controls; no process or candidate execution."""
import ast
import importlib.util
import os
import re
import stat
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

SOURCE=Path(__file__).with_name('images.py')
spec=importlib.util.spec_from_file_location('retained_images',SOURCE)
i=importlib.util.module_from_spec(spec);spec.loader.exec_module(i)
OLD_MAP_LINE=re.compile(r'([0-9a-f]{1,16})-([0-9a-f]{1,16}) ([r-][w-][x-][ps]) ([0-9a-f]{1,16}) ([0-9a-f]{1,8}):([0-9a-f]{1,8}) ([0-9]{1,20})(?:[ \t]+([^\r\n]+))?')


class Session:
    def __init__(self):
        self.kind='inner';self.sealed=False;self.children=[];self.anchors={}
        self.available=Mock();self.live=Mock();self.phase=Mock()


class Child:pass
class Bridge:pass
class Artifacts:pass


def mapping(paths):
    return ''.join(f'{index*4096:x}-{(index+1)*4096:x} r-xp 00000000 00:1f {inode} {path}\n'
                   for index,(path,inode) in enumerate(paths,1))


class Controls(unittest.TestCase):
    def setUp(self):
        self.clock=patch.object(i.time,'monotonic',return_value=0.0);self.clock.start()
        self.values=[]

    def tearDown(self):
        for value in self.values:
            for fd in value.held:os.close(fd)
        self.clock.stop()

    def fixture(self,role='core'):
        owner=Session();child=Child();owner.children=[child]
        owner.anchors[role]={'child':child,'proc_fd':71}
        ownership=SimpleNamespace(Session=Session,OwnedProcess=Child)
        copies=Bridge();copies.state='ready';copies.verify=Mock();copies._verify_target=Mock()
        names=['/usr/bin/dbus-daemon','/usr/lib/systemd/systemd-resolved',
               '/usr/lib/libc.so.6','/usr/lib/ld-linux-x86-64.so.2']
        names.extend(f'/usr/lib/fixed{index}.so' for index in range(21))
        copies.records={path:{'device':31,'inode':index,'size':64,'sha256':'a'*64}
                        for index,path in enumerate(names,1)}
        copies.fds={path:100+index for index,path in enumerate(names)}
        artifacts=Artifacts();artifacts.sealed=False;artifacts.recheck=Mock()
        artifacts.files={name:(55,SimpleNamespace(st_dev=31,st_ino=90),'b'*64)
                         for name in ('developer-manifest.json','mihomo','omavless-dns-broker','host-fixture')}
        def native(name,device,inode,deadline):
            self.assertEqual((device,inode),(31,90))
            self.assertIs(type(deadline),float)
            return {'path':'/artifacts/'+name,'device':device,'inode':inode,'size':64,'sha256':'b'*64}
        artifacts.mapped_identity=Mock(side_effect=native)
        value=i.Images(owner,ownership,copies,SimpleNamespace(Bridge=Bridge),artifacts,
                       SimpleNamespace(Sources=Artifacts,TABLE=artifacts.files))
        value.executable=Mock(return_value=(31,90 if role in ('core','broker','host')
                                           else copies.records[i.ROLES[role]]['inode']))
        self.values.append(value)
        return value,child

    def maps(self,value,role):
        paths=[(i.ROLES[role],90)] if role in ('core','broker','host') else []
        paths.extend((path,row['inode']) for path,row in value.copies.records.items()
                     if path==i.ROLES[role] or path in ('/usr/lib/libc.so.6','/usr/lib/ld-linux-x86-64.so.2'))
        return mapping(paths)

    def sealed(self,value):
        self.assertTrue(value.sealed and value.owner.sealed and value.artifacts.sealed)
        self.assertEqual(value.copies.state,'refused')

    def test_all_five_role_loaded_objects_match_then_complete_reread(self):
        for role in i.ROLES:
            value,child=self.fixture(role);raw=self.maps(value,role)
            with patch.object(value,'text',side_effect=[raw,raw]) as read:
                result=value.inventory(child,5.0,'initial_'+role)
            self.assertEqual(read.call_count,2)
            self.assertIn(i.ROLES[role],{row['path'] for row in result})
            self.assertFalse(value.sealed)
            self.assertEqual(value.artifacts.mapped_identity.call_count,int(role in ('core','broker','host')))

    def test_unknown_path_wrong_copy_identity_or_other_native_never_hashes(self):
        for path,inode in (('/usr/lib/unadmitted.so',5),('/usr/lib/libc.so.6',999),
                           ('/artifacts/host-fixture',90),('/artifacts/mihomo',91)):
            value,child=self.fixture();raw=mapping([(path,inode)])
            if path=='/artifacts/mihomo':value.artifacts.mapped_identity.side_effect=i.Refused()
            with patch.object(value,'text',return_value=raw):
                with self.assertRaises(i.Refused):value.inventory(child,5.0)
            value.copies._verify_target.assert_not_called()
            if path!='/artifacts/mihomo':value.artifacts.mapped_identity.assert_not_called()
            self.sealed(value)

    def test_changed_complete_mapping_or_live_anchor_permanently_seals(self):
        for variant in ('map','live'):
            value,child=self.fixture();raw=self.maps(value,'core')
            after=raw.replace(' 90 /artifacts',' 91 /artifacts') if variant=='map' else raw
            if variant=='live':value.owner.live.side_effect=RuntimeError('private must not escape')
            with patch.object(value,'text',side_effect=[raw,after]):
                with self.assertRaisesRegex(i.Refused,'^fixed_loaded_image_refused$'):value.inventory(child,5.0)
            self.sealed(value)
            with patch.object(value,'text') as read:
                with self.assertRaises(i.Refused):value.inventory(child,5.0)
                read.assert_not_called()

    def test_concrete_child_membership_role_and_context_precede_maps_read(self):
        for variant in ('type','role','duplicate','context'):
            value,child=self.fixture()
            if variant=='type':child=SimpleNamespace()
            if variant=='role':value.owner.anchors={'wrong':value.owner.anchors['core']}
            if variant=='duplicate':value.owner.anchors['host']=value.owner.anchors['core']
            with patch.object(value,'text') as read:
                with self.assertRaises(i.Refused):value.inventory(child,5.0,'final_host' if variant=='context' else None)
                read.assert_not_called()
            self.sealed(value)

    def test_original_proc_fd_literal_maps_two_reads_no_candidate_open(self):
        value,child=self.fixture();raw=self.maps(value,'core').encode()
        with tempfile.TemporaryDirectory() as temp:
            path=Path(temp)/'maps';path.write_bytes(raw)
            directory=os.open(temp,os.O_RDONLY|os.O_DIRECTORY)
            try:
                value.owner.anchors['core']['proc_fd']=directory
                real_open=os.open
                def opened(name,flags,**kwargs):
                    self.assertEqual(name,'maps');self.assertEqual(kwargs,{'dir_fd':directory})
                    self.assertEqual(flags,i.FLAGS);return real_open(name,flags,**kwargs)
                with patch.object(i.os,'open',side_effect=opened) as read:
                    result=value.inventory(child,5.0)
                self.assertEqual(read.call_count,2);self.assertEqual(len(value.held),2)
                self.assertEqual(len(result),3)
            finally:os.close(directory)

    def test_maps_open_throw_or_late_retains_descriptor_and_never_fstats(self):
        for variant in ('throw','late'):
            value,child=self.fixture();clock=[0.0]
            with tempfile.TemporaryFile() as temp:
                fd=os.dup(temp.fileno())
                def opened(*args,**kwargs):
                    if variant=='throw':raise RuntimeError('private')
                    clock[0]=5.0;return fd
                with patch.object(i.time,'monotonic',side_effect=lambda:clock[0]), \
                     patch.object(i.os,'open',side_effect=opened) as opened_mock,patch.object(i.os,'fstat') as metadata:
                    with self.assertRaises(i.Refused):value.text(child,5.0)
                    metadata.assert_not_called();clock[0]=0.0
                    with self.assertRaises(i.Refused):value.text(child,5.0)
                    self.assertEqual(opened_mock.call_count,1)
                if variant=='throw':os.close(fd)
                else:self.assertIn(fd,value.held)
            self.sealed(value)

    def test_nonfinite_wrong_type_deadline_or_clock_precedes_maps_open(self):
        for bad in (True,1,float('nan'),float('inf')):
            for variant in ('deadline','clock'):
                value,child=self.fixture()
                with patch.object(i.time,'monotonic',return_value=bad if variant=='clock' else 0.0), \
                     patch.object(i.os,'open') as opened:
                    with self.assertRaises(i.Refused):value.text(child,bad if variant=='deadline' else 5.0)
                    opened.assert_not_called()
                self.sealed(value)

    def test_late_read_or_exception_prevents_next_io_after_clock_recovers(self):
        for variant in ('late','throw'):
            value,child=self.fixture();clock=[0.0];operation=Mock()
            def read():
                if variant=='throw':raise RuntimeError('private')
                clock[0]=5.0;return b'private'
            with patch.object(i.time,'monotonic',side_effect=lambda:clock[0]):
                with self.assertRaises(i.Refused):value.io(5.0,read)
                clock[0]=0.0
                with self.assertRaises(i.Refused):value.io(5.0,operation)
                operation.assert_not_called()
            self.sealed(value)

    def test_missing_executable_or_daemon_required_loader_refuses(self):
        for role in ('core','bus','resolved'):
            value,child=self.fixture(role)
            raw=mapping([('/usr/lib/libc.so.6',3)]) if role=='core' else mapping([(i.ROLES[role],1 if role=='bus' else 2)])
            with patch.object(value,'text',return_value=raw):
                with self.assertRaises(i.Refused):value.inventory(child,5.0)
            value.copies._verify_target.assert_not_called();value.artifacts.mapped_identity.assert_not_called()
            self.sealed(value)

    def test_whole_batch_bad_late_member_or_identity_precedes_any_target_hash(self):
        for bad,inode in (('/usr/lib/zz-unadmitted.so',90),('/usr/lib/libc.so.6',999),
                          ('/artifacts/host-fixture',90)):
            value,child=self.fixture()
            raw=mapping([('/artifacts/mihomo',90),('/usr/lib/ld-linux-x86-64.so.2',4),(bad,inode)])
            with patch.object(value,'text',return_value=raw):
                with self.assertRaises(i.Refused):value.inventory(child,5.0)
            value.copies._verify_target.assert_not_called();value.artifacts.mapped_identity.assert_not_called()
            self.sealed(value)

    def test_verify_late_or_throw_preserves_all_originals_and_no_followup(self):
        for variant in ('late','throw'):
            value,child=self.fixture();clock=[0.0]
            def verified(*args):
                if variant=='throw':raise RuntimeError('private')
                clock[0]=5.0
            value.copies.verify.side_effect=verified
            with patch.object(i.time,'monotonic',side_effect=lambda:clock[0]):
                with self.assertRaises(i.Refused):value.verify(5.0)
            value.artifacts.recheck.assert_not_called();self.sealed(value)

    def test_maps_parser_refuses_private_deleted_conflict_overflow_and_excess(self):
        bad=[mapping([('/home/private',1)]),mapping([('/usr/lib/libc.so.6 (deleted)',1)]),
             mapping([('/usr/lib/libc.so.6',1),('/usr/lib/libc.so.6',2)]),
             mapping([('/usr/lib/libc.so.6',2**64)]),
             mapping([(f'/usr/lib/f{index}',index+1) for index in range(65)]),
             '0-10000000000000000 r-xp 0 00:1f 1 /usr/lib/libc.so.6\n',
             '0-1000 rw-p 1 00:00 0 [heap]\n', '', 'x'*(1024*1024+1)]
        for raw in bad:
            with self.assertRaisesRegex(i.Refused,'^fixed_loaded_image_refused$'):i.map_objects(raw)
        self.assertEqual(i.map_objects(mapping([('/usr/lib/libc.so.6',1)])),{'/usr/lib/libc.so.6':(31,1)})

    def test_constructor_only_exact_pinned_classes_before_any_fs_read(self):
        with patch.object(i.os,'open') as opened:
            with self.assertRaises(i.Refused):i.Images(SimpleNamespace(sealed=False),SimpleNamespace(Session=Session),
                SimpleNamespace(state='ready'),SimpleNamespace(Bridge=Bridge),
                SimpleNamespace(sealed=False),SimpleNamespace(Sources=Artifacts))
            opened.assert_not_called()

    def test_go_annotation_is_bounded_zero_identity_only_and_discarded(self):
        original=mapping([('/usr/lib/libc.so.6',1)])
        for label in ('heap','heap reservation','gc bits','page alloc index','a'*74):
            annotated=original+'3000-4000 rw-p 0 00:00 0 [anon: Go: '+label+']\n'
            self.assertEqual(i.map_objects(annotated),i.map_objects(original))
        for row in ('3000-4000 rw-p 0 00:01 1 [anon: Go: heap]',
                    '3000-4000 rw-p 0 00:00 1 [anon: Go: heap]',
                    '3000-4000 rw-p 1 00:00 0 [anon: Go: heap]',
                    '3000-4000 rw-p 0 00:00 0 [anon: Other: heap]',
                    '3000-4000 rw-p 0 00:00 0 [anon: Go: '+('a'*75)+']',
                    '3000-4000 rw-p 0 00:00 0 [anon: Go: /home/private]',
                    '3000-4000 rw-p 0 00:00 0 [anon: Go: heap\tindex]'):
            with self.assertRaises(i.Refused):i.map_objects(original+row+'\n')

    def test_glibc_producer_annotation_counterexamples_remain_refused(self):
        # Producer-side source hypothesis only; no actual stopped scope read.
        # No policy relaxation: these spaced labels are not in the frozen
        # ANONYMOUS grammar and must refuse before any mapped-target hash.
        for label in ('malloc','malloc arena','loader malloc'):
            value,child=self.fixture('bus')
            raw=self.maps(value,'bus')+'5000-6000 rw-p 0 00:00 0 [anon: glibc: '+label+']\n'
            with patch.object(value,'text',return_value=raw) as text:
                with self.assertRaisesRegex(i.Refused,'^fixed_loaded_image_refused$'):
                    value.inventory(child,5.0,'initial_bus')
            self.assertEqual(text.call_count,1)
            value.copies._verify_target.assert_not_called()
            value.artifacts.mapped_identity.assert_not_called()
            self.sealed(value)
            with patch.object(value,'text') as again:
                with self.assertRaises(i.Refused):value.inventory(child,5.0,'initial_bus')
                again.assert_not_called()

    def test_kernel_unnamed_producer_separator_counterexample_frozen_parser_refuses(self):
        # Linux v6.17 show_vma_header_prefix always writes a separator after
        # inode; show_map_vma appends LF without a path for an unnamed VMA.
        # No stopped process/maps are inspected.
        row='5000-6000 rw-p 00000000 00:00 0 '
        self.assertIsNone(OLD_MAP_LINE.fullmatch(row))
        original=mapping([('/usr/lib/libc.so.6',1)])
        self.assertEqual(i.map_objects(original+row.rstrip(' ')+'\n'),i.map_objects(original))
        with patch.object(i,'MAP_LINE',OLD_MAP_LINE):
            with self.assertRaises(i.Refused):i.map_objects(original+row+'\n')

    def test_kernel_unnamed_counterexample_can_end_at_plain_bracket_boundary(self):
        value,child=self.fixture('bus')
        raw=self.maps(value,'bus')+'5000-6000 rw-p 0 00:00 0 [heap]\n6000-7000 rw-p 0 00:00 0 \n'
        with patch.object(value,'text',return_value=raw) as read,patch.object(i,'MAP_LINE',OLD_MAP_LINE):
            with self.assertRaises(i.Refused):value.inventory(child,5.0,'initial_bus')
        self.assertEqual(value.owner.phase.call_args.args,
                         ('before_bus_initial_inventory_first_parse_plain_bracket',5.0))
        self.assertEqual(read.call_count,1)
        value.copies._verify_target.assert_not_called();self.sealed(value)
        # Matching a public last literal is not evidence that this row existed
        # or caused any actual stopped invocation.

    def test_exact_kernel_single_ascii_separator_discards_only_unnamed_zero_row(self):
        original=mapping([('/usr/lib/libc.so.6',1)])
        row='5000-6000 rw-p 00000000 00:00 0 '
        self.assertIsNone(i.MAP_LINE.fullmatch(row).group(8))
        self.assertEqual(i.map_objects(original+row+'\n'),i.map_objects(original))
        seen=[]
        self.assertEqual(i.map_objects(original+row+'\n',seen.append),i.map_objects(original))
        self.assertEqual(seen,['unnamed'])

    def test_corrected_kernel_single_separator_inventory_complete_positive(self):
        value,child=self.fixture('bus')
        raw=self.maps(value,'bus')+'5000-6000 rw-p 0 00:00 0 [heap]\n6000-7000 rw-p 0 00:00 0 \n'
        with patch.object(value,'text',return_value=raw) as read:
            result=value.inventory(child,5.0,'initial_bus')
        self.assertEqual(read.call_count,2);self.assertFalse(value.sealed)
        self.assertEqual({row['path'] for row in result},set(i.map_objects(self.maps(value,'bus'))))
        labels=[call.args[0] for call in value.owner.phase.call_args_list]
        for step in ('first_parse','second_parse'):
            self.assertEqual(labels.count('before_bus_initial_inventory_'+step+'_unnamed'),1)
        self.assertEqual(labels[-1],'after_bus_initial_inventory_final_live')

    def test_unnamed_correction_refuses_multiple_blank_tab_or_line_aliases(self):
        base='5000-6000 rw-p 0 00:00 0'
        original=mapping([('/usr/lib/libc.so.6',1)])
        for suffix in ('  ','\t',' \t','\t ','\t\t',' \r',' \n','\n',' \x0b'):
            if '\n' in suffix or '\r' in suffix:
                self.assertIsNone(i.MAP_LINE.fullmatch(base+suffix))
            with self.assertRaises(i.Refused):i.map_objects(original+base+suffix+'\n')

    def test_lf_exact_rows_reject_old_splitlines_alias_normalization(self):
        row='5000-6000 r-xp 0 00:1f 1 /usr/lib/libc.so.6'
        expected={'/usr/lib/libc.so.6':(31,1)}
        for raw in (row,row+'\n'):
            self.assertEqual(i.map_objects(raw),expected)
        for ending in ('\r\n','\r','\v','\f','\x85','\u2028','\u2029'):
            raw=row+ending
            # Freeze the old normalization as a counterexample, not a
            # production parser import or an observed process/maps claim.
            self.assertEqual(raw.splitlines(),[row])
            self.assertEqual(i.map_objects('\n'.join(raw.splitlines())),expected)
            with self.assertRaises(i.Refused):i.map_objects(raw)
        for raw in (row+'\n\n','\n'+row+'\n',row+'\n\n'+row+'\n'):
            with self.assertRaises(i.Refused):i.map_objects(raw)

    def test_unnamed_single_separator_still_requires_zero_device_inode_and_offset(self):
        for device,inode,offset in (('00:01',0,0),('00:00',1,0),('00:00',0,1)):
            value,child=self.fixture('bus')
            raw=self.maps(value,'bus')+f'5000-6000 rw-p {offset} {device} {inode} \n'
            with patch.object(value,'text',return_value=raw) as read:
                with self.assertRaises(i.Refused):value.inventory(child,5.0,'initial_bus')
            self.assertEqual(read.call_count,1)
            value.copies._verify_target.assert_not_called();self.sealed(value)
            self.assertEqual(value.owner.phase.call_args.args,
                             ('before_bus_initial_inventory_first_parse_invalid_zero_identity',5.0))

    def test_named_single_separator_path_and_padding_remain_exact_without_stripping(self):
        path='/usr/lib/libc.so.6'
        for separator in (' ','   ','\t',' \t '):
            raw='5000-6000 r-xp 0 00:1f 1'+separator+path
            self.assertEqual(i.MAP_LINE.fullmatch(raw).group(8),path)
            self.assertEqual(i.map_objects(raw+'\n'),{path:(31,1)})
        for changed in (path+' ',path+'\t',path+' (deleted)',path+'\nprivate',
                        '/usr/lib/../libc.so.6','/usr/lib/unknown.so '):
            raw='5000-6000 r-xp 0 00:1f 1 '+changed+'\n'
            with self.assertRaises(i.Refused):i.map_objects(raw)

    def test_go_and_glibc_annotation_predicates_are_not_relaxed_by_separator_fix(self):
        original=mapping([('/usr/lib/libc.so.6',1)])
        accepted='5000-6000 rw-p 0 00:00 0 [anon: Go: heap]\n'
        self.assertEqual(i.map_objects(original+accepted),i.map_objects(original))
        for annotation in ('[anon: Go: heap] ','[anon: Go: /private]',
                           '[anon: glibc: malloc]','[anon: glibc: malloc arena]',
                           '[anon: glibc: loader malloc]'):
            with self.assertRaises(i.Refused):i.map_objects(original+'5000-6000 rw-p 0 00:00 0 '+annotation+'\n')

    def test_initial_bus_host_and_core_exact_substeps_and_accepted_classes_once_per_parse(self):
        for role in i.INVENTORY_ROLES:
            value,child=self.fixture(role)
            rows=['','[heap]','[anon: Go: heap]','[heap]','[anon: Go: heap]']
            raw=self.maps(value,role)+''.join(
                f'{(n+5)*4096:x}-{(n+6)*4096:x} rw-p 0 00:00 0'+(' '+label if label else '')+'\n'
                for n,label in enumerate(rows))
            with patch.object(value,'text',return_value=raw):
                result=value.inventory(child,5.0,f'initial_{role}')
            expected=[]
            for step in i.INVENTORY_STEPS:
                expected.append(f'before_{role}_initial_inventory_'+step)
                if step in ('first_parse','second_parse'):
                    expected.extend(f'before_{role}_initial_inventory_'+step+'_'+category
                                    for category in ('unnamed','plain_bracket','go'))
                if role in i.REQUIRED_ROLES and step=='required_members':
                    expected.extend(f'before_{role}_initial_inventory_required_members_'+category
                                    for category in ('present','identity_equal'))
                expected.append(f'after_{role}_initial_inventory_'+step)
            self.assertEqual(value.owner.phase.call_args_list,
                             [unittest.mock.call(label,5.0) for label in expected])
            self.assertEqual(len(expected),28+2*int(role in i.REQUIRED_ROLES))
            self.assertEqual({row['path'] for row in result},set(i.map_objects(self.maps(value,role))))
            self.assertTrue(getattr(value,'initial_'+role+'_observed'));self.assertFalse(value.sealed)

    def test_anonymous_rejecting_classes_precede_unchanged_predicate_no_targets(self):
        for role in i.INVENTORY_ROLES:
            cases=(('[anon: glibc: malloc]','00:00',0,'glibc_malloc'),
                   ('[anon: glibc: malloc arena]','00:00',0,'glibc_malloc_arena'),
                   ('[anon: glibc: loader malloc]','00:00',0,'glibc_loader_malloc'),
                   ('[private annotation]','00:00',0,'foreign_bracket'),
                   ('[heap]','00:01',1,'invalid_zero_identity'))
            for label,device,inode,category in cases:
                value,child=self.fixture(role)
                raw=self.maps(value,role)+f'5000-6000 rw-p 0 {device} {inode} {label}\n'
                with patch.object(value,'text',return_value=raw) as read:
                    with self.assertRaises(i.Refused):value.inventory(child,5.0,f'initial_{role}')
                self.assertEqual(value.owner.phase.call_args.args,
                                 (f'before_{role}_initial_inventory_first_parse_'+
                                  ('reject_anonymous' if role=='core' else category),5.0))
                self.assertEqual(read.call_count,1)
                value.copies._verify_target.assert_not_called();self.sealed(value)
            for device,inode,offset in (('00:00',1,0),('00:00',0,1),('00:01',0,0)):
                seen=[]
                with self.assertRaises(i.Refused):
                    i.map_objects(mapping([('/usr/lib/libc.so.6',1)])+
                        f'5000-6000 rw-p {offset} {device} {inode} [heap]\n',seen.append)
                self.assertEqual(seen,['invalid_zero_identity'])

    def test_unknown_class_refuses_without_emitting_raw_label(self):
        for role in i.INVENTORY_ROLES:
            value,child=self.fixture(role)
            with patch.object(value,'text',return_value=self.maps(value,role)+
                              '5000-6000 rw-p 0 00:00 0 [heap]\n'), \
                 patch.object(i,'anonymous_class',return_value='private arbitrary value'):
                with self.assertRaises(i.Refused):value.inventory(child,5.0,f'initial_{role}')
            self.assertEqual(value.owner.phase.call_args.args,
                             (f'before_{role}_initial_inventory_first_parse',5.0))
            value.copies._verify_target.assert_not_called();self.sealed(value)

    def test_only_single_initial_bus_host_or_core_inventory_can_emit_diagnostics(self):
        for role,context in (('core',None),('core','final_core'),('resolved','initial_resolved'),
                             ('broker','initial_broker'),('bus',None),('bus','final_bus'),
                             ('host',None),('host','final_host')):
            value,child=self.fixture(role)
            with patch.object(value,'text',return_value=self.maps(value,role)):
                value.inventory(child,5.0,context)
            value.owner.phase.assert_not_called()
            self.assertFalse(value.initial_bus_observed or value.initial_host_observed or value.initial_core_observed)
        for role in i.INVENTORY_ROLES:
            value,child=self.fixture(role)
            with patch.object(value,'text',return_value=self.maps(value,role)) as read:
                value.inventory(child,5.0,'initial_'+role)
                count=value.owner.phase.call_count
                with self.assertRaises(i.Refused):value.inventory(child,5.0,'initial_'+role)
            self.assertEqual(read.call_count,2);self.assertEqual(value.owner.phase.call_count,count)
            self.sealed(value)

    def test_bus_host_and_core_latches_are_independent_in_same_retained_images(self):
        value,bus=self.fixture('bus');host=Child();core=Child()
        value.owner.children.extend((host,core))
        value.owner.anchors.update(host={'child':host,'proc_fd':72},core={'child':core,'proc_fd':73})
        with patch.object(value,'text',return_value=self.maps(value,'bus')):
            value.inventory(bus,5.0,'initial_bus')
        self.assertTrue(value.initial_bus_observed);self.assertFalse(value.initial_host_observed)
        value.executable.return_value=(31,90)
        with patch.object(value,'text',return_value=self.maps(value,'host')):
            value.inventory(host,5.0,'initial_host')
        self.assertTrue(value.initial_bus_observed and value.initial_host_observed)
        self.assertFalse(value.initial_core_observed)
        with patch.object(value,'text',return_value=self.maps(value,'core')):
            value.inventory(core,5.0,'initial_core')
        self.assertTrue(value.initial_core_observed)
        self.assertEqual(value.owner.phase.call_count,70);self.assertFalse(value.sealed)

    def test_wrong_role_or_unknown_context_refuses_before_diagnostics_and_maps(self):
        for role,context in (('host','initial_bus'),('bus','initial_host'),
                             ('core','initial_host'),('host','initial_core'),('bus','initial_core'),('host','initial_private')):
            value,child=self.fixture(role)
            with patch.object(value,'text') as read:
                with self.assertRaises(i.Refused):value.inventory(child,5.0,context)
            read.assert_not_called();value.owner.phase.assert_not_called()
            self.assertFalse(value.initial_bus_observed or value.initial_host_observed or value.initial_core_observed)
            self.sealed(value)

    def test_each_substep_or_class_label_failure_has_no_next_label_or_target(self):
        for role in i.INVENTORY_ROLES:
            raw_suffix='5000-6000 rw-p 0 00:00 0 [heap]\n'
            labels=[]
            for step in i.INVENTORY_STEPS:
                labels.append(f'before_{role}_initial_inventory_'+step)
                if step in ('first_parse','second_parse'):
                    labels.append(f'before_{role}_initial_inventory_'+step+'_plain_bracket')
                if role in i.REQUIRED_ROLES and step=='required_members':
                    labels.extend(f'before_{role}_initial_inventory_required_members_'+category
                                  for category in ('present','identity_equal'))
                labels.append(f'after_{role}_initial_inventory_'+step)
            for index,label in enumerate(labels):
                value,child=self.fixture(role)
                def phase(current,deadline):
                    if current==label:raise RuntimeError('private synthetic detail')
                value.owner.phase.side_effect=phase
                with patch.object(value,'text',return_value=self.maps(value,role)+raw_suffix):
                    with self.assertRaisesRegex(i.Refused,'^fixed_loaded_image_refused$'):
                        value.inventory(child,5.0,f'initial_{role}')
                self.assertEqual(value.owner.phase.call_count,index+1)
                self.assertEqual(value.owner.phase.call_args.args,(label,5.0))
                if index<labels.index(f'before_{role}_initial_inventory_targets'):
                    value.copies._verify_target.assert_not_called()
                self.sealed(value)

    def test_late_substep_or_label_return_cannot_emit_after_label(self):
        for role in i.INVENTORY_ROLES:
            for variant in ('text','phase'):
                value,child=self.fixture(role);now=[0.0]
                def read(*args):
                    now[0]=5.0;return self.maps(value,role)
                if variant=='phase':
                    def phase(label,deadline):now[0]=5.0
                    value.owner.phase.side_effect=phase
                with patch.object(i.time,'monotonic',side_effect=lambda:now[0]), \
                     patch.object(value,'text',side_effect=read) as text:
                    with self.assertRaises(i.Refused):value.inventory(child,5.0,f'initial_{role}')
                expected=f'before_{role}_initial_inventory_first_text' if variant=='text' else f'before_{role}_initial_inventory_executable'
                self.assertEqual(value.owner.phase.call_args.args,(expected,5.0))
                self.assertEqual(text.call_count,int(variant=='text'))
                value.copies._verify_target.assert_not_called();self.sealed(value)

    def test_closed_inventory_vocabulary_matches_owner_and_conservative_path_budget(self):
        spec=importlib.util.spec_from_file_location('budget_owner',SOURCE.with_name('lifecycle.py'))
        owner=importlib.util.module_from_spec(spec);spec.loader.exec_module(owner)
        self.assertEqual(i.INVENTORY_ROLES,owner.INVENTORY_ROLES)
        self.assertEqual(i.INVENTORY_ROLES,('bus','host','core'))
        self.assertEqual(i.INVENTORY_STEPS,owner.INVENTORY_STEPS)
        self.assertEqual(i.ANONYMOUS_CLASSES,owner.ANONYMOUS_CLASSES)
        self.assertEqual(i.REQUIRED_ROLES,owner.REQUIRED_ROLES)
        self.assertEqual(i.REQUIRED_ROLES,('host','core'))
        self.assertEqual(i.REQUIRED_CLASSES,owner.REQUIRED_CLASSES)
        self.assertEqual(i.REQUIRED_CLASSES,('present','absent','identity_equal','identity_different'))
        self.assertEqual(i.PARSE_REJECTIONS,owner.PARSE_REJECTIONS)
        self.assertEqual(i.PARSE_REJECTIONS,('shape','range','anonymous','named_path',
                                            'named_identity','object_count','empty'))
        self.assertEqual((len(i.INVENTORY_STEPS),len(i.ANONYMOUS_CLASSES)),(11,8))
        labels={side+'_'+role+'_initial_inventory_'+step for role in i.INVENTORY_ROLES for step in i.INVENTORY_STEPS
                for side in ('before','after')}
        labels|={'before_'+role+'_initial_inventory_'+step+'_'+category for role in i.INVENTORY_ROLES
                 for step in ('first_parse','second_parse') for category in i.ANONYMOUS_CLASSES}
        labels|={'before_'+role+'_initial_inventory_required_members_'+category
                 for role in i.REQUIRED_ROLES for category in i.REQUIRED_CLASSES}
        labels|={'before_core_initial_inventory_'+step+'_reject_'+category
                 for step in ('first_parse','second_parse') for category in i.PARSE_REJECTIONS}
        self.assertEqual(len(labels),136);self.assertLessEqual(labels,owner.PHASES)
        self.assertTrue(all(len(('T3_RETAINED_PHASE_V1 '+label+'\n').encode('ascii'))<=128 for label in labels))
        tree=ast.parse(SOURCE.read_text())
        inventory=next(n for n in ast.walk(tree) if isinstance(n,ast.FunctionDef) and n.name=='inventory')
        marks=[(n.args[0].value,n.args[1].value) for n in ast.walk(inventory)
               if isinstance(n,ast.Call) and isinstance(n.func,ast.Name) and n.func.id=='mark']
        self.assertEqual(set(marks),{(side,step) for step in i.INVENTORY_STEPS for side in ('before','after')})
        self.assertEqual(len(marks),22)
        self.assertEqual((116+len(i.INVENTORY_ROLES)*(len(marks)+2*len(i.ANONYMOUS_CLASSES))+2*len(i.REQUIRED_ROLES),owner.PHASE_LIMIT,128+owner.PHASE_LIMIT),(234,289,417))

    def rejecting_maps(self):
        good=mapping([('/artifacts/mihomo',90)])
        excess=good+''.join(f'{n*4096:x}-{(n+1)*4096:x} r-xp 0 00:1f {n} /usr/lib/f{n}.so\n'
                            for n in range(2,66))
        return good,{
            'shape':good+'malformed synthetic row\n',
            'range':good+'1800-2800 rw-p 0 00:00 0 \n',
            'anonymous':good+'3000-4000 rw-p 0 00:00 0 [anon: glibc: malloc]\n',
            'named_path':good+'3000-4000 rw-p 0 00:1f 91 /dev/zero (deleted)\n',
            'named_identity':good+'3000-4000 r-xp 0 00:1f 0 /usr/lib/libc.so.6\n',
            'object_count':excess,
            'empty':'1000-2000 rw-p 0 00:00 0 \n',
        }

    def test_first_false_predicate_categories_preserve_every_refusal_and_stop_once(self):
        good,cases=self.rejecting_maps()
        self.assertEqual(set(cases),set(i.PARSE_REJECTIONS))
        rejected=[]
        self.assertEqual(i.map_objects(good,before_reject=rejected.append),{'/artifacts/mihomo':(31,90)})
        self.assertEqual(rejected,[])
        for category,raw in cases.items():
            rejected=[]
            with self.assertRaisesRegex(i.Refused,'^fixed_loaded_image_refused$'):
                i.map_objects(raw,before_reject=rejected.append)
            self.assertEqual(rejected,[category])
            with self.assertRaises(i.Refused):i.map_objects(raw)

    def test_last_unnamed_is_not_the_false_predicate_or_an_actual_cause(self):
        good=mapping([('/artifacts/mihomo',90)])+'2000-3000 rw-p 0 00:00 0 \n'
        seen=[];rejected=[]
        self.assertEqual(i.map_objects(good,seen.append,rejected.append),{'/artifacts/mihomo':(31,90)})
        self.assertEqual((seen,rejected),(['unnamed'],[]))
        for suffix,category in (('malformed synthetic row\n','shape'),
                ('3000-4000 rw-p 0 00:1f 91 /dev/zero (deleted)\n','named_path'),
                ('2800-4000 rw-p 0 00:00 0 \n','range'),('\n','shape')):
            seen=[];rejected=[]
            with self.assertRaises(i.Refused):i.map_objects(good+suffix,seen.append,rejected.append)
            self.assertEqual((seen,rejected),(['unnamed'],[category]))

    def test_core_only_first_or_second_parse_refusal_precedes_remaining_steps(self):
        good,cases=self.rejecting_maps()
        for step in ('first_parse','second_parse'):
            for category,raw in cases.items():
                value,child=self.fixture('core')
                texts=[raw] if step=='first_parse' else [good,raw]
                with patch.object(value,'text',side_effect=texts) as read:
                    with self.assertRaises(i.Refused):value.inventory(child,5.0,'initial_core')
                expected='before_core_initial_inventory_'+step+'_reject_'+category
                labels=[call.args[0] for call in value.owner.phase.call_args_list]
                self.assertEqual(labels[-1],expected)
                self.assertEqual(sum('_reject_' in label for label in labels),1)
                self.assertNotIn('after_core_initial_inventory_'+step,labels)
                self.assertEqual(read.call_count,1 if step=='first_parse' else 2)
                self.assertEqual(value.artifacts.mapped_identity.call_count,int(step=='second_parse'))
                self.sealed(value)
        for role,context in (('core','final_core'),('core',None),('host','initial_host'),
                             ('bus','initial_bus'),('broker','initial_broker'),('resolved','initial_resolved')):
            value,child=self.fixture(role)
            with patch.object(value,'text',return_value=cases['shape']):
                with self.assertRaises(i.Refused):value.inventory(child,5.0,context)
            self.assertFalse(any('_reject_' in call.args[0] for call in value.owner.phase.call_args_list))
            self.sealed(value)

    def test_rejection_label_throw_or_late_seals_without_secondary_output_or_replay(self):
        _,cases=self.rejecting_maps()
        for variant in ('throw','late'):
            value,child=self.fixture('core');now=[0.0]
            def phase(label,deadline):
                if label.endswith('_reject_shape'):
                    if variant=='throw':raise RuntimeError('synthetic private detail')
                    now[0]=5.0
            value.owner.phase.side_effect=phase
            with patch.object(i.time,'monotonic',side_effect=lambda:now[0]), \
                 patch.object(value,'text',return_value=cases['shape']) as read:
                with self.assertRaisesRegex(i.Refused,'^fixed_loaded_image_refused$'):
                    value.inventory(child,5.0,'initial_core')
                count=value.owner.phase.call_count;now[0]=0.0
                with self.assertRaises(i.Refused):value.inventory(child,5.0,'initial_core')
            self.assertEqual(read.call_count,1);self.assertEqual(value.owner.phase.call_count,count)
            self.assertEqual(value.owner.phase.call_args.args,
                             ('before_core_initial_inventory_first_parse_reject_shape',5.0))
            value.artifacts.mapped_identity.assert_not_called();self.sealed(value)

    def test_unknown_category_or_device_conversion_exception_is_not_caught_or_logged(self):
        _,cases=self.rejecting_maps();observed=[]
        with patch.object(i,'PARSE_REJECTIONS',()):
            with self.assertRaises(i.Refused):i.map_objects(cases['shape'],before_reject=observed.append)
        self.assertEqual(observed,[])
        with patch.object(i.os,'makedev',side_effect=OverflowError('synthetic private')):
            with self.assertRaises(OverflowError):
                i.map_objects('1000-2000 rw-p 0 00:00 0 \n',before_reject=observed.append)
        self.assertEqual(observed,[])
        value,child=self.fixture('core')
        with patch.object(value,'text',return_value='1000-2000 rw-p 0 00:00 0 \n'), \
             patch.object(i.os,'makedev',side_effect=OverflowError('synthetic private')):
            with self.assertRaisesRegex(i.Refused,'^fixed_loaded_image_refused$'):
                value.inventory(child,5.0,'initial_core')
        self.assertEqual(value.owner.phase.call_args.args,('before_core_initial_inventory_first_parse',5.0))
        value.artifacts.mapped_identity.assert_not_called();self.sealed(value)

    def test_failed_parse_bound_fits_core40_and_real_owner_closed_labels(self):
        spec=importlib.util.spec_from_file_location('reject_real_owner',SOURCE.with_name('lifecycle.py'))
        owner_module=importlib.util.module_from_spec(spec);spec.loader.exec_module(owner_module)
        first=2*i.INVENTORY_STEPS.index('first_parse')+1+len(i.ANONYMOUS_CLASSES)+1
        second=(2*i.INVENTORY_STEPS.index('second_parse')+1+2*len(i.ANONYMOUS_CLASSES)+2+1)
        complete=2*len(i.INVENTORY_STEPS)+2*len(i.ANONYMOUS_CLASSES)+2
        self.assertEqual((first,second,complete),(14,34,40))
        self.assertEqual(116+38+40+max(first,second,complete),234)
        for category,raw in self.rejecting_maps()[1].items():
            old,_=self.fixture('core')
            owner=owner_module.Session('inner',bootstrap_scratch='/home/kdk_vm/.cache/t3-retained-native-tmpfs-review-4/scratch');owner.deadline=65.0;owner.live=Mock()
            child=object.__new__(owner_module.OwnedProcess)
            owner.children=[child];owner.anchors={'core':{'child':child,'proc_fd':71}}
            value=i.Images(owner,owner_module,old.copies,SimpleNamespace(Bridge=Bridge),
                old.artifacts,SimpleNamespace(Sources=Artifacts,TABLE=old.artifacts.files))
            value.executable=Mock(return_value=(31,90));self.values.append(value)
            with patch.object(value,'text',return_value=raw), \
                 patch.object(owner_module.os,'write',side_effect=lambda fd,raw:len(raw)) as emitted:
                with self.assertRaises(i.Refused):value.inventory(child,owner.local_deadline(5),'initial_core')
            self.assertEqual(emitted.call_args.args[1],
                ('T3_RETAINED_PHASE_V1 before_core_initial_inventory_first_parse_reject_'+category+'\n').encode('ascii'))
            self.assertLessEqual(owner.phase_count,first);self.sealed(value)

    def test_native_required_member_categories_preserve_exact_predicate_and_no_target_on_refusal(self):
        for role in i.REQUIRED_ROLES:
            for variant,classes in (('equal',('present','identity_equal')),
                                    ('missing',('absent','identity_different')),
                                    ('different',('present','identity_different'))):
                value,child=self.fixture(role)
                first=i.map_objects(self.maps(value,role))
                if variant=='missing':del first[i.ROLES[role]]
                elif variant=='different':first[i.ROLES[role]]=(31,91)
                with patch.object(value,'text',return_value='synthetic'),patch.object(i,'map_objects',return_value=first):
                    if variant=='equal':value.inventory(child,5.0,'initial_'+role)
                    else:
                        with self.assertRaises(i.Refused):value.inventory(child,5.0,'initial_'+role)
                observed=[call.args[0] for call in value.owner.phase.call_args_list
                          if call.args[0].startswith('before_'+role+'_initial_inventory_required_members_')]
                self.assertEqual(observed,['before_'+role+'_initial_inventory_required_members_'+c for c in classes])
                if variant!='equal':
                    value.copies._verify_target.assert_not_called();value.artifacts.mapped_identity.assert_not_called()
                    self.sealed(value)

    def test_initial_core_labels_use_real_session_closed_phase_and_shared_deadline(self):
        spec=importlib.util.spec_from_file_location('core_real_owner',SOURCE.with_name('lifecycle.py'))
        owner_module=importlib.util.module_from_spec(spec);spec.loader.exec_module(owner_module)
        old,_=self.fixture('core')
        owner=owner_module.Session('inner',bootstrap_scratch='/home/kdk_vm/.cache/t3-retained-native-tmpfs-review-4/scratch');owner.deadline=65.0;owner.live=Mock()
        # Allocate a typed inert shell only: Popen.__init__ is never called.
        child=object.__new__(owner_module.OwnedProcess)
        owner.children=[child];owner.anchors={'core':{'child':child,'proc_fd':71}}
        value=i.Images(owner,owner_module,old.copies,SimpleNamespace(Bridge=Bridge),
                       old.artifacts,SimpleNamespace(Sources=Artifacts,TABLE=old.artifacts.files))
        value.executable=Mock(return_value=(31,90));self.values.append(value)
        with patch.object(value,'text',return_value=self.maps(value,'core')), \
             patch.object(owner_module.os,'write',side_effect=lambda fd,raw:len(raw)) as emitted:
            value.inventory(child,owner.local_deadline(5),'initial_core')
            self.assertEqual(owner.phase_count,24)
            self.assertEqual(emitted.call_count,24)
            self.assertTrue(all(call.args[0]==2 and call.args[1].decode('ascii').split()[1]
                                in owner_module.PHASES for call in emitted.call_args_list))
            self.assertEqual(emitted.call_args.args[1],
                             b'T3_RETAINED_PHASE_V1 after_core_initial_inventory_final_live\n')
            with self.assertRaises(i.Refused):value.inventory(child,5.0,'initial_core')
            self.assertEqual(emitted.call_count,24)
        self.sealed(value)

    def test_each_native_required_category_late_return_stops_before_next_category_or_target(self):
        for role in i.REQUIRED_ROLES:
            for category in ('present','identity_equal'):
                value,child=self.fixture(role);now=[0.0]
                label='before_'+role+'_initial_inventory_required_members_'+category
                def phase(current,deadline):
                    if current==label:now[0]=5.0
                value.owner.phase.side_effect=phase
                with patch.object(i.time,'monotonic',side_effect=lambda:now[0]), \
                     patch.object(value,'text',return_value=self.maps(value,role)):
                    with self.assertRaises(i.Refused):value.inventory(child,5.0,'initial_'+role)
                self.assertEqual(value.owner.phase.call_args.args,(label,5.0))
                value.artifacts.mapped_identity.assert_not_called();self.sealed(value)

    def test_actual_kernel_exe_original_fd_identity_before_any_maps_hash(self):
        for role in i.ROLES:
            value,child=self.fixture(role)
            del value.executable
            inode=90 if role in ('core','broker','host') else value.copies.records[i.ROLES[role]]['inode']
            info=SimpleNamespace(st_dev=31,st_ino=inode,st_size=64,st_mode=stat.S_IFREG|0o555,
                                 st_uid=0,st_gid=0,st_nlink=1)
            if role in ('core','broker','host'):
                value.artifacts.files[i.ROLES[role].rsplit('/',1)[1]]=(55,info,'b'*64)
            else:value.copies.records[i.ROLES[role]].update(mode=info.st_mode,uid=0,gid=0,nlink=1)
            with tempfile.TemporaryFile() as temp:
                fd=os.dup(temp.fileno())
                with patch.object(i.os,'open',return_value=fd) as opened,patch.object(i.os,'fstat',return_value=info), \
                     patch.object(i.os,'stat',return_value=info) as current:
                    self.assertEqual(value.executable(child,role,5.0),(31,inode))
                opened.assert_called_once_with('exe',i.EXE_FLAGS,dir_fd=71)
                current.assert_called_once_with('exe',dir_fd=71,follow_symlinks=True)
                self.assertIn(fd,value.held)
            value.copies._verify_target.assert_not_called();value.artifacts.recheck.assert_not_called()

    def test_wrong_kernel_executable_identity_precedes_maps_and_hash(self):
        value,child=self.fixture();del value.executable
        original=SimpleNamespace(st_dev=31,st_ino=90,st_size=64,st_mode=stat.S_IFREG|0o555,
                                 st_uid=0,st_gid=0,st_nlink=1)
        value.artifacts.files['mihomo']=(55,original,'b'*64)
        wrong=SimpleNamespace(**vars(original));wrong.st_ino=91
        with tempfile.TemporaryFile() as temp:
            fd=os.dup(temp.fileno())
            with patch.object(i.os,'open',return_value=fd),patch.object(i.os,'fstat',return_value=wrong), \
                 patch.object(value,'text') as maps_read:
                with self.assertRaises(i.Refused):value.inventory(child,5.0)
                maps_read.assert_not_called();value.copies._verify_target.assert_not_called()
                value.artifacts.mapped_identity.assert_not_called()
        self.sealed(value)

    def kernel_fixture(self):
        value,child=self.fixture();del value.executable
        original=SimpleNamespace(st_dev=31,st_ino=90,st_size=64,st_mode=stat.S_IFREG|0o555,
                                 st_uid=0,st_gid=0,st_nlink=1)
        value.artifacts.files['mihomo']=(55,original,'b'*64)
        return value,child,original

    def test_thousand_rechecks_keep_one_original_fd_and_reread_current_exe(self):
        value,child,original=self.kernel_fixture()
        with tempfile.TemporaryFile() as temp:
            fd=os.dup(temp.fileno())
            with patch.object(i.os,'open',return_value=fd) as opened, \
                 patch.object(i.os,'fstat',return_value=original) as held, \
                 patch.object(i.os,'stat',return_value=original) as current:
                for _ in range(1000):
                    self.assertEqual(value.executable(child,'core',5.0),(31,90))
                self.assertEqual(opened.call_count,1)
                self.assertEqual(held.call_count,1000)
                self.assertEqual(current.call_count,1000)
                self.assertEqual(value.held,[fd])
                self.assertEqual(value.executables,{'core':(child,71,fd)})
            self.assertFalse(value.sealed)
            value.artifacts.recheck.assert_not_called()

    def test_changed_current_or_held_metadata_seals_before_next_query_even_if_restored(self):
        for source in ('held','current'):
            for key in ('st_dev','st_ino','st_size','st_mode','st_uid','st_gid','st_nlink'):
                value,child,original=self.kernel_fixture()
                changed=SimpleNamespace(**vars(original));setattr(changed,key,getattr(changed,key)+1)
                with tempfile.TemporaryFile() as temp:
                    fd=os.dup(temp.fileno())
                    with patch.object(i.os,'open',return_value=fd) as opened, \
                         patch.object(i.os,'fstat',return_value=original) as held, \
                         patch.object(i.os,'stat',return_value=original) as current:
                        value.executable(child,'core',5.0)
                        (held if source=='held' else current).return_value=changed
                        with self.assertRaises(i.Refused):value.executable(child,'core',5.0)
                        self.sealed(value)
                        held.return_value=current.return_value=original
                        opened.reset_mock();held.reset_mock();current.reset_mock();value.owner.live.reset_mock()
                        with self.assertRaises(i.Refused):value.executable(child,'core',5.0)
                        opened.assert_not_called();held.assert_not_called();current.assert_not_called()
                        value.owner.live.assert_not_called()

    def test_cached_child_proc_fd_and_held_membership_cannot_be_substituted(self):
        for variant in ('child','proc','held'):
            value,child,original=self.kernel_fixture()
            with tempfile.TemporaryFile() as temp:
                fd=os.dup(temp.fileno())
                with patch.object(i.os,'open',return_value=fd), \
                     patch.object(i.os,'fstat',return_value=original) as held, \
                     patch.object(i.os,'stat',return_value=original) as current:
                    value.executable(child,'core',5.0)
                    if variant=='child':value.executables['core']=(Child(),71,fd)
                    elif variant=='proc':value.owner.anchors['core']['proc_fd']=72
                    else:value.executables['core']=(child,71,fd+100)
                    held.reset_mock();current.reset_mock()
                    with self.assertRaises(i.Refused):value.executable(child,'core',5.0)
                    held.assert_not_called();current.assert_not_called()
                self.sealed(value)

    def test_late_first_open_retains_original_and_late_current_stat_has_no_followup(self):
        for stage in ('open','current'):
            value,child,original=self.kernel_fixture();clock=[0.0]
            with tempfile.TemporaryFile() as temp:
                fd=os.dup(temp.fileno())
                def opened(*args,**kwargs):
                    if stage=='open':clock[0]=5.0
                    return fd
                def observed(*args,**kwargs):
                    clock[0]=5.0;return original
                with patch.object(i.time,'monotonic',side_effect=lambda:clock[0]), \
                     patch.object(i.os,'open',side_effect=opened) as opening, \
                     patch.object(i.os,'fstat',return_value=original) as held, \
                     patch.object(i.os,'stat',side_effect=observed) as current:
                    with self.assertRaises(i.Refused):value.executable(child,'core',5.0)
                    self.assertEqual(value.held,[fd])
                    self.assertEqual(value.executables,{'core':(child,71,fd)})
                    self.assertEqual(held.call_count,int(stage=='current'))
                    self.assertEqual(current.call_count,int(stage=='current'))
                    self.assertEqual(value.owner.live.call_count,1)
                    clock[0]=0.0;opening.reset_mock();held.reset_mock();current.reset_mock()
                    with self.assertRaises(i.Refused):value.executable(child,'core',5.0)
                    opening.assert_not_called();held.assert_not_called();current.assert_not_called()
                self.sealed(value)

    def test_unknown_current_stat_type_or_throw_permanently_seals(self):
        for variant in ('bool','float','throw'):
            value,child,original=self.kernel_fixture()
            changed=SimpleNamespace(**vars(original));changed.st_uid=False if variant=='bool' else 0.0
            with tempfile.TemporaryFile() as temp:
                fd=os.dup(temp.fileno())
                with patch.object(i.os,'open',return_value=fd),patch.object(i.os,'fstat',return_value=original), \
                     patch.object(i.os,'stat',return_value=changed) as current:
                    if variant=='throw':current.side_effect=OSError('private must not escape')
                    with self.assertRaisesRegex(i.Refused,'^fixed_loaded_image_refused$'):
                        value.executable(child,'core',5.0)
                self.sealed(value)

    def test_no_execution_signal_reap_write_cleanup_or_dynamic_proc_path(self):
        forbidden={'exec','eval','spawn','kill','waitid','waitpid','system','write','unlink','close','rmdir','mkdir','chmod','chown'}
        tree=ast.parse(SOURCE.read_text())
        for node in ast.walk(tree):
            if isinstance(node,ast.Call):
                name=node.func.id if isinstance(node.func,ast.Name) else node.func.attr if isinstance(node.func,ast.Attribute) else ''
                self.assertNotIn(name,forbidden)
        self.assertNotIn('/proc/',SOURCE.read_text())


if __name__=='__main__':unittest.main()
