// SPDX-License-Identifier: MIT
//! TEST ONLY: regular-file proof and synthetic fixed outcomes on a private socket.
//! No root peer bypass in the public client, TUN admission or DNS effects.
use omavless_dns_channel::{Error, Listener, Response};
use std::{
    io::{self, BufRead, Read, Write},
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};

const PROOF: &[u8] = b"omavless-dns-channel-fixture\n";

fn run() -> Result<(), ()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4
        || args[1] != "--fixture"
        || !matches!(
            args[2].as_str(),
            "success" | "reject" | "recovery_on_release" | "loss_after_ready"
        )
    {
        return Err(());
    }
    let listener =
        Listener::bind(Path::new(&args[3]), rustix::process::geteuid().as_raw()).map_err(|_| ())?;
    println!("fixture_ready");
    io::stdout().flush().map_err(|_| ())?;
    let mut session = listener.accept().map_err(|_| ())?;
    let descriptor = session.receive_acquire().map_err(|_| ())?;
    let stat = rustix::fs::fstat(descriptor).map_err(|_| ())?;
    if rustix::fs::FileType::from_raw_mode(stat.st_mode) != rustix::fs::FileType::RegularFile
        || stat.st_size != PROOF.len() as i64
    {
        return Err(());
    }
    let mut data = [0_u8; 64];
    let count = rustix::io::pread(descriptor, &mut data[..], 0).map_err(|_| ())?;
    if data[..count] != *PROOF {
        return Err(());
    }
    if args[2] == "reject" {
        session.reply(Response::Rejected).map_err(|_| ())?;
    } else {
        session.reply(Response::Applying).map_err(|_| ())?;
        session.reply(Response::Ready).map_err(|_| ())?;
        if args[2] == "loss_after_ready" {
            let (send, receive) = mpsc::sync_channel(1);
            std::thread::spawn(move || {
                let mut command = String::new();
                let result = io::stdin().lock().take(16).read_line(&mut command);
                let _ = send.send(result.is_ok() && command == "drop\n");
            });
            if receive.recv_timeout(Duration::from_secs(5)) != Ok(true) {
                return Err(());
            }
            // The Go observer must classify this EOF as lost, never Released.
            drop(session);
            println!("fixture_done");
            return Ok(());
        }
        let until = Instant::now() + Duration::from_secs(15);
        loop {
            match session.receive_release() {
                Ok(()) => break,
                Err(Error::Idle) if Instant::now() < until => continue,
                _ => return Err(()),
            }
        }
        session.reply(Response::Releasing).map_err(|_| ())?;
        session
            .reply(if args[2] == "success" {
                Response::Released
            } else {
                Response::RecoveryRequired
            })
            .map_err(|_| ())?;
    }
    println!("fixture_done");
    Ok(())
}

fn main() {
    if run().is_err() {
        eprintln!("fixture_failed");
        std::process::exit(1);
    }
}
