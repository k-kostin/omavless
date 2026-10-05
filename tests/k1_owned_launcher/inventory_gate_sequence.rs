//! Fixed no-mutation entry order; also compiled independently for pure controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Observation { TableAbsent, ExactUntrusted, OtherUntrusted }
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Phase { OpenBegin, OpenOk, InventoryBegin, Inventory(Observation), FinishBegin, FinishOk }
impl Phase {
    pub(super) fn bytes(self) -> &'static [u8] {
        match self {
            Self::OpenBegin => b"K1_INVENTORY_OPEN_BEGIN\n",
            Self::OpenOk => b"K1_INVENTORY_OPEN_OK\n",
            Self::InventoryBegin => b"K1_INVENTORY_READ_BEGIN\n",
            Self::Inventory(Observation::TableAbsent) => b"K1_INVENTORY_TABLE_ABSENT\n",
            Self::Inventory(Observation::ExactUntrusted) => b"K1_INVENTORY_EXACT_UNTRUSTED\n",
            Self::Inventory(Observation::OtherUntrusted) => b"K1_INVENTORY_OTHER_UNTRUSTED\n",
            Self::FinishBegin => b"K1_INVENTORY_FINISH_BEGIN\n",
            Self::FinishOk => b"K1_INVENTORY_FINISH_OK\n",
        }
    }
}
#[derive(Debug, PartialEq)]
pub(super) struct Refused;
pub(super) trait Backend {
    fn phase(&mut self, phase: Phase) -> Result<(), Refused>;
    fn open(&mut self) -> Result<(), Refused>;
    fn inventory(&mut self) -> Result<Observation, Refused>;
    fn finish(&mut self) -> Result<(), Refused>;
}
#[derive(Default)]
pub(super) struct Attempt { sealed: bool }
impl Attempt {
    pub(super) fn run(&mut self, backend: &mut impl Backend) -> Result<(), Refused> {
        if self.sealed { return Err(Refused); }
        self.sealed = true;
        backend.phase(Phase::OpenBegin)?;
        backend.open()?;
        backend.phase(Phase::OpenOk)?;
        backend.phase(Phase::InventoryBegin)?;
        let observed = backend.inventory()?;
        backend.phase(Phase::Inventory(observed))?;
        backend.phase(Phase::FinishBegin)?;
        backend.finish()?;
        backend.phase(Phase::FinishOk)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Fake { calls: Vec<u8>, cut: usize, panic: bool, observed: Observation }
    impl Fake {
        fn step(&mut self, n: u8) -> Result<(), Refused> {
            self.calls.push(n);
            if self.calls.len() == self.cut {
                if self.panic { panic!("synthetic fixed-entry cut"); }
                return Err(Refused);
            }
            Ok(())
        }
    }
    impl Backend for Fake {
        fn phase(&mut self, _: Phase) -> Result<(), Refused> { self.step(1) }
        fn open(&mut self) -> Result<(), Refused> { self.step(2) }
        fn inventory(&mut self) -> Result<Observation, Refused> { self.step(3)?; Ok(self.observed) }
        fn finish(&mut self) -> Result<(), Refused> { self.step(4) }
    }
    #[test]
    fn complete_inventory_precedes_finish_and_every_outcome_is_untrusted() {
        for observed in [Observation::TableAbsent, Observation::ExactUntrusted, Observation::OtherUntrusted] {
            let mut fake = Fake { calls: vec![], cut: 0, panic: false, observed };
            let mut attempt = Attempt::default();
            assert_eq!(attempt.run(&mut fake), Ok(()));
            assert_eq!(fake.calls, [1,2,1,1,3,1,1,4,1]);
            assert_eq!(attempt.run(&mut fake), Err(Refused));
            assert_eq!(fake.calls.len(), 9);
            assert!(Phase::Inventory(observed).bytes().ends_with(b"\n"));
        }
    }
    #[test]
    fn every_refusal_or_unwind_stops_without_finish_or_later_output() {
        for cut in 1..=9 {
            for panic in [false, true] {
                let mut fake = Fake { calls: vec![], cut, panic, observed: Observation::TableAbsent };
                let mut attempt = Attempt::default();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| attempt.run(&mut fake)));
                if panic { assert!(result.is_err()); } else { assert_eq!(result.unwrap(), Err(Refused)); }
                assert_eq!(fake.calls, [1,2,1,1,3,1,1,4,1][..cut]);
                assert_eq!(attempt.run(&mut fake), Err(Refused));
                assert_eq!(fake.calls.len(), cut);
            }
        }
    }
}
