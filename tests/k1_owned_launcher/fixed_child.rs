//! Separate noninstalled fixed no-policy child; never invoked by source gates.
#![forbid(unsafe_code)]
#[path = "child_protocol.rs"]
#[allow(dead_code, reason = "the parent also uses this exact module's EOF verifier")]
mod protocol;
#[cfg(test)]
#[path = "retained_return.rs"]
mod retained_return;
#[cfg(test)]
#[path = "static_elf.rs"]
mod static_elf;
use std::os::fd::AsFd;
use std::time::{Duration,Instant};

fn run() -> protocol::Result<()> {
    let deadline=Instant::now().checked_add(Duration::from_secs(5)).ok_or(protocol::Refused)?;
    let mut gate=||if Instant::now()<deadline {Ok(())}else{Err(protocol::Refused)};
    gate()?;
    let args:Vec<_>=std::env::args_os().collect();
    if args.len()!=2 || args[1]!="--fixed-owned-child-no-policy" {return Err(protocol::Refused);}
    let mut input=std::io::stdin();let mut output=std::io::stdout();
    for fd in [input.as_fd(),output.as_fd()] {
        gate()?;let flags=nix::fcntl::fcntl(fd,nix::fcntl::FcntlArg::F_GETFL);gate()?;
        let flags=nix::fcntl::OFlag::from_bits(flags.map_err(|_|protocol::Refused)?).ok_or(protocol::Refused)?;
        gate()?;let result=nix::fcntl::fcntl(fd,nix::fcntl::FcntlArg::F_SETFL(flags|nix::fcntl::OFlag::O_NONBLOCK));gate()?;
        result.map_err(|_|protocol::Refused)?;
    }
    protocol::write_frame(&mut output,protocol::READY,&mut gate)?;
    protocol::read_frame(&mut input,protocol::FINISH,&mut gate)?;
    protocol::idle(&mut input,&mut gate)?;
    protocol::write_frame(&mut output,protocol::DONE,&mut gate)?;
    gate()
}
fn main() {
    // No panic/error/raw diagnostic or second output on protocol failure.
    // This child has no namespace, nft, inherited creator or cleanup operation.
    std::process::exit(if run().is_ok(){0}else{2});
}
