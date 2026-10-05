"""Fresh isolated positive-case coordinator, developer-only, no entry point.

Only the future pinned launcher supplies these concrete modules/objects. The
HOST builder/outer guard remain required before invocation.
No legacy exercise, failure cleanup or nonzero/quarantine acceptance is reached.
"""
import hashlib
import functools
import io
import json
import os
import re
import stat
import subprocess
import time

CORE_CONFIG = ("mixed-port: 19090\nexternal-controller-unix: /home/core/controller.sock\n"
    "allow-lan: false\nbind-address: 127.0.0.1\nmode: direct\nlog-level: silent\nipv6: false\n"
    "tun:\n  enable: true\n  device: Meta\n  stack: system\n  mtu: 1500\n  gso: false\n"
    "  auto-route: false\n  auto-detect-interface: false\n  disable-system-dns: true\n"
    "  omavless-dns-broker: true\n  dns-hijack: []\n"
    "dns:\n  enable: true\n  enhanced-mode: fake-ip\n  fake-ip-range: 198.18.0.1/16\n"
    "  ipv6: false\n  nameserver: [127.0.0.1:19093]\n"
    "profile:\n  store-selected: false\n  store-fake-ip: false\n"
    "proxies: []\nproxy-groups: []\nrules:\n  - MATCH,DIRECT\n").encode()
SIX = {'/usr/lib/libcrypto.so.3','/usr/lib/libidn2.so.0.4.0','/usr/lib/libssl.so.3',
       '/usr/lib/libunistring.so.5.2.1','/usr/lib/libz.so.1.3.2','/usr/lib/libzstd.so.1.5.7'}
NOTIFICATIONS = ['READY=1','FDSTORE=1\nFDNAME=omavless-tun-lease\nFDPOLL=0','BARRIER=1',
                 'FDSTOREREMOVE=1\nFDNAME=omavless-tun-lease','BARRIER=1']
METHODS = ['SetLinkDNS','SetLinkDomains','SetLinkDefaultRoute','RevertLink']
OBSERVER_FIELDS = {'notifications','stored','received_tun','actual_fixed_policy','observation',
    'reset_while_held','baseline','unrelated_baseline','owner_pinned','unrelated_preserved','effects',
    'phase','tun_exists','notify_alive','observer_finalized','monitor_alive'}
OBSERVATION_FIELDS = {'servers','extended','domains','route','llmnr','mdns','tls','dnssec','anchors'}
# Exact frozen c4 journal.rs enum, excluding terminal quarantine. These are
# PRIVATE readback phases, not authority to dispatch an effect or release a FD.
PENDING_PHASES = {None,'applying','active','releasing','cleanup_verified'}


class Refused(RuntimeError):
    def __init__(self):super().__init__('retained_positive_case_refused')


def require(value):
    if not value:raise Refused()


def observation(value, kind):
    """Fully typed finite private resolver projection, before any retry.

    Managed DNS components may be individually empty while known sequential
    methods are pending. Baseline and unrelated policy are exact; no arbitrary
    resolver strings/address/domain inventory is accepted or exported.
    """
    require(kind in ('managed','baseline','unrelated') and type(value) is dict
            and set(value) == OBSERVATION_FIELDS)
    address = [192,0,2,53] if kind=='unrelated' else [198,18,0,2]
    domain = 'unrelated.invalid' if kind=='unrelated' else '.'
    for key in ('servers','extended','domains','anchors'):
        require(type(value[key]) is list and len(value[key]) <= 1)
    for row in value['servers']:
        require(type(row) is list and len(row)==2 and type(row[0]) is int and row[0]==2
                and type(row[1]) is list and len(row[1])==4
                and all(type(byte) is int for byte in row[1]) and row[1]==address)
    for row in value['extended']:
        require(type(row) is list and len(row)==4 and type(row[0]) is int and row[0]==2
                and type(row[1]) is list and len(row[1])==4
                and all(type(byte) is int for byte in row[1]) and row[1]==address
                and type(row[2]) is int and row[2]==0 and type(row[3]) is str and row[3]=='')
    for row in value['domains']:
        require(type(row) is list and len(row)==2 and type(row[0]) is str and row[0]==domain
                and type(row[1]) is bool and row[1] is True)
    require(type(value['route']) is bool and value['anchors']==[])
    for key,eligible in (('llmnr',{'no','yes','resolve'}),('mdns',{'no','yes','resolve'}),
                         ('tls',{'no','yes','opportunistic'}),('dnssec',{'no','yes','allow-downgrade'})):
        require(type(value[key]) is str and value[key] in eligible)
    if kind=='baseline':
        require(value['servers']==value['extended']==value['domains']==[]
                and value['route'] is False and value['tls']==value['dnssec']=='no')
    if kind=='unrelated':
        require(value['servers'] and value['extended'] and value['domains'] and value['route'] is True)
    return value


def guarded(method):
    @functools.wraps(method)
    def call(self,*args,**kwargs):
        try:
            self.available()
            value = method(self,*args,**kwargs)
            self.available()
            return value
        except BaseException:
            self.sealed = self.owner.sealed = True
            raise Refused() from None
    return call


class Case:
    def __init__(self, owner, ownership, base, copies, copy_module, artifacts, artifact_module,
                 image_module, controller_module, helper_module, stream_module, bootstrap_module):
        self.owner, self.ownership, self.base = owner, ownership, base
        self.copies, self.artifacts = copies, artifacts
        self.image_module, self.controller_module, self.helper_module, self.stream_module = (
            image_module,controller_module,helper_module,stream_module)
        self.bootstrap_module = bootstrap_module
        self.children, self.initial, self.final, self.logs, self.stops = {}, {}, {}, {}, {}
        self.sealed = True
        try:
            require(type(owner) is ownership.Session and owner.kind == 'inner' and owner.isolated
                    and not owner.anchors and os.getpid() == 1 and os.geteuid() == os.getegid() == 0
                    and type(copies) is copy_module.Bridge and copies.state == 'ready'
                    and type(artifacts) is artifact_module.Sources and not artifacts.sealed)
            owner.retained.append(self)
            owner.available()
            self.images = image_module.Images(owner,ownership,copies,copy_module,artifacts,artifact_module)
            self.sealed = False
        except BaseException:
            self.sealed = owner.sealed = True
            raise Refused() from None

    def available(self):
        try:
            require(not self.sealed)
            self.owner.available()
        except BaseException:
            self.sealed = self.owner.sealed = True
            raise Refused() from None

    @guarded
    def call(self, deadline, operation, *args, **kwargs):
        self.available();self.owner.within(deadline)
        value = operation(*args, **kwargs)
        self.owner.within(deadline);self.available()
        return value

    @guarded
    def opened(self, deadline, path, flags, mode=0o600):
        self.available();self.owner.within(deadline)
        fd = os.open(path,flags,mode)
        self.owner.retained.append(fd)  # Retain even a late newly created FD.
        self.owner.within(deadline)
        return fd

    @guarded
    def write(self, path, raw, uid=0):
        require((path,uid) in (('/tmp/dbus.xml',0),('/home/core/config.yaml',1000))
                and type(raw) is bytes and 0 < len(raw) <= 65536)
        deadline = self.owner.local_deadline(5)
        fd = self.opened(deadline,path,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW|os.O_CLOEXEC)
        written = self.call(deadline,os.write,fd,raw)
        require(type(written) is int and written == len(raw))
        self.call(deadline,os.fsync,fd)
        self.call(deadline,os.fchmod,fd,0o600)
        self.call(deadline,os.fchown,fd,uid,uid)
        value = self.call(deadline,os.fstat,fd)
        require(stat.S_ISREG(value.st_mode) and value.st_uid == value.st_gid == uid
                and value.st_nlink == 1 and stat.S_IMODE(value.st_mode) == 0o600 and value.st_size == len(raw))

    @guarded
    def spawn(self, role, argv, *, pipes=False):
        require(role in ('bus','resolved','broker','host','core') and type(pipes) is bool)
        deadline = self.owner.local_deadline(5)
        self.owner.phase('before_'+role+'_log_open',deadline)
        log = self.opened(deadline,'/tmp/'+role+'.log',os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW|os.O_CLOEXEC)
        self.logs[role] = log
        self.owner.phase('after_'+role+'_log_open',deadline)
        self.owner.phase('before_'+role+'_owned_constructor',deadline)
        child = self.call(deadline,self.owner.spawn,argv,role=role,env=self.base.ENV,
            stdin=subprocess.PIPE if pipes else subprocess.DEVNULL,
            stdout=subprocess.PIPE if role=='host' else log,stderr=log,bufsize=0,
            preexec_fn=self.base.limits)
        self.children[role] = child
        self.owner.phase('after_'+role+'_owned_constructor',deadline)
        # A late spawn is already retained by Session.spawn, but must never
        # authorize anchor/proc IO. Only a known-in-budget spawn starts this
        # separate fixed anchor stage.
        deadline = self.owner.local_deadline(5)
        self.owner.phase('before_'+role+'_anchor',deadline)
        self.call(deadline,self.owner.anchor,role,child)
        self.owner.phase('after_'+role+'_anchor',deadline)
        return child

    @guarded
    def mapped(self, role):
        child = self.children[role]
        deadline = self.owner.local_deadline(5)
        self.owner.phase('before_'+role+'_first_images',deadline)
        first = self.images.inventory(child,deadline,'initial_'+role)
        self.owner.within(deadline)
        self.owner.phase('after_'+role+'_first_images',deadline)
        self.owner.phase('before_'+role+'_second_images',deadline)
        again = self.images.inventory(child,deadline,'final_'+role)
        self.owner.within(deadline)
        self.owner.phase('after_'+role+'_second_images',deadline)
        self.owner.phase('before_'+role+'_mapped',deadline)
        self.owner.mapped(role,first,again)
        self.initial[role] = first
        self.owner.phase('after_'+role+'_mapped',deadline)

    @guarded
    def final_map(self, role):
        deadline = self.owner.local_deadline(5)
        value = self.images.inventory(self.children[role],deadline,'final_'+role)
        self.owner.within(deadline)
        require(value == self.initial[role])
        self.final[role] = value

    @guarded
    def observer(self, value, *, final=False):
        require(type(final) is bool)
        require(type(value) is dict and set(value) == OBSERVER_FIELDS and type(value.get('notifications')) is list
                and len(value['notifications']) <= 5
                and all(type(item) is str for item in value['notifications'])
                and value['notifications'] == NOTIFICATIONS[:len(value['notifications'])]
                and type(value.get('effects')) is list and len(value['effects']) <= 4)
        serials = set()
        for index,effect in enumerate(value['effects']):
            require(type(effect) is dict and set(effect) == {'method','serial','sender','outcome'}
                    and effect['method'] == METHODS[index]
                    and type(effect['serial']) is int and 0 < effect['serial'] < 2**32
                    and type(effect['sender']) is str and re.fullmatch(r':[0-9]{1,10}\.[0-9]{1,10}',effect['sender'])
                    and (effect['outcome'] is None or type(effect['outcome']) is str
                         and effect['outcome']=='settled_success')
                    and (index==len(value['effects'])-1 or effect['outcome']=='settled_success'))
            serial = (effect['sender'],effect['serial'])
            require(serial not in serials);serials.add(serial)
        for key in ('stored','received_tun','actual_fixed_policy','reset_while_held','owner_pinned',
                    'unrelated_preserved','tun_exists','notify_alive','observer_finalized','monitor_alive'):
            require(type(value.get(key)) is bool)
        require(value['owner_pinned'] is True and value['unrelated_preserved'] is True)
        require(value['notify_alive'] is True and value['observer_finalized'] is final
                and value['monitor_alive'] is (not final)
                and (value['phase'] is None or type(value['phase']) is str)
                and value['phase'] in PENDING_PHASES)
        observation(value['unrelated_baseline'],'unrelated')
        if value['received_tun']:
            baseline=observation(value['baseline'],'baseline')
            managed=observation(value['observation'],'managed')
            require(all(managed[key]==baseline[key] for key in ('llmnr','mdns','tls','dnssec','anchors')))
        else:
            require(value['baseline'] is None and value['observation'] is None
                    and value['stored'] is False and value['reset_while_held'] is False
                    and value['actual_fixed_policy'] is False)
        require(not value['stored'] or value['received_tun'])
        return value

    @guarded
    def wait_snapshot(self, helper, kind):
        require(kind in ('broker_ready','active','released'))
        deadline = self.owner.local_deadline(8)
        while True:
            self.owner.within(deadline)
            for row in self.owner.anchors.values():
                if row['state'] != 'zero-reaped':self.owner.live(row['child']);self.owner.within(deadline)
            value = self.call(deadline,self.observer,self.call(deadline,helper.snapshot,deadline))
            eligible = {'broker_ready':{None},'active':{None,'applying','active'},
                        'released':{None,'active','releasing','cleanup_verified'}}
            require(value['phase'] in eligible[kind])
            if kind=='broker_ready' and value['notifications'][:1] == ['READY=1']:
                self.owner.within(deadline);return value
            settled = [(item['method'],item['outcome']) for item in value['effects']]
            if kind=='active' and value['phase']=='active' and settled == [(method,'settled_success') for method in METHODS[:3]]:
                self.call(deadline,self.base.active,value);return value
            if kind=='released' and value['phase'] is None and value['tun_exists'] is False \
                    and settled == [(method,'settled_success') for method in METHODS]:
                self.call(deadline,self.base.clean,value,'success');return value
            self.call(deadline,time.sleep,0.1)  # Known pending observation only, never unknown retry.

    @guarded
    def final_snapshot(self, helper):
        deadline = self.owner.local_deadline(5)
        raw = self.call(deadline,helper.snapshot,deadline,final=True)
        value = self.call(deadline,self.observer,raw,final=True)
        self.call(deadline,self.base.clean,value,'success')
        return value

    def run(self):
        try:
            self.available()
            self.owner.phase('before_initial_images_verify')
            self.images.verify(self.owner.local_deadline(5))
            self.owner.phase('after_initial_images_verify')
            self.write('/tmp/dbus.xml',self.base.bus_config('success').encode())
            for role,argv in (('bus',['/usr/bin/dbus-daemon','--nofork','--nopidfile','--config-file=/tmp/dbus.xml']),
                              ('resolved',self.base.resolved_exec())):
                self.owner.phase('before_'+role+'_spawn')
                child = self.spawn(role,argv);self.owner.ready(role)
                if role=='resolved':self.owner.perform(self.base.verify_child,child,974,self.base.RESOLVER_CAPS)
                self.mapped(role)
                self.owner.phase('after_'+role+'_maps')
            self.owner.phase('before_broker_spawn')
            broker_command = self.base.cap_exec('omavless-dns-broker',0,['--serve'])
            gate = "import os; assert os.read(0,1)==b'G'; os.execve("+repr(broker_command[0])+","+repr(broker_command)+","+repr(self.base.ENV)+")"
            broker = self.spawn('broker',['/usr/bin/python3','-I','-B','-c',gate],pipes=True)
            self.owner.phase('before_host_spawn')
            host = self.spawn('host',self.base.cap_exec('host-fixture',0,
                ['success',str(broker.pid),str(self.children['resolved'].pid)]),pipes=True)
            helper = self.helper_module.Helper(self.owner,host,self.ownership,self.images,
                                               self.image_module,self.controller_module)
            helper.ready(self.owner.local_deadline(5))
            self.owner.perform(self.base.verify_child,host,0,self.base.CAP)
            self.owner.native_ready('host');self.mapped('host')
            self.owner.phase('after_host_maps')
            deadline = self.owner.local_deadline(5)
            require(type(broker.stdin) is io.FileIO)
            self.owner.live(broker);self.owner.within(deadline)
            self.owner.phase('before_broker_release',deadline);self.owner.within(deadline)
            written = self.call(deadline,os.write,broker.stdin.fileno(),b'G')
            require(type(written) is int and written == 1)
            self.owner.live(broker);self.owner.within(deadline)
            require(self.call(deadline,broker.stdin.close) is None)
            self.wait_snapshot(helper,'broker_ready')
            self.owner.perform(self.base.verify_child,broker,0,self.base.CAP)
            self.owner.native_ready('broker');self.mapped('broker')
            self.owner.phase('after_broker_maps')
            deadline = self.owner.local_deadline(5)
            self.call(deadline,os.mkdir,'/home/core',0o700)
            self.call(deadline,os.chown,'/home/core',1000,1000)
            self.write('/home/core/config.yaml',CORE_CONFIG,1000)
            self.owner.phase('before_core_spawn')
            core = self.spawn('core',self.base.cap_exec('mihomo',1000,
                ['-d','/home/core','-f','/home/core/config.yaml']))
            self.wait_snapshot(helper,'active')
            self.owner.phase('after_core_active')
            self.owner.perform(self.base.verify_child,core,1000,self.base.CAP)
            bootstrap = self.call(self.owner.local_deadline(5),self.bootstrap_module.Bootstrap,
                self.owner,self.ownership,self.images,self.image_module)
            socket_bootstrap = self.call(self.owner.local_deadline(5),bootstrap.receipt)
            self.owner.phase('after_core_bootstrap')
            control = self.call(self.owner.local_deadline(5),self.controller_module.Controller,
                self.owner,core,self.ownership)
            control.ready();self.owner.native_ready('core');self.mapped('core')
            self.owner.phase('after_core_maps')
            streams = self.stream_module.Streams(self.owner,self.ownership,self.images,self.image_module,
                                                control,self.controller_module)
            self.owner.phase('before_stream_witness')
            stream_witness = self.call(self.owner.local_deadline(8),streams.witness)
            self.owner.phase('after_stream_witness')
            deadline = self.owner.local_deadline(5)
            self.call(deadline,self.base.active,self.call(deadline,self.observer,
                self.call(deadline,helper.snapshot,deadline)))
            control.ready();self.available()
            self.owner.phase('before_stream_finish')
            stream_finish = self.call(self.owner.local_deadline(5),streams.finish_positive)
            self.owner.phase('after_stream_finish')
            for role in ('core','broker','host'):self.final_map(role)
            self.owner.phase('before_core_shutdown')
            self.stops['core'] = self.owner.shutdown('core',self.images,self.base)
            self.owner.phase('after_core_shutdown')
            self.wait_snapshot(helper,'released')
            self.owner.phase('after_dns_release')
            self.owner.phase('before_broker_shutdown')
            self.stops['broker'] = self.owner.shutdown('broker',self.images,self.base)
            self.owner.phase('after_broker_shutdown')
            self.owner.phase('before_host_finish')
            final = self.final_snapshot(helper)
            helper.positive_eof(self.owner.local_deadline(5))
            self.stops['host'] = self.owner.host_finished_zero()
            self.owner.phase('after_host_zero')
            self.owner.phase('before_daemon_shutdown')
            for role in ('resolved','bus'):
                self.final_map(role)
                self.stops[role] = self.owner.shutdown(role,self.images,self.base)
                self.owner.phase('after_'+role+'_zero')
            for rows in (self.initial,self.final):
                loaded = {row['path'] for role in ('bus','resolved') for row in rows[role]}
                require(SIX <= loaded)
            self.owner.phase('before_complete')
            complete = self.owner.complete()
            self.owner.phase('after_complete')
            self.available()
            return {'case':'success','initial':self.initial,'final':self.final,'copies':self.copies.records,
                'stream_witness':stream_witness,'stream_positive_finish':stream_finish,
                'fresh_socket_bootstrap':socket_bootstrap,
                'shutdown':self.stops,'inner_owned_zero':complete,
                'monitor_final_sha256':hashlib.sha256(json.dumps(final,sort_keys=True,separators=(',',':')).encode()).hexdigest(),
                'actual_core_broker_and_helper_original_loaded_bound':True,
                'conditional_close_kept_actual_dns_lease':True,'actual_dns_reset_while_descriptor_held':True,
                'all_effects_settled_before_monitor_freeze':True,
                'parent_whole_known_zero':False,'production_effect_authority':False,
                'installed_compatibility':False,'normal_owner_adoption':False}
        except BaseException:
            self.sealed = self.owner.sealed = True
            raise Refused() from None  # No second observation, signal, output or failure close.
