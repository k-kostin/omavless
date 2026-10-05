"""Pure synthetic receipt controls, not an actual fixture/PID/mapping result."""
import copy
import importlib.util
import json
from pathlib import Path
import unittest

HERE=Path(__file__).parent
spec=importlib.util.spec_from_file_location('retained_receipt',HERE/'validate_receipt.py')
v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
RAW=(HERE.parent/'six_library_copy_admission/copy-manifest.json').read_bytes()


def fixture():
    source=json.loads(RAW)['source_provenance'];copies={}
    for index,(path,row) in enumerate(sorted(source.items())):
        copies[path]={'device':42,'inode':1000+index,'size':row['size'],'mode':row['mode'],
            'uid':0,'gid':0,'nlink':1,'sha256':row['sha256'],
            'source_device':row['device'],'source_inode':row['inode']}
    initial={}
    for index,role in enumerate(v.ROLES):
        if role in v.NATIVE:
            size,sha=v.NATIVE[role];initial[role]=[{'path':v.ROLES[role],
                'device':43,'inode':2000+index,'size':size,'sha256':sha}]
        else:
            initial[role]=[{'path':path,**{key:row[key] for key in ('device','inode','size','sha256')}}
                           for path,row in copies.items()]
    ledger={'schema':'retained-positive-zero-ledger-v1','scope':'inner','roles':sorted(v.ROLES),
        'owned_child_count':85,'utility_count':80,'all_owned_direct_children_exact_zero_reaped':True,
        'global_shared_argv_or_uid_absence_claimed':False,'production_effect_authority':False}
    shutdown={role:({'state':'zero-reaped','exit_code':0,'signal_count':0} if role=='host' else
        {'pid':100+index,'starttime':1000+index,'namespaces':{'pid':[7,900],'net':[7,901]},
         'signal':'SIGTERM','signal_count':1,'exit_code':0,'state':'zero-reaped'})
        for index,role in enumerate(v.ROLES)}
    return {'case':'success','initial':initial,'final':copy.deepcopy(initial),'copies':copies,
        'shutdown':shutdown,'inner_owned_zero':ledger,'monitor_final_sha256':'a'*64,
        'stream_witness':dict.fromkeys(('wrong_token_changed','exact_target_closed','selected_eof_after_ack',
            'unselected_byte_stream_survived','replay_missing','survivor_identity_unchanged'),True),
        'stream_positive_finish':{'positive_sockets_closed':True,'owned_echo_thread_returned_without_error':True},
        'fresh_socket_bootstrap':{'fresh_original_controller_socket_mode_0600':True,'production_effect_authority':False},
        **dict.fromkeys(v.FALSE,False),**dict.fromkeys(v.TRUE,True)}


class Controls(unittest.TestCase):
    def test_strict_synthetic_semantic_receipt_is_recorded_only(self):
        value=fixture();self.assertIs(v.validate_case(value,RAW),value)
        self.assertEqual(v.validate_case(v.decode(json.dumps(value).encode()),RAW),value)
        self.assertFalse(value['parent_whole_known_zero'])

    def test_unknown_field_wrong_case_or_any_adoption_flag_refuses(self):
        for key in (*v.FALSE,'unknown'):
            value=fixture();value[key]=True
            with self.assertRaises(v.Refused):v.validate_case(value,RAW)
        value=fixture();value['case']='owner-loss'
        with self.assertRaises(v.Refused):v.validate_case(value,RAW)

    def test_copy_original_identity_hash_alias_and_count_refuse(self):
        path=next(iter(fixture()['copies']))
        for key,bad in (('inode',True),('uid',False),('nlink',2),('size',0),('sha256','f'*64),
                        ('source_inode',1),('device',1.0)):
            value=fixture();value['copies'][path][key]=bad
            with self.assertRaises(v.Refused):v.validate_case(value,RAW)
        value=fixture();value['copies'].pop(path)
        with self.assertRaises(v.Refused):v.validate_case(value,RAW)
        value=fixture();row=value['copies'][path];row['device']=row['source_device'];row['inode']=row['source_inode']
        with self.assertRaises(v.Refused):v.validate_case(value,RAW)

    def test_unknown_other_native_wrong_live_identity_or_second_inventory_refuses(self):
        for variant in ('unknown','native','identity','after','missing','order','duplicate'):
            value=fixture();rows=value['initial']['core']
            if variant=='unknown':rows[0]['path']='/home/private'
            elif variant=='native':rows[0]['path']='/artifacts/host-fixture'
            elif variant=='identity':rows[0]['inode']=True
            elif variant=='after':value['final']['core'][0]['inode']+=1
            elif variant=='missing':value['initial']['bus']=[]
            elif variant=='order':value['initial']['bus'].reverse()
            else:rows.append(copy.deepcopy(rows[0]))
            with self.assertRaises(v.Refused):v.validate_case(value,RAW)

    def test_missing_six_library_witness_or_daemon_loader_refuses(self):
        for path in ('/usr/lib/libcrypto.so.3','/usr/lib/ld-linux-x86-64.so.2'):
            value=fixture()
            for stage in ('initial','final'):
                for role in ('bus','resolved'):
                    value[stage][role]=[row for row in value[stage][role] if row['path']!=path]
            with self.assertRaises(v.Refused):v.validate_case(value,RAW)

    def test_stream_bootstrap_effect_and_ledger_aliases_or_unknowns_refuse(self):
        for section in ('stream_witness','stream_positive_finish','fresh_socket_bootstrap'):
            value=fixture();value[section][next(iter(value[section]))]=1
            with self.assertRaises(v.Refused):v.validate_case(value,RAW)
        for key,bad in (('owned_child_count',84),('utility_count',80.0),
                        ('all_owned_direct_children_exact_zero_reaped',1),('roles',['bus'])):
            value=fixture();value['inner_owned_zero'][key]=bad
            with self.assertRaises(v.Refused):v.validate_case(value,RAW)

    def test_nonzero_alias_signal_pid_and_namespace_shutdown_refuse(self):
        for key,bad in (('exit_code',False),('signal_count',True),('pid',True),('starttime',0),
                        ('signal','SIGKILL'),('namespaces',{'pid':[7,902],'net':[7,901]})):
            value=fixture();value['shutdown']['core'][key]=bad
            with self.assertRaises(v.Refused):v.validate_case(value,RAW)
        value=fixture();value['shutdown']['core']['pid']=value['shutdown']['bus']['pid']
        with self.assertRaises(v.Refused):v.validate_case(value,RAW)

    def test_duplicate_nonfinite_unbounded_json_and_wrong_manifest_refuse(self):
        for raw in (b'{"x":1,"x":2}',b'{"x":NaN}',b'{"x":Infinity}',b'x'*(4*1024*1024+1)):
            with self.assertRaises(v.Refused):v.decode(raw)
        with self.assertRaises(v.Refused):v.validate_case(fixture(),RAW+b'\n')

    def test_cross_role_native_identity_alias_and_namespace_kind_alias_refuse(self):
        value=fixture()
        for stage in ('initial','final'):
            value[stage]['broker'][0]['inode']=value[stage]['core'][0]['inode']
        with self.assertRaises(v.Refused):v.validate_case(value,RAW)
        value=fixture()
        for role,row in value['shutdown'].items():
            if role!='host':row['namespaces']['net']=row['namespaces']['pid'][:]
        with self.assertRaises(v.Refused):v.validate_case(value,RAW)


if __name__=='__main__':unittest.main()
