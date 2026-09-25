//! Test-only executable: inherited stdin descriptor, synthetic namespace only.
//! Not packaged, no socket listener, method execution, DNS or route operations.
#![forbid(unsafe_code)]
use omavless_dns_tun::HeldTun;
use std::os::fd::AsFd;

fn run() -> Result<bool, ()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 || !rustix::process::geteuid().is_root() {
        return Err(());
    }
    for (kind, original) in ["net", "user", "pid"].into_iter().zip(args) {
        let current = std::fs::read_link(format!("/proc/thread-self/ns/{kind}")).map_err(|_| ())?;
        if current.to_str() == Some(&original) {
            return Err(());
        }
    }
    let descriptor = std::io::stdin()
        .as_fd()
        .try_clone_to_owned()
        .map_err(|_| ())?;
    match HeldTun::admit(descriptor) {
        Ok(held) => {
            held.recheck().map_err(|_| ())?;
            Ok(true)
        }
        Err(
            omavless_dns_tun::Error::InvalidDescriptor
            | omavless_dns_tun::Error::InvalidTun
            | omavless_dns_tun::Error::NamespaceMismatch,
        ) => Ok(false),
        Err(_) => Err(()),
    }
}

fn main() {
    match run() {
        Ok(true) => println!("{{\"accepted\":true}}"),
        Ok(false) => println!("{{\"accepted\":false}}"),
        Err(()) => {
            println!("{{\"conformance_failed\":true}}");
            std::process::exit(1);
        }
    }
}
