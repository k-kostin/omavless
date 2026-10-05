"""Compile-only case sources for the ACTUAL exported module graph.

This file never invokes Cargo/rustc or a produced program. Each negative case
is a separately preserved bin source, with an explicit expected Rust diagnostic.
The positive case must compile before any negative result can be accepted.
"""

PRELUDE = b'''
use launch_acquisition::{AcquiredCreator, CreatorOwner};
use launch_acquisition::owned_launcher::Prototype;
use kernel_observer::owned_creator::ActualCreator;
use kernel_observer::owned_creator::ActualInventory;
'''


def cases():
    result = {
        'positive': (None, '''
fn typed(owner: Prototype, creator: &ActualCreator) {
    let _: std::os::fd::BorrowedFd<'_> = creator.creator_socket();
    let _: Result<(), effect_port::EffectError> = owner.finish();
}
fn acquisition(_: &mut AcquiredCreator<ActualCreator>) {}
fn constructor() -> Result<Prototype, effect_port::EffectError> { Prototype::open_fixed() }
'''),
        'escape_socket': ('E0515', '''
fn rejected(creator: ActualCreator) -> std::os::fd::BorrowedFd<'static> {
    creator.creator_socket()
}
'''),
        'private_callback': ('E0624', '''
fn rejected(owner: &mut AcquiredCreator<ActualCreator>) {
    let _ = owner.with_lease(|_| Ok(()));
}
'''),
        'replace_creator': ('E0616', '''
fn rejected(owner: &mut AcquiredCreator<ActualCreator>, replacement: ActualCreator) {
    let _ = std::mem::replace(&mut owner.retained.creator, replacement);
}
'''),
        'no_supplied_constructor': ('E0599', '''
fn rejected(creator: ActualCreator) { let _ = AcquiredCreator::synthetic(creator); }
'''),
        'no_canonical_conversion': ('E0277', '''
fn canonical<T: authority_composition::CanonicalCreator>() {}
fn rejected() { canonical::<ActualCreator>(); }
'''),
        'private_originals': ('E0616', '''
fn rejected(owner: &Prototype) { let _ = &owner.life; }
'''),
        'inventory_escape': ('E0515', '''
fn rejected(mut owner: ActualCreator) -> ActualInventory<'static> {
    owner.borrow_inventory().unwrap()
}
'''),
        'inventory_overlap': ('E0499', '''
fn rejected(owner: &mut ActualCreator) {
    let mut lease = owner.borrow_inventory().unwrap();
    owner.seal();
    let _ = lease.recheck();
}
'''),
        'inventory_private_session': ('E0616', '''
fn rejected(lease: &mut ActualInventory<'_>) { let _ = &mut lease.lease; }
'''),
    }
    for label, owner in (('prototype', 'Prototype'), ('creator', 'ActualCreator'),
                         ('acquired', 'AcquiredCreator<ActualCreator>'),
                         ('inventory', "ActualInventory<'static>")):
        for bound in ('Send', 'Sync', 'Copy'):
            result[f'{label}_{bound.lower()}'] = (
                'E0277', f'fn require<T: {bound}>() {{}}\n'
                f'fn rejected() {{ require::<{owner}>(); }}\n')
    return result


def render(library):
    """Preserve the exact source tree; append only uncalled type-check functions."""
    if type(library) is not bytes:
        raise ValueError('fixed_library_bytes')
    return {f'type_{name}.rs': library + PRELUDE + body.encode() + b'\nfn main() {}\n'
            for name, (_, body) in cases().items()}
