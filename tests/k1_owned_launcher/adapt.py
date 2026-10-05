"""Pure exact-source adapter for the isolated launcher workspace, not product code.

No filesystem, subprocess, dependency adoption or execution occurs here.
The actual acquisition semantic methods and authority session body are reused.
"""
import hashlib

BASE='e6488ed5c39486a5f967859c20a0b84859ae4b5d'
PINS={
 'launch_acquisition.rs':'f5867e166d496d4ef82599daf32ab28c52878bbce732ca4d30183ed1813e564f',
 'authority_composition.rs':'9f2829da32aa34fafaf02c353fb16ca38bbc1a2f7ac31ed450ba248c76ae26fa',
 'kernel_observer.rs':'34ff223034f2e1cf817cd6a73456c126d581914d3f732c7e7242da24cb39f891',
 'kernel_inventory.rs':'49e3c0d8daec3f12014b05d992772bade1f5d884a94384e09355f4a3d0e72d12',
 'kernel_chain_observer.rs':'1d1c6fe007f862cc0306e7534141d69d1d16454fd2bd8a5dc0b7cfc1bb9185d7',
 'kernel_rule_wire.rs':'52d39aef99eb1c4bc99d26fffc3798ba286ab23a4a7af36e43a6554a46c89097',
}

def replace(raw,old,new,count=1):
    if raw.count(old)!=count:raise ValueError('fixed_adapter_source_mismatch')
    return raw.replace(old,new)

def adapt(sources):
    if sources.keys()!=PINS.keys():raise ValueError('fixed_adapter_catalog')
    for name,pin in PINS.items():
        if type(sources[name]) is not bytes or hashlib.sha256(sources[name]).hexdigest()!=pin:
            raise ValueError('fixed_adapter_pin')
    result={name:raw.decode('utf-8','strict') for name,raw in sources.items()}
    text=result['launch_acquisition.rs']
    text=replace(text,'use std::os::fd::{AsFd, BorrowedFd, OwnedFd};',
        'use std::os::fd::{AsFd, BorrowedFd};')
    text=replace(text,'    creator_socket: OwnedFd,\n','')
    text=replace(text,'pub(crate) struct AcquiredCreator<C: CanonicalCreator> {','pub(crate) struct AcquiredCreator<C: CreatorOwner> {')
    text=replace(text,'impl<C: CanonicalCreator> AcquiredCreator<C> {','impl<C: CreatorOwner> AcquiredCreator<C> {')
    start=text.index('        let borrow = || LaunchBorrow {')
    end=text.index('        self.sealed = false;',start)
    text=text[:start]+'''        originals.verifier.recheck(LaunchBorrow {
            anchor: originals.anchor.as_fd(),
            thread_namespace: originals.thread_namespace.as_fd(),
            creator_socket: creator.creator_socket(),
            _same_thread: PhantomData,
        })?;
        let value = callback(creator)?;
        originals.verifier.recheck(LaunchBorrow {
            anchor: originals.anchor.as_fd(),
            thread_namespace: originals.thread_namespace.as_fd(),
            creator_socket: creator.creator_socket(),
            _same_thread: PhantomData,
        })?;
'''+text[end:]
    text=replace(text,'    pub(crate) fn retained_epoch(',''' }

impl<C: CanonicalCreator> AcquiredCreator<C> {
    pub(crate) fn retained_epoch(''')
    # Export the normal body only, not the old synthetic second-socket factory.
    # Its dedicated replacement tests belong to the external harness.
    text=text[:text.index('    // The ONLY constructor is synthetic.')]+ '}\n'
    text=replace(text,'    anchor: File,\n','    anchor: Rc<File>,\n')
    text=replace(text,'    thread_namespace: File,\n','    thread_namespace: Rc<File>,\n')
    text+='''
/// Internal only: must borrow the ACTUAL exclusive session owner.
/// A matching second socket, supplied descriptor or numeric identity is not an implementation.
pub(crate) trait CreatorOwner {
    fn creator_socket(&self) -> BorrowedFd<'_>;
}

#[path = "owned_launcher.rs"]
pub(crate) mod owned_launcher;
'''
    result['launch_acquisition.rs']=text
    result['authority_composition.rs']=replace(result['authority_composition.rs'],
        'pub(crate) trait CanonicalCreator: EffectPort + sealed::Sealed {',
        'pub(crate) trait CanonicalCreator: EffectPort + sealed::Sealed + crate::launch_acquisition::CreatorOwner {')
    result['kernel_observer.rs']+='''
// External review-only extension; absent from the product source.
#[path = "owned_creator.rs"]
pub(crate) mod owned_creator;
'''
    result['kernel_observer.rs']=replace(result['kernel_observer.rs'],
        '    last_generation: Option<u32>,\n}',
        '    last_generation: Option<u32>,\n    launch: Option<std::rc::Rc<crate::launch_acquisition::owned_launcher::LaunchLife>>,\n}')
    result['kernel_observer.rs']=replace(result['kernel_observer.rs'],
        '            last_generation: None,\n',
        '            last_generation: None,\n            launch: None,\n',
        result['kernel_observer.rs'].count('            last_generation: None,\n'))
    result['kernel_observer.rs']=replace(result['kernel_observer.rs'],
        '    fn check(&self, deadline: Instant) -> Result<()> {\n',
        '''    fn check(&self, deadline: Instant) -> Result<()> {
        if let Some(launch) = &self.launch {
            require(!self.poisoned && Instant::now() < deadline)?;
            launch.session_check(std::os::fd::AsFd::as_fd(&self.namespace),
                std::os::fd::AsFd::as_fd(&self.socket)).map_err(|_| REFUSE)?;
            let actual: NetlinkAddr = self.launch_leaf(|| getsockname(self.socket.as_raw_fd()))?
                .map_err(|_| REFUSE)?;
            require(Instant::now() < deadline && actual == self.local)?;
            return Ok(());
        }
''')
    result['kernel_observer.rs']=replace(result['kernel_observer.rs'],
        '    fn sequences(&mut self) -> Result<[u32; 3]> {',
        '''    // This private callback accepts neither socket replacement nor an
    // external budget. Only fixed internal readback leaves use it.
    fn launch_leaf<T>(&self, action: impl FnOnce() -> T) -> Result<T> {
        if let Some(launch) = &self.launch { launch.gate().map_err(|_| REFUSE)?; }
        let value = action();
        if let Some(launch) = &self.launch { launch.gate().map_err(|_| REFUSE)?; }
        Ok(value)
    }

    fn sequences(&mut self) -> Result<[u32; 3]> {''')
    # Exactly the four shared complete-inventory request/receive paths. No
    # mutation sender or generic socket escape is added to ActualCreator.
    for name in ('kernel_observer.rs','kernel_inventory.rs','kernel_chain_observer.rs','kernel_rule_wire.rs'):
        text=result[name]
        text=replace(text,'            sendto(\n','            self.launch_leaf(|| sendto(\n')
        text=replace(text,'                MsgFlags::MSG_DONTWAIT,\n            )\n            .map_err',
            '                MsgFlags::MSG_DONTWAIT,\n            ))?\n            .map_err')
        text=replace(text,'            match recvmsg::<NetlinkAddr>(\n',
            '            match self.launch_leaf(|| recvmsg::<NetlinkAddr>(\n')
        text=replace(text,'                MsgFlags::MSG_DONTWAIT,\n            ) {',
            '                MsgFlags::MSG_DONTWAIT,\n            ))? {')
        result[name]=text
    result['kernel_inventory.rs']=replace(result['kernel_inventory.rs'],
        '    fn inspect_policy_inventory_before(',
        '    pub(super) fn inspect_policy_inventory_before(')
    # Reuse the SAME #641 lease constructor and parser. Only the external
    # creator supplies its earlier original launch deadline; no renewed second.
    result['kernel_inventory.rs']=replace(result['kernel_inventory.rs'],
        '''    pub fn borrow_policy_inventory(&mut self) -> Result<LocalInventoryLease<'_>> {
        let deadline = Instant::now() + Duration::from_secs(1);
''', '''    pub fn borrow_policy_inventory(&mut self) -> Result<LocalInventoryLease<'_>> {
        self.borrow_policy_inventory_before(Instant::now() + Duration::from_secs(1))
    }
    pub(super) fn borrow_policy_inventory_before(&mut self, deadline: Instant) -> Result<LocalInventoryLease<'_>> {
''')
    return {name:raw.encode() for name,raw in result.items()}
