"""Pure private semantic receipt validation, not independent live authority.

No IO, process observation or path reopening. Known-zero parent/whole-wrapper
and canonical-baseline proof are separate mandatory caller evidence.
"""
import hashlib
import json
import re

MANIFEST='3caa3d2bfdace10e97617b1e222192bf0c78f8ac194f51ee0a2f90f9defdd896'
ROLES={'bus':'/usr/bin/dbus-daemon','resolved':'/usr/lib/systemd/systemd-resolved',
       'core':'/artifacts/mihomo','broker':'/artifacts/omavless-dns-broker','host':'/artifacts/host-fixture'}
NATIVE={'core':(61083808,'3b1da75d3c9fd8440216f9c256c6c59da812faae88debc936f3c72fef9724544'),
        'broker':(5126984,'ea958302d745b901294df6164c624a431a7493b67457a255306ec8216545eb9d'),
        'host':(49630768,'fbd19fc83f5d6548ff8f1fe89d4d66ab9032d85cb551a2364f5719a5cad234f7')}
SIX={'/usr/lib/libcrypto.so.3','/usr/lib/libidn2.so.0.4.0','/usr/lib/libssl.so.3',
     '/usr/lib/libunistring.so.5.2.1','/usr/lib/libz.so.1.3.2','/usr/lib/libzstd.so.1.5.7'}
FALSE={'parent_whole_known_zero','production_effect_authority','installed_compatibility','normal_owner_adoption'}
TRUE={'actual_core_broker_and_helper_original_loaded_bound','conditional_close_kept_actual_dns_lease',
      'actual_dns_reset_while_descriptor_held','all_effects_settled_before_monitor_freeze'}
FIELDS={'case','initial','final','copies','stream_witness','stream_positive_finish',
    'fresh_socket_bootstrap','shutdown','inner_owned_zero','monitor_final_sha256'}|FALSE|TRUE


class Refused(RuntimeError):
    def __init__(self):super().__init__('retained_positive_receipt_refused')


def require(value):
    if not value:raise Refused()


def integer(value,minimum=0,maximum=2**64-1):
    require(type(value) is int and minimum<=value<=maximum)
    return value


def pairs(items):
    value={}
    for key,item in items:
        require(key not in value);value[key]=item
    return value


def decode(raw):
    require(type(raw) is bytes and 0<len(raw)<=4*1024*1024)
    try:
        return json.loads(raw,object_pairs_hook=pairs,
            parse_constant=lambda _:(_ for _ in ()).throw(Refused()))
    except BaseException:raise Refused() from None


def flags(value,expected):
    require(type(value) is dict and set(value)==set(expected)
            and all(type(value[key]) is bool and value[key] is flag for key,flag in expected.items()))


def ledger(value,kind):
    require(type(kind) is str and kind in ('inner','outer'))
    roles=sorted(ROLES) if kind=='inner' else ['namespace']
    require(type(value) is dict and set(value)=={'schema','scope','roles','owned_child_count',
        'utility_count','all_owned_direct_children_exact_zero_reaped',
        'global_shared_argv_or_uid_absence_claimed','production_effect_authority'}
        and value['schema']=='retained-positive-zero-ledger-v1' and value['scope']==kind
        and type(value['roles']) is list and value['roles']==roles)
    count=integer(value['owned_child_count'],len(roles),96)
    utilities=integer(value['utility_count'],0,96)
    require(count==len(roles)+utilities and (utilities>0 if kind=='inner' else utilities==0))
    flags({key:value[key] for key in ('all_owned_direct_children_exact_zero_reaped',
        'global_shared_argv_or_uid_absence_claimed','production_effect_authority')},
        {'all_owned_direct_children_exact_zero_reaped':True,
         'global_shared_argv_or_uid_absence_claimed':False,'production_effect_authority':False})


def validate_case(value,manifest_raw):
    try:
        require(type(manifest_raw) is bytes and hashlib.sha256(manifest_raw).hexdigest()==MANIFEST)
        manifest=decode(manifest_raw);source=manifest['source_provenance']
        require(type(value) is dict and set(value)==FIELDS and value['case']=='success')
        flags({key:value[key] for key in FALSE|TRUE},{**dict.fromkeys(FALSE,False),**dict.fromkeys(TRUE,True)})
        require(type(value['monitor_final_sha256']) is str
                and re.fullmatch('[0-9a-f]{64}',value['monitor_final_sha256']))
        copies=value['copies'];require(type(copies) is dict and set(copies)==set(source) and len(copies)==25)
        identities=set();devices=set()
        for path,row in copies.items():
            expected=source[path]
            require(type(row) is dict and set(row)=={'device','inode','size','mode','uid','gid','nlink',
                'sha256','source_device','source_inode'})
            for key in ('device','inode','size','mode','uid','gid','nlink','source_device','source_inode'):
                integer(row[key],1 if key in ('inode','size','source_inode','nlink') else 0)
            require(row['uid']==row['gid']==0 and row['nlink']==1 and row['mode']==expected['mode']
                and row['size']==expected['size'] and row['sha256']==expected['sha256']
                and row['source_device']==expected['device'] and row['source_inode']==expected['inode'])
            identity=(row['device'],row['inode']);require(identity not in identities
                and identity!=(row['source_device'],row['source_inode']))
            identities.add(identity);devices.add(row['device'])
        require(len(devices)==1 and type(value['initial']) is dict and set(value['initial'])==set(ROLES)
                and type(value['final']) is dict and set(value['final'])==set(ROLES))
        native_identities={}
        for stage in ('initial','final'):
            for role,rows in value[stage].items():
                require(type(rows) is list and 0<len(rows)<=26)
                paths=[]
                for row in rows:
                    require(type(row) is dict and set(row)=={'path','device','inode','size','sha256'})
                    path=row['path'];require(type(path) is str and path not in paths)
                    integer(row['device']);integer(row['inode'],1);integer(row['size'],1)
                    if path in copies:
                        expected=copies[path]
                        require(all(row[key]==expected[key] for key in ('device','inode','size','sha256')))
                    else:
                        require(role in NATIVE and path==ROLES[role]
                            and (row['size'],row['sha256'])==NATIVE[role]
                            and (row['device'],row['inode']) not in identities)
                        pair=(row['device'],row['inode'])
                        require(all(other_role==role or other!=pair
                            for other_role,other in native_identities.items()))
                        require(role not in native_identities or native_identities[role]==pair)
                        native_identities[role]=pair
                    paths.append(path)
                require(paths==sorted(paths) and ROLES[role] in paths)
                if role in ('bus','resolved'):
                    require({'/usr/lib/libc.so.6','/usr/lib/ld-linux-x86-64.so.2'}<=set(paths))
            require(SIX <= {row['path'] for role in ('bus','resolved') for row in value[stage][role]})
        require(value['initial']==value['final'])
        flags(value['stream_witness'],dict.fromkeys(('wrong_token_changed','exact_target_closed',
            'selected_eof_after_ack','unselected_byte_stream_survived','replay_missing','survivor_identity_unchanged'),True))
        flags(value['stream_positive_finish'],{'positive_sockets_closed':True,'owned_echo_thread_returned_without_error':True})
        flags(value['fresh_socket_bootstrap'],{'fresh_original_controller_socket_mode_0600':True,
                                               'production_effect_authority':False})
        ledger(value['inner_owned_zero'],'inner')
        shutdown=value['shutdown'];require(type(shutdown) is dict and set(shutdown)==set(ROLES))
        pids=set();namespaces=None
        for role,row in shutdown.items():
            require(type(row) is dict and type(row['exit_code']) is int and row['exit_code']==0
                    and row['state']=='zero-reaped' and type(row['signal_count']) is int)
            if role=='host':
                require(set(row)=={'state','exit_code','signal_count'} and row['signal_count']==0);continue
            require(set(row)=={'pid','starttime','namespaces','signal','signal_count','exit_code','state'}
                    and row['signal']=='SIGTERM' and row['signal_count']==1)
            pid=integer(row['pid'],2,2**31-1);require(pid not in pids);pids.add(pid)
            integer(row['starttime'],1)
            ns=row['namespaces'];require(type(ns) is dict and set(ns)=={'pid','net'})
            for pair in ns.values():
                require(type(pair) is list and len(pair)==2);integer(pair[0]);integer(pair[1],1)
            require(ns['pid']!=ns['net'])
            require(namespaces is None or namespaces==ns);namespaces=ns
        return value
    except BaseException:raise Refused() from None
