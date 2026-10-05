//! The actual fixed completion order, with no reap before an exact zero witness.
//! Kernel methods exist only in the private retained-launch adapter.
use std::cell::Cell;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Refused;
type Result<T> = std::result::Result<T, Refused>;
#[derive(Clone, Copy)]
pub(super) enum Observed { Alive, ExactZero, Other }
pub(super) trait Backend {
    fn gate(&mut self) -> Result<()>;
    fn finish_frame(&mut self) -> Result<()>;
    fn done_frame(&mut self) -> Result<()>;
    fn observe(&mut self) -> Result<Observed>;
    fn eof(&mut self) -> Result<()>;
    fn image(&mut self) -> Result<()>;
    fn reap_exact_zero(&mut self) -> Result<()>;
}
pub(super) struct Attempt { attempted: Cell<bool>, complete: Cell<bool> }
impl Attempt {
    pub(super) fn new() -> Self { Self { attempted: Cell::new(false), complete: Cell::new(false) } }
    pub(super) fn run(&self, backend: &mut impl Backend) -> Result<()> {
        if self.attempted.replace(true) { return Err(Refused); }
        backend.gate()?; let result=backend.finish_frame(); backend.gate()?; result?;
        backend.gate()?; let result=backend.done_frame(); backend.gate()?; result?;
        loop {
            backend.gate()?; let result=backend.observe(); backend.gate()?;
            match result? {
                Observed::Alive => std::hint::spin_loop(),
                Observed::ExactZero => break,
                Observed::Other => return Err(Refused),
            }
        }
        backend.gate()?; let result=backend.eof(); backend.gate()?; result?;
        backend.gate()?; let result=backend.image(); backend.gate()?; result?;
        backend.gate()?; let result=backend.reap_exact_zero(); backend.gate()?; result?;
        self.complete.set(true); Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{AssertUnwindSafe,catch_unwind};
    struct Fake {calls:Vec<&'static str>,gates:usize,cut_call:Option<usize>,cut_gate:Option<usize>,panic:bool,observed:Observed}
    impl Fake {
        fn new()->Self {Self{calls:Vec::new(),gates:0,cut_call:None,cut_gate:None,panic:false,observed:Observed::ExactZero}}
        fn call(&mut self,name:&'static str)->Result<()> {
            let at=self.calls.len();self.calls.push(name);
            if self.cut_call==Some(at) {if self.panic {panic!("inert completion leaf");}return Err(Refused);}Ok(())
        }
    }
    impl Backend for Fake {
        fn gate(&mut self)->Result<()> {
            let at=self.gates;self.gates+=1;
            if self.cut_gate==Some(at) {if self.panic {panic!("inert completion gate");}return Err(Refused);}Ok(())
        }
        fn finish_frame(&mut self)->Result<()> {self.call("finish")}
        fn done_frame(&mut self)->Result<()> {self.call("done")}
        fn observe(&mut self)->Result<Observed> {self.call("observe")?;Ok(self.observed)}
        fn eof(&mut self)->Result<()> {self.call("eof")}
        fn image(&mut self)->Result<()> {self.call("image")}
        fn reap_exact_zero(&mut self)->Result<()> {self.call("reap")}
    }
    const CALLS:[&str;6]=["finish","done","observe","eof","image","reap"];
    fn no_retry(attempt:&Attempt,backend:&mut Fake) {
        let before=(backend.calls.len(),backend.gates);
        assert_eq!(attempt.run(backend),Err(Refused));
        assert_eq!((backend.calls.len(),backend.gates),before);
    }
    #[test]
    fn all_completion_leaf_failures_or_panics_are_permanent_without_next_operation() {
        for cut in 0..6 {for panic in [false,true] {
            let attempt=Attempt::new();let mut backend=Fake::new();backend.cut_call=Some(cut);backend.panic=panic;
            let result=catch_unwind(AssertUnwindSafe(||attempt.run(&mut backend)));
            if panic {assert!(result.is_err());}else{assert_eq!(result.unwrap(),Err(Refused));}
            assert_eq!(backend.calls,&CALLS[..=cut]);assert!(!attempt.complete.get());no_retry(&attempt,&mut backend);
        }}
    }
    #[test]
    fn all_pre_post_budget_cuts_include_late_zero_and_late_reap_without_success() {
        for cut in 0_usize..12 {for panic in [false,true] {
            let attempt=Attempt::new();let mut backend=Fake::new();backend.cut_gate=Some(cut);backend.panic=panic;
            let result=catch_unwind(AssertUnwindSafe(||attempt.run(&mut backend)));
            if panic {assert!(result.is_err());}else{assert_eq!(result.unwrap(),Err(Refused));}
            assert_eq!(backend.calls,&CALLS[..cut.div_ceil(2)]);assert!(!attempt.complete.get());no_retry(&attempt,&mut backend);
        }}
    }
    #[test]
    fn wrong_terminal_or_alive_expiry_never_reaps_and_timely_zero_reaps_once() {
        let attempt=Attempt::new();let mut backend=Fake::new();backend.observed=Observed::Other;
        assert_eq!(attempt.run(&mut backend),Err(Refused));assert_eq!(backend.calls,&CALLS[..3]);no_retry(&attempt,&mut backend);
        let attempt=Attempt::new();let mut backend=Fake::new();backend.observed=Observed::Alive;backend.cut_gate=Some(8);
        assert_eq!(attempt.run(&mut backend),Err(Refused));assert_eq!(backend.calls,["finish","done","observe","observe"]);no_retry(&attempt,&mut backend);
        let attempt=Attempt::new();let mut backend=Fake::new();assert_eq!(attempt.run(&mut backend),Ok(()));
        assert_eq!(backend.calls,CALLS);assert!(attempt.complete.get());no_retry(&attempt,&mut backend);
    }
}
