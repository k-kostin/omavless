//! Private fixed construction sequence shared by the actual adapter and inert
//! owner-return fault controls. Not a public provider or authority constructor.
use std::mem::ManuallyDrop;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Refused;
type Result<T> = std::result::Result<T, Refused>;
pub(super) trait Backend {
    type File;
    type Attr;
    type Actions;
    type Pid: Copy;
    fn gate(&mut self) -> Result<()>;
    fn pipe(&mut self) -> Result<(Self::File, Self::File)>;
    fn descriptor(file: &Self::File) -> i32;
    fn attr(&mut self) -> Result<Self::Attr>;
    fn actions(&mut self) -> Result<Self::Actions>;
    fn dup(&mut self, actions: &mut Self::Actions, source: i32, target: i32) -> Result<()>;
    fn recheck(&mut self) -> Result<()>;
    fn spawn(&mut self, actions: &Self::Actions, attr: &Self::Attr) -> Result<Self::Pid>;
    fn valid_pid(pid: Self::Pid) -> bool;
}
pub(super) struct Spawned<F, P> {
    pub(super) pid: P,
    pub(super) child_read: ManuallyDrop<F>,
    pub(super) parent_write: ManuallyDrop<F>,
    pub(super) parent_read: ManuallyDrop<F>,
    pub(super) child_write: ManuallyDrop<F>,
}
// Retain a successfully returned owner BEFORE any post-call refusal or panic.
fn acquired<T>(result: Result<T>, backend: &mut impl Backend) -> Result<ManuallyDrop<T>> {
    let held = result.map(ManuallyDrop::new);
    backend.gate()?;
    held
}
pub(super) fn construct<B: Backend>(backend: &mut B) -> Result<Spawned<B::File, B::Pid>> {
    backend.gate()?;
    let first = acquired(backend.pipe(), backend)?;
    let (r, w) = ManuallyDrop::into_inner(first);
    let child_read = ManuallyDrop::new(r);
    let parent_write = ManuallyDrop::new(w);
    backend.gate()?;
    let second = acquired(backend.pipe(), backend)?;
    let (r, w) = ManuallyDrop::into_inner(second);
    let parent_read = ManuallyDrop::new(r);
    let child_write = ManuallyDrop::new(w);
    for file in [&*child_read, &*parent_write, &*parent_read, &*child_write] {
        if B::descriptor(file) < 3 { return Err(Refused); }
    }
    backend.gate()?;
    let attr = acquired(backend.attr(), backend)?;
    backend.gate()?;
    let mut actions = acquired(backend.actions(), backend)?;
    for (source, target) in [(B::descriptor(&child_read), 0),
        (B::descriptor(&child_write), 1), (B::descriptor(&child_write), 2)] {
        backend.gate()?;
        let result = backend.dup(&mut actions, source, target);
        backend.gate()?;
        result?;
    }
    backend.gate()?;
    let result = backend.recheck();
    backend.gate()?;
    result?;
    backend.gate()?;
    let pid = acquired(backend.spawn(&actions, &attr), backend)?;
    if !B::valid_pid(*pid) { return Err(Refused); }
    Ok(Spawned { pid: *pid, child_read, parent_write, parent_read, child_write })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    struct Owner(i32, Rc<Cell<usize>>);
    impl Drop for Owner { fn drop(&mut self) { self.1.set(self.1.get()+1); } }
    struct Fake {
        drops: Rc<Cell<usize>>, calls: Vec<&'static str>, gates: usize,
        cut_gate: Option<usize>, cut_call: Option<usize>, panic: bool,
        dup: Vec<(i32,i32)>, invalid_fd: bool, invalid_pid: bool,
    }
    impl Fake {
        fn new() -> Self { Self { drops: Rc::new(Cell::new(0)), calls: Vec::new(), gates: 0,
            cut_gate: None, cut_call: None, panic: false, dup: Vec::new(),
            invalid_fd: false, invalid_pid: false } }
        fn call(&mut self, name: &'static str) -> Result<()> {
            let index=self.calls.len(); self.calls.push(name);
            if self.cut_call==Some(index) { if self.panic {panic!("inert call");} return Err(Refused); }
            Ok(())
        }
    }
    impl Backend for Fake {
        type File=Owner; type Attr=Owner; type Actions=Owner; type Pid=i32;
        fn gate(&mut self)->Result<()> {
            let index=self.gates;self.gates+=1;
            if self.cut_gate==Some(index) {if self.panic {panic!("inert gate");}return Err(Refused);}
            Ok(())
        }
        fn pipe(&mut self)->Result<(Owner,Owner)> {
            self.call("pipe")?;let base=if self.invalid_fd {0}else{3+(self.calls.len() as i32-1)*2};
            Ok((Owner(base,self.drops.clone()),Owner(base+1,self.drops.clone())))
        }
        fn descriptor(file:&Owner)->i32 {file.0}
        fn attr(&mut self)->Result<Owner> {self.call("attr")?;Ok(Owner(90,self.drops.clone()))}
        fn actions(&mut self)->Result<Owner> {self.call("actions")?;Ok(Owner(91,self.drops.clone()))}
        fn dup(&mut self,_:&mut Owner,source:i32,target:i32)->Result<()> {
            self.dup.push((source,target));self.call("dup")
        }
        fn recheck(&mut self)->Result<()> {self.call("recheck")}
        fn spawn(&mut self,_:&Owner,_:&Owner)->Result<i32> {
            self.call("spawn")?;Ok(if self.invalid_pid {0}else{123})
        }
        fn valid_pid(pid:i32)->bool {pid>1}
    }
    const CALLS:[&str;9]=["pipe","pipe","attr","actions","dup","dup","dup","recheck","spawn"];
    #[test]
    fn every_constructor_leaf_error_or_panic_stops_and_retains_prior_owners() {
        for cut in 0..9 {for panic in [false,true] {
            let mut backend=Fake::new();backend.cut_call=Some(cut);backend.panic=panic;
            let result=catch_unwind(AssertUnwindSafe(||construct(&mut backend)));
            if panic {assert!(result.is_err());}else{assert!(result.unwrap().is_err());}
            assert_eq!(backend.calls,&CALLS[..=cut]);assert_eq!(backend.drops.get(),0);
        }}
    }
    #[test]
    fn every_original_pre_post_gate_cut_has_no_next_leaf_or_drop() {
        for cut in 0..18 {for panic in [false,true] {
            let mut backend=Fake::new();backend.cut_gate=Some(cut);backend.panic=panic;
            let result=catch_unwind(AssertUnwindSafe(||construct(&mut backend)));
            if panic {assert!(result.is_err());}else{assert!(result.unwrap().is_err());}
            assert_eq!(backend.calls,&CALLS[..cut.div_ceil(2)]);
            assert_eq!(backend.gates,cut+1);assert_eq!(backend.drops.get(),0);
        }}
    }
    #[test]
    fn fixed_alias_pid_refusals_and_success_preserve_exact_returned_graph() {
        let mut backend=Fake::new();backend.invalid_fd=true;
        assert!(construct(&mut backend).is_err());assert_eq!(backend.calls,["pipe","pipe"]);
        assert_eq!(backend.drops.get(),0);
        let mut backend=Fake::new();backend.invalid_pid=true;
        assert!(construct(&mut backend).is_err());assert_eq!(backend.calls,CALLS);
        assert_eq!(backend.drops.get(),0);
        let mut backend=Fake::new();let actual=construct(&mut backend).unwrap();
        assert_eq!(backend.calls,CALLS);assert_eq!(backend.dup,[(3,0),(6,1),(6,2)]);
        assert_eq!(actual.pid,123);
        assert_eq!([actual.child_read.0,actual.parent_write.0,actual.parent_read.0,actual.child_write.0],[3,4,5,6]);
        drop(actual);assert_eq!(backend.drops.get(),0);
    }
}
