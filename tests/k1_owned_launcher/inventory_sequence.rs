//! Private fixed post-read sequence; actual and synthetic adapters share it.
//! No sender, alternate constructor, effect callback or caller-selected steps.
#[derive(Debug, PartialEq)]
pub(super) struct Refused;
pub(super) trait Backend {
    fn lease(&mut self) -> Result<(), Refused>;
    fn original_owner(&mut self) -> Result<(), Refused>;
}
#[derive(Default)]
pub(super) struct Attempt { sealed: bool }
impl Attempt {
    pub(super) fn run(&mut self, backend: &mut impl Backend) -> Result<(), Refused> {
        if self.sealed { return Err(Refused); }
        self.sealed = true;
        backend.lease()?;
        backend.original_owner()?;
        backend.lease()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake { calls: Vec<u8>, cut: usize, panic: bool }
    impl Fake {
        fn step(&mut self, label: u8) -> Result<(), Refused> {
            self.calls.push(label);
            if self.calls.len() == self.cut {
                if self.panic { panic!("synthetic inventory cut"); }
                return Err(Refused);
            }
            Ok(())
        }
    }
    impl Backend for Fake {
        fn lease(&mut self) -> Result<(), Refused> { self.step(1) }
        fn original_owner(&mut self) -> Result<(), Refused> { self.step(2) }
    }
    #[test]
    fn fixed_order_and_success_cannot_reenter() {
        let mut attempt = Attempt::default();
        let mut fake = Fake { calls: vec![], cut: 0, panic: false };
        assert_eq!(attempt.run(&mut fake), Ok(()));
        assert_eq!(fake.calls, [1, 2, 1]);
        assert_eq!(attempt.run(&mut fake), Err(Refused));
        assert_eq!(fake.calls, [1, 2, 1]);
    }
    #[test]
    fn every_error_or_unwind_stops_before_next_operation() {
        for cut in 1..=3 {
            for panic in [false, true] {
                let mut attempt = Attempt::default();
                let mut fake = Fake { calls: vec![], cut, panic };
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| attempt.run(&mut fake)));
                if panic { assert!(result.is_err()); } else { assert_eq!(result.unwrap(), Err(Refused)); }
                assert_eq!(fake.calls, [1, 2, 1][..cut]);
                assert_eq!(attempt.run(&mut fake), Err(Refused));
                assert_eq!(fake.calls.len(), cut);
            }
        }
    }
}
