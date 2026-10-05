//! Private one-attempt release of the two original parent-held child ends.
//! A consuming close error is terminal: never reconstruct or retry a raw FD.
use std::cell::{Cell, RefCell};
use std::mem::ManuallyDrop;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Refused;
pub(super) struct Handoff<T> {
    originals: ManuallyDrop<RefCell<[Option<T>; 2]>>,
    attempted: Cell<bool>,
    complete: Cell<bool>,
}
impl<T> Handoff<T> {
    pub(super) fn new(originals: [T; 2]) -> Self {
        Self { originals: ManuallyDrop::new(RefCell::new(originals.map(Some))),
            attempted: Cell::new(false), complete: Cell::new(false) }
    }
    // Only the private OwnedChild adapter supplies the safe consuming close.
    // Tests substitute non-kernel owners; no normal caller receives this seam.
    pub(super) fn release<E>(&self, ready: bool, mut gate: impl FnMut() -> Result<(), Refused>,
        mut close: impl FnMut(T) -> Result<(), E>) -> Result<(), Refused> {
        if self.attempted.replace(true) || !ready { return Err(Refused); }
        for index in 0..2 {
            gate()?;
            let original = self.originals.borrow_mut()[index].take().ok_or(Refused)?;
            // The adapter consumes this exact original, not a borrowed number.
            // On error ownership is uncertain/consumed; no re-adoption or retry.
            let result = close(original);
            gate()?;
            result.map_err(|_|Refused)?;
        }
        self.complete.set(true);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    struct Owner(usize, Rc<Cell<usize>>);
    impl Drop for Owner { fn drop(&mut self) { self.1.set(self.1.get()+1); } }
    fn originals() -> (Handoff<Owner>, Rc<Cell<usize>>) {
        let drops=Rc::new(Cell::new(0));
        (Handoff::new([Owner(0,drops.clone()),Owner(1,drops.clone())]),drops)
    }
    #[test]
    fn every_pre_post_gate_error_or_panic_stops_without_next_close_or_retry() {
        for cut in 0_usize..4 { for panic in [false,true] {
            let (held,drops)=originals(); let mut gates=0; let mut closed=Vec::new();
            let result=catch_unwind(AssertUnwindSafe(||held.release(true,||{
                let at=gates; gates+=1;
                if at==cut { if panic {panic!("inert gate");} return Err(Refused); } Ok(())
            },|owner| {closed.push(owner.0); drop(owner); Ok::<(),()>(())})));
            if panic {assert!(result.is_err());} else {assert_eq!(result.unwrap(),Err(Refused));}
            assert_eq!(closed.len(),cut.div_ceil(2));
            assert!(!held.complete.get());
            assert_eq!(held.release::<()>(true,||panic!("retry gate"),|_|panic!("retry close")),Err(Refused));
            drop(held); assert_eq!(drops.get(),closed.len());
        }}
    }
    #[test]
    fn close_eintr_other_error_and_panic_are_consumed_once_never_success() {
        for cut in 0..2 { for outcome in 0..3 {
            let (held,drops)=originals(); let mut calls=0;
            let result=catch_unwind(AssertUnwindSafe(||held.release(true,||Ok(()),|owner| {
                let at=calls; calls+=1;
                // Synthetic syscall has consumed the original before returning
                // an EINTR/other error or losing control. No kernel operation.
                drop(owner);
                if at==cut { if outcome==2 {panic!("inert close");}
                    return Err(if outcome==0 {std::io::ErrorKind::Interrupted}else{std::io::ErrorKind::Other}); }
                Ok(())
            })));
            if outcome==2 {assert!(result.is_err());}else{assert_eq!(result.unwrap(),Err(Refused));}
            assert_eq!(calls,cut+1); assert!(!held.complete.get());
            assert_eq!(held.release::<()>(true,||panic!("retry"),|_|panic!("retry")),Err(Refused));
            drop(held); assert_eq!(drops.get(),cut+1);
        }}
    }
    #[test]
    fn no_ready_is_permanent_and_only_two_timely_zero_closes_complete() {
        let (held,drops)=originals();
        assert_eq!(held.release::<()>(false,||panic!("before ready"),|_|panic!("before ready")),Err(Refused));
        assert_eq!(held.release::<()>(true,||panic!("retry"),|_|panic!("retry")),Err(Refused));
        drop(held); assert_eq!(drops.get(),0);
        let (held,drops)=originals(); let mut order=Vec::new();
        assert_eq!(held.release(true,||Ok(()),|owner|{order.push(owner.0);drop(owner);Ok::<(),()>(())}),Ok(()));
        assert_eq!(order,[0,1]); assert!(held.complete.get()); assert_eq!(drops.get(),2);
        assert_eq!(held.release::<()>(true,||panic!("retry"),|_|panic!("retry")),Err(Refused));
    }
}
