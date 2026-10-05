// Opt-in external parent only. No product caller, policy mutation or FD export.
#[path = "inventory_gate_sequence.rs"]
mod inventory_gate_sequence;
use inventory_gate_sequence::{Backend, Observation, Phase, Refused};
use std::mem::ManuallyDrop;
use std::time::Instant;
use launch_acquisition::owned_launcher::Prototype;

struct FixedInventoryGate {
    owner: ManuallyDrop<Option<Prototype>>,
    deadline: Instant,
}
impl FixedInventoryGate {
    fn budget(&self) -> Result<(), Refused> {
        if Instant::now() < self.deadline { Ok(()) } else { Err(Refused) }
    }
}
impl Backend for FixedInventoryGate {
    fn phase(&mut self, phase: Phase) -> Result<(), Refused> {
        self.budget()?;
        let bytes = phase.bytes();
        let result = rustix::io::write(rustix::stdio::stdout(), bytes);
        self.budget()?;
        match result { Ok(n) if n == bytes.len() => Ok(()), _ => Err(Refused) }
    }
    fn open(&mut self) -> Result<(), Refused> {
        self.budget()?;
        if self.owner.is_some() { return Err(Refused); }
        let owner = Prototype::open_fixed_before(self.deadline).map_err(|_| Refused)?;
        // Retain original owner BEFORE post-call budget or any phase output.
        *self.owner = Some(owner);
        self.budget()
    }
    fn inventory(&mut self) -> Result<Observation, Refused> {
        self.budget()?;
        let observed = self.owner.as_mut().ok_or(Refused)?.inventory().map_err(|_| Refused)?;
        self.budget()?;
        Ok(match observed {
            kernel_observer::LocalPolicyInventory::TableAbsent => Observation::TableAbsent,
            kernel_observer::LocalPolicyInventory::ExactUntrusted(_) => Observation::ExactUntrusted,
            kernel_observer::LocalPolicyInventory::OtherUntrusted => Observation::OtherUntrusted,
        })
    }
    fn finish(&mut self) -> Result<(), Refused> {
        self.budget()?;
        self.owner.take().ok_or(Refused)?.finish().map_err(|_| Refused)?;
        self.budget()
    }
}
fn main() -> std::process::ExitCode {
    let Some(deadline) = Instant::now().checked_add(std::time::Duration::from_secs(5)) else {
        return std::process::ExitCode::from(2);
    };
    let mut args = std::env::args_os();
    if args.next().is_none()
        || args.next().as_deref() != Some(std::ffi::OsStr::new("--fixed-owned-readonly-inventory"))
        || args.next().is_some()
    { return std::process::ExitCode::from(2); }
    let mut backend = FixedInventoryGate { owner: ManuallyDrop::new(None), deadline };
    match inventory_gate_sequence::Attempt::default().run(&mut backend) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(_) => std::process::ExitCode::from(2),
    }
}
